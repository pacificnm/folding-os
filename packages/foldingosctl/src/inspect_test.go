package main

import (
	"bytes"
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"testing"
)

func TestAutomationSuccessEnvelope(t *testing.T) {
	automationCtx = automationContext{
		format:  formatJSON,
		command: "inspect node",
	}
	var stdout bytes.Buffer
	previous := os.Stdout
	read, write, err := os.Pipe()
	if err != nil {
		t.Fatal(err)
	}
	os.Stdout = write
	t.Cleanup(func() {
		os.Stdout = previous
	})

	if err := writeAutomationSuccess(map[string]string{"node_id": "test"}); err != nil {
		t.Fatal(err)
	}
	write.Close()
	stdout.ReadFrom(read)

	var document automationSuccessDocument
	if err := json.Unmarshal(stdout.Bytes(), &document); err != nil {
		t.Fatal(err)
	}
	if !document.OK || document.SchemaVersion != automationSchemaVersion || document.Command != "inspect node" {
		t.Fatalf("document: %+v", document)
	}
}

func TestAutomationFailureEnvelope(t *testing.T) {
	automationCtx = automationContext{
		format:  formatJSON,
		command: "registry list",
	}
	read, write, err := os.Pipe()
	if err != nil {
		t.Fatal(err)
	}
	previous := os.Stdout
	os.Stdout = write
	t.Cleanup(func() {
		os.Stdout = previous
	})

	err = writeAutomationFailure(fmt.Errorf("operation requires supervisor role, found %q", "agent"))
	write.Close()
	if err == nil {
		t.Fatal("expected role error")
	}

	var stdout bytes.Buffer
	stdout.ReadFrom(read)
	var document automationFailureDocument
	if err := json.Unmarshal(stdout.Bytes(), &document); err != nil {
		t.Fatal(err)
	}
	if document.OK || document.Error.Code != "role_required" {
		t.Fatalf("document: %+v", document)
	}
}

func TestInspectFoldOpsJSONGolden(t *testing.T) {
	setupInspectRuntimePaths(t, t.TempDir())
	automationCtx = automationContext{
		format:  formatJSON,
		command: "inspect foldops",
	}
	restoreUser := stubAutomationUser("foldops")
	defer restoreUser()

	data, err := collectInspectFoldOpsData()
	if err != nil {
		t.Fatal(err)
	}
	content, err := marshalAutomationData(data)
	if err != nil {
		t.Fatal(err)
	}
	assertMatchesGolden(t, "inspect_foldops.json", content)
}

func TestInspectToolsJSONGolden(t *testing.T) {
	setupInspectRuntimePaths(t, t.TempDir())
	automationCtx = automationContext{
		format:  formatJSON,
		command: "inspect tools",
	}
	restoreUser := stubAutomationUser("foldops")
	defer restoreUser()

	data, err := collectInspectToolsData()
	if err != nil {
		t.Fatal(err)
	}
	data.Binary.Path = "/usr/bin/foldingosctl"
	data.Binary.ModTimeUnix = 1704067200
	content, err := marshalAutomationData(data)
	if err != nil {
		t.Fatal(err)
	}
	assertMatchesGolden(t, "inspect_tools.json", content)
}

func TestInspectAllowedForFoldOpsUserWithoutRole(t *testing.T) {
	root := t.TempDir()
	restoreConfig := setConfigTestPaths(root)
	defer restoreConfig()
	restoreRole := setInstallationRolePaths(
		filepath.Join(root, "efi", "installation-role"),
		filepath.Join(root, "data", "installation-role"),
	)
	defer restoreRole()
	restoreUser := stubAutomationUser("foldops")
	defer restoreUser()

	if err := requireInspectableRole(); err != nil {
		t.Fatalf("foldops user should be allowed: %v", err)
	}
}

func TestProvisionListEnrollmentsJSON(t *testing.T) {
	root := t.TempDir()
	restoreProvision := setProvisionPaths(root)
	defer restoreProvision()
	restoreRole := setInstallationRolePaths(
		filepath.Join(root, "provision-role"),
		filepath.Join(root, "config", "installation-role"),
	)
	defer restoreRole()
	if err := os.MkdirAll(filepath.Join(root, "config"), 0755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(root, "config", "installation-role"), []byte("supervisor"), 0644); err != nil {
		t.Fatal(err)
	}
	writeEnrollmentToken(t, root, "test-enrollment-token")
	if _, err := registerAgent(sampleRegistrationRequest("test-enrollment-token")); err != nil {
		t.Fatal(err)
	}

	automationCtx = automationContext{
		format:  formatJSON,
		command: "provision list-enrollments",
	}
	read, write, err := os.Pipe()
	if err != nil {
		t.Fatal(err)
	}
	previous := os.Stdout
	os.Stdout = write
	t.Cleanup(func() {
		os.Stdout = previous
	})
	if err := provisionListEnrollments(); err != nil {
		t.Fatal(err)
	}
	write.Close()

	var stdout bytes.Buffer
	stdout.ReadFrom(read)
	var document automationSuccessDocument
	if err := json.Unmarshal(stdout.Bytes(), &document); err != nil {
		t.Fatal(err)
	}
	if !document.OK {
		t.Fatalf("document: %+v", document)
	}
}

