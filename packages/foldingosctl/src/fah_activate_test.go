package main

import (
	"errors"
	"os"
	"path/filepath"
	"testing"
)

func TestActivateFAHCurrentSymlink(t *testing.T) {
	appsRoot := t.TempDir()
	versionDir := filepath.Join(appsRoot, "8.5.6")
	if err := os.MkdirAll(versionDir, 0755); err != nil {
		t.Fatal(err)
	}

	restoreApps := setFAHAppsRoot(appsRoot)
	defer restoreApps()

	if err := activateFAHCurrentSymlink("8.5.6"); err != nil {
		t.Fatal(err)
	}

	currentPath := filepath.Join(appsRoot, "current")
	target, err := os.Readlink(currentPath)
	if err != nil {
		t.Fatal(err)
	}
	if target != "8.5.6" {
		t.Fatalf("current target = %q", target)
	}
	if filepath.IsAbs(target) {
		t.Fatal("current must be a relative symlink")
	}
}

func TestActivateFAHPreservesPreviousCurrentOnFailure(t *testing.T) {
	appsRoot := t.TempDir()
	oldVersion := filepath.Join(appsRoot, "8.5.6")
	if err := os.MkdirAll(oldVersion, 0755); err != nil {
		t.Fatal(err)
	}
	if err := os.Symlink("8.5.6", filepath.Join(appsRoot, "current")); err != nil {
		t.Fatal(err)
	}

	restoreApps := setFAHAppsRoot(appsRoot)
	defer restoreApps()

	if err := activateFAHCurrentSymlink("9.9.9"); err == nil {
		t.Fatal("missing version activation was accepted")
	}

	target, err := os.Readlink(filepath.Join(appsRoot, "current"))
	if err != nil {
		t.Fatal(err)
	}
	if target != "8.5.6" {
		t.Fatalf("previous current target = %q, want 8.5.6", target)
	}
}

func TestActivateFAHReplacesExistingCurrentAtomically(t *testing.T) {
	appsRoot := t.TempDir()
	for _, version := range []string{"8.5.5", "8.5.6"} {
		if err := os.MkdirAll(filepath.Join(appsRoot, version), 0755); err != nil {
			t.Fatal(err)
		}
	}
	if err := os.Symlink("8.5.5", filepath.Join(appsRoot, "current")); err != nil {
		t.Fatal(err)
	}

	restoreApps := setFAHAppsRoot(appsRoot)
	defer restoreApps()

	if err := activateFAHCurrentSymlink("8.5.6"); err != nil {
		t.Fatal(err)
	}
	target, err := os.Readlink(filepath.Join(appsRoot, "current"))
	if err != nil {
		t.Fatal(err)
	}
	if target != "8.5.6" {
		t.Fatalf("current target = %q", target)
	}
	if _, err := os.Stat(filepath.Join(appsRoot, "8.5.5")); err != nil {
		t.Fatal("previous verified version directory was removed")
	}
}

func TestFAHActivateRejectsUnverifiedVersion(t *testing.T) {
	restoreManifest := setFAHApprovedManifestLoader(testFAHManifestLoader(t))
	defer restoreManifest()
	restoreCompatibility := setFAHFoldingOSCompatibilityCheck(func(string) error { return nil })
	defer restoreCompatibility()

	appsRoot := t.TempDir()
	versionDir := filepath.Join(appsRoot, "8.5.6")
	if err := os.MkdirAll(filepath.Join(versionDir, "usr", "bin"), 0755); err != nil {
		t.Fatal(err)
	}
	executable := filepath.Join(versionDir, "usr", "bin", "fah-client")
	if err := copyFile("/tmp/fah-manifest-work/deb-inspect/data/usr/bin/fah-client", executable); err != nil {
		t.Skip("approved FAH executable is unavailable for activation test")
	}
	if err := os.Chmod(executable, 0755); err != nil {
		t.Fatal(err)
	}

	restoreApps := setFAHAppsRoot(appsRoot)
	defer restoreApps()
	fahRestartServiceAfterActivation = func() error { return nil }
	defer func() {
		fahRestartServiceAfterActivation = restartFAHServiceAfterActivation
	}()

	if err := fahActivate("8.5.6"); err == nil {
		t.Fatal("unverified version activation was accepted")
	}
}

func TestFAHActivateIdempotentWhenAlreadyActive(t *testing.T) {
	restoreManifest := setFAHApprovedManifestLoader(testFAHManifestLoader(t))
	defer restoreManifest()
	restoreCompatibility := setFAHFoldingOSCompatibilityCheck(func(string) error { return nil })
	defer restoreCompatibility()

	appsRoot := t.TempDir()
	versionDir := filepath.Join(appsRoot, "8.5.6")
	if err := os.MkdirAll(filepath.Join(versionDir, "usr", "bin"), 0755); err != nil {
		t.Fatal(err)
	}
	executable := filepath.Join(versionDir, "usr", "bin", "fah-client")
	if err := copyFile("/tmp/fah-manifest-work/deb-inspect/data/usr/bin/fah-client", executable); err != nil {
		t.Skip("approved FAH executable is unavailable for activation test")
	}
	if err := os.Chmod(executable, 0755); err != nil {
		t.Fatal(err)
	}
	marker := "client_version=8.5.6\nartifact_sha256=643de04033a1cb972a81e3a193d710e919a4f34634a987f11adc4cee61fdaefe\n"
	if err := os.WriteFile(filepath.Join(versionDir, fahVerifiedMarkerName), []byte(marker), 0644); err != nil {
		t.Fatal(err)
	}
	if err := os.Symlink("8.5.6", filepath.Join(appsRoot, "current")); err != nil {
		t.Fatal(err)
	}

	restoreApps := setFAHAppsRoot(appsRoot)
	defer restoreApps()
	restarted := false
	fahRestartServiceAfterActivation = func() error {
		restarted = true
		return nil
	}
	defer func() {
		fahRestartServiceAfterActivation = restartFAHServiceAfterActivation
	}()

	if err := fahActivate("8.5.6"); err != nil {
		t.Fatal(err)
	}
	if !restarted {
		t.Fatal("service restart hook was not invoked for already-active version")
	}
}

func testFAHManifestLoader(t *testing.T) func(string) (fahManifest, error) {
	t.Helper()
	manifest, err := parseFAHManifest(validFAHManifest)
	if err != nil {
		t.Fatal(err)
	}
	return func(path string) (fahManifest, error) {
		if path != embeddedFAHManifestPath {
			return fahManifest{}, errors.New("v0.1.0 accepts only the embedded approved manifest")
		}
		return manifest, nil
	}
}

func setFAHApprovedManifestLoader(loader func(string) (fahManifest, error)) func() {
	previous := fahLoadApprovedManifest
	fahLoadApprovedManifest = loader
	return func() {
		fahLoadApprovedManifest = previous
	}
}

func setFAHFoldingOSCompatibilityCheck(check func(string) error) func() {
	previous := fahValidateFoldingOSCompatibility
	fahValidateFoldingOSCompatibility = check
	return func() {
		fahValidateFoldingOSCompatibility = previous
	}
}
