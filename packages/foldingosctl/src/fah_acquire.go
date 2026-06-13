package main

import (
	"crypto/sha256"
	"encoding/hex"
	"errors"
	"fmt"
	"io"
	"net/http"
	"os"
	"path/filepath"
	"strings"
)

var (
	fahHTTPClient                    = defaultFAHHTTPClient()
	fahCheckAcquisitionPrerequisites = requireFAHAcquisitionPrerequisites
	fahHasVerifiedActiveClient       = hasVerifiedActiveClient
	fahNTPSynchronized               = fahNTPSynchronizedFromTimedatectl
)

func fahAcquire() error {
	manifest, err := loadFAHManifest(embeddedFAHManifestPath)
	if err != nil {
		return err
	}
	if err := validateFoldingOSCompatibility(manifest.MinimumFoldingOSVersion); err != nil {
		return err
	}
	if fahHasVerifiedActiveClient(manifest) {
		fmt.Printf(
			"Verified Folding@home client %s is already active; acquisition not required.\n",
			manifest.ClientVersion,
		)
		return nil
	}
	if err := fahCheckAcquisitionPrerequisites(); err != nil {
		return err
	}
	stagedPath, err := downloadAndStageFAHArtifact(manifest)
	if err != nil {
		return err
	}
	fmt.Printf("Staged verified Folding@home %s artifact at %s.\n", manifest.ClientVersion, stagedPath)

	versionDir, err := extractAndInstallFAHArtifact(manifest)
	if err != nil {
		return err
	}
	fmt.Printf("Installed and verified Folding@home %s at %s.\n", manifest.ClientVersion, versionDir)
	return nil
}

func defaultFAHHTTPClient() *http.Client {
	return &http.Client{
		CheckRedirect: func(req *http.Request, via []*http.Request) error {
			return errors.New("artifact download redirects are not allowed")
		},
	}
}

func fahNTPSynchronizedFromTimedatectl() (bool, error) {
	value, err := output("timedatectl", "show", "-p", "NTPSynchronized", "--value")
	if err != nil {
		return false, err
	}
	return strings.TrimSpace(value) == "yes", nil
}

func requireFAHAcquisitionPrerequisites() error {
	if err := run("systemctl", "is-active", "--quiet", "network-online.target"); err != nil {
		return errors.New("network is not online")
	}
	synchronized, err := fahNTPSynchronized()
	if err != nil {
		return fmt.Errorf("check time synchronization: %w", err)
	}
	if !synchronized {
		return errors.New("system time is not synchronized")
	}
	return nil
}

func hasVerifiedActiveClient(manifest fahManifest) bool {
	version, err := readFAHCurrentVersion()
	if err != nil {
		return false
	}
	return fahInstallationVerified(version, manifest)
}

func readFAHCurrentVersion() (string, error) {
	currentPath := filepath.Join(fahAppsRoot, "current")
	target, err := os.Readlink(currentPath)
	if err != nil {
		return "", err
	}
	if filepath.IsAbs(target) || strings.HasPrefix(target, "/") {
		return "", errors.New("current must be a relative symlink")
	}
	cleaned := filepath.Clean(target)
	if cleaned != target || strings.Contains(target, "..") {
		return "", errors.New("current must not contain path traversal")
	}
	versionDir := filepath.Join(fahAppsRoot, cleaned)
	info, err := os.Stat(versionDir)
	if err != nil || !info.IsDir() {
		return "", errors.New("current does not reference an installed version")
	}
	return cleaned, nil
}

func fahInstallationVerified(version string, manifest fahManifest) bool {
	markerPath := filepath.Join(fahAppsRoot, version, fahVerifiedMarkerName)
	content, err := os.ReadFile(markerPath)
	if err != nil {
		return false
	}
	values := parseKeyValueLines(string(content))
	if values["client_version"] != manifest.ClientVersion {
		return false
	}
	if values["artifact_sha256"] != manifest.SHA256 {
		return false
	}
	executable, err := fahExecutableForVersion(version, manifest.ExecutablePath)
	if err != nil {
		return false
	}
	info, err := os.Stat(executable)
	return err == nil && !info.IsDir()
}

