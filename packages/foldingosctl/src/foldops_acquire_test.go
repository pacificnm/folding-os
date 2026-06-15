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

	if err := downloadAndStageFoldOpsPackage(pkg); err != nil {
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

func setFoldOpsDownloadsDir(path string) func() {
	previous := foldOpsDownloadsDir
	foldOpsDownloadsDir = path
	return func() {
		foldOpsDownloadsDir = previous
	}
}
