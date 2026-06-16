package main

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestWriteProvisionPersistentFiles(t *testing.T) {
	root := t.TempDir()
	if err := writeProvisionPersistentFiles(
		root,
		"agent",
		"http://192.168.4.17:8743",
		"test-enrollment-token",
		[]byte("test-ca\n"),
	); err != nil {
		t.Fatal(err)
	}

	role, err := os.ReadFile(filepath.Join(root, "config", "installation-role"))
	if err != nil {
		t.Fatal(err)
	}
	if string(role) != "agent" {
		t.Fatalf("installation role = %q", role)
	}

	supervisorURL, err := os.ReadFile(filepath.Join(root, "config", "provision", "supervisor.url"))
	if err != nil {
		t.Fatal(err)
	}
	if strings.TrimSpace(string(supervisorURL)) != "http://192.168.4.17:8743" {
		t.Fatalf("supervisor url = %q", supervisorURL)
	}

	token, err := os.ReadFile(filepath.Join(root, "config", "provision", "enrollment-token"))
	if err != nil {
		t.Fatal(err)
	}
	if strings.TrimSpace(string(token)) != "test-enrollment-token" {
		t.Fatalf("enrollment token = %q", token)
	}

	supervisorCA, err := os.ReadFile(filepath.Join(root, "config", "foldops", "supervisor-ca.pem"))
	if err != nil {
		t.Fatal(err)
	}
	if strings.TrimSpace(string(supervisorCA)) != "test-ca" {
		t.Fatalf("supervisor CA = %q", supervisorCA)
	}
}

func TestWriteProvisionPersistentFilesResetsInheritedDataState(t *testing.T) {
	root := t.TempDir()
	for _, relative := range agentDataPartitionResetPaths {
		path := filepath.Join(root, relative, "stale")
		if err := os.MkdirAll(filepath.Dir(path), 0755); err != nil {
			t.Fatal(err)
		}
		if err := os.WriteFile(path, []byte("stale"), 0644); err != nil {
			t.Fatal(err)
		}
	}

	if err := writeProvisionPersistentFiles(
		root,
		"agent",
		"http://192.168.4.17:8743",
		"test-enrollment-token",
		[]byte("test-ca\n"),
	); err != nil {
		t.Fatal(err)
	}
	for _, relative := range agentDataPartitionResetPaths {
		if relative == "config" {
			continue
		}
		if _, err := os.Stat(filepath.Join(root, relative)); !os.IsNotExist(err) {
			t.Fatalf("inherited %s should be removed, stat err = %v", relative, err)
		}
	}
	if _, err := os.Stat(filepath.Join(root, "config", "node-id")); !os.IsNotExist(err) {
		t.Fatalf("inherited node-id should be removed, stat err = %v", err)
	}
}

func TestWriteProvisionPersistentFilesRejectsInvalidRole(t *testing.T) {
	root := t.TempDir()
	if err := writeProvisionPersistentFiles(root, "invalid", "http://192.168.4.17:8743", "token", []byte("test-ca\n")); err == nil {
		t.Fatal("invalid installation role was accepted")
	}
}

func TestStageFoldOpsIngestTokenOnEFI(t *testing.T) {
	provisionDir := filepath.Join(t.TempDir(), "provision")
	if err := os.MkdirAll(provisionDir, 0755); err != nil {
		t.Fatal(err)
	}
	token := "abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789"
	if err := stageFoldOpsIngestTokenOnEFI(provisionDir, token); err != nil {
		t.Fatal(err)
	}
	content, err := os.ReadFile(filepath.Join(provisionDir, "foldops-ingest-token"))
	if err != nil {
		t.Fatal(err)
	}
	if strings.TrimSpace(string(content)) != token {
		t.Fatalf("token = %q", content)
	}
}
