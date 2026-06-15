package main

import (
	"os"
	"path/filepath"
	"testing"
)

func TestParseNodeIDFileAcceptsTextUUID(t *testing.T) {
	got, err := parseNodeIDFile([]byte("550e8400-e29b-41d4-a716-446655440000\n"))
	if err != nil {
		t.Fatal(err)
	}
	if got != "550e8400-e29b-41d4-a716-446655440000" {
		t.Fatalf("node id = %q", got)
	}
}

func TestParseNodeIDFileAcceptsBinaryUUID(t *testing.T) {
	raw := []byte{
		0x55, 0x0e, 0x84, 0x00, 0xe2, 0x9b, 0x41, 0xd4,
		0xa7, 0x16, 0x44, 0x66, 0x55, 0x44, 0x00, 0x00,
	}
	got, err := parseNodeIDFile(raw)
	if err != nil {
		t.Fatal(err)
	}
	if got != "550e8400-e29b-41d4-a716-446655440000" {
		t.Fatalf("node id = %q", got)
	}
	if !uuidPattern.MatchString(got) {
		t.Fatalf("normalized node id is not uuid v4: %q", got)
	}
}

func TestParseNodeIDFileNormalizesNonV4BinaryUUID(t *testing.T) {
	raw := []byte{
		0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x00, 0x88,
		0x00, 0x99, 0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff,
	}
	got, err := parseNodeIDFile(raw)
	if err != nil {
		t.Fatal(err)
	}
	if !uuidPattern.MatchString(got) {
		t.Fatalf("normalized node id is not uuid v4: %q", got)
	}
}

func TestEnsureIdentityRegeneratesCorruptNodeID(t *testing.T) {
	root := t.TempDir()
	restore := setConfigTestPaths(root)
	defer restore()

	nodeIDPath := filepath.Join(root, "config", "node-id")
	if err := os.WriteFile(nodeIDPath, make([]byte, 32), 0644); err != nil {
		t.Fatal(err)
	}

	got, err := ensureNodeIDFile(nodeIDPath)
	if err != nil {
		t.Fatal(err)
	}
	if !uuidPattern.MatchString(got) {
		t.Fatalf("regenerated node id is not uuid v4: %q", got)
	}
	content, err := os.ReadFile(nodeIDPath)
	if err != nil {
		t.Fatal(err)
	}
	if string(content) != got+"\n" {
		t.Fatalf("stored node id = %q", content)
	}
}

func TestEnsureIdentityNormalizesBinaryNodeID(t *testing.T) {
	root := t.TempDir()
	restore := setConfigTestPaths(root)
	defer restore()

	raw := []byte{
		0x55, 0x0e, 0x84, 0x00, 0xe2, 0x9b, 0x41, 0xd4,
		0xa7, 0x16, 0x44, 0x66, 0x55, 0x44, 0x00, 0x00,
	}
	nodeIDPath := filepath.Join(root, "config", "node-id")
	if err := os.WriteFile(nodeIDPath, raw, 0644); err != nil {
		t.Fatal(err)
	}

	got, err := ensureNodeIDFile(nodeIDPath)
	if err != nil {
		t.Fatal(err)
	}
	if got != "550e8400-e29b-41d4-a716-446655440000" {
		t.Fatalf("normalized node id = %q", got)
	}
}

func TestParseNodeIDFileRejectsInvalid(t *testing.T) {
	if _, err := parseNodeIDFile([]byte("not-a-uuid")); err == nil {
		t.Fatal("expected invalid node identity error")
	}
}
