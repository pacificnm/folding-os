package main

import (
	"bytes"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"net/http"
	"os"
	"path/filepath"
	"strings"
	"time"
)

const (
	stagedUpdateImagePathDefault   = "/data/state/provision/staged-update.img"
	stagedUpdateMetaPathDefault    = "/data/state/provision/staged-update.json"
	stagedUpdatePartialPathDefault = "/data/state/provision/staged-update.partial"
	updateGrubEntryName            = "FoldingOS Update"
	updateGrubEnvPath              = "/boot/efi/EFI/BOOT/grubenv"
	updateBootAssetsDir            = "/boot/efi/foldingos/update"
	sharedUpdateVmlinuzPath        = "/usr/share/foldingos/boot/vmlinuz"
	sharedUpdateInitramfsPath      = "/usr/share/foldingos/boot/install-initramfs.cpio.gz"
	updateSessionHeader            = "X-FoldingOS-Update-Session"
)

var (
	stagedUpdateImagePath      = stagedUpdateImagePathDefault
	stagedUpdateMetaPath       = stagedUpdateMetaPathDefault
	stagedUpdatePartialPath    = stagedUpdatePartialPathDefault
	scheduleUpdateRebootFn    = scheduleUpdateReboot
	applyStagedUpdateOfflineFn = applyStagedUpdateOffline
)

type stagedUpdateMetadata struct {
	SchemaVersion   int    `json:"schema_version"`
	NodeID          string `json:"node_id"`
	CurrentVersion  string `json:"current_version"`
	DesiredVersion  string `json:"desired_version"`
	ImageSHA256     string `json:"image_sha256"`
	ImageSizeBytes  int64  `json:"image_size_bytes"`
	BootDisk        string `json:"boot_disk"`
	StagedAt        string `json:"staged_at"`
}

type updateAuthorizeRequest struct {
	SchemaVersion       int    `json:"schema_version"`
	NodeID              string `json:"node_id"`
	EnrollmentToken     string `json:"enrollment_token"`
	CurrentImageVersion string `json:"current_image_version"`
	DesiredImageVersion string `json:"desired_image_version"`
}

type updateAuthorizeResponse struct {
	SchemaVersion    int    `json:"schema_version"`
	UpdateSessionID  string `json:"update_session_id"`
	ImageVersion     string `json:"image_version"`
	ImageSizeBytes   int64  `json:"image_size_bytes"`
	ImageSHA256      string `json:"image_sha256"`
	ImageStreamPath  string `json:"image_stream_path"`
}

type updateStatusRequest struct {
	SchemaVersion   int    `json:"schema_version"`
	NodeID          string `json:"node_id"`
	EnrollmentToken string `json:"enrollment_token"`
	ImageVersion    string `json:"image_version"`
	Status          string `json:"status"`
	Message         string `json:"message,omitempty"`
}

type updateSession struct {
	SchemaVersion  int    `json:"schema_version"`
	SessionID      string `json:"session_id"`
	CreatedAt      string `json:"created_at"`
	NodeID         string `json:"node_id"`
	ImageVersion   string `json:"image_version"`
	ImageSHA256    string `json:"image_sha256"`
	ImageSizeBytes int64  `json:"image_size_bytes"`
	Completed      bool   `json:"completed"`
}

var validUpdateStatuses = map[string]struct{}{
	"staging":  {},
	"staged":   {},
	"applying": {},
	"applied":  {},
	"failed":   {},
}

func updateSessionPath(sessionID string) string {
	return filepath.Join(provisionSessionsDir, "update-"+sessionID+".json")
}

