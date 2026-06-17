package main

import (
	"encoding/hex"
	"net/http"
	"net/http/httptest"
	"os"
	"path/filepath"
	"testing"
)

func TestDownloadAndStageFoldOpsPackage(t *testing.T) {
	artifact := []byte("foldingos-foldops-test-artifact")
	pkg := foldOpsPackage{
		Name:         "foldops-agent",
		Version:      "0.1.0-1",
		ArtifactSize: int64(len(artifact)),
		SHA256:       hex.EncodeToString(sha256Sum(artifact)),
	}

	server := httptest.NewTLSServer(http.HandlerFunc(func(writer http.ResponseWriter, request *http.Request) {
		writer.WriteHeader(http.StatusOK)
		_, _ = writer.Write(artifact)
	}))
	defer server.Close()

	pkg.ArtifactURL = server.URL + "/approved.deb"

	restoreClient := setFoldOpsHTTPClient(server.Client())
	defer restoreClient()

	downloadsDir := filepath.Join(t.TempDir(), ".downloads")
	restoreDownloads := setFoldOpsDownloadsDir(downloadsDir)
	defer restoreDownloads()

	if err := downloadAndStageFoldOpsPackage(foldOpsManifestArtifactFormatDeb, pkg); err != nil {
		t.Fatal(err)
	}
	stagedPath := foldOpsStagedDebPath(pkg)
	if _, err := os.Stat(stagedPath); err != nil {
		t.Fatal(err)
	}
}

func TestExtractFoldOpsDebDataXZ(t *testing.T) {
	tarPayload, err := buildTestTarXZArchive(map[string][]byte{
		"usr/bin/foldops-agent": []byte("foldops-binary"),
	})
	if err != nil {
		t.Fatal(err)
	}
	debPath := filepath.Join(t.TempDir(), "foldops-agent.deb")
	if err := writeTestDebArtifact(debPath, "data.tar.xz", tarPayload); err != nil {
		t.Fatal(err)
	}

	destination := filepath.Join(t.TempDir(), "extracted")
	if err := extractFoldOpsDebData(debPath, destination); err != nil {
		t.Fatal(err)
	}
	content, err := os.ReadFile(filepath.Join(destination, "usr", "bin", "foldops-agent"))
	if err != nil {
		t.Fatal(err)
	}
	if string(content) != "foldops-binary" {
		t.Fatalf("unexpected extracted content: %q", string(content))
	}
}

func TestExtractFoldOpsDebDataZST(t *testing.T) {
	tarPayload, err := buildTestTarZSTArchive(map[string][]byte{
		"usr/share/foldops/web/index.html": []byte("<html></html>"),
	})
	if err != nil {
		t.Fatal(err)
	}
	debPath := filepath.Join(t.TempDir(), "foldops-web.deb")
	if err := writeTestDebArtifact(debPath, "data.tar.zst", tarPayload); err != nil {
		t.Fatal(err)
	}

	destination := filepath.Join(t.TempDir(), "extracted")
	if err := extractFoldOpsDebData(debPath, destination); err != nil {
		t.Fatal(err)
	}
	content, err := os.ReadFile(filepath.Join(destination, "usr", "share", "foldops", "web", "index.html"))
	if err != nil {
		t.Fatal(err)
	}
	if string(content) != "<html></html>" {
		t.Fatalf("unexpected extracted content: %q", string(content))
	}
}

func TestExtractFoldOpsLayoutBundle(t *testing.T) {
	tarPayload, err := buildTestLayoutTarZSTArchive("foldops-agent", map[string][]byte{
		"usr/bin/foldops-agent": []byte("foldops-binary"),
	})
	if err != nil {
		t.Fatal(err)
	}
	bundlePath := filepath.Join(t.TempDir(), "foldops-agent.tar.zst")
	if err := os.WriteFile(bundlePath, tarPayload, 0644); err != nil {
		t.Fatal(err)
	}

	stagingRoot := filepath.Join(t.TempDir(), "staging")
	if err := extractFoldOpsLayoutBundle(bundlePath, stagingRoot, "foldops-agent"); err != nil {
		t.Fatal(err)
	}
	content, err := os.ReadFile(filepath.Join(stagingRoot, "foldops-agent", "usr", "bin", "foldops-agent"))
	if err != nil {
		t.Fatal(err)
	}
	if string(content) != "foldops-binary" {
		t.Fatalf("unexpected extracted content: %q", string(content))
	}
}