func fahExecutableForVersion(version, manifestExecutablePath string) (string, error) {
	return fahExecutableInRoot(filepath.Join(fahAppsRoot, version), manifestExecutablePath)
}

func parseKeyValueLines(content string) map[string]string {
	values := make(map[string]string)
	for _, line := range strings.Split(content, "\n") {
		line = strings.TrimSpace(line)
		if line == "" || strings.HasPrefix(line, "#") {
			continue
		}
		key, value, ok := strings.Cut(line, "=")
		if !ok {
			continue
		}
		values[strings.TrimSpace(key)] = strings.TrimSpace(value)
	}
	return values
}

func downloadAndStageFAHArtifact(manifest fahManifest) (string, error) {
	if err := os.MkdirAll(fahDownloadsDir, 0755); err != nil {
		return "", fmt.Errorf("create downloads directory: %w", err)
	}

	partialPath := filepath.Join(fahDownloadsDir, manifest.ClientVersion+".partial")
	stagedPath := filepath.Join(fahDownloadsDir, manifest.ClientVersion+".deb")

	if err := os.Remove(partialPath); err != nil && !os.IsNotExist(err) {
		return "", fmt.Errorf("remove stale partial download: %w", err)
	}
	if err := os.Remove(stagedPath); err != nil && !os.IsNotExist(err) {
		return "", fmt.Errorf("remove stale staged artifact: %w", err)
	}

	if err := downloadFAHArtifact(manifest, partialPath); err != nil {
		_ = os.Remove(partialPath)
		return "", err
	}
	if err := verifyFAHArtifactFile(partialPath, manifest); err != nil {
		_ = os.Remove(partialPath)
		return "", err
	}
	if err := os.Rename(partialPath, stagedPath); err != nil {
		_ = os.Remove(partialPath)
		return "", fmt.Errorf("stage verified artifact: %w", err)
	}
	return stagedPath, nil
}

func downloadFAHArtifact(manifest fahManifest, destination string) error {
	request, err := http.NewRequest(http.MethodGet, manifest.ArtifactURL, nil)
	if err != nil {
		return err
	}

	response, err := fahHTTPClient.Do(request)
	if err != nil {
		return fmt.Errorf("download artifact: %w", err)
	}
	defer response.Body.Close()

	if response.Request.URL.String() != manifest.ArtifactURL {
		return errors.New("artifact download resolved to an unexpected URL")
	}
	if response.StatusCode != http.StatusOK {
		return fmt.Errorf("artifact download failed with status %s", response.Status)
	}

	file, err := os.OpenFile(destination, os.O_CREATE|os.O_WRONLY|os.O_TRUNC, 0644)
	if err != nil {
		return fmt.Errorf("open partial download: %w", err)
	}
	defer file.Close()

	limited := io.LimitReader(response.Body, manifest.ArtifactSize+1)
	written, err := io.Copy(file, limited)
	if err != nil {
		return fmt.Errorf("write partial download: %w", err)
	}
	if written > manifest.ArtifactSize {
		return fmt.Errorf(
			"artifact download exceeded expected size %d bytes",
			manifest.ArtifactSize,
		)
	}
	if written != manifest.ArtifactSize {
		return fmt.Errorf(
			"artifact download size %d does not match expected size %d",
			written,
			manifest.ArtifactSize,
		)
	}
	if err := file.Sync(); err != nil {
		return fmt.Errorf("sync partial download: %w", err)
	}
	return nil
}

func verifyFAHArtifactFile(path string, manifest fahManifest) error {
	file, err := os.Open(path)
	if err != nil {
		return err
	}
	defer file.Close()

	hasher := sha256.New()
	limited := io.LimitReader(file, manifest.ArtifactSize+1)
	written, err := io.Copy(hasher, limited)
	if err != nil {
		return fmt.Errorf("hash artifact: %w", err)
	}
	if written != manifest.ArtifactSize {
		return fmt.Errorf(
			"artifact size %d does not match expected size %d",
			written,
			manifest.ArtifactSize,
		)
	}
	digest := hex.EncodeToString(hasher.Sum(nil))
	if digest != manifest.SHA256 {
		return errors.New("artifact SHA-256 digest does not match approved manifest")
	}
	return nil
}
