package main

import (
	"bytes"
	"encoding/hex"
	"encoding/json"
	"io"
	"net/http"
	"net/http/httptest"
	"os"
	"path/filepath"
	"testing"
)

func TestDownloadAndStageToolsBinary(t *testing.T) {
	artifact := testToolsExecutableBytes(t)
	assignment := toolsAssignment{
		SchemaVersion: 1,
		ToolsVersion:  "0.2.0",
		ArtifactSize:  int64(len(artifact)),
		SHA256:        hex.EncodeToString(sha256Sum(artifact)),
	}

	server := httptest.NewTLSServer(http.HandlerFunc(func(writer http.ResponseWriter, request *http.Request) {
		writer.WriteHeader(http.StatusOK)
		_, _ = writer.Write(artifact)
	}))
	defer server.Close()

	assignment.ArtifactURL = server.URL + "/foldingosctl-x86_64"

	restoreClient := setToolsHTTPClient(server.Client())
	defer restoreClient()
	restoreVerify := setVerifyToolsExecutable(func(string) error { return nil })
	defer restoreVerify()

	downloadsDir := filepath.Join(t.TempDir(), ".downloads")
	restoreDownloads := setToolsDownloadsDir(downloadsDir)
	defer restoreDownloads()

	stagedPath, err := downloadAndStageToolsBinary(assignment)
	if err != nil {
		t.Fatal(err)
	}
	if _, err := os.Stat(stagedPath); err != nil {
		t.Fatal(err)
	}
}

