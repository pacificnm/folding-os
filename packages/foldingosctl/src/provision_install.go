package main

import (
	"bytes"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"net/http"
	"os"
	"path/filepath"
	"strings"
)

var (
	provisionInstallHTTPClient = &http.Client{}
	writeProvisionImageToDisk  = writeProvisionImageToDiskDirect
	stageProvisionBootFiles    = stageProvisionBootFilesOnDisk
	relocateProvisionGPT       = relocateProvisionGPTOnDisk
)

func provisionInstall(args []string) error {
	var disk string
	var version string
	var supervisorURL string
	var enrollmentToken string

	for i := 0; i < len(args); i++ {
		switch args[i] {
		case "--disk":
			if i+1 >= len(args) {
				return errors.New("missing value for --disk")
			}
			disk = args[i+1]
			i++
		case "--version":
			if i+1 >= len(args) {
				return errors.New("missing value for --version")
			}
			version = args[i+1]
			i++
		case "--supervisor-url":
			if i+1 >= len(args) {
				return errors.New("missing value for --supervisor-url")
			}
			supervisorURL = args[i+1]
			i++
		case "--enrollment-token":
			if i+1 >= len(args) {
				return errors.New("missing value for --enrollment-token")
			}
			enrollmentToken = args[i+1]
			i++
		default:
			return fmt.Errorf("unknown install option %q", args[i])
		}
	}
	if disk == "" {
		return errors.New("target disk is required (--disk)")
	}

	if supervisorURL == "" {
		var err error
		supervisorURL, err = readSupervisorBaseURL()
		if err != nil {
			return err
		}
	}
	if supervisorURL == "" {
		return errors.New("supervisor URL is not configured")
	}
	if enrollmentToken == "" {
		var err error
		enrollmentToken, err = readEnrollmentToken()
		if err != nil {
			return fmt.Errorf("enrollment token is not configured: %w", err)
		}
	}

	target, err := validateProvisionTargetDisk(disk)
	if err != nil {
		return err
	}
	macAddresses, err := collectMACAddresses()
	if err != nil {
		return err
	}

	authorizeURL, err := joinSupervisorURL(supervisorURL, "/v1/provision/authorize")
	if err != nil {
		return err
	}
	authorizeBody, err := json.Marshal(provisionAuthorizeRequest{
		SchemaVersion:   1,
		EnrollmentToken: enrollmentToken,
		MACAddresses:    macAddresses,
		TargetDisk:      target.Path,
		TargetSerial:    target.Serial,
		ImageVersion:    version,
	})
	if err != nil {
		return err
	}
	authorizeRequest, err := http.NewRequest(http.MethodPost, authorizeURL, bytes.NewReader(authorizeBody))
	if err != nil {
		return err
	}
	authorizeRequest.Header.Set("Content-Type", "application/json")
	authorizeResponse, err := provisionInstallHTTPClient.Do(authorizeRequest)
	if err != nil {
		return err
	}
	defer authorizeResponse.Body.Close()
	authorizePayload, err := io.ReadAll(io.LimitReader(authorizeResponse.Body, 1<<20))
	if err != nil {
		return err
	}
	if authorizeResponse.StatusCode != http.StatusOK {
		return fmt.Errorf(
			"provisioning authorization failed with status %s: %s",
			authorizeResponse.Status,
			strings.TrimSpace(string(authorizePayload)),
		)
	}
	var authorization provisionAuthorizeResponse
	if err := json.Unmarshal(authorizePayload, &authorization); err != nil {
		return err
	}
	if authorization.InstallationRole != agentInstallationRole {
		return fmt.Errorf("supervisor returned unexpected installation role %q", authorization.InstallationRole)
	}
	if authorization.TargetDisk != target.Path {
		return fmt.Errorf("supervisor authorized disk %q, expected %q", authorization.TargetDisk, target.Path)
	}

	streamURL, err := joinSupervisorURL(supervisorURL, authorization.ImageStreamPath)
	if err != nil {
		return err
	}
	streamRequest, err := http.NewRequest(http.MethodGet, streamURL, nil)
	if err != nil {
		return err
	}
	streamRequest.Header.Set("X-FoldingOS-Enrollment-Token", enrollmentToken)
	streamRequest.Header.Set(installSessionHeader, authorization.InstallSessionID)
	streamResponse, err := provisionInstallHTTPClient.Do(streamRequest)
	if err != nil {
		return err
	}
	defer streamResponse.Body.Close()
	if streamResponse.StatusCode != http.StatusOK {
		body, _ := io.ReadAll(io.LimitReader(streamResponse.Body, 1<<20))
		return fmt.Errorf(
			"image stream failed with status %s: %s",
			streamResponse.Status,
			strings.TrimSpace(string(body)),
		)
	}

	digest, written, err := writeProvisionImageToDisk(target.Path, streamResponse.Body, authorization.ImageSizeBytes)
	if err != nil {
		return err
	}
	if written != authorization.ImageSizeBytes {
		return fmt.Errorf("installed image size %d does not match expected %d", written, authorization.ImageSizeBytes)
	}
	if !strings.EqualFold(digest, authorization.ImageSHA256) {
		return errors.New("installed image failed SHA-256 verification")
	}
	fmt.Printf("Verified FoldingOS %s on %s (%s)\n", authorization.ImageVersion, target.Path, digest)

	if err := relocateProvisionGPT(target.Path, target.SizeBytes, authorization.ImageSizeBytes); err != nil {
		return err
	}
	if err := stageProvisionBootFiles(target.Path, authorization.InstallationRole, []byte(authorization.AuthorizedKeys)); err != nil {
		return err
	}

	fmt.Printf("Provisioned %s with role %s.\n", target.Path, authorization.InstallationRole)
	fmt.Println("Reboot the target into internal storage to complete installation.")
	return nil
}

