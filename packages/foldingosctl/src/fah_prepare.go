package main

import (
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"regexp"
	"strconv"
	"strings"
)

const (
	fahServiceGID = 200
)

var (
	fahRuntimeDir        = "/run/foldingos/fah"
	fahRuntimeConfigPath = "/run/foldingos/fah/config.xml"
	fahPasskeyPattern    = regexp.MustCompile(`^[0-9a-fA-F]{32}$`)
)

func fahPrepare() error {
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

	config, passkey, err := loadFAHRuntimeConfiguration()
	if err != nil {
		return err
	}
	content := renderFAHConfigXML(config, passkey)
	if err := atomicWriteRootFAH(fahRuntimeConfigPath, []byte(content)); err != nil {
		return fmt.Errorf("write runtime configuration: %w", err)
	}
	fmt.Printf("Rendered Folding@home runtime configuration at %s.\n", fahRuntimeConfigPath)
	return nil
}

func loadFAHRuntimeConfiguration() (domainConfig, string, error) {
	merged, err := loadEffective("foldinghome", filepath.Join(configDir, "foldinghome.toml"), true)
	if err != nil {
		return nil, "", fmt.Errorf("invalid Folding@home configuration: %w", err)
	}
	passkey, err := readFAHPasskey(merged["identity.passkey_secret"].text)
	if err != nil {
		return nil, "", err
	}
	return merged, passkey, nil
}

func readFAHPasskey(secretName string) (string, error) {
	if secretName == "" {
		return "", nil
	}
	if err := validateSecretReference(secretName); err != nil {
		return "", err
	}
	path := filepath.Join(configDir, "secrets", secretName)
	content, err := os.ReadFile(path)
	if err != nil {
		return "", fmt.Errorf("read passkey secret: %w", err)
	}
	passkey := strings.TrimSuffix(string(content), "\n")
	if !fahPasskeyPattern.MatchString(passkey) {
		return "", errors.New("passkey secret must be exactly 32 hexadecimal characters")
	}
	return passkey, nil
}

func renderFAHConfigXML(config domainConfig, passkey string) string {
	var builder strings.Builder
	builder.WriteString("<config>\n")
	builder.WriteString(`  <user v="`)
	builder.WriteString(xmlEscapeAttribute(config["identity.username"].text))
	builder.WriteString("\"/>\n")
	builder.WriteString(`  <team v="`)
	builder.WriteString(strconv.FormatInt(config["identity.team"].ival, 10))
	builder.WriteString("\"/>\n")
	if passkey != "" {
		builder.WriteString(`  <passkey v="`)
		builder.WriteString(xmlEscapeAttribute(passkey))
		builder.WriteString("\"/>\n")
	}
	builder.WriteString(`  <cpus v="`)
	builder.WriteString(strconv.FormatInt(config["resources.cpus"].ival, 10))
	builder.WriteString("\"/>\n")
	builder.WriteString("</config>\n")
	return builder.String()
}

func xmlEscapeAttribute(value string) string {
	value = strings.ReplaceAll(value, "&", "&amp;")
	value = strings.ReplaceAll(value, "<", "&lt;")
	value = strings.ReplaceAll(value, ">", "&gt;")
	value = strings.ReplaceAll(value, "\"", "&quot;")
	return value
}

func atomicWriteRootFAH(path string, content []byte) error {
	if err := os.MkdirAll(filepath.Dir(path), 0750); err != nil {
		return err
	}
	if requireFAHRootOwnership() {
		if err := os.Chown(filepath.Dir(path), 0, fahServiceGID); err != nil {
			return fmt.Errorf("set runtime directory ownership: %w", err)
		}
	}

	temp, err := os.CreateTemp(filepath.Dir(path), "."+filepath.Base(path)+".tmp-")
	if err != nil {
		return err
	}
	tempName := temp.Name()
	defer os.Remove(tempName)

	mode := os.FileMode(0640)
	if err := temp.Chmod(mode); err != nil {
		temp.Close()
		return err
	}
	if requireFAHRootOwnership() {
		if err := os.Chown(tempName, 0, fahServiceGID); err != nil {
			temp.Close()
			return fmt.Errorf("set runtime configuration ownership: %w", err)
		}
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
	if err := os.Rename(tempName, path); err != nil {
		return err
	}

	dir, err := os.Open(filepath.Dir(path))
	if err != nil {
		return err
	}
	defer dir.Close()
	if err := dir.Sync(); err != nil {
		return fmt.Errorf("sync %s: %w", filepath.Dir(path), err)
	}
	return nil
}
