package main

import (
	"bytes"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"io"
	"net/http"
	"net/http/httptest"
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestAuthorizeAgentUpdateSuccess(t *testing.T) {
	root := t.TempDir()
	restore := setUpdateTestPaths(root)
	defer restore()
	writeEnrollmentTokenForStreamTest(root, "test-enrollment-token")
	if _, err := registerAgent(sampleRegistrationRequest("test-enrollment-token")); err != nil {
		t.Fatal(err)
	}
	record, err := loadEnrollmentRecord(testAgentNodeID)
	if err != nil {
		t.Fatal(err)
	}
	record.DesiredImageVersion = "0.2.0"
	if err := saveEnrollmentRecord(record); err != nil {
		t.Fatal(err)
	}
	stageRegistryVersion(t, root, "0.2.0", []byte("b"))

	response, err := authorizeAgentUpdate(updateAuthorizeRequest{
		SchemaVersion:       1,
		NodeID:              testAgentNodeID,
		EnrollmentToken:     "test-enrollment-token",
		CurrentImageVersion: "0.1.0",
		DesiredImageVersion: "0.2.0",
	})
	if err != nil {
		t.Fatal(err)
	}
	if response.ImageVersion != "0.2.0" || response.UpdateSessionID == "" {
		t.Fatalf("response = %+v", response)
	}
}

func TestAuthorizeAgentUpdateRejectsUnassignedVersion(t *testing.T) {
	root := t.TempDir()
	restore := setUpdateTestPaths(root)
	defer restore()
	writeEnrollmentTokenForStreamTest(root, "test-enrollment-token")
	if _, err := registerAgent(sampleRegistrationRequest("test-enrollment-token")); err != nil {
		t.Fatal(err)
	}
	stageRegistryVersion(t, root, "0.2.0", []byte("b"))

	_, err := authorizeAgentUpdate(updateAuthorizeRequest{
		SchemaVersion:       1,
		NodeID:              testAgentNodeID,
		EnrollmentToken:     "test-enrollment-token",
		CurrentImageVersion: "0.1.0",
		DesiredImageVersion: "0.2.0",
	})
	if err == nil || !strings.Contains(err.Error(), "not assigned") {
		t.Fatalf("err = %v", err)
	}
}

func TestValidateUpdateStreamAccess(t *testing.T) {
	root := t.TempDir()
	restore := setUpdateTestPaths(root)
	defer restore()
	writeEnrollmentTokenForStreamTest(root, "test-enrollment-token")
	stageRegistryVersion(t, root, "0.2.0", []byte("b"))

	session := updateSession{
		SchemaVersion:  1,
		SessionID:      "abc123",
		NodeID:         testAgentNodeID,
		ImageVersion:   "0.2.0",
		ImageSHA256:    registryDigest([]byte("b")),
		ImageSizeBytes: releaseImageSizeBytes,
	}
	if err := saveUpdateSession(session); err != nil {
		t.Fatal(err)
	}

	if _, _, err := validateUpdateStreamAccess("abc123", "0.2.0", "test-enrollment-token"); err != nil {
		t.Fatal(err)
	}
}

func TestRecordAgentUpdateStatusAppliedUpdatesCurrentVersion(t *testing.T) {
	root := t.TempDir()
	restore := setUpdateTestPaths(root)
	defer restore()
	writeEnrollmentTokenForStreamTest(root, "test-enrollment-token")
	if _, err := registerAgent(sampleRegistrationRequest("test-enrollment-token")); err != nil {
		t.Fatal(err)
	}
	record, err := loadEnrollmentRecord(testAgentNodeID)
	if err != nil {
		t.Fatal(err)
	}
	record.DesiredImageVersion = "0.2.0"
	if err := saveEnrollmentRecord(record); err != nil {
		t.Fatal(err)
	}

	if err := recordAgentUpdateStatus(testAgentNodeID, "0.2.0", "applied", ""); err != nil {
		t.Fatal(err)
	}
	updated, err := loadEnrollmentRecord(testAgentNodeID)
	if err != nil {
		t.Fatal(err)
	}
	if updated.CurrentImageVersion != "0.2.0" || updated.DesiredImageVersion != "current" {
		t.Fatalf("record = %+v", updated)
	}
	if updated.LastUpdateStatus != "applied" {
		t.Fatalf("last update status = %q", updated.LastUpdateStatus)
	}
}

func TestVerifyStagedUpdateFile(t *testing.T) {
	root := t.TempDir()
	restore := setUpdateTestPaths(root)
	defer restore()
	payload := bytes.Repeat([]byte("c"), int(releaseImageSizeBytes))
	if err := os.WriteFile(stagedUpdateImagePath, payload, 0600); err != nil {
		t.Fatal(err)
	}
	metadata := stagedUpdateMetadata{
		SchemaVersion:  1,
		ImageSHA256:    registryDigest(payload),
		ImageSizeBytes: releaseImageSizeBytes,
	}
	if err := verifyStagedUpdateFile(metadata); err != nil {
		t.Fatal(err)
	}
}

func TestHandleUpdateStatusEndpoint(t *testing.T) {
	root := t.TempDir()
	restore := setUpdateTestPaths(root)
	defer restore()
	writeEnrollmentTokenForStreamTest(root, "test-enrollment-token")
	if _, err := registerAgent(sampleRegistrationRequest("test-enrollment-token")); err != nil {
		t.Fatal(err)
	}

	server := httptest.NewServer(http.HandlerFunc(handleUpdateStatus))
	defer server.Close()

	body, err := json.Marshal(updateStatusRequest{
		SchemaVersion:   1,
		NodeID:          testAgentNodeID,
		EnrollmentToken: "test-enrollment-token",
		ImageVersion:    "0.2.0",
		Status:          "staged",
	})
	if err != nil {
		t.Fatal(err)
	}
	response, err := http.Post(server.URL, "application/json", bytes.NewReader(body))
	if err != nil {
		t.Fatal(err)
	}
	defer response.Body.Close()
	if response.StatusCode != http.StatusOK {
		payload, _ := io.ReadAll(response.Body)
		t.Fatalf("status = %s body = %s", response.Status, payload)
	}
}

func setUpdateTestPaths(root string) func() {
	previousImageSize := releaseImageSizeBytes
	releaseImageSizeBytes = 4096
	restoreProvision := setProvisionPaths(root)
	restoreRegistry := setRegistryPaths(root)
	restoreSessions := setProvisionSessionsDir(filepath.Join(root, "sessions"))
	restoreUpdate := setAgentUpdatePaths(root)
	restoreBootDisk := setHostBootDiskResolver(func() (string, error) {
		return "/dev/vda", nil
	})
	return func() {
		releaseImageSizeBytes = previousImageSize
		restoreBootDisk()
		restoreUpdate()
		restoreSessions()
		restoreRegistry()
		restoreProvision()
	}
}

func setAgentUpdatePaths(root string) func() {
	previous := struct {
		imagePath, metaPath, partialPath string
	}{
		stagedUpdateImagePath,
		stagedUpdateMetaPath,
		stagedUpdatePartialPath,
	}
	stagedUpdateImagePath = filepath.Join(root, "state", "provision", "staged-update.img")
	stagedUpdateMetaPath = filepath.Join(root, "state", "provision", "staged-update.json")
	stagedUpdatePartialPath = filepath.Join(root, "state", "provision", "staged-update.partial")
	if err := os.MkdirAll(filepath.Dir(stagedUpdateImagePath), 0755); err != nil {
		panic(err)
	}
	return func() {
		stagedUpdateImagePath = previous.imagePath
		stagedUpdateMetaPath = previous.metaPath
		stagedUpdatePartialPath = previous.partialPath
	}
}

func stageRegistryVersion(t *testing.T, root, version string, payload []byte) {
	t.Helper()
	if int64(len(payload)) != releaseImageSizeBytes {
		t.Fatalf("payload size = %d", len(payload))
	}
	imagePath := filepath.Join(root, "images", "foldingos-x86_64-"+version+".img")
	if err := os.MkdirAll(filepath.Dir(imagePath), 0755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(imagePath, bytes.Repeat(payload, 1), 0644); err != nil {
		t.Fatal(err)
	}
	entry := registryEntry{
		SchemaVersion:      1,
		FoldingOSVersion:   version,
		GitRevision:        "abc",
		ImageSHA256:        registryDigest(bytes.Repeat(payload, 1)),
		ImageSizeBytes:     releaseImageSizeBytes,
		VerificationMethod: "sha256",
		ImportTimestamp:    "2026-06-13T20:00:00Z",
		RolloutState:       "ready",
		LocalImagePath:     imagePath,
	}
	if err := saveRegistryEntry(entry); err != nil {
		t.Fatal(err)
	}
}

func registryDigest(payload []byte) string {
	digest := sha256.Sum256(payload)
	return hex.EncodeToString(digest[:])
}