func writeProvisionImageToDiskDirect(disk string, source io.Reader, size int64) (string, int64, error) {
	file, err := os.OpenFile(disk, os.O_WRONLY, 0)
	if err != nil {
		return "", 0, err
	}
	defer file.Close()

	hasher := sha256.New()
	written, err := io.CopyN(file, io.TeeReader(source, hasher), size)
	if err != nil {
		return "", written, fmt.Errorf("write release image to %s: %w", disk, err)
	}
	if err := file.Sync(); err != nil {
		return "", written, err
	}
	return hex.EncodeToString(hasher.Sum(nil)), written, nil
}

func relocateProvisionGPTOnDisk(disk string, deviceSize, imageSize int64) error {
	if deviceSize <= imageSize {
		return nil
	}
	fmt.Printf("Relocating backup GPT header on %s\n", disk)
	if err := run("sgdisk", "-e", disk); err != nil {
		return err
	}
	if _, err := execCommand("partprobe", disk).CombinedOutput(); err == nil {
		return run("sync")
	}
	return run("sync")
}

func stageProvisionBootFilesOnDisk(disk, role string, authorizedKeys []byte) error {
	if strings.TrimSpace(role) == "" {
		return errors.New("installation role is required")
	}
	if _, err := validateAuthorizedKeys(authorizedKeys); err != nil {
		return fmt.Errorf("authorized keys are invalid: %w", err)
	}
	efiPartition := efiPartitionPath(disk)
	if mounted(efiPartition) {
		return fmt.Errorf("EFI partition %s is mounted", efiPartition)
	}

	mountPoint, err := os.MkdirTemp("", "foldingos-provision-esp-")
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

	provisionDir := filepath.Join(mountPoint, "foldingos", "provision")
	if err := os.MkdirAll(provisionDir, 0755); err != nil {
		return err
	}
	if err := os.WriteFile(filepath.Join(provisionDir, "installation-role"), []byte(role), 0644); err != nil {
		return err
	}
	if err := os.WriteFile(filepath.Join(provisionDir, "authorized_keys"), authorizedKeys, 0644); err != nil {
		return err
	}
	if err := run("sync"); err != nil {
		return err
	}
	fmt.Printf("Staged installation role and SSH keys on %s\n", efiPartition)
	return nil
}
