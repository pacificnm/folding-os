package main

import (
	"strings"
	"testing"
)

const validFAHManifest = `schema_version = 1
client_version = "8.5.6"
architecture = "x86_64"
artifact_url = "https://download.foldingathome.org/releases/beta/fah-client/debian-10-64bit/release/fah-client_8.5.6_amd64.deb"
artifact_size = 3205180
sha256 = "643de04033a1cb972a81e3a193d710e919a4f34634a987f11adc4cee61fdaefe"
artifact_format = "deb"
minimum_foldingos_version = "0.1.0"
terms_url = "https://foldingathome.org/faq/opensource/"
executable_path = "/data/apps/fah/current/usr/bin/fah-client"
arguments = [
  "--config=/run/foldingos/fah/config.xml",
  "--log=/data/fah/log.txt",
  "--log-rotate-dir=/data/fah/log/",
]
`

func TestParseApprovedFAHManifest(t *testing.T) {
	manifest, err := parseFAHManifest(validFAHManifest)
	if err != nil {
		t.Fatal(err)
	}
	if err := validateFAHManifest(manifest); err != nil {
		t.Fatal(err)
	}
	if manifest.ClientVersion != "8.5.6" || len(manifest.Arguments) != 3 {
		t.Fatalf("unexpected manifest: %+v", manifest)
	}
}

func TestRejectUnknownFAHManifestKey(t *testing.T) {
	content := strings.Replace(validFAHManifest, `artifact_format = "deb"`, `artifact_format = "deb"
latest = true`, 1)
	if _, err := parseFAHManifest(content); err == nil {
		t.Fatal("unknown manifest key was accepted")
	}
}

func TestRejectUnpinnedLatestArtifactURL(t *testing.T) {
	content := strings.Replace(
		validFAHManifest,
		`fah-client_8.5.6_amd64.deb"`,
		`latest.deb"`,
		1,
	)
	manifest, err := parseFAHManifest(content)
	if err != nil {
		t.Fatal(err)
	}
	if err := validateFAHManifest(manifest); err == nil {
		t.Fatal("unpinned latest artifact URL was accepted")
	}
}

func TestRejectInvalidFAHOrigin(t *testing.T) {
	content := strings.Replace(
		validFAHManifest,
		`https://download.foldingathome.org/`,
		`https://evil.example/`,
		1,
	)
	manifest, err := parseFAHManifest(content)
	if err != nil {
		t.Fatal(err)
	}
	if err := validateFAHManifest(manifest); err == nil {
		t.Fatal("non-approved origin was accepted")
	}
}

func TestRejectExecutablePathOutsideCurrent(t *testing.T) {
	content := strings.Replace(
		validFAHManifest,
		`executable_path = "/data/apps/fah/current/usr/bin/fah-client"`,
		`executable_path = "/usr/bin/fah-client"`,
		1,
	)
	manifest, err := parseFAHManifest(content)
	if err != nil {
		t.Fatal(err)
	}
	if err := validateFAHManifest(manifest); err == nil {
		t.Fatal("executable path outside current was accepted")
	}
}

func TestRejectExternalFAHManifestPath(t *testing.T) {
	if _, err := loadFAHManifest("/tmp/fah.toml"); err == nil {
		t.Fatal("external manifest path was accepted")
	}
}

func TestRejectUppercaseSHA256(t *testing.T) {
	content := strings.Replace(
		validFAHManifest,
		`643de04033a1cb972a81e3a193d710e919a4f34634a987f11adc4cee61fdaefe`,
		`643DE04033A1CB972A81E3A193D710E919A4F34634A987F11ADC4CEE61FDAEFE`,
		1,
	)
	manifest, err := parseFAHManifest(content)
	if err != nil {
		t.Fatal(err)
	}
	if err := validateFAHManifest(manifest); err == nil {
		t.Fatal("uppercase sha256 was accepted")
	}
}

func TestRejectInvalidFAHArchitecture(t *testing.T) {
	content := strings.Replace(validFAHManifest, `architecture = "x86_64"`, `architecture = "aarch64"`, 1)
	manifest, err := parseFAHManifest(content)
	if err != nil {
		t.Fatal(err)
	}
	if err := validateFAHManifest(manifest); err == nil {
		t.Fatal("unsupported architecture was accepted")
	}
}

func TestRejectHTTPArtifactURL(t *testing.T) {
	content := strings.Replace(
		validFAHManifest,
		`artifact_url = "https://download.foldingathome.org/`,
		`artifact_url = "http://download.foldingathome.org/`,
		1,
	)
	manifest, err := parseFAHManifest(content)
	if err != nil {
		t.Fatal(err)
	}
	if err := validateFAHManifest(manifest); err == nil {
		t.Fatal("non-HTTPS artifact URL was accepted")
	}
}

func TestRejectIncompatibleFoldingOSVersion(t *testing.T) {
	content := strings.Replace(
		validFAHManifest,
		`minimum_foldingos_version = "0.1.0"`,
		`minimum_foldingos_version = "9.9.9"`,
		1,
	)
	manifest, err := parseFAHManifest(content)
	if err != nil {
		t.Fatal(err)
	}
	if err := validateFAHManifest(manifest); err == nil {
		t.Fatal("incompatible FoldingOS version was accepted")
	}
}