func authorizeAgentUpdate(request updateAuthorizeRequest) (updateAuthorizeResponse, error) {
	if request.SchemaVersion != 1 {
		return updateAuthorizeResponse{}, fmt.Errorf("unsupported update authorize schema version %d", request.SchemaVersion)
	}
	if err := validateEnrollmentToken(strings.TrimSpace(request.EnrollmentToken)); err != nil {
		return updateAuthorizeResponse{}, err
	}
	nodeID := strings.TrimSpace(request.NodeID)
	if !uuidPattern.MatchString(nodeID) {
		return updateAuthorizeResponse{}, errors.New("node_id is invalid")
	}
	currentVersion := strings.TrimSpace(request.CurrentImageVersion)
	if currentVersion == "" {
		return updateAuthorizeResponse{}, errors.New("current_image_version is required")
	}
	desiredVersion := strings.TrimSpace(request.DesiredImageVersion)
	if desiredVersion == "" || desiredVersion == "current" {
		return updateAuthorizeResponse{}, errors.New("desired_image_version is required")
	}
	if desiredVersion == currentVersion {
		return updateAuthorizeResponse{}, errors.New("desired image version matches the current image")
	}

	record, err := loadEnrollmentRecord(nodeID)
	if err != nil {
		if os.IsNotExist(err) {
			return updateAuthorizeResponse{}, errors.New("agent is not registered")
		}
		return updateAuthorizeResponse{}, err
	}
	if record.DesiredImageVersion != desiredVersion {
		return updateAuthorizeResponse{}, fmt.Errorf(
			"desired image version %q is not assigned to node %s",
			desiredVersion,
			nodeID,
		)
	}
	entry, err := loadRegistryEntry(desiredVersion)
	if err != nil {
		return updateAuthorizeResponse{}, fmt.Errorf("desired image version %q is not in registry: %w", desiredVersion, err)
	}
	if entry.RolloutState != "ready" {
		return updateAuthorizeResponse{}, fmt.Errorf("image version %q is not ready for rollout", desiredVersion)
	}
	if err := verifyRegistryImageFile(entry.LocalImagePath, entry.ImageSHA256, entry.ImageSizeBytes); err != nil {
		return updateAuthorizeResponse{}, fmt.Errorf("registry image for %s is invalid: %w", desiredVersion, err)
	}

	sessionID, err := newInstallSessionID()
	if err != nil {
		return updateAuthorizeResponse{}, err
	}
	session := updateSession{
		SchemaVersion:  1,
		SessionID:      sessionID,
		CreatedAt:      time.Now().UTC().Format(time.RFC3339),
		NodeID:         nodeID,
		ImageVersion:   entry.FoldingOSVersion,
		ImageSHA256:    entry.ImageSHA256,
		ImageSizeBytes: entry.ImageSizeBytes,
	}
	if err := saveUpdateSession(session); err != nil {
		return updateAuthorizeResponse{}, err
	}
	if err := recordAgentUpdateStatus(nodeID, desiredVersion, "staging", ""); err != nil {
		return updateAuthorizeResponse{}, err
	}

	return updateAuthorizeResponse{
		SchemaVersion:   1,
		UpdateSessionID: sessionID,
		ImageVersion:    entry.FoldingOSVersion,
		ImageSizeBytes:    entry.ImageSizeBytes,
		ImageSHA256:     entry.ImageSHA256,
		ImageStreamPath: fmt.Sprintf("/v1/provision/images/%s/stream", entry.FoldingOSVersion),
	}, nil
}

func saveUpdateSession(session updateSession) error {
	content, err := json.MarshalIndent(session, "", "  ")
	if err != nil {
		return err
	}
	return atomicWrite(updateSessionPath(session.SessionID), append(content, '\n'), 0600)
}

func loadUpdateSession(sessionID string) (updateSession, error) {
	content, err := os.ReadFile(updateSessionPath(sessionID))
	if err != nil {
		return updateSession{}, err
	}
	var session updateSession
	if err := json.Unmarshal(content, &session); err != nil {
		return updateSession{}, fmt.Errorf("invalid update session: %w", err)
	}
	if session.SchemaVersion != 1 {
		return updateSession{}, fmt.Errorf("unsupported update session schema version %d", session.SchemaVersion)
	}
	if strings.TrimSpace(session.SessionID) == "" {
		return updateSession{}, errors.New("update session is missing session_id")
	}
	return session, nil
}

