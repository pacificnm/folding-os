package main

import (
	"os"
	"path/filepath"
	"testing"
)

func TestResetAgentDataPartitionState(t *testing.T) {
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

	if err := resetAgentDataPartitionState(root); err != nil {
		t.Fatal(err)
	}
	for _, relative := range agentDataPartitionResetPaths {
		if _, err := os.Stat(filepath.Join(root, relative)); !os.IsNotExist(err) {
			t.Fatalf("expected %s to be removed, stat err = %v", relative, err)
		}
	}
}

func TestClearGrubNextEntry(t *testing.T) {
	root := t.TempDir()
	path := filepath.Join(root, "grubenv")
	initial, err := formatGrubEnvBlock(map[string]string{
		"next_entry": "1",
	})
	if err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(path, initial, 0644); err != nil {
		t.Fatal(err)
	}

	if err := clearGrubNextEntry(path); err != nil {
		t.Fatal(err)
	}

	content, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	vars, err := parseGrubEnvBlock(content)
	if err != nil {
		t.Fatal(err)
	}
	if _, ok := vars["next_entry"]; ok {
		t.Fatalf("next_entry should be cleared, vars = %#v", vars)
	}
}
