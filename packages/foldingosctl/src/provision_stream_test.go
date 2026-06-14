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

func TestAuthorizeProvisionInstallRejectsInvalidToken(t *testing.T) {
	root := t.TempDir()
	restore := setProvisionStreamPaths(root)
	defer restore()

	_, err := authorizeProvisionInstall(provisionAuthorizeRequest{
		SchemaVersion:   1,
		EnrollmentToken: "wrong-token",
		MACAddresses:    []string{"52:54:00:12:34:56"},
		TargetDisk:      "/dev/vda",
		TargetSerial:    "DISK-001",
	})
	if err == nil || !strings.Contains(err.Error(), "enrollment token") {
		t.Fatalf("err = %v", err)
	}
}

func TestAuthorizeProvisionInstallRejectsMissingSerial(t *testing.T) {
	root := t.TempDir()
	restore := setProvisionStreamPaths(root)
	defer restore()

	request := sampleAuthorizeRequest()
	request.TargetSerial = ""
	_, err := authorizeProvisionInstall(request)
	if err == nil || !strings.Contains(err.Error(), "target_serial is required") {
		t.Fatalf("err = %v", err)
	}
}

func TestAuthorizeProvisionInstallTrustsClientTargetReport(t *testing.T) {
	root := t.TempDir()
	restore := setProvisionStreamPaths(root)
	defer restore()
	restoreInspect := setProvisionTargetInspect(func(string) (provisionTargetDisk, error) {
		return provisionTargetDisk{
			Path:       "/dev/nvme0n1",
			SizeBytes:  releaseImageSizeBytes,
			Serial:     "SUPERVISOR-LOCAL-SERIAL",
			Transport:  "nvme",
			DeviceType: "disk",
		}, nil
	})
	defer restoreInspect()

	request := sampleAuthorizeRequest()
	request.TargetDisk = "/dev/nvme0n1"
	request.TargetSerial = "CLIENT-REPORTED-SERIAL"

	response, err := authorizeProvisionInstall(request)
	if err != nil {
		t.Fatal(err)
	}
	if response.TargetSerial != "CLIENT-REPORTED-SERIAL" {
		t.Fatalf("response serial = %q", response.TargetSerial)
	}
}

func TestAuthorizeProvisionInstallSuccess(t *testing.T) {
	root := t.TempDir()
	restore := setProvisionStreamPaths(root)
	defer restore()

	response, err := authorizeProvisionInstall(sampleAuthorizeRequest())
	if err != nil {
		t.Fatal(err)
	}
	if response.InstallSessionID == "" || response.ImageVersion != "0.1.0" {
		t.Fatalf("response: %+v", response)
	}
	if !strings.Contains(response.AuthorizedKeys, "ssh-ed25519") {
		t.Fatalf("authorized keys missing: %q", response.AuthorizedKeys)
	}
}