func validateUpdateStreamAccess(sessionID, version, enrollmentToken string) (updateSession, registryEntry, error) {
	if err := validateEnrollmentToken(strings.TrimSpace(enrollmentToken)); err != nil {
		return updateSession{}, registryEntry{}, err
	}
	session, err := loadUpdateSession(strings.TrimSpace(sessionID))
	if err != nil {
		if os.IsNotExist(err) {
			return updateSession{}, registryEntry{}, errors.New("update session is invalid")
		}
		return updateSession{}, registryEntry{}, err
	}
	if session.Completed {
		return updateSession{}, registryEntry{}, errors.New("update session is already completed")
	}
	version = strings.TrimSpace(version)
	if session.ImageVersion != version {
		return updateSession{}, registryEntry{}, fmt.Errorf("update session does not authorize image version %q", version)
	}
	entry, err := loadRegistryEntry(version)
	if err != nil {
		return updateSession{}, registryEntry{}, err
	}
	if err := verifyRegistryImageFile(entry.LocalImagePath, entry.ImageSHA256, entry.ImageSizeBytes); err != nil {
		return updateSession{}, registryEntry{}, fmt.Errorf("registry image for %s is invalid: %w", version, err)
	}
	return session, entry, nil
}

func recordAgentUpdateStatus(nodeID, version, status, message string) error {
	status = strings.TrimSpace(status)
	if _, ok := validUpdateStatuses[status]; !ok {
		return fmt.Errorf("unsupported update status %q", status)
	}
	record, err := loadEnrollmentRecord(nodeID)
	if err != nil {
		return err
	}
	record.LastUpdateStatus = status
	record.LastUpdateVersion = strings.TrimSpace(version)
	record.LastUpdateMessage = strings.TrimSpace(message)
	record.LastUpdateAt = time.Now().UTC().Format(time.RFC3339)
	if status == "applied" {
		record.CurrentImageVersion = record.LastUpdateVersion
		record.FoldingOSVersion = record.LastUpdateVersion
		if record.DesiredImageVersion == record.LastUpdateVersion {
			record.DesiredImageVersion = "current"
		}
	}
	return saveEnrollmentRecord(record)
}

func reportAgentUpdateStatus(supervisorURL, nodeID, token, version, status, message string) error {
	if supervisorURL == "" {
		return nil
	}
	endpoint, err := joinSupervisorURL(supervisorURL, "/v1/agents/update/status")
	if err != nil {
		return err
	}
	body, err := json.Marshal(updateStatusRequest{
		SchemaVersion:   1,
		NodeID:          nodeID,
		EnrollmentToken: token,
		ImageVersion:    version,
		Status:          status,
		Message:         message,
	})
	if err != nil {
		return err
	}
	request, err := http.NewRequest(http.MethodPost, endpoint, bytes.NewReader(body))
	if err != nil {
		return err
	}
	request.Header.Set("Content-Type", "application/json")
	response, err := provisionHTTPClient.Do(request)
	if err != nil {
		return err
	}
	defer response.Body.Close()
	responseBody, err := io.ReadAll(io.LimitReader(response.Body, 1<<20))
	if err != nil {
		return err
	}
	if response.StatusCode != http.StatusOK {
		return fmt.Errorf(
			"update status report failed with status %s: %s",
			response.Status,
			strings.TrimSpace(string(responseBody)),
		)
	}
	return nil
}

func provisionCheckVersionAndStage() error {
	if err := requireAgentRole(); err != nil {
		return err
	}

	nodeID, err := agentEnrollmentNodeID()
	if err != nil {
		if os.IsNotExist(err) {
			fmt.Println("current")
			return nil
		}
		return err
	}

	currentVersion, err := installedFoldingOSVersionReader()
	if err != nil {
		return err
	}

	supervisorURL, err := readSupervisorBaseURL()
	if err != nil {
		return err
	}
	if supervisorURL == "" {
		fmt.Println("current")
		return nil
	}
	token, err := readEnrollmentToken()
	if err != nil {
		fmt.Printf("current\n")
		return nil
	}

	desired, err := queryDesiredVersion(supervisorURL, nodeID, token)
	if err != nil {
		fmt.Printf("current\n")
		return nil
	}
	if desired == "current" || desired == currentVersion {
		if err := clearStagedUpdate(); err != nil {
			return err
		}
		fmt.Println("current")
		return nil
	}

	if staged, err := loadStagedUpdateMetadata(); err == nil {
		if staged.DesiredVersion == desired && staged.CurrentVersion == currentVersion {
			if err := verifyStagedUpdateFile(staged); err == nil {
				fmt.Println(desired)
				return nil
			}
		}
		if err := clearStagedUpdate(); err != nil {
			return err
		}
	} else if !os.IsNotExist(err) {
		return err
	}

	if err := stageAgentUpdate(supervisorURL, nodeID, token, currentVersion, desired); err != nil {
		_ = reportAgentUpdateStatus(supervisorURL, nodeID, token, desired, "failed", err.Error())
		return err
	}
	fmt.Println(desired)
	return nil
}

