package main

import (
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"strings"
)

func foldOpsActivate(release string) error {
	if err := validateFoldOpsReleaseLabel(release); err != nil {
		return err
	}
	manifest, err := resolveEffectiveFoldOpsManifest()
	if err != nil {
		return err
	}
	if err := validateFoldingOSCompatibility(manifest.MinimumFoldingOSVersion); err != nil {
		return err
	}
	if release != manifest.ManifestRelease {
		return fmt.Errorf(
			"release %s does not match approved manifest release %s",
			release,
			manifest.ManifestRelease,
		)
	}
	role, err := readActiveInstallationRole()
	if err != nil {
		return err
	}
	packages, err := foldOpsPackagesForRole(manifest, role)
	if err != nil {
		return err
	}
	if !foldOpsInstallationVerified(release, role, packages) {
		return fmt.Errorf("release %s is not a verified installation", release)
	}

	currentRelease, currentErr := readFoldOpsCurrentRelease()
	if currentErr == nil && currentRelease == release {
		fmt.Printf("FoldOps release %s is already active.\n", release)
		return nil
	}

	if err := activateFoldOpsCurrentSymlink(release); err != nil {
		return err
	}
	fmt.Printf("Activated FoldOps release %s at %s.\n", release, filepath.Join(foldOpsAppsRoot, "current"))
	return nil
}

func activateFoldOpsCurrentSymlink(release string) error {
	if strings.Contains(release, string(os.PathSeparator)) {
		return errors.New("activation release must not contain path separators")
	}
	releaseDir := filepath.Join(foldOpsAppsRoot, release)
	info, err := os.Stat(releaseDir)
	if err != nil {
		return fmt.Errorf("verified release directory is missing: %w", err)
	}
	if !info.IsDir() {
		return errors.New("activation target is not a directory")
	}

	currentPath := filepath.Join(foldOpsAppsRoot, "current")
	tempPath := filepath.Join(foldOpsAppsRoot, ".current.tmp-activate")
	if err := os.Remove(tempPath); err != nil && !os.IsNotExist(err) {
		return fmt.Errorf("remove stale activation symlink: %w", err)
	}
	if err := os.Symlink(release, tempPath); err != nil {
		return fmt.Errorf("create activation symlink: %w", err)
	}
	if err := os.Rename(tempPath, currentPath); err != nil {
		_ = os.Remove(tempPath)
		return fmt.Errorf("activate current symlink: %w", err)
	}

	dir, err := os.Open(foldOpsAppsRoot)
	if err != nil {
		return fmt.Errorf("open apps root for sync: %w", err)
	}
	defer dir.Close()
	if err := dir.Sync(); err != nil {
		return fmt.Errorf("sync apps root: %w", err)
	}
	return nil
}

func readFoldOpsCurrentRelease() (string, error) {
	currentPath := filepath.Join(foldOpsAppsRoot, "current")
	target, err := os.Readlink(currentPath)
	if err != nil {
		return "", err
	}
	if filepath.IsAbs(target) || strings.HasPrefix(target, "/") {
		return "", errors.New("current must be a relative symlink")
	}
	cleaned := filepath.Clean(target)
	if cleaned != target || strings.Contains(target, "..") {
		return "", errors.New("current must not contain path traversal")
	}
	releaseDir := filepath.Join(foldOpsAppsRoot, cleaned)
	info, err := os.Stat(releaseDir)
	if err != nil || !info.IsDir() {
		return "", errors.New("current does not reference an installed release")
	}
	return cleaned, nil
}

func validateFoldOpsReleaseLabel(release string) error {
	release = strings.TrimSpace(release)
	if release == "" {
		return errors.New("release must be non-empty")
	}
	if release != filepath.Clean(release) || strings.Contains(release, "..") || strings.ContainsAny(release, `/\`) {
		return errors.New("release must not contain path separators or traversal")
	}
	return nil
}

func readActiveInstallationRole() (string, error) {
	content, err := os.ReadFile(activeInstallationRole)
	if err != nil {
		return "", fmt.Errorf("read installation role: %w", err)
	}
	return parseInstallationRole(content)
}
