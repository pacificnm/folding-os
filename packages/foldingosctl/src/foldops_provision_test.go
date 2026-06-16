package main

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestParseFoldOpsIngestToken(t *testing.T) {
	token, err := parseFoldOpsIngestToken("abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789\n")
	if err != nil {
		t.Fatal(err)
	}
	if len(token) != 64 {
		t.Fatalf("token length = %d", len(token))
	}
}

func TestRejectInvalidFoldOpsIngestToken(t *testing.T) {
	if _, err := parseFoldOpsIngestToken("not-a-valid-token"); err == nil {
		t.Fatal("invalid token was accepted")
	}
}

func TestFoldOpsSupervisorHostFromURL(t *testing.T) {
	host, err := foldOpsSupervisorHostFromURL("http://192.168.88.238:8743/\n")
	if err != nil {
		t.Fatal(err)
	}
	if host != "192.168.88.238" {
		t.Fatalf("host = %q", host)
	}
}

func TestWriteFoldOpsEnvFile(t *testing.T) {
	path := filepath.Join(t.TempDir(), "agent.env")
	values := map[string]string{
		"AGENT_TOKEN":    "abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789",
		"SUPERVISOR_URL": "https://192.168.88.238:3443",
	}
	if err := writeFoldOpsEnvFile(path, values, 0600); err != nil {
		t.Fatal(err)
	}
	content, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	if !strings.Contains(string(content), "AGENT_TOKEN=") || !strings.Contains(string(content), "SUPERVISOR_URL=") {
		t.Fatalf("unexpected env content: %s", string(content))
	}
}

func TestGenerateFoldOpsSelfSignedTLS(t *testing.T) {
	tempDir := t.TempDir()
	restoreTLSDir := setFoldOpsTLSDir(tempDir)
	defer restoreTLSDir()

	if err := generateFoldOpsSelfSignedTLS("folding-test"); err != nil {
		t.Fatal(err)
	}
	for _, name := range []string{"cert.pem", "key.pem", "ca.pem"} {
		if !fileExists(filepath.Join(tempDir, name)) {
			t.Fatalf("missing TLS file %s", name)
		}
	}
}

func TestWriteFoldOpsProvisionedMarker(t *testing.T) {
	tempDir := t.TempDir()
	restoreMarker := setFoldOpsProvisionedMarkerPath(filepath.Join(tempDir, "provisioned.json"))
	defer restoreMarker()

	if err := writeFoldOpsProvisionedMarker("supervisor", "0.1.0-1"); err != nil {
		t.Fatal(err)
	}
	marker, err := loadFoldOpsProvisionedMarker()
	if err != nil {
		t.Fatal(err)
	}
	if marker == nil || marker.Role != "supervisor" || marker.ManifestRelease != "0.1.0-1" {
		t.Fatalf("unexpected marker: %+v", marker)
	}
}

func setFoldOpsTLSDir(path string) func() {
	previous := foldOpsTLSDir
	foldOpsTLSDir = path
	return func() {
		foldOpsTLSDir = previous
	}
}

func setFoldOpsProvisionedMarkerPath(path string) func() {
	previous := foldOpsProvisionedMarkerPath
	foldOpsProvisionedMarkerPath = path
	return func() {
		foldOpsProvisionedMarkerPath = previous
	}
}
