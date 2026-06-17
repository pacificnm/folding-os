package main

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
)

const validFoldOpsManifest = `schema_version = 1
manifest_release = "0.1.0-1"
architecture = "x86_64"
artifact_format = "deb"
minimum_foldingos_version = "0.1.0"

[[packages]]
name = "foldops-agent"
version = "0.1.0-1"
roles = ["agent", "supervisor"]
artifact_url = "https://deb.folding-os.com/pool/main/f/foldops-agent/foldops-agent_0.1.0-1_amd64.deb"
artifact_size = 3127044
sha256 = "9022c393630e11d5cec5794ac77281671c7b0d634d630c92d95ad6de22d2151a"
verification_path = "/data/apps/foldops/current/foldops-agent/usr/bin/foldops-agent"

[[packages]]
name = "foldops-supervisor"
version = "0.1.0-1"
roles = ["supervisor"]
artifact_url = "https://deb.folding-os.com/pool/main/f/foldops-supervisor/foldops-supervisor_0.1.0-1_amd64.deb"
artifact_size = 3111920
sha256 = "a8b91ec03803259ade0bc3595218d74408390f6ac4e0f077cc47ba85edaaa8d5"
verification_path = "/data/apps/foldops/current/foldops-supervisor/usr/bin/foldops-supervisor"

[[packages]]
name = "foldops-web"
version = "0.1.0"
roles = ["supervisor"]
artifact_url = "https://deb.folding-os.com/pool/main/f/foldops-web/foldops-web_0.1.0_all.deb"
artifact_size = 174466
sha256 = "e560956f0aa6f77677af9bbac464a71ebcf0ff1da19877070f6f8dc05f738ecf"
verification_path = "/data/apps/foldops/current/foldops-web/usr/share/foldops/web/index.html"
`

const validFoldOpsManifestV2 = `schema_version = 2
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

func TestParseApprovedFoldOpsManifest(t *testing.T) {
	manifest, err := parseFoldOpsManifest(validFoldOpsManifest)
	if err != nil {
		t.Fatal(err)
	}
	if err := validateFoldOpsManifest(manifest); err != nil {
		t.Fatal(err)
	}
	if manifest.ManifestRelease != "0.1.0-1" || len(manifest.Packages) != 3 {
		t.Fatalf("unexpected manifest: %+v", manifest)
	}
}

func TestFoldOpsPackagesForRole(t *testing.T) {
	manifest, err := parseFoldOpsManifest(validFoldOpsManifest)
	if err != nil {
		t.Fatal(err)
	}
	agentPackages, err := foldOpsPackagesForRole(manifest, "agent")
	if err != nil {
		t.Fatal(err)
	}
	if len(agentPackages) != 1 || agentPackages[0].Name != "foldops-agent" {
		t.Fatalf("unexpected agent packages: %+v", agentPackages)
	}
	supervisorPackages, err := foldOpsPackagesForRole(manifest, "supervisor")
	if err != nil {
		t.Fatal(err)
	}
	if len(supervisorPackages) != 3 {
		t.Fatalf("unexpected supervisor packages: %+v", supervisorPackages)
	}
}

func TestRejectUnknownFoldOpsManifestKey(t *testing.T) {
	content := strings.Replace(validFoldOpsManifest, `artifact_format = "deb"`, `artifact_format = "deb"
latest = true`, 1)
	if _, err := parseFoldOpsManifest(content); err == nil {
		t.Fatal("unknown manifest key was accepted")
	}
}

func TestRejectUnpinnedLatestFoldOpsArtifactURL(t *testing.T) {
	content := strings.Replace(
		validFoldOpsManifest,
		`foldops-agent_0.1.0-1_amd64.deb"`,
		`latest.deb"`,
		1,
	)
	manifest, err := parseFoldOpsManifest(content)
	if err != nil {
		t.Fatal(err)
	}
	if err := validateFoldOpsManifest(manifest); err == nil {
		t.Fatal("unpinned latest artifact URL was accepted")
	}
}

func TestRejectInvalidFoldOpsOrigin(t *testing.T) {
	content := strings.Replace(
		validFoldOpsManifest,
		`https://deb.folding-os.com/`,
		`https://evil.example/`,
		1,
	)
	manifest, err := parseFoldOpsManifest(content)
	if err != nil {
		t.Fatal(err)
	}
	if err := validateFoldOpsManifest(manifest); err == nil {
		t.Fatal("non-approved origin was accepted")
	}
}