func setupInspectRuntimePaths(t *testing.T, root string) {
	t.Helper()
	restoreFoldOpsPaths := setFoldOpsManifestPaths(
		filepath.Join(root, "bootstrap-foldops.toml"),
		filepath.Join(root, "assigned-foldops.toml"),
	)
	t.Cleanup(restoreFoldOpsPaths)
	restoreToolsPaths := setToolsAssignmentPaths(
		filepath.Join(root, "bootstrap-tools.json"),
		filepath.Join(root, "assigned-tools.json"),
	)
	t.Cleanup(restoreToolsPaths)

	previousAppsRoot := foldOpsAppsRoot
	previousProvisioned := foldOpsProvisionedMarkerPath
	previousToolsBinary := toolsBinaryPath
	previousToolsActive := toolsActiveStatePath

	foldOpsAppsRoot = filepath.Join(root, "apps", "foldops")
	foldOpsProvisionedMarkerPath = filepath.Join(root, "state", "foldops", "provisioned.json")
	toolsBinaryPath = filepath.Join(root, "foldingosctl")
	toolsActiveStatePath = filepath.Join(root, "state", "tools", "active.json")

	t.Cleanup(func() {
		foldOpsAppsRoot = previousAppsRoot
		foldOpsProvisionedMarkerPath = previousProvisioned
		toolsBinaryPath = previousToolsBinary
		toolsActiveStatePath = previousToolsActive
	})

	if err := os.MkdirAll(filepath.Dir(foldOpsEmbeddedManifestPath), 0755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(foldOpsEmbeddedManifestPath, []byte(validFoldOpsManifest), 0644); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(foldOpsAssignedManifestPath, []byte(validFoldOpsManifestV2), 0644); err != nil {
		t.Fatal(err)
	}
	if err := os.MkdirAll(filepath.Join(foldOpsAppsRoot, "0.2.0-1"), 0755); err != nil {
		t.Fatal(err)
	}
	if err := os.Symlink("0.2.0-1", filepath.Join(foldOpsAppsRoot, "current")); err != nil {
		t.Fatal(err)
	}
	if err := os.MkdirAll(filepath.Dir(foldOpsProvisionedMarkerPath), 0755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(foldOpsProvisionedMarkerPath, []byte(`{"schema_version":1,"role":"agent","manifest_release":"0.2.0-1","provisioned_at":"2026-01-01T00:00:00Z"}`+"\n"), 0644); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(toolsBootstrapManifestPath, []byte(testToolsBootstrapAssignmentJSON), 0644); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(toolsAssignedVersionPath, []byte(testToolsAssignedAssignmentJSON), 0644); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(toolsBinaryPath, []byte("foldingosctl-binary"), 0755); err != nil {
		t.Fatal(err)
	}
}

func stubAutomationUser(username string) func() {
	previous := currentUnixUsername
	currentUnixUsername = func() string { return username }
	return func() {
		currentUnixUsername = previous
	}
}

func assertMatchesGolden(t *testing.T, name string, actual []byte) {
	t.Helper()
	goldenPath := filepath.Join("testdata", name)
	if os.Getenv("UPDATE_GOLDEN") == "1" {
		if err := os.MkdirAll(filepath.Dir(goldenPath), 0755); err != nil {
			t.Fatal(err)
		}
		if err := os.WriteFile(goldenPath, actual, 0644); err != nil {
			t.Fatal(err)
		}
	}
	expected, err := os.ReadFile(goldenPath)
	if err != nil {
		t.Fatalf("read golden %s: %v", name, err)
	}
	if !bytes.Equal(bytes.TrimSpace(expected), bytes.TrimSpace(actual)) {
		t.Fatalf("golden mismatch for %s\nexpected:\n%s\nactual:\n%s", name, expected, actual)
	}
}

const testToolsBootstrapAssignmentJSON = `{
  "schema_version": 1,
  "tools_version": "0.1.0",
  "artifact_url": "https://packages.folding-os.com/foldingos-tools/0.1.0/foldingosctl-x86_64",
  "artifact_size": 12000000,
  "sha256": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
}
`

const testToolsAssignedAssignmentJSON = `{
  "schema_version": 1,
  "tools_version": "0.2.0",
  "artifact_url": "https://packages.folding-os.com/foldingos-tools/0.2.0/foldingosctl-x86_64",
  "artifact_size": 12000000,
  "sha256": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
}
`
