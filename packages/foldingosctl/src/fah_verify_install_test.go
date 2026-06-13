package main

import (
	"os"
	"path/filepath"
	"testing"
)

func TestExtractAndInstallFAHArtifact(t *testing.T) {
	debPath := "/tmp/fah-manifest-work/fah-client_8.5.6_amd64.deb"
	if _, err := os.Stat(debPath); err != nil {
		t.Skip("approved FAH deb artifact is unavailable for install test")
	}

	appsRoot := t.TempDir()
	downloadsDir := filepath.Join(appsRoot, ".downloads")
	versionDir := filepath.Join(appsRoot, "8.5.6")
	if err := os.MkdirAll(downloadsDir, 0755); err != nil {
		t.Fatal(err)
	}
	if err := copyFile(debPath, filepath.Join(downloadsDir, "8.5.6.deb")); err != nil {
		t.Fatal(err)
	}

	manifest, err := parseFAHManifest(validFAHManifest)
	if err != nil {
		t.Fatal(err)
	}

	restoreApps := setFAHAppsRoot(appsRoot)
	defer restoreApps()
	restoreDownloads := setFAHDownloadsDir(downloadsDir)
	defer restoreDownloads()

	installed, err := extractAndInstallFAHArtifact(manifest)
	if err != nil {
		t.Fatal(err)
	}
	if installed != versionDir {
		t.Fatalf("installed path = %q", installed)
	}
	if !fahInstallationVerified("8.5.6", manifest) {
		t.Fatal("installation was not marked verified")
	}
}

func TestVerifyFAHExecutableELF(t *testing.T) {
	executable := "/tmp/fah-manifest-work/deb-inspect/data/usr/bin/fah-client"
	if _, err := os.Stat(executable); err != nil {
		t.Skip("approved FAH executable is unavailable for ELF test")
	}
	if err := verifyFAHExecutableELF(executable); err != nil {
		t.Fatal(err)
	}
}

func TestRejectUnsafeFAHInstallPermissions(t *testing.T) {
	root := t.TempDir()
	unsafe := filepath.Join(root, "unsafe.txt")
	if err := os.WriteFile(unsafe, []byte("x"), 0666); err != nil {
		t.Fatal(err)
	}
	executable := filepath.Join(root, "usr", "bin", "fah-client")
	if err := os.MkdirAll(filepath.Dir(executable), 0755); err != nil {
		t.Fatal(err)
	}
	if err := copyFile("/tmp/fah-manifest-work/deb-inspect/data/usr/bin/fah-client", executable); err != nil {
		t.Skip("approved FAH executable is unavailable for permission test")
	}
	if err := os.Chmod(executable, 0755); err != nil {
		t.Fatal(err)
	}
	if err := verifyFAHInstallLayout(root, executable); err == nil {
		t.Fatal("world-writable file was accepted")
	}
}

func TestValidateFAHVersionLabel(t *testing.T) {
	if err := validateFAHVersionLabel("8.5.6"); err != nil {
		t.Fatal(err)
	}
	if err := validateFAHVersionLabel("../8.5.6"); err == nil {
		t.Fatal("path traversal version was accepted")
	}
	if err := validateFAHVersionLabel("9.0.0"); err == nil {
		t.Fatal("unsupported version family was accepted")
	}
}

func copyFile(source, destination string) error {
	content, err := os.ReadFile(source)
	if err != nil {
		return err
	}
	if err := os.MkdirAll(filepath.Dir(destination), 0755); err != nil {
		return err
	}
	return os.WriteFile(destination, content, 0644)
}
