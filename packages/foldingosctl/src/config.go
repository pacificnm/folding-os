package main

import (
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"regexp"
	"sort"
	"strconv"
	"strings"
	"syscall"
	"time"
	"unicode/utf8"
)

const (
	defaultsDir  = "/etc/foldingos/defaults"
	configDir    = "/data/config"
	effectiveDir = "/run/foldingos/effective"
)

var (
	domains         = []string{"system", "network", "foldinghome"}
	hostnamePattern = regexp.MustCompile(`^[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?$`)
	secretPattern   = regexp.MustCompile(`^[A-Za-z0-9._-]+$`)
)

type configValue struct {
	kind string
	text string
	ival int64
	bval bool
}

type domainConfig map[string]configValue

func validateConfig(domain string) error {
	if domain == "--all" {
		for _, name := range domains {
			if _, err := effectiveConfig(name, true); err != nil {
				return err
			}
		}
		return nil
	}
	_, err := effectiveConfig(domain, true)
	return err
}

func printEffectiveConfig(domain string) error {
	content, err := effectiveConfig(domain, true)
	if err != nil {
		return err
	}
	fmt.Print(content)
	return nil
}

func effectiveConfig(domain string, write bool) (string, error) {
	if !validDomain(domain) {
		return "", fmt.Errorf("unknown configuration domain %q", domain)
	}

	merged, err := loadEffective(domain, filepath.Join(configDir, domain+".toml"), true)
	if err != nil {
		fmt.Fprintf(os.Stderr, "foldingosctl: invalid active %s configuration, trying last-known-good: %v\n", domain, err)
		merged, err = loadEffective(domain, filepath.Join(configDir, "last-good", domain+".toml"), false)
	}
	if err != nil {
		fmt.Fprintf(os.Stderr, "foldingosctl: invalid last-known-good %s configuration, using image defaults: %v\n", domain, err)
		merged, err = loadEffective(domain, "", false)
	}
	if err != nil {
		return "", err
	}
	content := renderDomain(domain, merged)
	if write {
		if err := atomicWrite(filepath.Join(effectiveDir, domain+".toml"), []byte(content), 0644); err != nil {
			return "", err
		}
	}
	return content, nil
}

func loadEffective(domain, activePath string, includeOverride bool) (domainConfig, error) {
	merged := make(domainConfig)
	paths := []string{filepath.Join(defaultsDir, domain+".toml")}
	if activePath != "" {
		paths = append(paths, activePath)
	}
	if includeOverride {
		paths = append(paths, filepath.Join(configDir, "overrides", domain+".toml"))
	}
	for index, path := range paths {
		content, err := os.ReadFile(path)
		if err != nil {
			if os.IsNotExist(err) && index > 0 {
				continue
			}
			return nil, fmt.Errorf("read %s: %w", path, err)
		}
		values, err := parseDomain(domain, string(content), index == 0)
		if err != nil {
			return nil, fmt.Errorf("%s: %w", path, err)
		}
		for key, value := range values {
			merged[key] = value
		}
	}
	if err := validateDomain(domain, merged); err != nil {
		return nil, err
	}
	if domain == "foldinghome" {
		if err := validateSecretReference(merged["identity.passkey_secret"].text); err != nil {
			return nil, err
		}
	}
	return merged, nil
}

func activateConfig(domain, candidate string) error {
	if !validDomain(domain) {
		return fmt.Errorf("unknown configuration domain %q", domain)
	}
	resolved, err := filepath.EvalSymlinks(candidate)
	if err != nil {
		return err
	}
	if resolved != "/data" && !strings.HasPrefix(resolved, "/data"+string(os.PathSeparator)) {
		return errors.New("configuration candidate must be a regular file on /data")
	}
	info, err := os.Stat(resolved)
	if err != nil {
		return err
	}
	if !info.Mode().IsRegular() {
		return errors.New("configuration candidate must be a regular file")
	}

	lockPath := filepath.Join("/run/lock", "foldingos-config-"+domain+".lock")
	lock, err := os.OpenFile(lockPath, os.O_CREATE|os.O_RDWR, 0600)
	if err != nil {
		return err
	}
	defer lock.Close()
	if err := syscall.Flock(int(lock.Fd()), syscall.LOCK_EX); err != nil {
		return err
	}
	defer syscall.Flock(int(lock.Fd()), syscall.LOCK_UN)

	candidateContent, err := os.ReadFile(resolved)
	if err != nil {
		return err
	}
	candidateValues, err := parseDomain(domain, string(candidateContent), false)
	if err != nil {
		return err
	}
	if err := validateCandidate(domain, candidateValues); err != nil {
		return err
	}

	active := filepath.Join(configDir, domain+".toml")
	previous, previousErr := os.ReadFile(active)
	if previousErr == nil {
		if err := atomicWrite(filepath.Join(configDir, "last-good", domain+".toml"), previous, 0644); err != nil {
			return err
		}
	} else if !os.IsNotExist(previousErr) {
		return previousErr
	}
	if err := atomicWrite(active, candidateContent, 0644); err != nil {
		return err
	}
	if _, err := effectiveConfig(domain, true); err != nil {
		return rollbackConfig(domain, active, previous, previousErr, err)
	}
	if err := applyDomain(domain); err != nil {
		return rollbackConfig(domain, active, previous, previousErr, err)
	}
	return nil
}

