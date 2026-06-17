package main

import (
	"encoding/json"
	"errors"
	"fmt"
	"net/url"
	"os"
	"path/filepath"
	"regexp"
	"sort"
	"strings"
	"time"
)

const (
	provisionedFoldOpsIngestTokenDefault = "/boot/efi/foldingos/provision/foldops-ingest-token"
	foldOpsProvisionServiceName            = "foldingos-foldops-provision.service"
	foldOpsServeHTTPSServiceName           = "foldingos-foldops-serve-https.service"
	foldOpsSupervisorServiceName           = "foldingos-foldops-supervisor.service"
	foldOpsAgentServiceName                = "foldingos-foldops-agent.service"
	foldOpsSupervisorEnvPath               = "/data/config/foldops/supervisor.env"
	foldOpsAgentEnvPath                    = "/data/config/foldops/agent.env"
	foldOpsSupervisorCAPath                = "/data/config/foldops/supervisor-ca.pem"
	foldOpsHTTPSPort                       = 3443
	foldOpsSupervisorLoopbackPort          = 3000
	foldOpsProvisionedSchemaVersion        = 1
)

var (
	provisionedFoldOpsIngestToken = provisionedFoldOpsIngestTokenDefault
	foldOpsIngestTokenPath        = "/data/config/foldops/ingest-token"
	foldOpsTLSDir                 = "/data/foldops/tls"
	foldOpsProvisionedMarkerPath  = "/data/state/foldops/provisioned.json"
	foldOpsIngestTokenPattern     = regexp.MustCompile(`^[0-9a-f]{64}$`)
	foldOpsNowUnix                = func() int64 { return time.Now().Unix() }
)

type foldOpsProvisionedMarker struct {
	SchemaVersion   int    `json:"schema_version"`
	Role            string `json:"role"`
	ManifestRelease string `json:"manifest_release"`
	ProvisionedUnix int64  `json:"provisioned_at_unix"`
}

func foldOpsProvision() error {
	role, err := readActiveInstallationRole()
	if err != nil {
		return err
	}
	if provisioned, err := loadFoldOpsProvisionedMarker(); err != nil {
		return err
	} else if provisioned != nil {
		fmt.Printf("FoldOps is already provisioned for role %s.\n", provisioned.Role)
		return startFoldOpsRuntimeServices()
	}

	manifest, err := resolveEffectiveFoldOpsManifest()
	if err != nil {
		return err
	}
	packages, err := foldOpsPackagesForRole(manifest, role)
	if err != nil {
		return err
	}
	if !foldOpsHasVerifiedActiveRelease(manifest.ManifestRelease, role, packages) {
		return errors.New("FoldOps packages must be acquired before provision")
	}

	token, importedFromEFI, err := importFoldOpsIngestToken()
	if err != nil {
		return err
	}

	switch role {
	case "supervisor":
		if err := provisionFoldOpsSupervisor(manifest.ManifestRelease, token); err != nil {
			return err
		}
	case "agent":
		if err := provisionFoldOpsAgent(manifest.ManifestRelease, token); err != nil {
			return err
		}
	default:
		return fmt.Errorf("unsupported installation role %q", role)
	}

	if importedFromEFI {
		if err := os.Remove(provisionedFoldOpsIngestToken); err != nil && !os.IsNotExist(err) {
			return fmt.Errorf("remove EFI ingest token staging file: %w", err)
		}
	}

	fmt.Printf("FoldOps provision completed for role %s.\n", role)
	return startFoldOpsRuntimeServices()
}

func startFoldOpsProvisionService() error {
	return startSystemdUnitIfLoaded(foldOpsProvisionServiceName, false)
}

func startFoldOpsRuntimeServices() error {
	for _, unit := range []string{
		foldOpsSupervisorServiceName,
		foldOpsServeHTTPSServiceName,
		foldOpsAgentServiceName,
	} {
		if err := startSystemdUnitIfLoaded(unit, true); err != nil {
			return err
		}
	}
	refreshCommissioningDisplay()
	return nil
}