func TestAtomicReplaceToolsBinary(t *testing.T) {
	artifact := testToolsExecutableBytes(t)
	stagedPath := filepath.Join(t.TempDir(), "staged")
	destination := filepath.Join(t.TempDir(), "bin", "foldingosctl")
	if err := os.WriteFile(stagedPath, artifact, 0755); err != nil {
		t.Fatal(err)
	}
	if err := os.MkdirAll(filepath.Dir(destination), 0755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(destination, []byte("old-binary"), 0755); err != nil {
		t.Fatal(err)
	}

	restoreVerify := setVerifyToolsExecutable(func(string) error { return nil })
	defer restoreVerify()

	if err := atomicReplaceToolsBinary(stagedPath, destination); err != nil {
		t.Fatal(err)
	}
	content, err := os.ReadFile(destination)
	if err != nil {
		t.Fatal(err)
	}
	if string(content) != string(artifact) {
		t.Fatal("tools binary was not replaced")
	}
}

func TestToolsAcquireInstallsVerifiedBinary(t *testing.T) {
	root := t.TempDir()
	artifact := testToolsExecutableBytes(t)
	assignment := toolsAssignment{
		SchemaVersion: 1,
		ToolsVersion:  "0.2.0",
		ArtifactURL:   "https://packages.folding-os.com/foldingos-tools/0.2.0/foldingosctl-x86_64",
		ArtifactSize:  int64(len(artifact)),
		SHA256:        hex.EncodeToString(sha256Sum(artifact)),
	}

	assignedPath := filepath.Join(root, "assigned.json")
	if err := os.WriteFile(assignedPath, []byte(mustJSON(assignment)), 0644); err != nil {
		t.Fatal(err)
	}
	binaryPath := filepath.Join(root, "foldingosctl")
	if err := os.WriteFile(binaryPath, []byte("bootstrap-binary"), 0755); err != nil {
		t.Fatal(err)
	}

	restoreAssignment := setToolsAssignmentPaths(
		filepath.Join(root, "missing-bootstrap.json"),
		assignedPath,
	)
	defer restoreAssignment()
	restoreBinary := setToolsBinaryPath(binaryPath)
	defer restoreBinary()
	restoreState := setToolsActiveStatePath(filepath.Join(root, "active.json"))
	defer restoreState()
	restoreAcquireState := setToolsAcquireStatePath(filepath.Join(root, "acquire.state"))
	defer restoreAcquireState()
	restoreDownloads := setToolsDownloadsDir(filepath.Join(root, ".downloads"))
	defer restoreDownloads()
	restoreClient := setToolsHTTPClient(&http.Client{
		Transport: roundTripToolsArtifact(artifact),
	})
	defer restoreClient()
	restoreVerify := setVerifyToolsExecutable(func(string) error { return nil })
	defer restoreVerify()
	previousPrerequisites := toolsCheckAcquisitionPrerequisites
	toolsCheckAcquisitionPrerequisites = func() error { return nil }
	defer func() {
		toolsCheckAcquisitionPrerequisites = previousPrerequisites
	}()
	restarted := false
	previousRestart := toolsRestartDependentUnits
	toolsRestartDependentUnits = func() error {
		restarted = true
		return nil
	}
	defer func() {
		toolsRestartDependentUnits = previousRestart
	}()

	if err := toolsAcquire(); err != nil {
		t.Fatal(err)
	}
	if !restarted {
		t.Fatal("expected dependent units to restart")
	}
	content, err := os.ReadFile(binaryPath)
	if err != nil {
		t.Fatal(err)
	}
	if string(content) != string(artifact) {
		t.Fatal("tools binary was not updated")
	}
	if err := toolsAcquire(); err != nil {
		t.Fatal(err)
	}
}

func roundTripToolsArtifact(artifact []byte) http.RoundTripper {
	return roundTripFunc(func(request *http.Request) (*http.Response, error) {
		return &http.Response{
			StatusCode: http.StatusOK,
			Body:       io.NopCloser(bytes.NewReader(artifact)),
			Header:     make(http.Header),
			Request:    request,
		}, nil
	})
}

type roundTripFunc func(*http.Request) (*http.Response, error)

func (fn roundTripFunc) RoundTrip(request *http.Request) (*http.Response, error) {
	return fn(request)
}

func TestVerifyToolsArtifactRejectsBadHash(t *testing.T) {
	artifact := []byte("not-the-approved-binary")
	path := filepath.Join(t.TempDir(), "artifact")
	if err := os.WriteFile(path, artifact, 0644); err != nil {
		t.Fatal(err)
	}
	assignment := toolsAssignment{
		ArtifactSize: int64(len(artifact)),
		SHA256:       "9022c393630e11d5cec5794ac77281671c7b0d634d630c92d95ad6de22d2151a",
	}
	if err := verifyToolsArtifactFile(path, assignment); err == nil {
		t.Fatal("hash mismatch was accepted")
	}
}

func testToolsExecutableBytes(t *testing.T) []byte {
	t.Helper()
	executable, err := os.Executable()
	if err != nil {
		t.Fatal(err)
	}
	content, err := os.ReadFile(executable)
	if err != nil {
		t.Fatal(err)
	}
	return content
}

func mustJSON(value any) string {
	content, err := json.Marshal(value)
	if err != nil {
		panic(err)
	}
	return string(content)
}

func setToolsAssignmentPaths(bootstrapPath, assignedPath string) func() {
	previousBootstrap := toolsBootstrapManifestPath
	previousAssigned := toolsAssignedVersionPath
	toolsBootstrapManifestPath = bootstrapPath
	toolsAssignedVersionPath = assignedPath
	return func() {
		toolsBootstrapManifestPath = previousBootstrap
		toolsAssignedVersionPath = previousAssigned
	}
}

func setToolsBinaryPath(path string) func() {
	previous := toolsBinaryPath
	toolsBinaryPath = path
	return func() {
		toolsBinaryPath = previous
	}
}

func setToolsActiveStatePath(path string) func() {
	previous := toolsActiveStatePath
	toolsActiveStatePath = path
	return func() {
		toolsActiveStatePath = previous
	}
}

func setToolsAcquireStatePath(path string) func() {
	previous := toolsAcquireStatePath
	toolsAcquireStatePath = path
	return func() {
		toolsAcquireStatePath = previous
	}
}

func setToolsDownloadsDir(path string) func() {
	previous := toolsDownloadsDir
	toolsDownloadsDir = path
	return func() {
		toolsDownloadsDir = previous
	}
}

func setToolsHTTPClient(client *http.Client) func() {
	previous := toolsHTTPClient
	toolsHTTPClient = client
	return func() {
		toolsHTTPClient = previous
	}
}

func setVerifyToolsExecutable(fn func(string) error) func() {
	previous := verifyToolsExecutable
	verifyToolsExecutable = fn
	return func() {
		verifyToolsExecutable = previous
	}
}
