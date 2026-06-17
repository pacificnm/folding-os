package main

import (
	"debug/elf"
	"errors"
	"fmt"
	"os"
	"path/filepath"
)

var verifyToolsExecutable = verifyToolsExecutableELF

func verifyToolsExecutableELF(path string) error {
	file, err := elf.Open(path)
	if err != nil {
		return fmt.Errorf("read tools executable ELF header: %w", err)
	}
	defer file.Close()

	if file.Machine != fahRequiredELFMachine {
		return fmt.Errorf("tools executable architecture %v is not x86_64", file.Machine)
	}
	if file.Type != elf.ET_EXEC && file.Type != elf.ET_DYN {
		return fmt.Errorf("tools executable type %v is not supported", file.Type)
	}
	return nil
}

func atomicReplaceToolsBinary(stagedPath, destination string) error {
	if err := verifyToolsExecutable(stagedPath); err != nil {
		return err
	}
	info, err := os.Stat(stagedPath)
	if err != nil {
		return fmt.Errorf("inspect staged tools binary: %w", err)
	}
	if !info.Mode().IsRegular() {
		return errors.New("staged tools artifact is not a regular file")
	}

	if err := os.MkdirAll(filepath.Dir(destination), 0755); err != nil {
		return fmt.Errorf("create tools binary directory: %w", err)
	}
	temp, err := os.CreateTemp(filepath.Dir(destination), "."+filepath.Base(destination)+".tmp-")
	if err != nil {
		return fmt.Errorf("create temporary tools binary: %w", err)
	}
	tempName := temp.Name()
	defer os.Remove(tempName)

	if err := temp.Chmod(0755); err != nil {
		temp.Close()
		return err
	}
	content, err := os.ReadFile(stagedPath)
	if err != nil {
		temp.Close()
		return fmt.Errorf("read staged tools binary: %w", err)
	}
	if _, err := temp.Write(content); err != nil {
		temp.Close()
		return err
	}
	if err := temp.Sync(); err != nil {
		temp.Close()
		return err
	}
	if err := temp.Close(); err != nil {
		return err
	}
	if err := os.Rename(tempName, destination); err != nil {
		return fmt.Errorf("replace tools binary: %w", err)
	}

	dir, err := os.Open(filepath.Dir(destination))
	if err != nil {
		return err
	}
	defer dir.Close()
	if err := dir.Sync(); err != nil {
		return fmt.Errorf("sync %s: %w", filepath.Dir(destination), err)
	}
	return nil
}