func startSystemdUnitIfLoaded(unit string, noBlock bool) error {
	state, err := output("systemctl", "show", "-p", "LoadState", "--value", unit)
	if err != nil {
		return fmt.Errorf("inspect %s: %w", unit, err)
	}
	if strings.TrimSpace(state) != "loaded" {
		return nil
	}
	args := []string{"start"}
	if noBlock {
		// Runtime units declare After=foldingos-foldops-provision.service. Starting
		// them synchronously from inside provision would deadlock the oneshot job.
		args = append(args, "--no-block")
	}
	args = append(args, unit)
	if err := run("systemctl", args...); err != nil {
		return fmt.Errorf("start %s: %w", unit, err)
	}
	return nil
}

func importFoldOpsIngestToken() (string, bool, error) {
	if content, err := os.ReadFile(foldOpsIngestTokenPath); err == nil {
		token, err := parseFoldOpsIngestToken(string(content))
		if err != nil {
			return "", false, fmt.Errorf("persistent ingest token is invalid: %w", err)
		}
		return token, false, nil
	} else if !os.IsNotExist(err) {
		return "", false, fmt.Errorf("read persistent ingest token: %w", err)
	}

	content, err := os.ReadFile(provisionedFoldOpsIngestToken)
	if err != nil {
		if os.IsNotExist(err) {
			return "", false, errors.New("foldops ingest token is not staged on EFI")
		}
		return "", false, fmt.Errorf("read EFI ingest token: %w", err)
	}
	token, err := parseFoldOpsIngestToken(string(content))
	if err != nil {
		return "", false, fmt.Errorf("EFI ingest token is invalid: %w", err)
	}
	if err := atomicWrite(foldOpsIngestTokenPath, []byte(token+"\n"), 0600); err != nil {
		return "", false, err
	}
	return token, true, nil
}

func parseFoldOpsIngestToken(content string) (string, error) {
	token := strings.TrimSpace(content)
	if token == "" {
		return "", errors.New("ingest token is empty")
	}
	if strings.Contains(token, "\n") {
		return "", errors.New("ingest token must be a single line")
	}
	if !foldOpsIngestTokenPattern.MatchString(token) {
		return "", errors.New("ingest token must be 64 lowercase hex characters")
	}
	return token, nil
}

func provisionFoldOpsSupervisor(manifestRelease, token string) error {
	if err := ensureFoldOpsTLSMaterial(); err != nil {
		return err
	}
	caBytes, err := os.ReadFile(filepath.Join(foldOpsTLSDir, "ca.pem"))
	if err != nil {
		return fmt.Errorf("read TLS CA material: %w", err)
	}
	if err := atomicWrite(foldOpsSupervisorCAPath, caBytes, 0644); err != nil {
		return err
	}

	supervisorEnv := map[string]string{
		"HOST":         "127.0.0.1",
		"PORT":         strconvItoa(foldOpsSupervisorLoopbackPort),
		"INGEST_TOKEN": token,
		"DB_PATH":      "/data/foldops/foldops.db",
		"WEB_ROOT":     foldOpsWebRoot(),
	}
	if err := writeFoldOpsEnvFile(foldOpsSupervisorEnvPath, supervisorEnv, 0600); err != nil {
		return err
	}

	supervisorHost, err := readHostname()
	if err != nil {
		return err
	}
	agentEnv := map[string]string{
		"SUPERVISOR_URL":    fmt.Sprintf("https://%s:%d", supervisorHost, foldOpsHTTPSPort),
		"SUPERVISOR_TLS_CA": foldOpsSupervisorCAPath,
		"AGENT_TOKEN":       token,
		"FAH_LOG_PATH":      "/data/fah/log.txt",
		"FAH_DB_PATH":       "/data/fah/client.db",
		"FAH_WORK_DIR":      "/data/fah/work",
	}
	if err := writeFoldOpsEnvFile(foldOpsAgentEnvPath, agentEnv, 0600); err != nil {
		return err
	}

	return writeFoldOpsProvisionedMarker("supervisor", manifestRelease)
}

