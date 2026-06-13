package main

import (
	"crypto/rand"
	"encoding/hex"
	"encoding/json"
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"time"
)

const (
	provisionSessionsDirDefault = "/data/provision/sessions"
	agentInstallationRole       = "agent"
)

var provisionSessionsDir = provisionSessionsDirDefault

type provisionAuthorizeRequest struct {
	SchemaVersion   int      `json:"schema_version"`
	EnrollmentToken string   `json:"enrollment_token"`
	MACAddresses    []string `json:"mac_addresses"`
	TargetDisk      string   `json:"target_disk"`
	TargetSerial    string   `json:"target_serial"`
	ImageVersion    string   `json:"image_version,omitempty"`
}

type provisionAuthorizeResponse struct {
	SchemaVersion      int    `json:"schema_version"`
	InstallSessionID   string `json:"install_session_id"`
	ImageVersion       string `json:"image_version"`
	ImageSizeBytes     int64  `json:"image_size_bytes"`
	ImageSHA256        string `json:"image_sha256"`
	ImageStreamPath    string `json:"image_stream_path"`
	InstallationRole   string `json:"installation_role"`
	AuthorizedKeys     string `json:"authorized_keys"`
	RebootRequired     bool   `json:"reboot_required"`
	TargetDisk         string `json:"target_disk"`
	TargetSerial       string `json:"target_serial"`
}

type installSession struct {
	SchemaVersion    int      `json:"schema_version"`
	SessionID        string   `json:"session_id"`
	CreatedAt        string   `json:"created_at"`
	MACAddresses     []string `json:"mac_addresses"`
	TargetDisk       string   `json:"target_disk"`
	TargetSerial     string   `json:"target_serial"`
	ImageVersion     string   `json:"image_version"`
	ImageSHA256      string   `json:"image_sha256"`
	ImageSizeBytes   int64    `json:"image_size_bytes"`
	AuthorizedKeys   string   `json:"authorized_keys"`
	Completed        bool     `json:"completed"`
}

func installSessionPath(sessionID string) string {
	return filepath.Join(provisionSessionsDir, sessionID+".json")
}

func authorizeProvisionInstall(request provisionAuthorizeRequest) (provisionAuthorizeResponse, error) {
	if request.SchemaVersion != 1 {
		return provisionAuthorizeResponse{}, fmt.Errorf("unsupported authorize schema version %d", request.SchemaVersion)
	}
	if err := validateEnrollmentToken(strings.TrimSpace(request.EnrollmentToken)); err != nil {
		return provisionAuthorizeResponse{}, err
	}
	macAddresses := normalizeMACAddresses(request.MACAddresses)
	if len(macAddresses) == 0 {
		return provisionAuthorizeResponse{}, errors.New("at least one MAC address is required")
	}
	targetDisk := strings.TrimSpace(request.TargetDisk)
	if targetDisk == "" {
		return provisionAuthorizeResponse{}, errors.New("target_disk is required")
	}
	disk, err := validateProvisionTargetDisk(targetDisk)
	if err != nil {
		return provisionAuthorizeResponse{}, err
	}
	requestedSerial := strings.TrimSpace(request.TargetSerial)
	if requestedSerial == "" {
		return provisionAuthorizeResponse{}, errors.New("target_serial is required")
	}
	if !strings.EqualFold(requestedSerial, disk.Serial) {
		return provisionAuthorizeResponse{}, fmt.Errorf(
			"target serial %q does not match disk %q serial %q",
			requestedSerial,
			targetDisk,
			disk.Serial,
		)
	}

	version := strings.TrimSpace(request.ImageVersion)
	entry, err := resolveProvisionImageVersion(version)
	if err != nil {
		return provisionAuthorizeResponse{}, err
	}
	if entry.RolloutState != "ready" {
		return provisionAuthorizeResponse{}, fmt.Errorf("image version %q is not ready for provisioning", entry.FoldingOSVersion)
	}
	if err := verifyRegistryImageFile(entry.LocalImagePath, entry.ImageSHA256, entry.ImageSizeBytes); err != nil {
		return provisionAuthorizeResponse{}, fmt.Errorf("registry image for %s is invalid: %w", entry.FoldingOSVersion, err)
	}

	authorizedKeys, err := readSupervisorAuthorizedKeys()
	if err != nil {
		return provisionAuthorizeResponse{}, err
	}

	sessionID, err := newInstallSessionID()
	if err != nil {
		return provisionAuthorizeResponse{}, err
	}
	session := installSession{
		SchemaVersion:  1,
		SessionID:      sessionID,
		CreatedAt:      time.Now().UTC().Format(time.RFC3339),
		MACAddresses:   macAddresses,
		TargetDisk:     disk.Path,
		TargetSerial:   disk.Serial,
		ImageVersion:   entry.FoldingOSVersion,
		ImageSHA256:    entry.ImageSHA256,
		ImageSizeBytes: entry.ImageSizeBytes,
		AuthorizedKeys: authorizedKeys,
	}
	if err := saveInstallSession(session); err != nil {
		return provisionAuthorizeResponse{}, err
	}

	return provisionAuthorizeResponse{
		SchemaVersion:    1,
		InstallSessionID: sessionID,
		ImageVersion:     entry.FoldingOSVersion,
		ImageSizeBytes:   entry.ImageSizeBytes,
		ImageSHA256:      entry.ImageSHA256,
		ImageStreamPath:  fmt.Sprintf("/v1/provision/images/%s/stream", entry.FoldingOSVersion),
		InstallationRole: agentInstallationRole,
		AuthorizedKeys:   authorizedKeys,
		RebootRequired:   true,
		TargetDisk:       disk.Path,
		TargetSerial:     disk.Serial,
	}, nil
}

