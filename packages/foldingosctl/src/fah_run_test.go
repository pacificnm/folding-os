package main

import (
	"errors"
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestFahExecArgvUsesManifestArgumentsExactly(t *testing.T) {
	manifest, err := parseFAHManifest(validFAHManifest)
	if err != nil {
		t.Fatal(err)
	}
	executable := "/data/apps/fah/8.5.6/usr/bin/fah-client"
	argv := fahExecArgv(executable, manifest.Arguments)
	expected := append([]string{executable}, manifest.Arguments...)
	if len(argv) != len(expected) {
		t.Fatalf("argv length = %d, want %d", len(argv), len(expected))
	}
	for index := range expected {
		if argv[index] != expected[index] {
			t.Fatalf("argv[%d] = %q, want %q", index, argv[index], expected[index])
		}
	}
}

func TestFahRunRequiresRuntimeConfiguration(t *testing.T) {
	appsRoot, runtimeDir := setupVerifiedFAHInstallForRun(t)
	restoreApps := setFAHAppsRoot(appsRoot)
	defer restoreApps()
	restoreRuntime := setFAHRuntimePaths(runtimeDir)
	defer restoreRuntime()
	restoreManifest := setFAHApprovedManifestLoader(testFAHManifestLoader(t))
	defer restoreManifest()
	restoreCompatibility := setFAHFoldingOSCompatibilityCheck(func(string) error { return nil })
	defer restoreCompatibility()

	if err := fahRun(); err == nil {
		t.Fatal("missing runtime configuration was accepted")
	} else if !strings.Contains(err.Error(), "runtime configuration is missing") {
		t.Fatalf("unexpected error: %v", err)
	}
}

func TestFahRunRejectsUnverifiedInstallation(t *testing.T) {
	appsRoot := t.TempDir()
	versionDir := filepath.Join(appsRoot, "8.5.6")
	if err := os.MkdirAll(filepath.Join(versionDir, "usr", "bin"), 0755); err != nil {
		t.Fatal(err)
	}
	executable := filepath.Join(versionDir, "usr", "bin", "fah-client")
	if err := copyFile("/tmp/fah-manifest-work/deb-inspect/data/usr/bin/fah-client", executable); err != nil {
		t.Skip("approved FAH executable is unavailable for run test")
	}
	if err := os.Chmod(executable, 0755); err != nil {
		t.Fatal(err)
	}
	if err := os.Symlink("8.5.6", filepath.Join(appsRoot, "current")); err != nil {
		t.Fatal(err)
	}

	runtimeDir := t.TempDir()
	configPath := filepath.Join(runtimeDir, "config.xml")
	if err := os.WriteFile(configPath, []byte("<config></config>\n"), 0640); err != nil {
		t.Fatal(err)
	}

	restoreApps := setFAHAppsRoot(appsRoot)
	defer restoreApps()
	restoreRuntime := setFAHRuntimePaths(runtimeDir)
	defer restoreRuntime()
	restoreManifest := setFAHApprovedManifestLoader(testFAHManifestLoader(t))
	defer restoreManifest()
	restoreCompatibility := setFAHFoldingOSCompatibilityCheck(func(string) error { return nil })
	defer restoreCompatibility()

	if err := fahRun(); err == nil {
		t.Fatal("unverified installation was accepted")
	} else if !strings.Contains(err.Error(), "not verified") {
		t.Fatalf("unexpected error: %v", err)
	}
}

func TestVerifyFAHExecutableUnderResolvedCurrentRejectsEscape(t *testing.T) {
	appsRoot := t.TempDir()
	versionDir := filepath.Join(appsRoot, "8.5.6")
	if err := os.MkdirAll(versionDir, 0755); err != nil {
		t.Fatal(err)
	}
	if err := os.Symlink("8.5.6", filepath.Join(appsRoot, "current")); err != nil {
		t.Fatal(err)
	}

	restoreApps := setFAHAppsRoot(appsRoot)
	defer restoreApps()

	escapedExecutable := filepath.Join(appsRoot, "outside", "fah-client")
	if err := verifyFAHExecutableUnderResolvedCurrent(escapedExecutable); err == nil {
		t.Fatal("executable outside resolved current was accepted")
	}
}

func TestFahRunExecsResolvedExecutableWithManifestArguments(t *testing.T) {
	appsRoot, runtimeDir := setupVerifiedFAHInstallForRun(t)
	configPath := filepath.Join(runtimeDir, "config.xml")
	if err := os.WriteFile(configPath, []byte("<config></config>\n"), 0640); err != nil {
		t.Fatal(err)
	}

	restoreApps := setFAHAppsRoot(appsRoot)
	defer restoreApps()
	restoreRuntime := setFAHRuntimePaths(runtimeDir)
	defer restoreRuntime()
	restoreManifest := setFAHApprovedManifestLoader(testFAHManifestLoader(t))
	defer restoreManifest()
	restoreCompatibility := setFAHFoldingOSCompatibilityCheck(func(string) error { return nil })
	defer restoreCompatibility()

	var execPath string
	var execArgv []string
	restoreExec := setFAHExecProcess(func(path string, argv []string, envv []string) error {
		execPath = path
		execArgv = append([]string(nil), argv...)
		return errors.New("exec intercepted for test")
	})
	defer restoreExec()

	err := fahRun()
	if err == nil || err.Error() != "exec intercepted for test" {
		t.Fatalf("unexpected error: %v", err)
	}

	expectedExecutable := filepath.Join(appsRoot, "8.5.6", "usr", "bin", "fah-client")
	if execPath != expectedExecutable {
		t.Fatalf("exec path = %q, want %q", execPath, expectedExecutable)
	}
	manifest, err := parseFAHManifest(validFAHManifest)
	if err != nil {
		t.Fatal(err)
	}
	expectedArgv := fahExecArgv(expectedExecutable, manifest.Arguments)
	if len(execArgv) != len(expectedArgv) {
		t.Fatalf("exec argv length = %d, want %d", len(execArgv), len(expectedArgv))
	}
	for index := range expectedArgv {
		if execArgv[index] != expectedArgv[index] {
			t.Fatalf("exec argv[%d] = %q, want %q", index, execArgv[index], expectedArgv[index])
		}
	}
}

func setupVerifiedFAHInstallForRun(t *testing.T) (string, string) {
	t.Helper()
	appsRoot := t.TempDir()
	runtimeDir := t.TempDir()
	versionDir := filepath.Join(appsRoot, "8.5.6")
	if err := os.MkdirAll(filepath.Join(versionDir, "usr", "bin"), 0755); err != nil {
		t.Fatal(err)
	}
	executable := filepath.Join(versionDir, "usr", "bin", "fah-client")
	if err := copyFile("/tmp/fah-manifest-work/deb-inspect/data/usr/bin/fah-client", executable); err != nil {
		t.Skip("approved FAH executable is unavailable for run test")
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
	return appsRoot, runtimeDir
}

func setFAHExecProcess(exec func(string, []string, []string) error) func() {
	previous := fahExecProcess
	fahExecProcess = exec
	return func() {
		fahExecProcess = previous
	}
}
