package main

import (
	"encoding/hex"
	"os"
	"path/filepath"
	"testing"
)

const validToolsAssignmentJSON = `{
  "schema_version": 1,
  "tools_version": "0.2.0",
  "artifact_url": "https://packages.folding-os.com/foldingos-tools/0.2.0/foldingosctl-x86_64",
  "artifact_size": 12345,
  "sha256": "9022c393630e11d5cec5794ac77281671c7b0d634d630c92d95ad6de22d2151a"
}`

func TestParseToolsAssignment(t *testing.T) {
	assignment, err := parseToolsAssignment([]byte(validToolsAssignmentJSON))
	if err != nil {
		t.Fatal(err)
	}
	if err := validateToolsAssignment(assignment); err != nil {
		t.Fatal(err)
	}
	if assignment.ToolsVersion != "0.2.0" {
		t.Fatalf("unexpected tools version: %+v", assignment)
	}
}

func TestRejectInvalidToolsAssignmentOrigin(t *testing.T) {
	content := `{
  "schema_version": 1,
  "tools_version": "0.2.0",
  "artifact_url": "https://evil.example/foldingos-tools/0.2.0/foldingosctl-x86_64",
  "artifact_size": 12345,
  "sha256": "9022c393630e11d5cec5794ac77281671c7b0d634d630c92d95ad6de22d2151a"
}`
	assignment, err := parseToolsAssignment([]byte(content))
	if err != nil {
		t.Fatal(err)
	}
	if err := validateToolsAssignment(assignment); err == nil {
		t.Fatal("non-approved origin was accepted")
	}
}

func TestResolveEffectiveToolsAssignmentPrefersAssigned(t *testing.T) {
	root := t.TempDir()
	bootstrapPath := filepath.Join(root, "bootstrap.json")
	assignedPath := filepath.Join(root, "assigned.json")
	if err := os.WriteFile(bootstrapPath, []byte(`{
  "schema_version": 1,
  "tools_version": "0.1.0",
  "artifact_url": "https://packages.folding-os.com/foldingos-tools/0.1.0/foldingosctl-x86_64",
  "artifact_size": 1000,
  "sha256": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
}`), 0644); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(assignedPath, []byte(validToolsAssignmentJSON), 0644); err != nil {
		t.Fatal(err)
	}
	restore := setToolsAssignmentPaths(bootstrapPath, assignedPath)
	defer restore()

	assignment, found, err := resolveEffectiveToolsAssignment()
	if err != nil {
		t.Fatal(err)
	}
	if !found {
		t.Fatal("expected tools assignment")
	}
	if assignment.ToolsVersion != "0.2.0" {
		t.Fatalf("expected assigned tools version, got %q", assignment.ToolsVersion)
	}
}

func TestToolsAcquireWithoutAssignment(t *testing.T) {
	root := t.TempDir()
	restore := setToolsAssignmentPaths(
		filepath.Join(root, "missing-bootstrap.json"),
		filepath.Join(root, "missing-assigned.json"),
	)
	defer restore()

	if err := toolsAcquire(); err != nil {
		t.Fatal(err)
	}
}

func TestToolsInstallationVerified(t *testing.T) {
	root := t.TempDir()
	binaryPath := filepath.Join(root, "foldingosctl")
	assignment := toolsAssignment{
		SchemaVersion: 1,
		ToolsVersion:  "0.2.0",
		ArtifactURL:   "https://packages.folding-os.com/foldingos-tools/0.2.0/foldingosctl-x86_64",
		ArtifactSize:  16,
		SHA256:        "9022c393630e11d5cec5794ac77281671c7b0d634d630c92d95ad6de22d2151a",
	}
	payload := []byte{0x7f, 'E', 'L', 'F', 2, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0}
	assignment.ArtifactSize = int64(len(payload))
	assignment.SHA256 = hex.EncodeToString(sha256Sum(payload))
	if err := os.WriteFile(binaryPath, payload, 0755); err != nil {
		t.Fatal(err)
	}

	restoreBinary := setToolsBinaryPath(binaryPath)
	defer restoreBinary()
	restoreState := setToolsActiveStatePath(filepath.Join(root, "active.json"))
	defer restoreState()

	if toolsInstallationVerified(assignment) {
		t.Fatal("missing active state was treated as verified")
	}

	if err := saveToolsActiveState(toolsActiveState{
		ToolsVersion: assignment.ToolsVersion,
		SHA256:       assignment.SHA256,
	}); err != nil {
		t.Fatal(err)
	}
	if !toolsInstallationVerified(assignment) {
		t.Fatal("expected verified installation")
	}
}
