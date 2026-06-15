package main

import (
	"os"
	"path/filepath"
	"testing"
)

func TestSetGrubEnvVarUpdatesNextEntry(t *testing.T) {
	root := t.TempDir()
	path := filepath.Join(root, "grubenv")
	initial, err := formatGrubEnvBlock(map[string]string{})
	if err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(path, initial, 0644); err != nil {
		t.Fatal(err)
	}

	if err := setGrubEnvVar(path, "next_entry", "1"); err != nil {
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
	if vars["next_entry"] != "1" {
		t.Fatalf("next_entry = %q", vars["next_entry"])
	}
	if len(content) != grubEnvBlockSize {
		t.Fatalf("grubenv size = %d", len(content))
	}
}