func resolveProvisionImageVersion(version string) (registryEntry, error) {
	if version != "" {
		return loadRegistryEntry(version)
	}
	index, err := loadRegistryIndex()
	if err != nil {
		return registryEntry{}, err
	}
	if len(index.Versions) == 0 {
		return registryEntry{}, errors.New("supervisor registry has no release images")
	}
	installed, err := installedFoldingOSVersionReader()
	if err == nil {
		if entry, err := loadRegistryEntry(installed); err == nil && entry.RolloutState == "ready" {
			return entry, nil
		}
	}
	var latestReady registryEntry
	found := false
	for _, candidate := range index.Versions {
		entry, err := loadRegistryEntry(candidate)
		if err != nil {
			return registryEntry{}, err
		}
		if entry.RolloutState != "ready" {
			continue
		}
		latestReady = entry
		found = true
	}
	if !found {
		return registryEntry{}, errors.New("supervisor registry has no ready release images")
	}
	return latestReady, nil
}

func readSupervisorAuthorizedKeys() (string, error) {
	content, err := os.ReadFile(activeKeys)
	if err != nil {
		if os.IsNotExist(err) {
			return "", errors.New("supervisor administrator authorized keys are not configured")
		}
		return "", err
	}
	keys, err := validateAuthorizedKeys(content)
	if err != nil {
		return "", fmt.Errorf("supervisor administrator authorized keys are invalid: %w", err)
	}
	return string(keys), nil
}

func newInstallSessionID() (string, error) {
	value := make([]byte, 16)
	if _, err := rand.Read(value); err != nil {
		return "", err
	}
	return hex.EncodeToString(value), nil
}

func saveInstallSession(session installSession) error {
	content, err := json.MarshalIndent(session, "", "  ")
	if err != nil {
		return err
	}
	return atomicWrite(installSessionPath(session.SessionID), append(content, '\n'), 0600)
}

func loadInstallSession(sessionID string) (installSession, error) {
	content, err := os.ReadFile(installSessionPath(sessionID))
	if err != nil {
		return installSession{}, err
	}
	var session installSession
	if err := json.Unmarshal(content, &session); err != nil {
		return installSession{}, fmt.Errorf("invalid install session: %w", err)
	}
	if session.SchemaVersion != 1 {
		return installSession{}, fmt.Errorf("unsupported install session schema version %d", session.SchemaVersion)
	}
	if strings.TrimSpace(session.SessionID) == "" {
		return installSession{}, errors.New("install session is missing session_id")
	}
	return session, nil
}

func validateInstallStreamAccess(sessionID, version, enrollmentToken string) (installSession, registryEntry, error) {
	if err := validateEnrollmentToken(strings.TrimSpace(enrollmentToken)); err != nil {
		return installSession{}, registryEntry{}, err
	}
	session, err := loadInstallSession(strings.TrimSpace(sessionID))
	if err != nil {
		if os.IsNotExist(err) {
			return installSession{}, registryEntry{}, errors.New("install session is invalid")
		}
		return installSession{}, registryEntry{}, err
	}
	if session.Completed {
		return installSession{}, registryEntry{}, errors.New("install session is already completed")
	}
	version = strings.TrimSpace(version)
	if session.ImageVersion != version {
		return installSession{}, registryEntry{}, fmt.Errorf("install session does not authorize image version %q", version)
	}
	entry, err := loadRegistryEntry(version)
	if err != nil {
		return installSession{}, registryEntry{}, err
	}
	if err := verifyRegistryImageFile(entry.LocalImagePath, entry.ImageSHA256, entry.ImageSizeBytes); err != nil {
		return installSession{}, registryEntry{}, fmt.Errorf("registry image for %s is invalid: %w", version, err)
	}
	return session, entry, nil
}

func normalizeMACAddresses(addresses []string) []string {
	seen := make(map[string]struct{}, len(addresses))
	var normalized []string
	for _, address := range addresses {
		address = strings.ToLower(strings.TrimSpace(address))
		if address == "" {
			continue
		}
		if _, ok := seen[address]; ok {
			continue
		}
		seen[address] = struct{}{}
		normalized = append(normalized, address)
	}
	sortStrings(normalized)
	return normalized
}
