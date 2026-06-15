package main

import (
	"fmt"
	"os"
	"path/filepath"
)

var agentDataPartitionResetPaths = []string{
	"config",
	"registry",
	"provision",
	"state",
}

func resetAgentDataPartitionState(root string) error {
	for _, relative := range agentDataPartitionResetPaths {
		if err := os.RemoveAll(filepath.Join(root, relative)); err != nil {
			return fmt.Errorf("reset inherited data at %s: %w", relative, err)
		}
	}
	return nil
}

func clearGrubNextEntry(grubEnvPath string) error {
	content, err := os.ReadFile(grubEnvPath)
	if err != nil {
		if os.IsNotExist(err) {
			return nil
		}
		return err
	}
	vars, err := parseGrubEnvBlock(content)
	if err != nil {
		return nil
	}
	if _, ok := vars["next_entry"]; !ok {
		return nil
	}
	delete(vars, "next_entry")
	updated, err := formatGrubEnvBlock(vars)
	if err != nil {
		return err
	}
	return atomicWrite(grubEnvPath, updated, 0644)
}

func clearGrubNextEntryOnDisk(disk string) error {
	if _, err := os.Stat(updateGrubEnvPath); err == nil {
		return clearGrubNextEntry(updateGrubEnvPath)
	}

	efiPartition := efiPartitionPath(disk)
	if mounted(efiPartition) {
		return fmt.Errorf("EFI partition %s is mounted", efiPartition)
	}

	mountPoint, err := os.MkdirTemp(provisionScratchDir(), "foldingos-grubenv-")
	if err != nil {
		return err
	}
	defer os.RemoveAll(mountPoint)

	if err := run("mount", efiPartition, mountPoint); err != nil {
		return fmt.Errorf("mount EFI partition %s: %w", efiPartition, err)
	}
	defer func() {
		_ = run("umount", mountPoint)
	}()

	if err := clearGrubNextEntry(filepath.Join(mountPoint, "EFI", "BOOT", "grubenv")); err != nil {
		return err
	}
	return run("sync")
}