func TestRejectVerificationPathOutsideCurrent(t *testing.T) {
	content := strings.Replace(
		validFoldOpsManifest,
		`verification_path = "/data/apps/foldops/current/foldops-agent/usr/bin/foldops-agent"`,
		`verification_path = "/usr/bin/foldops-agent"`,
		1,
	)
	manifest, err := parseFoldOpsManifest(content)
	if err != nil {
		t.Fatal(err)
	}
	if err := validateFoldOpsManifest(manifest); err == nil {
		t.Fatal("verification path outside current was accepted")
	}
}

func TestRejectExternalFoldOpsManifestPath(t *testing.T) {
	if _, err := loadFoldOpsManifestFromFile("/tmp/foldops.toml"); err == nil {
		t.Fatal("external manifest path was accepted")
	}
}

func TestParseSchemaV2LayoutManifest(t *testing.T) {
	manifest, err := parseFoldOpsManifest(validFoldOpsManifestV2)
	if err != nil {
		t.Fatal(err)
	}
	if err := validateFoldOpsManifest(manifest); err != nil {
		t.Fatal(err)
	}
	if manifest.SchemaVersion != 2 || manifest.ArtifactFormat != "layout-tar-zst" {
		t.Fatalf("unexpected manifest header: %+v", manifest)
	}
	if manifest.Packages[0].InstallPrefix != "foldops-agent" {
		t.Fatalf("unexpected install_prefix: %+v", manifest.Packages[0])
	}
}

func TestRejectSchemaV2LayoutMissingInstallPrefix(t *testing.T) {
	content := strings.Replace(
		validFoldOpsManifestV2,
		"install_prefix = \"foldops-agent\"\n",
		"",
		1,
	)
	manifest, err := parseFoldOpsManifest(content)
	if err != nil {
		t.Fatal(err)
	}
	if err := validateFoldOpsManifest(manifest); err == nil {
		t.Fatal("layout manifest without install_prefix was accepted")
	}
}

func TestResolveEffectiveFoldOpsManifestPrefersAssigned(t *testing.T) {
	root := t.TempDir()
	bootstrapPath := filepath.Join(root, "bootstrap.toml")
	assignedPath := filepath.Join(root, "assigned.toml")
	if err := os.WriteFile(bootstrapPath, []byte(validFoldOpsManifest), 0644); err != nil {
		t.Fatal(err)
	}
	assigned := validFoldOpsManifestV2
	if err := os.WriteFile(assignedPath, []byte(assigned), 0644); err != nil {
		t.Fatal(err)
	}
	restore := setFoldOpsManifestPaths(bootstrapPath, assignedPath)
	defer restore()

	manifest, err := resolveEffectiveFoldOpsManifest()
	if err != nil {
		t.Fatal(err)
	}
	if manifest.ManifestRelease != "0.2.0-1" {
		t.Fatalf("expected assigned manifest release, got %q", manifest.ManifestRelease)
	}
	if manifest.ArtifactFormat != "layout-tar-zst" {
		t.Fatalf("expected layout manifest, got %q", manifest.ArtifactFormat)
	}
}

func TestRejectMissingRequiredFoldOpsPackage(t *testing.T) {
	content := strings.Replace(validFoldOpsManifest, `
[[packages]]
name = "foldops-web"
version = "0.1.0"
roles = ["supervisor"]
artifact_url = "https://deb.folding-os.com/pool/main/f/foldops-web/foldops-web_0.1.0_all.deb"
artifact_size = 174466
sha256 = "e560956f0aa6f77677af9bbac464a71ebcf0ff1da19877070f6f8dc05f738ecf"
verification_path = "/data/apps/foldops/current/foldops-web/usr/share/foldops/web/index.html"
`, "", 1)
	manifest, err := parseFoldOpsManifest(content)
	if err != nil {
		t.Fatal(err)
	}
	if err := validateFoldOpsManifest(manifest); err == nil {
		t.Fatal("missing required package was accepted")
	}
}

func TestRejectInvalidFoldOpsPackageRoles(t *testing.T) {
	content := strings.Replace(
		validFoldOpsManifest,
		`roles = ["agent", "supervisor"]`,
		`roles = ["agent"]`,
		1,
	)
	manifest, err := parseFoldOpsManifest(content)
	if err != nil {
		t.Fatal(err)
	}
	if err := validateFoldOpsManifest(manifest); err == nil {
		t.Fatal("invalid package roles were accepted")
	}
}