func provisionFoldOpsAgent(manifestRelease, token string) error {
	supervisorURLBytes, err := os.ReadFile(supervisorURLPathDefault)
	if err != nil {
		return fmt.Errorf("read supervisor URL: %w", err)
	}
	host, err := foldOpsSupervisorHostFromURL(string(supervisorURLBytes))
	if err != nil {
		return err
	}
	if _, err := os.Stat(foldOpsSupervisorCAPath); err != nil {
		return fmt.Errorf("supervisor CA trust anchor is missing: %w", err)
	}

	agentEnv := map[string]string{
		"SUPERVISOR_URL":    fmt.Sprintf("https://%s:%d", host, foldOpsHTTPSPort),
		"SUPERVISOR_TLS_CA": foldOpsSupervisorCAPath,
		"AGENT_TOKEN":       token,
		"FAH_LOG_PATH":      "/data/fah/log.txt",
		"FAH_DB_PATH":       "/data/fah/client.db",
		"FAH_WORK_DIR":      "/data/fah/work",
	}
	if err := writeFoldOpsEnvFile(foldOpsAgentEnvPath, agentEnv, 0600); err != nil {
		return err
	}
	return writeFoldOpsProvisionedMarker("agent", manifestRelease)
}

func foldOpsSupervisorHostFromURL(rawURL string) (string, error) {
	rawURL = strings.TrimSpace(rawURL)
	if rawURL == "" {
		return "", errors.New("supervisor URL is empty")
	}
	parsed, err := url.Parse(rawURL)
	if err != nil {
		return "", fmt.Errorf("supervisor URL is invalid: %w", err)
	}
	host := parsed.Hostname()
	if host == "" {
		return "", errors.New("supervisor URL host is empty")
	}
	return host, nil
}

func foldOpsWebRoot() string {
	return filepath.Join(foldOpsAppsRoot, "current", "foldops-web", "usr", "share", "foldops", "web")
}

func foldOpsSupervisorBinary() string {
	return filepath.Join(foldOpsAppsRoot, "current", "foldops-supervisor", "usr", "bin", "foldops-supervisor")
}

func foldOpsAgentBinary() string {
	return filepath.Join(foldOpsAppsRoot, "current", "foldops-agent", "usr", "bin", "foldops-agent")
}

func writeFoldOpsEnvFile(path string, values map[string]string, mode os.FileMode) error {
	keys := make([]string, 0, len(values))
	for key := range values {
		keys = append(keys, key)
	}
	sort.Strings(keys)
	lines := make([]string, 0, len(keys))
	for _, key := range keys {
		value := values[key]
		if strings.ContainsAny(value, "\n\r") {
			return fmt.Errorf("env value for %s must not contain newlines", key)
		}
		lines = append(lines, key+"="+value)
	}
	return atomicWrite(path, []byte(strings.Join(lines, "\n")+"\n"), mode)
}

func writeFoldOpsProvisionedMarker(role, manifestRelease string) error {
	marker := foldOpsProvisionedMarker{
		SchemaVersion:   foldOpsProvisionedSchemaVersion,
		Role:            role,
		ManifestRelease: manifestRelease,
		ProvisionedUnix: foldOpsNowUnix(),
	}
	content, err := json.Marshal(marker)
	if err != nil {
		return fmt.Errorf("encode provisioned marker: %w", err)
	}
	return atomicWrite(foldOpsProvisionedMarkerPath, append(content, '\n'), 0644)
}

func loadFoldOpsProvisionedMarker() (*foldOpsProvisionedMarker, error) {
	content, err := os.ReadFile(foldOpsProvisionedMarkerPath)
	if err != nil {
		if os.IsNotExist(err) {
			return nil, nil
		}
		return nil, fmt.Errorf("read provisioned marker: %w", err)
	}
	marker := foldOpsProvisionedMarker{}
	if err := json.Unmarshal(bytesTrimSpace(content), &marker); err != nil {
		return nil, fmt.Errorf("parse provisioned marker: %w", err)
	}
	if marker.SchemaVersion != foldOpsProvisionedSchemaVersion {
		return nil, errors.New("provisioned marker schema_version is unsupported")
	}
	if marker.Role != "agent" && marker.Role != "supervisor" {
		return nil, errors.New("provisioned marker role is invalid")
	}
	if strings.TrimSpace(marker.ManifestRelease) == "" {
		return nil, errors.New("provisioned marker manifest_release is empty")
	}
	return &marker, nil
}

func foldOpsProvisioned() bool {
	marker, err := loadFoldOpsProvisionedMarker()
	return err == nil && marker != nil
}

func bytesTrimSpace(content []byte) []byte {
	return []byte(strings.TrimSpace(string(content)))
}

func strconvItoa(value int) string {
	return fmt.Sprintf("%d", value)
}