func applyDomain(domain string) error {
	switch domain {
	case "system":
		return ensureIdentity()
	case "network":
		if err := run("systemctl", "try-restart", "systemd-networkd.service"); err != nil {
			return err
		}
		deadline := time.Now().Add(30 * time.Second)
		for {
			if execCommand("systemctl", "is-active", "--quiet", "systemd-networkd.service").Run() == nil {
				return nil
			}
			if time.Now().After(deadline) {
				return errors.New("systemd-networkd did not become active within 30 seconds")
			}
			time.Sleep(time.Second)
		}
	case "foldinghome":
		return nil
	default:
		return fmt.Errorf("unknown configuration domain %q", domain)
	}
}

func validateCandidate(domain string, candidate domainConfig) error {
	merged := make(domainConfig)
	defaultContent, err := os.ReadFile(filepath.Join(defaultsDir, domain+".toml"))
	if err != nil {
		return err
	}
	defaults, err := parseDomain(domain, string(defaultContent), true)
	if err != nil {
		return err
	}
	for key, value := range defaults {
		merged[key] = value
	}
	for key, value := range candidate {
		merged[key] = value
	}
	overridePath := filepath.Join(configDir, "overrides", domain+".toml")
	if overrideContent, err := os.ReadFile(overridePath); err == nil {
		overrides, err := parseDomain(domain, string(overrideContent), false)
		if err != nil {
			return err
		}
		for key, value := range overrides {
			merged[key] = value
		}
	} else if !os.IsNotExist(err) {
		return err
	}
	if err := validateDomain(domain, merged); err != nil {
		return err
	}
	if domain == "foldinghome" {
		return validateSecretReference(merged["identity.passkey_secret"].text)
	}
	return nil
}

func rollbackConfig(domain, active string, previous []byte, previousErr, cause error) error {
	if previousErr == nil {
		if err := atomicWrite(active, previous, 0644); err != nil {
			return fmt.Errorf("%v; rollback failed: %w", cause, err)
		}
	} else if os.IsNotExist(previousErr) {
		if err := os.Remove(active); err != nil && !os.IsNotExist(err) {
			return fmt.Errorf("%v; rollback failed: %w", cause, err)
		}
	}
	_, _ = effectiveConfig(domain, true)
	_ = applyDomain(domain)
	return fmt.Errorf("configuration activation failed and was rolled back: %w", cause)
}

func parseDomain(domain, content string, requireComplete bool) (domainConfig, error) {
	allowed := allowedKeys(domain)
	values := make(domainConfig)
	section := ""

	for number, raw := range strings.Split(content, "\n") {
		line := strings.TrimSpace(raw)
		if line == "" || strings.HasPrefix(line, "#") {
			continue
		}
		if strings.HasPrefix(line, "[") && strings.HasSuffix(line, "]") {
			section = strings.TrimSpace(line[1 : len(line)-1])
			if section == "" {
				return nil, fmt.Errorf("line %d: empty table name", number+1)
			}
			if !validSection(domain, section) {
				return nil, fmt.Errorf("line %d: unknown table %q", number+1, section)
			}
			continue
		}
		parts := strings.SplitN(line, "=", 2)
		if len(parts) != 2 {
			return nil, fmt.Errorf("line %d: expected key = value", number+1)
		}
		key := strings.TrimSpace(parts[0])
		if section != "" {
			key = section + "." + key
		}
		kind, ok := allowed[key]
		if !ok {
			return nil, fmt.Errorf("line %d: unknown key %q", number+1, key)
		}
		if _, exists := values[key]; exists {
			return nil, fmt.Errorf("line %d: duplicate key %q", number+1, key)
		}
		value, err := parseValue(kind, strings.TrimSpace(parts[1]))
		if err != nil {
			return nil, fmt.Errorf("line %d: %w", number+1, err)
		}
		values[key] = value
	}
	if requireComplete {
		for key := range allowed {
			if _, ok := values[key]; !ok {
				return nil, fmt.Errorf("missing required key %q", key)
			}
		}
	}
	if schema, ok := values["schema_version"]; !ok || schema.ival != 1 {
		return nil, errors.New("schema_version must be present and equal 1")
	}
	return values, nil
}