func queryDesiredVersion(supervisorURL, nodeID, token string) (string, error) {
	endpoint, err := joinSupervisorURL(supervisorURL, "/v1/agents/desired-version?node_id="+nodeID)
	if err != nil {
		return "", err
	}
	request, err := http.NewRequest(http.MethodGet, endpoint, nil)
	if err != nil {
		return "", err
	}
	request.Header.Set("X-FoldingOS-Enrollment-Token", token)
	response, err := provisionHTTPClient.Do(request)
	if err != nil {
		return "", err
	}
	defer response.Body.Close()
	body, err := io.ReadAll(io.LimitReader(response.Body, 1<<20))
	if err != nil {
		return "", err
	}
	if response.StatusCode != http.StatusOK {
		return "", fmt.Errorf(
			"desired-version query failed with status %s: %s",
			response.Status,
			strings.TrimSpace(string(body)),
		)
	}
	var result desiredVersionResponse
	if err := json.Unmarshal(body, &result); err != nil {
		return "", err
	}
	return strings.TrimSpace(result.DesiredVersion), nil
}

func stageAgentUpdate(supervisorURL, nodeID, token, currentVersion, desiredVersion string) error {
	authorizeURL, err := joinSupervisorURL(supervisorURL, "/v1/agents/update/authorize")
	if err != nil {
		return err
	}
	body, err := json.Marshal(updateAuthorizeRequest{
		SchemaVersion:       1,
		NodeID:              nodeID,
		EnrollmentToken:     token,
		CurrentImageVersion: currentVersion,
		DesiredImageVersion: desiredVersion,
	})
	if err != nil {
		return err
	}
	request, err := http.NewRequest(http.MethodPost, authorizeURL, bytes.NewReader(body))
	if err != nil {
		return err
	}
	request.Header.Set("Content-Type", "application/json")
	response, err := provisionHTTPClient.Do(request)
	if err != nil {
		return err
	}
	defer response.Body.Close()
	payload, err := io.ReadAll(io.LimitReader(response.Body, 1<<20))
	if err != nil {
		return err
	}
	if response.StatusCode != http.StatusOK {
		return fmt.Errorf(
			"update authorization failed with status %s: %s",
			response.Status,
			strings.TrimSpace(string(payload)),
		)
	}
	var authorization updateAuthorizeResponse
	if err := json.Unmarshal(payload, &authorization); err != nil {
		return err
	}

	bootDisk, err := resolveHostBootDisk()
	if err != nil {
		return err
	}
	if bootDisk == "" {
		return errors.New("host boot disk is unavailable")
	}

	if err := os.MkdirAll(filepath.Dir(stagedUpdateImagePath), 0755); err != nil {
		return err
	}
	if err := os.Remove(stagedUpdatePartialPath); err != nil && !os.IsNotExist(err) {
		return fmt.Errorf("remove stale staged update partial: %w", err)
	}

	streamURL, err := joinSupervisorURL(supervisorURL, authorization.ImageStreamPath)
	if err != nil {
		return err
	}
	streamRequest, err := http.NewRequest(http.MethodGet, streamURL, nil)
	if err != nil {
		return err
	}
	streamRequest.Header.Set("X-FoldingOS-Enrollment-Token", token)
	streamRequest.Header.Set(updateSessionHeader, authorization.UpdateSessionID)
	streamResponse, err := provisionHTTPClient.Do(streamRequest)
	if err != nil {
		return err
	}
	defer streamResponse.Body.Close()
	if streamResponse.StatusCode != http.StatusOK {
		body, _ := io.ReadAll(io.LimitReader(streamResponse.Body, 1<<20))
		return fmt.Errorf(
			"update image stream failed with status %s: %s",
			streamResponse.Status,
			strings.TrimSpace(string(body)),
		)
	}

	digest, written, err := writeStagedUpdateImage(
		streamResponse.Body,
		authorization.ImageSizeBytes,
	)
	if err != nil {
		return err
	}
	if written != authorization.ImageSizeBytes {
		return fmt.Errorf("staged update size %d does not match expected %d", written, authorization.ImageSizeBytes)
	}
	if !strings.EqualFold(digest, authorization.ImageSHA256) {
		_ = os.Remove(stagedUpdateImagePath)
		return errors.New("staged update failed SHA-256 verification")
	}

	metadata := stagedUpdateMetadata{
		SchemaVersion:  1,
		NodeID:         nodeID,
		CurrentVersion: currentVersion,
		DesiredVersion: desiredVersion,
		ImageSHA256:    authorization.ImageSHA256,
		ImageSizeBytes: authorization.ImageSizeBytes,
		BootDisk:       bootDisk,
		StagedAt:       time.Now().UTC().Format(time.RFC3339),
	}
	if err := saveStagedUpdateMetadata(metadata); err != nil {
		return err
	}
	return reportAgentUpdateStatus(supervisorURL, nodeID, token, desiredVersion, "staged", "")
}

