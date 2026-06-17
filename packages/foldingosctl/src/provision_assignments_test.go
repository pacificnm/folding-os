package main

import (
	"encoding/hex"
	"os"
	"path/filepath"
	"testing"
)

const testFoldOpsManifestV2Assignment = `schema_version = 2
manifest_release = "0.2.0-1"
architecture = "x86_64"
artifact_format = "layout-tar-zst"
minimum_foldingos_version = "0.1.0"

[[packages]]
name = "foldops-agent"
version = "0.1.0"
roles = ["agent", "supervisor"]
install_prefix = "foldops-agent"
artifact_url = "https://packages.folding-os.com/foldops/0.2.0-1/foldops-agent-x86_64.tar.zst"
artifact_size = 3740000
sha256 = "9022c393630e11d5cec5794ac77281671c7b0d634d630c92d95ad6de22d2151a"
verification_path = "/data/apps/foldops/current/foldops-agent/usr/bin/foldops-agent"

[[packages]]
name = "foldops-supervisor"
version = "0.1.0"
roles = ["supervisor"]
install_prefix = "foldops-supervisor"
artifact_url = "https://packages.folding-os.com/foldops/0.2.0-1/foldops-supervisor-x86_64.tar.zst"
artifact_size = 3720000
sha256 = "a8b91ec03803259ade0bc3595218d74408390f6ac4e0f077cc47ba85edaaa8d5"
verification_path = "/data/apps/foldops/current/foldops-supervisor/usr/bin/foldops-supervisor"

[[packages]]
name = "foldops-web"
version = "0.1.0"
roles = ["supervisor"]
install_prefix = "foldops-web"
artifact_url = "https://packages.folding-os.com/foldops/0.2.0-1/foldops-web-x86_64.tar.zst"
artifact_size = 174000
sha256 = "e560956f0aa6f77677af9bbac464a71ebcf0ff1da19877070f6f8dc05f738ecf"
verification_path = "/data/apps/foldops/current/foldops-web/usr/share/foldops/web/index.html"
`

