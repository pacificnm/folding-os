package main

import (
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"syscall"
)

var fahExecProcess = syscall.Exec

func fahRun() error {
	manifest, err := fahLoadApprovedManifest(embeddedFAHManifestPath)
	if err != nil {
		return err
	}
	if err := fahValidateFoldingOSCompatibility(manifest.MinimumFoldingOSVersion); err != nil {
		return err
	}

	activeVersion, err := readFAHCurrentVersion()
	if err != nil {
		return fmt.Errorf("no active Folding@home installation: %w", err)
	}
	if !fahInstallationVerified(activeVersion, manifest) {
		return errors.New("active Folding@home installation is not verified")
	}
	if err := verifyFAHInstalledVersion(activeVersion, manifest); err != nil {
		return err
	}
	if _, err := os.Stat(fahRuntimeConfigPath); err != nil {
		return fmt.Errorf("runtime configuration is missing: %w", err)
	}

	executable, err := fahExecutableForVersion(activeVersion, manifest.ExecutablePath)
	if err != nil {
		return err
	}
	if err := verifyFAHExecutableUnderResolvedCurrent(executable); err != nil {
		return err
	}

	argv := fahExecArgv(executable, manifest.Arguments)
	return fahExecProcess(executable, argv, os.Environ())
}

func verifyFAHExecutableUnderResolvedCurrent(executable string) error {
	currentPath := filepath.Join(fahAppsRoot, "current")
	resolvedCurrent, err := filepath.EvalSymlinks(currentPath)
	if err != nil {
		return fmt.Errorf("resolve current symlink: %w", err)
	}

	executableClean := filepath.Clean(executable)
	resolvedCurrentClean := filepath.Clean(resolvedCurrent)
	if executableClean != resolvedCurrentClean &&
		!strings.HasPrefix(executableClean, resolvedCurrentClean+string(os.PathSeparator)) {
		return errors.New("resolved executable escapes verified active installation")
	}
	return nil
}

func fahExecArgv(executable string, arguments []string) []string {
	argv := make([]string, 0, len(arguments)+1)
	argv = append(argv, executable)
	argv = append(argv, arguments...)
	return argv
}