func TestProvisionImageStreamRequiresAuthorizedSession(t *testing.T) {
	root := t.TempDir()
	restore := setProvisionStreamPaths(root)
	defer restore()

	server := httptest.NewServer(http.HandlerFunc(func(writer http.ResponseWriter, request *http.Request) {
		switch {
		case request.URL.Path == "/v1/provision/authorize":
			handleProvisionAuthorize(writer, request)
		case strings.HasPrefix(request.URL.Path, "/v1/provision/images/"):
			handleProvisionImageStream(writer, request)
		default:
			http.NotFound(writer, request)
		}
	}))
	defer server.Close()

	authorizeBody, err := json.Marshal(sampleAuthorizeRequest())
	if err != nil {
		t.Fatal(err)
	}
	authorizeResponse, err := http.Post(server.URL+"/v1/provision/authorize", "application/json", bytes.NewReader(authorizeBody))
	if err != nil {
		t.Fatal(err)
	}
	defer authorizeResponse.Body.Close()
	if authorizeResponse.StatusCode != http.StatusOK {
		body, _ := io.ReadAll(authorizeResponse.Body)
		t.Fatalf("authorize status = %s body = %s", authorizeResponse.Status, body)
	}
	var authorization provisionAuthorizeResponse
	if err := json.NewDecoder(authorizeResponse.Body).Decode(&authorization); err != nil {
		t.Fatal(err)
	}

	badRequest, err := http.NewRequest(http.MethodGet, server.URL+authorization.ImageStreamPath, nil)
	if err != nil {
		t.Fatal(err)
	}
	badRequest.Header.Set("X-FoldingOS-Enrollment-Token", "wrong-token")
	badRequest.Header.Set(installSessionHeader, authorization.InstallSessionID)
	badResponse, err := http.DefaultClient.Do(badRequest)
	if err != nil {
		t.Fatal(err)
	}
	defer badResponse.Body.Close()
	if badResponse.StatusCode != http.StatusUnauthorized {
		t.Fatalf("stream status = %s", badResponse.Status)
	}

	goodRequest, err := http.NewRequest(http.MethodGet, server.URL+authorization.ImageStreamPath, nil)
	if err != nil {
		t.Fatal(err)
	}
	goodRequest.Header.Set("X-FoldingOS-Enrollment-Token", "test-enrollment-token")
	goodRequest.Header.Set(installSessionHeader, authorization.InstallSessionID)
	goodResponse, err := http.DefaultClient.Do(goodRequest)
	if err != nil {
		t.Fatal(err)
	}
	defer goodResponse.Body.Close()
	body, err := io.ReadAll(goodResponse.Body)
	if err != nil {
		t.Fatal(err)
	}
	if goodResponse.StatusCode != http.StatusOK {
		t.Fatalf("stream status = %s body = %s", goodResponse.Status, body)
	}
	if int64(len(body)) != releaseImageSizeBytes {
		t.Fatalf("stream size = %d", len(body))
	}
	digest := sha256.Sum256(body)
	if hex.EncodeToString(digest[:]) != authorization.ImageSHA256 {
		t.Fatalf("stream digest mismatch")
	}
}

func TestWriteProvisionImageToDiskVerifiesSize(t *testing.T) {
	root := t.TempDir()
	target := filepath.Join(root, "target.img")
	if err := os.WriteFile(target, make([]byte, 0), 0644); err != nil {
		t.Fatal(err)
	}
	payload := []byte("release-image-bytes")
	source := bytes.NewReader(payload)

	_, written, err := writeProvisionImageToDiskDirect(target, source, int64(len(payload)+1))
	if err == nil {
		t.Fatalf("expected short write error, wrote %d", written)
	}
}

func TestWriteProvisionImageToDiskStoresVerifiedBytes(t *testing.T) {
	root := t.TempDir()
	target := filepath.Join(root, "target.img")
	if err := os.WriteFile(target, make([]byte, 0), 0644); err != nil {
		t.Fatal(err)
	}
	payload := []byte("verified-release-image")
	source := bytes.NewReader(payload)

	digest, written, err := writeProvisionImageToDiskDirect(target, source, int64(len(payload)))
	if err != nil {
		t.Fatal(err)
	}
	if written != int64(len(payload)) {
		t.Fatalf("written = %d", written)
	}
	expected := sha256.Sum256(payload)
	if digest != hex.EncodeToString(expected[:]) {
		t.Fatalf("digest = %s", digest)
	}
}

func sampleAuthorizeRequest() provisionAuthorizeRequest {
	return provisionAuthorizeRequest{
		SchemaVersion:   1,
		EnrollmentToken: "test-enrollment-token",
		MACAddresses:    []string{"52:54:00:12:34:56"},
		TargetDisk:      "/dev/vda",
		TargetSerial:    "DISK-001",
		ImageVersion:    "0.1.0",
	}
}

