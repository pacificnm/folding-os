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
	"time"
)

var (
	toolsHTTPClient                    = defaultToolsHTTPClient()
	toolsCheckAcquisitionPrerequisites = requireToolsAcquisitionPrerequisites
	toolsDownloadsDir                  = "/data/state/tools/.downloads"
	toolsRestartDependentUnits         = restartToolsDependentUnits
)

var toolsDependentSystemdUnits = []string{
	"foldingos-provision.service",
	"foldingos-provision-boot.service",
	"foldingos-foldops-serve-https.service",
}

func toolsAcquire() error {
	assignment, found, err := resolveEffectiveToolsAssignment()
	if err != nil {
		return err
	}
	if !found {
		fmt.Println("No supervisor-assigned or bootstrap tools version is configured; image bootstrap foldingosctl remains active.")
		return nil
	}

	if toolsInstallationVerified(assignment) {
		if err := clearToolsAcquireState(); err != nil {
			return err
		}
		fmt.Printf(
			"Verified foldingosctl tools release %s is already active; acquisition not required.\n",
			assignment.ToolsVersion,
		)
		return nil
	}

	state, err := loadToolsAcquireState()
	if err != nil {
		return err
	}
	if deferred, remaining, err := deferToolsAcquisitionAttempt(state); err != nil {
		return err
	} else if deferred {
		fmt.Printf(
			"Tools acquisition deferred for %s (next attempt at %s).\n",
			remaining.Round(time.Second),
			time.Unix(state.NextAttemptUnix, 0).UTC().Format(time.RFC3339),
		)
		return nil
	}

	if err := toolsCheckAcquisitionPrerequisites(); err != nil {
		return recordToolsAcquisitionFailure(err)
	}
	stagedPath, err := downloadAndStageToolsBinary(assignment)
	if err != nil {
		return recordToolsAcquisitionFailure(err)
	}
	fmt.Printf("Staged verified foldingosctl %s artifact at %s.\n", assignment.ToolsVersion, stagedPath)

	if err := atomicReplaceToolsBinary(stagedPath, toolsBinaryPath); err != nil {
		return recordToolsAcquisitionFailure(err)
	}
	digest, err := hashFileAtPath(toolsBinaryPath, assignment.ArtifactSize)
	if err != nil {
		return recordToolsAcquisitionFailure(err)
	}
	if digest != assignment.SHA256 {
		return recordToolsAcquisitionFailure(errors.New("installed tools binary SHA-256 digest does not match approved assignment"))
	}

	activeState := toolsActiveState{
		ToolsVersion:    assignment.ToolsVersion,
		SHA256:          assignment.SHA256,
		InstalledAtUnix: toolsNow().Unix(),
	}
	if err := saveToolsActiveState(activeState); err != nil {
		return recordToolsAcquisitionFailure(err)
	}
	if err := toolsRestartDependentUnits(); err != nil {
		return recordToolsAcquisitionFailure(err)
	}
	if err := clearToolsAcquireState(); err != nil {
		return err
	}
	fmt.Printf("Installed and verified foldingosctl tools release %s at %s.\n", assignment.ToolsVersion, toolsBinaryPath)
	return nil
}

func defaultToolsHTTPClient() *http.Client {
	return &http.Client{
		CheckRedirect: func(req *http.Request, via []*http.Request) error {
			return errors.New("tools artifact download redirects are not allowed")
		},
	}
}

func requireToolsAcquisitionPrerequisites() error {
	if err := run("systemctl", "is-active", "--quiet", "network-online.target"); err != nil {
		return errors.New("network is not online")
	}
	synchronized, err := foldOpsNTPSynchronized()
	if err != nil {
		return fmt.Errorf("check time synchronization: %w", err)
	}
	if !synchronized {
		return errors.New("system time is not synchronized")
	}
	return nil
}

func toolsStagedArtifactPath(assignment toolsAssignment) string {
	return filepath.Join(toolsDownloadsDir, "foldingosctl_"+assignment.ToolsVersion)
}