func TestAssignSoftwareVersionsUpdatesEnrollmentAndLocalFiles(t *testing.T) {
	root := t.TempDir()
	restoreProvision := setProvisionPaths(root)
	defer restoreProvision()
	restoreConfig := setConfigTestPaths(root)
	defer restoreConfig()
	restoreRole := setInstallationRolePaths(
		filepath.Join(root, "efi", "installation-role"),
		filepath.Join(root, "config", "installation-role"),
	)
	defer restoreRole()
	restoreFoldOpsRegistry := setFoldOpsManifestRegistryPaths(root)
	defer restoreFoldOpsRegistry()
	restoreToolsRegistry := setToolsVersionRegistryPaths(root)
	defer restoreToolsRegistry()
	restoreFoldOpsAssigned := setFoldOpsManifestPaths(
		filepath.Join(root, "bootstrap.toml"),
		filepath.Join(root, "config", "foldops", "assigned-manifest.toml"),
	)
	defer restoreFoldOpsAssigned()
	restoreToolsAssigned := setToolsAssignmentPaths(
		filepath.Join(root, "bootstrap-tools.json"),
		filepath.Join(root, "config", "tools", "assigned-version.json"),
	)
	defer restoreToolsAssigned()

	if err := os.MkdirAll(filepath.Join(root, "config"), 0755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(root, "config", "installation-role"), []byte("supervisor\n"), 0644); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(root, "config", "node-id"), []byte(testAgentNodeID+"\n"), 0644); err != nil {
		t.Fatal(err)
	}
	if err := os.MkdirAll(filepath.Join(root, "registry", "entries"), 0755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(
		filepath.Join(root, "registry", "entries", "0.2.0.json"),
		[]byte(`{"schema_version":1,"foldingos_version":"0.2.0","git_revision":"abc","image_sha256":"9022c393630e11d5cec5794ac77281671c7b0d634d630c92d95ad6de22d2151a","image_size_bytes":4294967296,"verification_method":"sha256","import_timestamp":"2026-01-01T00:00:00Z","rollout_state":"ready","local_image_path":"/tmp/image.img"}`+"\n"),
		0644,
	); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(root, "registry", "index.json"), []byte(`{"schema_version":1,"versions":["0.2.0"]}`+"\n"), 0644); err != nil {
		t.Fatal(err)
	}

	manifestPath := filepath.Join(root, "foldops-manifest.toml")
	if err := os.WriteFile(manifestPath, []byte(testFoldOpsManifestV2Assignment), 0644); err != nil {
		t.Fatal(err)
	}
	if err := registryImportFoldOpsManifest([]string{"--manifest", manifestPath}); err != nil {
		t.Fatal(err)
	}

	toolsDir := filepath.Join(root, "tools-release")
	if err := os.MkdirAll(toolsDir, 0755); err != nil {
		t.Fatal(err)
	}
	toolsBinary := []byte("foldingosctl-test-binary")
	if err := os.WriteFile(filepath.Join(toolsDir, toolsArtifactBasename), toolsBinary, 0755); err != nil {
		t.Fatal(err)
	}
	digest := hexEncodeSHA256(toolsBinary)
	if err := os.WriteFile(filepath.Join(toolsDir, "SHA256SUMS"), []byte(digest+"  "+toolsArtifactBasename+"\n"), 0644); err != nil {
		t.Fatal(err)
	}
	if err := registryImportToolsRelease([]string{"--dir", toolsDir, "--version", "0.2.0"}); err != nil {
		t.Fatal(err)
	}

	record := enrollmentRecord{
		SchemaVersion:       1,
		NodeID:              testAgentNodeID,
		InstallationRole:    "agent",
		RegisteredAt:        "2026-01-01T00:00:00Z",
		LastSeenAt:          "2026-01-01T00:00:00Z",
		MACAddresses:        []string{"52:54:00:12:34:56"},
		CurrentImageVersion: "0.1.0",
		FoldingOSVersion:    "0.1.0",
		Hostname:            "folding-test",
		DesiredImageVersion: "current",
	}
	if err := saveEnrollmentRecord(record); err != nil {
		t.Fatal(err)
	}

	foldOpsRelease := "0.2.0-1"
	toolsVersion := "0.2.0"
	updated, err := assignSoftwareVersions("node", testAgentNodeID, softwareAssignmentUpdate{
		foldOpsManifestRelease: &foldOpsRelease,
		toolsVersion:           &toolsVersion,
	})
	if err != nil {
		t.Fatal(err)
	}
	if updated != 1 {
		t.Fatalf("updated = %d", updated)
	}

	stored, err := loadEnrollmentRecord(testAgentNodeID)
	if err != nil {
		t.Fatal(err)
	}
	if stored.DesiredFoldOpsManifestRelease != "0.2.0-1" || stored.DesiredToolsVersion != "0.2.0" {
		t.Fatalf("unexpected enrollment assignment: %+v", stored)
	}

	response, err := desiredVersionForNode(testAgentNodeID)
	if err != nil {
		t.Fatal(err)
	}
	if response.SchemaVersion != 2 || response.DesiredFoldOpsManifest == "" || response.DesiredToolsAssignment == nil {
		t.Fatalf("unexpected desired-version response: %+v", response)
	}

	if _, err := os.Stat(foldOpsAssignedManifestPath); err != nil {
		t.Fatalf("assigned foldops manifest was not written locally: %v", err)
	}
	if _, err := os.Stat(toolsAssignedVersionPath); err != nil {
		t.Fatalf("assigned tools version was not written locally: %v", err)
	}
}

func setFoldOpsManifestRegistryPaths(root string) func() {
	previousDir := foldopsManifestRegistryDir
	previousIndex := foldopsManifestRegistryIndex
	foldopsManifestRegistryDir = filepath.Join(root, "registry", "foldops")
	foldopsManifestRegistryIndex = filepath.Join(foldopsManifestRegistryDir, "index.json")
	return func() {
		foldopsManifestRegistryDir = previousDir
		foldopsManifestRegistryIndex = previousIndex
	}
}

func setToolsVersionRegistryPaths(root string) func() {
	previousDir := toolsVersionRegistryDir
	previousIndex := toolsVersionRegistryIndex
	toolsVersionRegistryDir = filepath.Join(root, "registry", "tools")
	toolsVersionRegistryIndex = filepath.Join(toolsVersionRegistryDir, "index.json")
	return func() {
		toolsVersionRegistryDir = previousDir
		toolsVersionRegistryIndex = previousIndex
	}
}

func hexEncodeSHA256(data []byte) string {
	return hex.EncodeToString(sha256Sum(data))
}
