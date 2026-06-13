package main

import (
	"encoding/json"
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"sort"
	"strings"
	"time"
)

const (
	registryDirDefault              = "/data/registry"
	registryImagesDirDefault        = "/data/registry/images"
	registryEntriesDirDefault       = "/data/registry/entries"
	registryIndexPathDefault        = "/data/registry/index.json"
	upstreamReleasesURLPathDefault  = "/data/config/provision/upstream-releases.url"
	embeddedBuildRevisionPath       = "/usr/share/foldingos/build-revision"
	releaseImageSizeBytes     int64 = 4294967296
)

var (
	registryDir              = registryDirDefault
	registryImagesDir        = registryImagesDirDefault
	registryEntriesDir         = registryEntriesDirDefault
	registryIndexPath          = registryIndexPathDefault
	upstreamReleasesURLPath    = upstreamReleasesURLPathDefault
	installedFoldingOSVersionReader = readInstalledFoldingOSVersionFromRelease
	embeddedBuildRevisionReader     = readEmbeddedBuildRevisionFromFile
	validRegistryRolloutStates = map[string]struct{}{
		"staged":  {},
		"ready":   {},
		"retired": {},
	}
)

type registryEntry struct {
	SchemaVersion      int    `json:"schema_version"`
	FoldingOSVersion   string `json:"foldingos_version"`
	GitRevision        string `json:"git_revision"`
	ImageSHA256        string `json:"image_sha256"`
	ImageSizeBytes     int64  `json:"image_size_bytes"`
	RetrievalURL       string `json:"retrieval_url,omitempty"`
	VerificationMethod string `json:"verification_method"`
	ImportTimestamp    string `json:"import_timestamp"`
	RolloutState       string `json:"rollout_state"`
	LocalImagePath     string `json:"local_image_path"`
}

type registryIndex struct {
	SchemaVersion int      `json:"schema_version"`
	Versions      []string `json:"versions"`
}

type upstreamReleasesManifest struct {
	SchemaVersion int               `json:"schema_version"`
	Releases      []upstreamRelease `json:"releases"`
}

type upstreamRelease struct {
	FoldingOSVersion string `json:"foldingos_version"`
	GitRevision      string `json:"git_revision"`
	ImageURL         string `json:"image_url"`
	ImageSHA256      string `json:"image_sha256"`
	ImageSizeBytes   int64  `json:"image_size_bytes"`
	MetadataURL      string `json:"metadata_url,omitempty"`
	ChecksumURL      string `json:"checksum_url,omitempty"`
}

func requireSupervisorRole() error {
	return requireInstallationRole("supervisor")
}

func requireAgentRole() error {
	return requireInstallationRole("agent")
}

func requireInstallationRole(expected string) error {
	content, err := os.ReadFile(activeInstallationRole)
	if err != nil {
		return err
	}
	role, err := parseInstallationRole(content)
	if err != nil {
		return err
	}
	if role != expected {
		return fmt.Errorf("operation requires %s role, found %q", expected, role)
	}
	return nil
}

func registryEntryPath(version string) string {
	return filepath.Join(registryEntriesDir, version+".json")
}

func registryImagePath(version string) string {
	return filepath.Join(registryImagesDir, fmt.Sprintf("foldingos-x86_64-%s.img", version))
}

func loadRegistryIndex() (registryIndex, error) {
	content, err := os.ReadFile(registryIndexPath)
	if err != nil {
		if os.IsNotExist(err) {
			return registryIndex{SchemaVersion: 1, Versions: []string{}}, nil
		}
		return registryIndex{}, err
	}
	var index registryIndex
	if err := json.Unmarshal(content, &index); err != nil {
		return registryIndex{}, fmt.Errorf("invalid registry index: %w", err)
	}
	if index.SchemaVersion != 1 {
		return registryIndex{}, fmt.Errorf("unsupported registry index schema version %d", index.SchemaVersion)
	}
	return index, nil
}

func saveRegistryIndex(index registryIndex) error {
	index.SchemaVersion = 1
	sort.Strings(index.Versions)
	content, err := json.MarshalIndent(index, "", "  ")
	if err != nil {
		return err
	}
	return atomicWrite(registryIndexPath, append(content, '\n'), 0644)
}

func loadRegistryEntry(version string) (registryEntry, error) {
	content, err := os.ReadFile(registryEntryPath(version))
	if err != nil {
		return registryEntry{}, err
	}
	var entry registryEntry
	if err := json.Unmarshal(content, &entry); err != nil {
		return registryEntry{}, fmt.Errorf("invalid registry entry for %s: %w", version, err)
	}
	return validateRegistryEntry(entry)
}