func downloadAndStageToolsBinary(assignment toolsAssignment) (string, error) {
	if err := os.MkdirAll(toolsDownloadsDir, 0755); err != nil {
		return "", fmt.Errorf("create tools downloads directory: %w", err)
	}

	partialPath := toolsStagedArtifactPath(assignment) + ".partial"
	stagedPath := toolsStagedArtifactPath(assignment)

	if err := os.Remove(partialPath); err != nil && !os.IsNotExist(err) {
		return "", fmt.Errorf("remove stale partial download: %w", err)
	}
	if err := os.Remove(stagedPath); err != nil && !os.IsNotExist(err) {
		return "", fmt.Errorf("remove stale staged artifact: %w", err)
	}

	if err := downloadToolsBinary(assignment, partialPath); err != nil {
		_ = os.Remove(partialPath)
		return "", err
	}
	if err := verifyToolsArtifactFile(partialPath, assignment); err != nil {
		_ = os.Remove(partialPath)
		return "", err
	}
	if err := os.Rename(partialPath, stagedPath); err != nil {
		_ = os.Remove(partialPath)
		return "", fmt.Errorf("stage verified tools artifact: %w", err)
	}
	return stagedPath, nil
}

func downloadToolsBinary(assignment toolsAssignment, destination string) error {
	request, err := http.NewRequest(http.MethodGet, assignment.ArtifactURL, nil)
	if err != nil {
		return err
	}

	response, err := toolsHTTPClient.Do(request)
	if err != nil {
		return fmt.Errorf("download foldingosctl artifact: %w", err)
	}
	defer response.Body.Close()

	if response.Request.URL.String() != assignment.ArtifactURL {
		return errors.New("foldingosctl artifact download resolved to an unexpected URL")
	}
	if response.StatusCode != http.StatusOK {
		return fmt.Errorf("foldingosctl artifact download failed with status %s", response.Status)
	}

	file, err := os.OpenFile(destination, os.O_CREATE|os.O_WRONLY|os.O_TRUNC, 0644)
	if err != nil {
		return fmt.Errorf("open partial download: %w", err)
	}
	defer file.Close()

	limited := io.LimitReader(response.Body, assignment.ArtifactSize+1)
	written, err := io.Copy(file, limited)
	if err != nil {
		return fmt.Errorf("write partial download: %w", err)
	}
	if written > assignment.ArtifactSize {
		return fmt.Errorf(
			"foldingosctl artifact download exceeded expected size %d bytes",
			assignment.ArtifactSize,
		)
	}
	if written != assignment.ArtifactSize {
		return fmt.Errorf(
			"foldingosctl artifact download size %d does not match expected size %d",
			written,
			assignment.ArtifactSize,
		)
	}
	if err := file.Sync(); err != nil {
		return fmt.Errorf("sync partial download: %w", err)
	}
	return nil
}

func verifyToolsArtifactFile(path string, assignment toolsAssignment) error {
	digest, err := hashFileAtPath(path, assignment.ArtifactSize)
	if err != nil {
		return err
	}
	if digest != assignment.SHA256 {
		return errors.New("foldingosctl artifact SHA-256 digest does not match approved assignment")
	}
	return verifyToolsExecutable(path)
}

func hashFileAtPath(path string, expectedSize int64) (string, error) {
	file, err := os.Open(path)
	if err != nil {
		return "", err
	}
	defer file.Close()

	hasher := sha256.New()
	limited := io.LimitReader(file, expectedSize+1)
	written, err := io.Copy(hasher, limited)
	if err != nil {
		return "", fmt.Errorf("hash artifact: %w", err)
	}
	if written != expectedSize {
		return "", fmt.Errorf("artifact size %d does not match expected size %d", written, expectedSize)
	}
	return hex.EncodeToString(hasher.Sum(nil)), nil
}

func restartToolsDependentUnits() error {
	for _, unit := range toolsDependentSystemdUnits {
		if err := run("systemctl", "try-restart", unit); err != nil {
			return fmt.Errorf("restart %s: %w", unit, err)
		}
	}
	return nil
}