func setProvisionStreamPaths(root string) func() {
	previousImageSize := releaseImageSizeBytes
	releaseImageSizeBytes = 4096
	restoreProvision := setProvisionPaths(root)
	restoreRegistry := setRegistryPaths(root)
	restoreSessions := setProvisionSessionsDir(filepath.Join(root, "sessions"))
	restoreKeys := setActiveAuthorizedKeysPath(filepath.Join(root, "config", "ssh", "authorized_keys"))
	if err := os.MkdirAll(filepath.Join(root, "config", "ssh"), 0755); err != nil {
		panic(err)
	}
	authorizedKeys := generateTestAuthorizedKeys()
	if err := os.WriteFile(filepath.Join(root, "config", "ssh", "authorized_keys"), authorizedKeys, 0644); err != nil {
		panic(err)
	}
	writeEnrollmentTokenForStreamTest(root, "test-enrollment-token")
	imagePath := filepath.Join(root, "images", "foldingos-x86_64-0.1.0.img")
	if err := os.MkdirAll(filepath.Dir(imagePath), 0755); err != nil {
		panic(err)
	}
	payload := bytes.Repeat([]byte("a"), int(releaseImageSizeBytes))
	if err := os.WriteFile(imagePath, payload, 0644); err != nil {
		panic(err)
	}
	digest := sha256.Sum256(payload)
	entry := registryEntry{
		SchemaVersion:      1,
		FoldingOSVersion:   "0.1.0",
		GitRevision:        "abc",
		ImageSHA256:        hex.EncodeToString(digest[:]),
		ImageSizeBytes:     releaseImageSizeBytes,
		VerificationMethod: "sha256",
		ImportTimestamp:    "2026-06-13T20:00:00Z",
		RolloutState:       "ready",
		LocalImagePath:     imagePath,
	}
	if err := saveRegistryEntry(entry); err != nil {
		panic(err)
	}
	restoreBootDisk := setHostBootDiskResolver(func() (string, error) {
		return "/dev/sda", nil
	})
	restoreMounted := setMountedBlockDevicesLister(func() ([]string, error) {
		return nil, nil
	})
	return func() {
		releaseImageSizeBytes = previousImageSize
		restoreKeys()
		restoreMounted()
		restoreBootDisk()
		restoreSessions()
		restoreRegistry()
		restoreProvision()
	}
}

func writeEnrollmentTokenForStreamTest(root, token string) {
	if err := os.MkdirAll(filepath.Join(root, "config", "provision"), 0755); err != nil {
		panic(err)
	}
	if err := os.WriteFile(filepath.Join(root, "config", "provision", "enrollment-token"), []byte(token+"\n"), 0600); err != nil {
		panic(err)
	}
}

func setProvisionSessionsDir(path string) func() {
	previous := provisionSessionsDir
	provisionSessionsDir = path
	return func() {
		provisionSessionsDir = previous
	}
}

func setProvisionTargetInspect(fn func(string) (provisionTargetDisk, error)) func() {
	previous := inspectProvisionTargetDisk
	inspectProvisionTargetDisk = fn
	return func() {
		inspectProvisionTargetDisk = previous
	}
}

func setHostBootDiskResolver(fn func() (string, error)) func() {
	previous := resolveHostBootDisk
	resolveHostBootDisk = fn
	return func() {
		resolveHostBootDisk = previous
	}
}

func setMountedBlockDevicesLister(fn func() ([]string, error)) func() {
	previous := listMountedBlockDevices
	listMountedBlockDevices = fn
	return func() {
		listMountedBlockDevices = previous
	}
}

func generateTestAuthorizedKeys() []byte {
	tempDir := os.TempDir()
	keyPath := filepath.Join(tempDir, "foldingos-provision-test-key")
	if err := run("ssh-keygen", "-q", "-t", "ed25519", "-N", "", "-f", keyPath); err != nil {
		panic(err)
	}
	defer os.Remove(keyPath)
	defer os.Remove(keyPath + ".pub")
	content, err := os.ReadFile(keyPath + ".pub")
	if err != nil {
		panic(err)
	}
	return content
}

func setActiveAuthorizedKeysPath(path string) func() {
	previous := activeKeys
	activeKeys = path
	return func() {
		activeKeys = previous
	}
}