func saveRegistryEntry(entry registryEntry) error {
	validated, err := validateRegistryEntry(entry)
	if err != nil {
		return err
	}
	content, err := json.MarshalIndent(validated, "", "  ")
	if err != nil {
		return err
	}
	if err := atomicWrite(registryEntryPath(validated.FoldingOSVersion), append(content, '\n'), 0644); err != nil {
		return err
	}
	index, err := loadRegistryIndex()
	if err != nil {
		return err
	}
	if !containsString(index.Versions, validated.FoldingOSVersion) {
		index.Versions = append(index.Versions, validated.FoldingOSVersion)
	}
	return saveRegistryIndex(index)
}

func validateRegistryEntry(entry registryEntry) (registryEntry, error) {
	if entry.SchemaVersion != 1 {
		return registryEntry{}, fmt.Errorf("unsupported registry entry schema version %d", entry.SchemaVersion)
	}
	entry.FoldingOSVersion = strings.TrimSpace(entry.FoldingOSVersion)
	if entry.FoldingOSVersion == "" {
		return registryEntry{}, errors.New("registry entry missing foldingos_version")
	}
	entry.GitRevision = strings.TrimSpace(entry.GitRevision)
	if entry.GitRevision == "" {
		return registryEntry{}, errors.New("registry entry missing git_revision")
	}
	entry.ImageSHA256 = strings.ToLower(strings.TrimSpace(entry.ImageSHA256))
	if len(entry.ImageSHA256) != 64 {
		return registryEntry{}, errors.New("registry entry image_sha256 must be 64 lowercase hex characters")
	}
	if entry.ImageSizeBytes <= 0 {
		return registryEntry{}, errors.New("registry entry image_size_bytes must be positive")
	}
	entry.RolloutState = strings.TrimSpace(entry.RolloutState)
	if _, ok := validRegistryRolloutStates[entry.RolloutState]; !ok {
		return registryEntry{}, fmt.Errorf("unsupported rollout state %q", entry.RolloutState)
	}
	entry.LocalImagePath = strings.TrimSpace(entry.LocalImagePath)
	if entry.LocalImagePath == "" {
		return registryEntry{}, errors.New("registry entry missing local_image_path")
	}
	if entry.VerificationMethod == "" {
		entry.VerificationMethod = "sha256"
	}
	if entry.VerificationMethod != "sha256" {
		return registryEntry{}, fmt.Errorf("unsupported verification method %q", entry.VerificationMethod)
	}
	if entry.ImportTimestamp == "" {
		entry.ImportTimestamp = time.Now().UTC().Format(time.RFC3339)
	}
	return entry, nil
}

func containsString(values []string, target string) bool {
	for _, value := range values {
		if value == target {
			return true
		}
	}
	return false
}

func readInstalledFoldingOSVersionFromRelease() (string, error) {
	content, err := os.ReadFile("/usr/lib/os-release")
	if err != nil {
		return "", err
	}
	for _, line := range strings.Split(string(content), "\n") {
		if strings.HasPrefix(line, "VERSION_ID=") {
			return strings.Trim(strings.TrimPrefix(line, "VERSION_ID="), `"`), nil
		}
	}
	return "", errors.New("installed FoldingOS version is unavailable")
}

func readEmbeddedBuildRevisionFromFile() string {
	content, err := os.ReadFile(embeddedBuildRevisionPath)
	if err != nil {
		return "unknown"
	}
	revision := strings.TrimSpace(string(content))
	if revision == "" {
		return "unknown"
	}
	return revision
}

func readUpstreamReleasesURL() (string, error) {
	content, err := os.ReadFile(upstreamReleasesURLPath)
	if err != nil {
		if os.IsNotExist(err) {
			return "", nil
		}
		return "", err
	}
	url := strings.TrimSpace(string(content))
	if url == "" {
		return "", nil
	}
	if !strings.HasPrefix(url, "https://") {
		return "", fmt.Errorf("upstream releases URL must use HTTPS: %q", url)
	}
	return url, nil
}

func listRegistry() error {
	if err := requireSupervisorRole(); err != nil {
		return err
	}
	index, err := loadRegistryIndex()
	if err != nil {
		return err
	}
	if len(index.Versions) == 0 {
		fmt.Println("Registry is empty.")
		return nil
	}
	sort.Strings(index.Versions)
	for _, version := range index.Versions {
		entry, err := loadRegistryEntry(version)
		if err != nil {
			return err
		}
		fmt.Printf(
			"%s\t%s\t%s\t%d bytes\n",
			entry.FoldingOSVersion,
			entry.RolloutState,
			entry.ImageSHA256,
			entry.ImageSizeBytes,
		)
	}
	return nil
}

func showRegistry(version string) error {
	if err := requireSupervisorRole(); err != nil {
		return err
	}
	entry, err := loadRegistryEntry(version)
	if err != nil {
		return err
	}
	content, err := json.MarshalIndent(entry, "", "  ")
	if err != nil {
		return err
	}
	fmt.Println(string(content))
	return nil
}