func parseValue(kind, text string) (configValue, error) {
	value := configValue{kind: kind}
	switch kind {
	case "string":
		parsed, err := strconv.Unquote(text)
		if err != nil {
			return value, errors.New("expected quoted string")
		}
		value.text = parsed
	case "int":
		parsed, err := strconv.ParseInt(text, 10, 64)
		if err != nil {
			return value, errors.New("expected integer")
		}
		value.ival = parsed
	case "bool":
		parsed, err := strconv.ParseBool(text)
		if err != nil {
			return value, errors.New("expected boolean")
		}
		value.bval = parsed
	}
	return value, nil
}

func validateDomain(domain string, values domainConfig) error {
	if value, ok := values["schema_version"]; !ok || value.ival != 1 {
		return errors.New("schema_version must be 1")
	}
	switch domain {
	case "system":
		hostname := values["identity.hostname"].text
		if hostname != "" && !hostnamePattern.MatchString(hostname) {
			return errors.New("identity.hostname is not a valid RFC 1123 host label")
		}
	case "network":
		if !values["ethernet.dhcp"].bval || !values["ethernet.required_for_online"].bval {
			return errors.New("v0.1.0 requires DHCP Ethernet for network-online")
		}
	case "foldinghome":
		username := values["identity.username"].text
		if username == "" || !utf8.ValidString(username) || len([]byte(username)) > 128 {
			return errors.New("identity.username must contain 1 through 128 UTF-8 bytes")
		}
		if team := values["identity.team"].ival; team < 0 || team > 2147483647 {
			return errors.New("identity.team is outside the supported range")
		}
		secret := values["identity.passkey_secret"].text
		if secret != "" && (!secretPattern.MatchString(secret) || secret == "." || secret == "..") {
			return errors.New("identity.passkey_secret must be a safe basename")
		}
		if cpus := values["resources.cpus"].ival; cpus < 0 {
			return errors.New("resources.cpus must be zero or positive")
		}
		if values["resources.gpus"].bval {
			return errors.New("resources.gpus must be false in v0.1.0")
		}
	}
	return nil
}

func validateSecretReference(name string) error {
	if name == "" {
		return nil
	}
	path := filepath.Join(configDir, "secrets", name)
	info, err := os.Stat(path)
	if err != nil {
		return fmt.Errorf("passkey secret %q is unavailable: %w", name, err)
	}
	if !info.Mode().IsRegular() || info.Mode().Perm() != 0640 {
		return fmt.Errorf("passkey secret %q must be a regular file with mode 0640", name)
	}
	stat, ok := info.Sys().(*syscall.Stat_t)
	if !ok || stat.Uid != 0 || stat.Gid != 200 {
		return fmt.Errorf("passkey secret %q must be owned by root:fah", name)
	}
	return nil
}

func renderDomain(domain string, values domainConfig) string {
	var builder strings.Builder
	if schema, ok := values["schema_version"]; ok {
		builder.WriteString("schema_version = ")
		builder.WriteString(strconv.FormatInt(schema.ival, 10))
		builder.WriteByte('\n')
	}

	keys := make([]string, 0, len(values)-1)
	for key := range values {
		if key == "schema_version" {
			continue
		}
		keys = append(keys, key)
	}
	sort.Strings(keys)
	currentSection := ""
	for _, key := range keys {
		section, name := splitKey(key)
		if section != currentSection {
			builder.WriteByte('\n')
			builder.WriteString("[")
			builder.WriteString(section)
			builder.WriteString("]\n")
			currentSection = section
		}
		value := values[key]
		builder.WriteString(name)
		builder.WriteString(" = ")
		switch value.kind {
		case "string":
			builder.WriteString(strconv.Quote(value.text))
		case "int":
			builder.WriteString(strconv.FormatInt(value.ival, 10))
		case "bool":
			builder.WriteString(strconv.FormatBool(value.bval))
		}
		builder.WriteByte('\n')
	}
	return builder.String()
}

func splitKey(key string) (string, string) {
	index := strings.LastIndexByte(key, '.')
	if index < 0 {
		return "", key
	}
	return key[:index], key[index+1:]
}

func allowedKeys(domain string) map[string]string {
	common := map[string]string{"schema_version": "int"}
	switch domain {
	case "system":
		common["identity.hostname"] = "string"
	case "network":
		common["ethernet.dhcp"] = "bool"
		common["ethernet.required_for_online"] = "bool"
	case "foldinghome":
		common["identity.username"] = "string"
		common["identity.team"] = "int"
		common["identity.passkey_secret"] = "string"
		common["resources.cpus"] = "int"
		common["resources.gpus"] = "bool"
	}
	return common
}

func validDomain(domain string) bool {
	for _, known := range domains {
		if domain == known {
			return true
		}
	}
	return false
}

func validSection(domain, section string) bool {
	switch domain {
	case "system":
		return section == "identity"
	case "network":
		return section == "ethernet"
	case "foldinghome":
		return section == "identity" || section == "resources"
	default:
		return false
	}
}