func writeStagedUpdateImage(source io.Reader, size int64) (string, int64, error) {
	file, err := os.OpenFile(stagedUpdatePartialPath, os.O_CREATE|os.O_WRONLY|os.O_TRUNC, 0600)
	if err != nil {
		return "", 0, err
	}
	defer file.Close()

	hasher := sha256.New()
	written, err := io.CopyN(file, io.TeeReader(source, hasher), size)
	if err != nil {
		return "", written, fmt.Errorf("write staged update image: %w", err)
	}
	if err := file.Sync(); err != nil {
		return "", written, err
	}
	if err := file.Close(); err != nil {
		return "", written, err
	}
	if err := os.Rename(stagedUpdatePartialPath, stagedUpdateImagePath); err != nil {
		return "", written, err
	}
	return hex.EncodeToString(hasher.Sum(nil)), written, nil
}

func saveStagedUpdateMetadata(metadata stagedUpdateMetadata) error {
	content, err := json.MarshalIndent(metadata, "", "  ")
	if err != nil {
		return err
	}
	return atomicWrite(stagedUpdateMetaPath, append(content, '\n'), 0600)
}

func loadStagedUpdateMetadata() (stagedUpdateMetadata, error) {
	content, err := os.ReadFile(stagedUpdateMetaPath)
	if err != nil {
		return stagedUpdateMetadata{}, err
	}
	var metadata stagedUpdateMetadata
	if err := json.Unmarshal(content, &metadata); err != nil {
		return stagedUpdateMetadata{}, fmt.Errorf("invalid staged update metadata: %w", err)
	}
	if metadata.SchemaVersion != 1 {
		return stagedUpdateMetadata{}, fmt.Errorf("unsupported staged update schema version %d", metadata.SchemaVersion)
	}
	return metadata, nil
}

func verifyStagedUpdateFile(metadata stagedUpdateMetadata) error {
	file, err := os.Open(stagedUpdateImagePath)
	if err != nil {
		return err
	}
	defer file.Close()
	info, err := file.Stat()
	if err != nil {
		return err
	}
	if info.Size() != metadata.ImageSizeBytes {
		return fmt.Errorf("staged update size %d does not match metadata %d", info.Size(), metadata.ImageSizeBytes)
	}
	hasher := sha256.New()
	if _, err := io.Copy(hasher, file); err != nil {
		return err
	}
	digest := hex.EncodeToString(hasher.Sum(nil))
	if !strings.EqualFold(digest, metadata.ImageSHA256) {
		return errors.New("staged update failed SHA-256 verification")
	}
	return nil
}

