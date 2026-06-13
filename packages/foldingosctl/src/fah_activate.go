package main

import (
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"strings"
)

const fahServiceName = "folding-at-home.service"

var (
	fahRestartServiceAfterActivation = restartFAHServiceAfterActivation
	fahLoadApprovedManifest          = loadFAHManifest
	fahValidateFoldingOSCompatibility = validateFoldingOSCompatibility
)

func fahActivate(version string) error {
	if err := validateFAHVersionLabel(version); err != nil {
		return err
	}
	manifest, err := fahLoadApprovedManifest(embeddedFAHManifestPath)
	if err != nil {
		return err
	}
	if err := fahValidateFoldingOSCompatibility(manifest.MinimumFoldingOSVersion); err != nil {
		return err
	}
	if version != manifest.ClientVersion {
		return fmt.Errorf(
			"version %s does not match approved manifest client %s",
			version,
			manifest.ClientVersion,
		)
	}
	if err := verifyFAHInstalledVersion(version, manifest); err != nil {
		return err
	}
	if !fahInstallationVerified(version, manifest) {
		return fmt.Errorf("version %s is not a verified installation", version)
	}

	currentVersion, currentErr := readFAHCurrentVersion()
	if currentErr == nil && currentVersion == version {
		fmt.Printf("Folding@home %s is already active.\n", version)
		return fahRestartServiceAfterActivation()
	}

	if err := activateFAHCurrentSymlink(version); err != nil {
		return err
	}
	fmt.Printf("Activated Folding@home %s at %s.\n", version, filepath.Join(fahAppsRoot, "current"))
	return fahRestartServiceAfterActivation()
}

func activateFAHCurrentSymlink(version string) error {
	if strings.Contains(version, string(os.PathSeparator)) {
		return errors.New("activation version must not contain path separators")
	}
	versionDir := filepath.Join(fahAppsRoot, version)
	info, err := os.Stat(versionDir)
	if err != nil {
		return fmt.Errorf("verified version directory is missing: %w", err)
	}
	if !info.IsDir() {
		return errors.New("activation target is not a directory")
	}

	currentPath := filepath.Join(fahAppsRoot, "current")
	tempPath := filepath.Join(fahAppsRoot, ".current.tmp-activate")
	if err := os.Remove(tempPath); err != nil && !os.IsNotExist(err) {
		return fmt.Errorf("remove stale activation symlink: %w", err)
	}
	if err := os.Symlink(version, tempPath); err != nil {
		return fmt.Errorf("create activation symlink: %w", err)
	}
	if err := os.Rename(tempPath, currentPath); err != nil {
		_ = os.Remove(tempPath)
		return fmt.Errorf("activate current symlink: %w", err)
	}

	dir, err := os.Open(fahAppsRoot)
	if err != nil {
		return fmt.Errorf("open apps root for sync: %w", err)
	}
	defer dir.Close()
	if err := dir.Sync(); err != nil {
		return fmt.Errorf("sync apps root: %w", err)
	}
	return nil
}

func restartFAHServiceAfterActivation() error {
	state, err := output("systemctl", "show", "-p", "LoadState", "--value", fahServiceName)
	if err != nil {
		return fmt.Errorf("inspect %s: %w", fahServiceName, err)
	}
	if strings.TrimSpace(state) != "loaded" {
		return nil
	}
	if err := run("systemctl", "try-restart", fahServiceName); err != nil {
		return fmt.Errorf("restart %s: %w", fahServiceName, err)
	}
	return nil
}
