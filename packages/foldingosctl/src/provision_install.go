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
	installProgressInterval    = int64(128 * 1024 * 1024)
)

func installLogf(format string, args ...any) {
	message := fmt.Sprintf(format, args...)
	if !strings.HasSuffix(message, "\n") {
		message += "\n"
	}
	_ = writeConsole(message)
	fmt.Print(message)
}

func formatInstallBytes(size int64) string {
	const gib = 1024 * 1024 * 1024
	const mib = 1024 * 1024
	switch {
	case size >= gib:
		return fmt.Sprintf("%.1f GiB", float64(size)/float64(gib))
	case size >= mib:
		return fmt.Sprintf("%.1f MiB", float64(size)/float64(mib))
	default:
		return fmt.Sprintf("%d B", size)
	}
}

type installProgressReader struct {
	inner      io.Reader
	total      int64
	written    int64
	lastReport int64
}

func (reader *installProgressReader) Read(buffer []byte) (int, error) {
	count, err := reader.inner.Read(buffer)
	if count > 0 {
		reader.written += int64(count)
		reader.report(false)
	}
	if err == io.EOF {
		reader.report(true)
	}
	return count, err
}

func (reader *installProgressReader) report(force bool) {
	if reader.total <= 0 {
		return
	}
	if !force && reader.written-reader.lastReport < installProgressInterval {
		return
	}
	reader.lastReport = reader.written
	percent := reader.written * 100 / reader.total
	installLogf(
		"FoldingOS install: wrote %s / %s (%d%%)",
		formatInstallBytes(reader.written),
		formatInstallBytes(reader.total),
		percent,
	)
}

func provisionInstall(args []string) error {
	var disk string
	var version string
	var supervisorURL string
	var enrollmentToken string
	autoDisk := false

	for i := 0; i < len(args); i++ {
		switch args[i] {
		case "--disk":
			if i+1 >= len(args) {
				return errors.New("missing value for --disk")
			}
			disk = args[i+1]
			i++
		case "--auto-disk":
			autoDisk = true
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
	if disk == "" && !autoDisk {
		return errors.New("target disk is required (--disk or --auto-disk)")
	}
	if disk != "" && autoDisk {
		return errors.New("use either --disk or --auto-disk, not both")
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

	if autoDisk {
		selected, err := selectProvisionInstallDisk()
		if err != nil {
			return err
		}
		disk = selected
		installLogf("Selected install disk %s.", disk)
	}

	target, err := validateProvisionTargetDisk(disk)
	if err != nil {
		return err
	}
	installLogf(
		"Validated target %s (serial %s, transport %s, size %s).",
		target.Path,
		target.Serial,
		target.Transport,
		formatInstallBytes(target.SizeBytes),
	)
	macAddresses, err := collectMACAddresses()
	if err != nil {
		return err
	}

	installLogf("Requesting install authorization from %s.", supervisorURL)
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
	installLogf(
		"Authorized FoldingOS %s (%s) for session %s.",
		authorization.ImageVersion,
		formatInstallBytes(authorization.ImageSizeBytes),
		authorization.InstallSessionID,
	)

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
	installLogf("Streaming release image from %s.", streamURL)
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

	digest, written, err := writeProvisionImageToDisk(
		target.Path,
		&installProgressReader{inner: streamResponse.Body, total: authorization.ImageSizeBytes},
		authorization.ImageSizeBytes,
	)
	if err != nil {
		return err
	}
	if written != authorization.ImageSizeBytes {
		return fmt.Errorf("installed image size %d does not match expected %d", written, authorization.ImageSizeBytes)
	}
	if !strings.EqualFold(digest, authorization.ImageSHA256) {
		return errors.New("installed image failed SHA-256 verification")
	}
	installLogf("Verified FoldingOS %s on %s (%s).", authorization.ImageVersion, target.Path, digest)

	if err := relocateProvisionGPT(target.Path, target.SizeBytes, authorization.ImageSizeBytes); err != nil {
		return err
	}
	if err := stageProvisionBootFiles(target.Path, authorization.InstallationRole, []byte(authorization.AuthorizedKeys)); err != nil {
		return err
	}

	installLogf("Provisioned %s with role %s.", target.Path, authorization.InstallationRole)
	installLogf("Reboot the target into internal storage to complete installation.")
	return nil
}

func writeProvisionImageToDiskDirect(disk string, source io.Reader, size int64) (string, int64, error) {
	installLogf("Writing %s to %s.", formatInstallBytes(size), disk)
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
	installLogf("Relocating backup GPT header on %s.", disk)
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

	mountPoint, err := os.MkdirTemp(provisionScratchDir(), "foldingos-provision-esp-")
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
	installLogf("Staged installation role and SSH keys on %s.", efiPartition)
	return nil
}

func provisionScratchDir() string {
	for _, directory := range []string{"/run", "/tmp"} {
		info, err := os.Stat(directory)
		if err == nil && info.IsDir() {
			return directory
		}
	}
	return ""
}
