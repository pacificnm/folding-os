package main

import (
	"crypto/sha256"
	"encoding/hex"
	"errors"
	"fmt"
	"io"
	"os"
	"path/filepath"
	"strings"
	"time"
)

var (
	registryCopyBootDiskImage      = copyBootDiskImage
	registryResolveBootDisk        = resolveBootDisk
	registryExpectedImageSizeBytes = releaseImageSizeBytes
	registryReportCopyProgress     = reportRegistryCopyProgress
	registryCopyProgressInterval   = int64(256 * 1024 * 1024)
	registryCopyChunkSize          = 4 * 1024 * 1024
)

func registryImportBootstrap() error {
	if err := requireSupervisorRole(); err != nil {
		return err
	}

	version, err := installedFoldingOSVersionReader()
	if err != nil {
		return err
	}

	if existing, err := loadRegistryEntry(version); err == nil {
		if err := verifyRegistryImageFile(existing.LocalImagePath, existing.ImageSHA256, existing.ImageSizeBytes); err != nil {
			return fmt.Errorf("existing registry image for %s is invalid: %w", version, err)
		}
		fmt.Printf("Registry already contains verified image for FoldingOS %s.\n", version)
		return nil
	} else if !os.IsNotExist(err) {
		return err
	}

	disk, err := registryResolveBootDisk()
	if err != nil {
		return err
	}

	imagePath := registryImagePath(version)
	if err := os.MkdirAll(filepath.Dir(imagePath), 0755); err != nil {
		return err
	}

	reportRegistryImportStarted(version, disk)
	digest, size, err := registryCopyBootDiskImage(disk, imagePath, registryExpectedImageSizeBytes)
	if err != nil {
		reportRegistryImportFailure(version, err)
		return err
	}
	if size != registryExpectedImageSizeBytes {
		return fmt.Errorf("imported image size %d does not match release image size %d", size, registryExpectedImageSizeBytes)
	}

	entry := registryEntry{
		SchemaVersion:      1,
		FoldingOSVersion:   version,
		GitRevision:        embeddedBuildRevisionReader(),
		ImageSHA256:        digest,
		ImageSizeBytes:     size,
		VerificationMethod: "sha256",
		ImportTimestamp:    time.Now().UTC().Format(time.RFC3339),
		RolloutState:       "ready",
		LocalImagePath:     imagePath,
	}
	if err := saveRegistryEntry(entry); err != nil {
		return err
	}
	reportRegistryImportComplete(version, digest)
	fmt.Printf("Imported FoldingOS %s into the supervisor registry.\n", version)
	fmt.Printf("Image SHA-256: %s\n", digest)
	return nil
}

func reportRegistryImportStarted(version, disk string) {
	message := fmt.Sprintf("Registry: copying FoldingOS %s from %s", version, disk)
	emitRegistryStatus(message)
}

func reportRegistryImportComplete(version, digest string) {
	message := fmt.Sprintf("Registry: imported FoldingOS %s (%s)", version, digest)
	emitRegistryStatus(message)
}

func reportRegistryImportFailure(version string, err error) {
	message := fmt.Sprintf("Registry: failed to import FoldingOS %s (%v)", version, err)
	emitRegistryStatus(message)
}

func reportRegistryCopyProgress(written, total int64) {
	emitRegistryStatus(formatRegistryCopyProgress(written, total))
}

func formatRegistryCopyProgress(written, total int64) string {
	percent := int((written * 100) / total)
	return fmt.Sprintf(
		"Registry: copying release image %d MiB / %d MiB (%d%%)",
		written/(1024*1024),
		total/(1024*1024),
		percent,
	)
}

func emitRegistryStatus(message string) {
	fmt.Println(message)
	_ = writeConsole(message + "\n")
}

func copyBootDiskImage(disk, destination string, size int64) (string, int64, error) {
	source, err := os.Open(disk)
	if err != nil {
		return "", 0, err
	}
	defer source.Close()

	temp, err := os.CreateTemp(filepath.Dir(destination), ".registry-image.tmp-")
	if err != nil {
		return "", 0, err
	}
	tempPath := temp.Name()
	cleanup := true
	defer func() {
		if cleanup {
			os.Remove(tempPath)
		}
	}()

	hasher := sha256.New()
	buffer := make([]byte, registryCopyChunkSize)
	var written int64
	var lastReport int64
	for written < size {
		remaining := size - written
		chunkSize := int64(len(buffer))
		if remaining < chunkSize {
			chunkSize = remaining
		}
		n, err := io.ReadFull(source, buffer[:chunkSize])
		if err != nil {
			temp.Close()
			return "", written, fmt.Errorf("copy boot disk image: %w", err)
		}
		if _, err := temp.Write(buffer[:n]); err != nil {
			temp.Close()
			return "", written, err
		}
		if _, err := hasher.Write(buffer[:n]); err != nil {
			temp.Close()
			return "", written, err
		}
		written += int64(n)
		if written == size || written-lastReport >= registryCopyProgressInterval {
			registryReportCopyProgress(written, size)
			lastReport = written
		}
	}
	if err := temp.Sync(); err != nil {
		temp.Close()
		return "", 0, err
	}
	if err := temp.Close(); err != nil {
		return "", 0, err
	}
	if err := os.Rename(tempPath, destination); err != nil {
		return "", 0, err
	}
	cleanup = false
	return hex.EncodeToString(hasher.Sum(nil)), written, nil
}

func verifyRegistryImageFile(path, expectedDigest string, expectedSize int64) error {
	info, err := os.Stat(path)
	if err != nil {
		return err
	}
	if info.Size() != expectedSize {
		return fmt.Errorf("image size %d does not match expected %d", info.Size(), expectedSize)
	}
	file, err := os.Open(path)
	if err != nil {
		return err
	}
	defer file.Close()

	hasher := sha256.New()
	if _, err := io.Copy(hasher, file); err != nil {
		return err
	}
	actual := hex.EncodeToString(hasher.Sum(nil))
	if actual != expectedDigest {
		return errors.New("image SHA-256 does not match registry metadata")
	}
	return nil
}

func resolveBootDisk() (string, error) {
	rootSource, err := output("findmnt", "-n", "-o", "SOURCE", "/")
	if err != nil {
		return "", err
	}
	rootSource = strings.TrimSpace(rootSource)
	if !strings.HasPrefix(rootSource, "/dev/") {
		return "", fmt.Errorf("root source is not a block device: %q", rootSource)
	}

	parentName, err := output("lsblk", "-n", "-o", "PKNAME", rootSource)
	if err != nil {
		return "", err
	}
	parentName = strings.TrimSpace(parentName)
	if parentName == "" || strings.Contains(parentName, "/") {
		return "", fmt.Errorf("could not resolve boot disk from %q", rootSource)
	}
	return filepath.Join("/dev", parentName), nil
}