func TestExtractAndInstallFoldOpsLayoutPackagesIsIdempotent(t *testing.T) {
	root := t.TempDir()
	restoreApps := setFoldOpsAppsRoot(root)
	defer restoreApps()
	downloadsDir := filepath.Join(root, ".downloads")
	restoreDownloads := setFoldOpsDownloadsDir(downloadsDir)
	defer restoreDownloads()
	rolePath := filepath.Join(root, "installation-role")
	if err := os.WriteFile(rolePath, []byte("supervisor\n"), 0644); err != nil {
		t.Fatal(err)
	}
	previousRole := activeInstallationRole
	activeInstallationRole = rolePath
	defer func() {
		activeInstallationRole = previousRole
	}()

	pkg := foldOpsPackage{
		Name:             "foldops-web",
		Version:          "0.1.0",
		InstallPrefix:    "foldops-web",
		ArtifactSize:     0,
		SHA256:           "",
		VerificationPath: "/data/apps/foldops/current/foldops-web/usr/share/foldops/web/index.html",
	}
	tarPayload, err := buildTestLayoutTarZSTArchive("foldops-web", map[string][]byte{
		"usr/share/foldops/web/index.html": []byte("<html></html>"),
	})
	if err != nil {
		t.Fatal(err)
	}
	pkg.ArtifactSize = int64(len(tarPayload))
	pkg.SHA256 = hex.EncodeToString(sha256Sum(tarPayload))
	stagedPath := foldOpsStagedArtifactPath(foldOpsManifestArtifactFormatLayout, pkg)
	if err := os.MkdirAll(filepath.Dir(stagedPath), 0755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(stagedPath, tarPayload, 0644); err != nil {
		t.Fatal(err)
	}

	release := "0.2.0-1"
	first, err := extractAndInstallFoldOpsPackages(release, foldOpsManifestArtifactFormatLayout, []foldOpsPackage{pkg})
	if err != nil {
		t.Fatal(err)
	}
	second, err := extractAndInstallFoldOpsPackages(release, foldOpsManifestArtifactFormatLayout, []foldOpsPackage{pkg})
	if err != nil {
		t.Fatal(err)
	}
	if first != second {
		t.Fatalf("expected idempotent install path %q, got %q", first, second)
	}
}

func TestFoldOpsAcquireStateRetryDelay(t *testing.T) {
	if foldOpsAcquisitionRetryDelay(1) != foldOpsAcquisitionRetryDelays[0] {
		t.Fatal("first retry delay mismatch")
	}
	if foldOpsAcquisitionRetryDelay(99) != foldOpsAcquisitionRetryDelays[len(foldOpsAcquisitionRetryDelays)-1] {
		t.Fatal("bounded retry delay mismatch")
	}
}

func setFoldOpsHTTPClient(client *http.Client) func() {
	previous := foldOpsHTTPClient
	foldOpsHTTPClient = client
	return func() {
		foldOpsHTTPClient = previous
	}
}

func setFoldOpsManifestPaths(bootstrapPath, assignedPath string) func() {
	previousEmbedded := foldOpsEmbeddedManifestPath
	previousAssigned := foldOpsAssignedManifestPath
	foldOpsEmbeddedManifestPath = bootstrapPath
	foldOpsAssignedManifestPath = assignedPath
	return func() {
		foldOpsEmbeddedManifestPath = previousEmbedded
		foldOpsAssignedManifestPath = previousAssigned
	}
}

func setFoldOpsDownloadsDir(path string) func() {
	previous := foldOpsDownloadsDir
	foldOpsDownloadsDir = path
	return func() {
		foldOpsDownloadsDir = previous
	}
}
