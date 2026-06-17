package main

import (
	"fmt"
	"os"
	"path/filepath"
)

func ensureSupervisorFleetAutomationPermissions() error {
	gid, err := foldOpsGroupGID()
	if err != nil {
		return err
	}
	if err := ensureGroupDirectory(provisionEnrollmentsDir, 02775, gid); err != nil {
		return fmt.Errorf("configure enrollment permissions: %w", err)
	}
	if err := ensureGroupFile(provisionBootAllowlistPath, 0664, gid); err != nil {
		return fmt.Errorf("configure boot allowlist permissions: %w", err)
	}
	if err := ensureGroupFile(provisionBootInstallDiskAllowlistPath, 0664, gid); err != nil {
		return fmt.Errorf("configure boot install-disk allowlist permissions: %w", err)
	}
	if err := os.MkdirAll(filepath.Dir(foldOpsAssignedManifestPath), 0755); err != nil {
		return fmt.Errorf("ensure foldops config directory: %w", err)
	}
	if err := ensureGroupFile(foldOpsAssignedManifestPath, 0664, gid); err != nil {
		return fmt.Errorf("configure assigned foldops manifest permissions: %w", err)
	}
	if err := ensureGroupDirectory(filepath.Dir(toolsAssignedVersionPath), 02775, gid); err != nil {
		return fmt.Errorf("configure tools assignment permissions: %w", err)
	}
	return nil
}

func ensureGroupDirectory(path string, mode os.FileMode, gid int) error {
	if err := os.MkdirAll(path, mode); err != nil {
		return err
	}
	if err := os.Chmod(path, mode); err != nil {
		return err
	}
	return os.Chown(path, 0, gid)
}

func ensureGroupFile(path string, mode os.FileMode, gid int) error {
	file, err := os.OpenFile(path, os.O_CREATE|os.O_RDONLY, mode)
	if err != nil {
		return err
	}
	if closeErr := file.Close(); closeErr != nil && err == nil {
		err = closeErr
	}
	if err != nil {
		return err
	}
	if err := os.Chmod(path, mode); err != nil {
		return err
	}
	return os.Chown(path, 0, gid)
}
