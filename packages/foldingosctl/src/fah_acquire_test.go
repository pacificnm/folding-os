package main

import (
	"crypto/sha256"
	"encoding/hex"
	"net/http"
	"net/http/httptest"
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestDownloadAndStageFAHArtifact(t *testing.T) {
	artifact := []byte("foldingos-test-artifact")
	manifest := testFAHManifest(artifact)

	server := httptest.NewTLSServer(http.HandlerFunc(func(writer http.ResponseWriter, request *http.Request) {
		if request.URL.Path != "/approved.deb" {
			t.Fatalf("unexpected request path: %s", request.URL.Path)
		}
		writer.WriteHeader(http.StatusOK)
		_, _ = writer.Write(artifact)
	}))
	defer server.Close()

	manifest.ArtifactURL = server.URL + "/approved.deb"
	manifest.ArtifactSize = int64(len(artifact))
	manifest.SHA256 = hex.EncodeToString(sha256Sum(artifact))

	restoreClient := setFAHHTTPClient(server.Client())
	defer restoreClient()

	downloadsDir := filepath.Join(t.TempDir(), ".downloads")
	restoreDownloads := setFAHDownloadsDir(downloadsDir)
	defer restoreDownloads()

	stagedPath, err := downloadAndStageFAHArtifact(manifest)
	if err != nil {
		t.Fatal(err)
	}
	if stagedPath != filepath.Join(downloadsDir, "8.5.6.deb") {
		t.Fatalf("staged path = %q", stagedPath)
	}
	if _, err := os.Stat(stagedPath); err != nil {
		t.Fatal(err)
	}
}

func TestRejectOversizedFAHDownload(t *testing.T) {
	artifact := []byte("foldingos-test-artifact-too-large")
	manifest := testFAHManifest(artifact)
	manifest.ArtifactSize = int64(len(artifact) - 1)

	server := httptest.NewTLSServer(http.HandlerFunc(func(writer http.ResponseWriter, request *http.Request) {
		writer.WriteHeader(http.StatusOK)
		_, _ = writer.Write(artifact)
	}))
	defer server.Close()

	manifest.ArtifactURL = server.URL + "/approved.deb"
	manifest.SHA256 = hex.EncodeToString(sha256Sum(artifact))

	restoreClient := setFAHHTTPClient(server.Client())
	defer restoreClient()

	downloadsDir := filepath.Join(t.TempDir(), ".downloads")
	restoreDownloads := setFAHDownloadsDir(downloadsDir)
	defer restoreDownloads()

	if _, err := downloadAndStageFAHArtifact(manifest); err == nil {
		t.Fatal("oversized download was accepted")
	}
	if _, err := os.Stat(filepath.Join(downloadsDir, "8.5.6.partial")); !os.IsNotExist(err) {
		t.Fatal("partial download was not removed after failure")
	}
}

func TestRejectWrongFAHArtifactHash(t *testing.T) {
	artifact := []byte("foldingos-test-artifact")
	manifest := testFAHManifest(artifact)

	server := httptest.NewTLSServer(http.HandlerFunc(func(writer http.ResponseWriter, request *http.Request) {
		writer.WriteHeader(http.StatusOK)
		_, _ = writer.Write(artifact)
	}))
	defer server.Close()

	manifest.ArtifactURL = server.URL + "/approved.deb"
	manifest.ArtifactSize = int64(len(artifact))
	manifest.SHA256 = strings.Repeat("a", 64)

	restoreClient := setFAHHTTPClient(server.Client())
	defer restoreClient()

	downloadsDir := filepath.Join(t.TempDir(), ".downloads")
	restoreDownloads := setFAHDownloadsDir(downloadsDir)
	defer restoreDownloads()

	if _, err := downloadAndStageFAHArtifact(manifest); err == nil {
		t.Fatal("wrong artifact hash was accepted")
	}
}

func TestRejectFAHDownloadRedirect(t *testing.T) {
	artifact := []byte("foldingos-test-artifact")
	manifest := testFAHManifest(artifact)

	redirectTarget := httptest.NewTLSServer(http.HandlerFunc(func(writer http.ResponseWriter, request *http.Request) {
		writer.WriteHeader(http.StatusOK)
		_, _ = writer.Write(artifact)
	}))
	defer redirectTarget.Close()

	server := httptest.NewTLSServer(http.HandlerFunc(func(writer http.ResponseWriter, request *http.Request) {
		http.Redirect(writer, request, redirectTarget.URL, http.StatusFound)
	}))
	defer server.Close()

	manifest.ArtifactURL = server.URL + "/approved.deb"
	manifest.ArtifactSize = int64(len(artifact))
	manifest.SHA256 = hex.EncodeToString(sha256Sum(artifact))

	restoreClient := setFAHHTTPClient(server.Client())
	defer restoreClient()

	downloadsDir := filepath.Join(t.TempDir(), ".downloads")
	restoreDownloads := setFAHDownloadsDir(downloadsDir)
	defer restoreDownloads()

	if _, err := downloadAndStageFAHArtifact(manifest); err == nil {
		t.Fatal("redirecting download was accepted")
	}
}

func TestFAHAcquireSkipsWhenVerifiedClientActive(t *testing.T) {
	appsRoot := t.TempDir()
	downloadsDir := filepath.Join(appsRoot, ".downloads")
	versionDir := filepath.Join(appsRoot, "8.5.6")
	if err := os.MkdirAll(filepath.Join(versionDir, "usr", "bin"), 0755); err != nil {
		t.Fatal(err)
	}
	executable := filepath.Join(versionDir, "usr", "bin", "fah-client")
	if err := os.WriteFile(executable, []byte("binary"), 0755); err != nil {
		t.Fatal(err)
	}
	if err := os.Symlink("8.5.6", filepath.Join(appsRoot, "current")); err != nil {
		t.Fatal(err)
	}
	marker := strings.Join([]string{
		"client_version=8.5.6",
		"artifact_sha256=643de04033a1cb972a81e3a193d710e919a4f34634a987f11adc4cee61fdaefe",
	}, "\n") + "\n"
	if err := os.WriteFile(filepath.Join(versionDir, fahVerifiedMarkerName), []byte(marker), 0644); err != nil {
		t.Fatal(err)
	}

	manifest, err := parseFAHManifest(validFAHManifest)
	if err != nil {
		t.Fatal(err)
	}

	restoreApps := setFAHAppsRoot(appsRoot)
	defer restoreApps()
	restoreDownloads := setFAHDownloadsDir(downloadsDir)
	defer restoreDownloads()

	calledPrereqs := false
	fahCheckAcquisitionPrerequisites = func() error {
		calledPrereqs = true
		return nil
	}
	defer func() {
		fahCheckAcquisitionPrerequisites = requireFAHAcquisitionPrerequisites
	}()

	if !fahHasVerifiedActiveClient(manifest) {
		t.Fatal("verified active client was not detected")
	}
	if calledPrereqs {
		t.Fatal("prerequisites were checked before skip")
	}
}

func testFAHManifest(artifact []byte) fahManifest {
	manifest, err := parseFAHManifest(validFAHManifest)
	if err != nil {
		panic(err)
	}
	manifest.ArtifactSize = int64(len(artifact))
	sum := sha256.Sum256(artifact)
	manifest.SHA256 = hex.EncodeToString(sum[:])
	return manifest
}

func setFAHHTTPClient(client *http.Client) func() {
	previous := fahHTTPClient
	fahHTTPClient = client
	return func() {
		fahHTTPClient = previous
	}
}

func setFAHDownloadsDir(path string) func() {
	previous := fahDownloadsDir
	fahDownloadsDir = path
	return func() {
		fahDownloadsDir = previous
	}
}

func setFAHAppsRoot(path string) func() {
	previous := fahAppsRoot
	fahAppsRoot = path
	return func() {
		fahAppsRoot = previous
	}
}

func TestFAHAcquireRequiresSynchronizedTime(t *testing.T) {
	restorePrereqs := setFAHNTPSynchronized(func() (bool, error) {
		return false, nil
	})
	defer restorePrereqs()

	manifest, err := parseFAHManifest(validFAHManifest)
	if err != nil {
		t.Fatal(err)
	}
	if err := fahCheckAcquisitionPrerequisites(); err == nil {
		t.Fatal("unsynchronized time was accepted")
	}
}

func setFAHNTPSynchronized(check func() (bool, error)) func() {
	previous := fahNTPSynchronized
	fahNTPSynchronized = check
	return func() {
		fahNTPSynchronized = previous
	}
}

func sha256Sum(data []byte) []byte {
	sum := sha256.Sum256(data)
	return sum[:]
}