func clearStagedUpdate() error {
	if err := os.Remove(stagedUpdateImagePath); err != nil && !os.IsNotExist(err) {
		return err
	}
	if err := os.Remove(stagedUpdateMetaPath); err != nil && !os.IsNotExist(err) {
		return err
	}
	if err := os.Remove(stagedUpdatePartialPath); err != nil && !os.IsNotExist(err) {
		return err
	}
	return nil
}

func provisionApplyUpdate(args []string) error {
	offline := false
	for _, arg := range args {
		switch arg {
		case "--offline":
			offline = true
		default:
			return fmt.Errorf("unknown apply-update option %q", arg)
		}
	}
	if offline {
		return applyStagedUpdateOfflineFn()
	}
	return scheduleStagedUpdateApply()
}

func scheduleStagedUpdateApply() error {
	if err := requireAgentRole(); err != nil {
		return err
	}
	metadata, err := loadStagedUpdateMetadata()
	if err != nil {
		if os.IsNotExist(err) {
			fmt.Println("No staged update is pending.")
			return nil
		}
		return err
	}
	if err := verifyStagedUpdateFile(metadata); err != nil {
		return err
	}
	if err := ensureUpdateBootAssets(); err != nil {
		return err
	}
	supervisorURL, _ := readSupervisorBaseURL()
	token, _ := readEnrollmentToken()
	_ = reportAgentUpdateStatus(supervisorURL, metadata.NodeID, token, metadata.DesiredVersion, "applying", "")

	if err := scheduleUpdateRebootFn(); err != nil {
		return err
	}
	fmt.Println("Scheduled staged update apply on reboot.")
	return nil
}

func scheduleUpdateReboot() error {
	if _, err := os.Stat(updateGrubEnvPath); err != nil {
		return fmt.Errorf("grub environment is unavailable: %w", err)
	}
	if err := setGrubEnvVar(updateGrubEnvPath, "next_entry", updateGrubEntryName); err != nil {
		return fmt.Errorf("schedule update boot entry: %w", err)
	}
	if err := run("sync"); err != nil {
		return err
	}
	return run("systemctl", "reboot")
}

func ensureUpdateBootAssets() error {
	if err := os.MkdirAll(updateBootAssetsDir, 0755); err != nil {
		return err
	}
	assets := []struct {
		source, destination string
	}{
		{sharedUpdateVmlinuzPath, filepath.Join(updateBootAssetsDir, "vmlinuz")},
		{sharedUpdateInitramfsPath, filepath.Join(updateBootAssetsDir, "install-initramfs.cpio.gz")},
	}
	for _, asset := range assets {
		if err := copyRegularFile(asset.source, asset.destination); err != nil {
			return fmt.Errorf("stage update boot asset %q: %w", filepath.Base(asset.destination), err)
		}
	}
	return nil
}

func applyStagedUpdateOffline() error {
	metadata, err := loadStagedUpdateMetadata()
	if err != nil {
		return err
	}
	if err := verifyStagedUpdateFile(metadata); err != nil {
		return err
	}
	bootDisk := strings.TrimSpace(metadata.BootDisk)
	if bootDisk == "" {
		bootDisk, err = resolveHostBootDisk()
		if err != nil {
			return err
		}
	}
	if bootDisk == "" {
		return errors.New("host boot disk is unavailable")
	}

	targetEFI := partitionDevice(bootDisk, "1")
	targetRoot := partitionDevice(bootDisk, "2")

	if err := copyStagedReleaseImageEFIPartition(stagedUpdateImagePath, targetEFI); err != nil {
		return fmt.Errorf("copy EFI partition: %w", err)
	}
	if err := copyStagedReleaseImageRootPartition(stagedUpdateImagePath, targetRoot); err != nil {
		return fmt.Errorf("copy root partition: %w", err)
	}
	if err := run("sync"); err != nil {
		return err
	}

	supervisorURL, _ := readSupervisorBaseURL()
	token, _ := readEnrollmentToken()
	_ = reportAgentUpdateStatus(supervisorURL, metadata.NodeID, token, metadata.DesiredVersion, "applied", "")
	if err := clearStagedUpdate(); err != nil {
		return err
	}

	fmt.Println("Applied staged FoldingOS update; rebooting.")
	if err := run("sync"); err != nil {
		return err
	}
	return run("/bin/busybox", "reboot", "-f")
}
