package main

import (
	"bufio"
	"encoding/json"
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"sort"
	"strconv"
	"strings"
	"time"
)

const (
	toolsVersionRegistryDirDefault   = "/data/registry/tools"
	toolsVersionRegistryIndexDefault = "/data/registry/tools/index.json"
)

var (
	toolsVersionRegistryDir   = toolsVersionRegistryDirDefault
	toolsVersionRegistryIndex = toolsVersionRegistryIndexDefault
)

type toolsVersionRegistryEntry struct {
	SchemaVersion   int             `json:"schema_version"`
	ToolsVersion    string          `json:"tools_version"`
	Assignment      toolsAssignment `json:"assignment"`
	RolloutState    string          `json:"rollout_state"`
	ImportTimestamp string          `json:"import_timestamp"`
}

type toolsVersionRegistryIndexDoc struct {
	SchemaVersion int      `json:"schema_version"`
	Versions      []string `json:"versions"`
}

func toolsVersionRegistryEntryPath(version string) string {
	return filepath.Join(toolsVersionRegistryDir, "releases", version+".json")
}

func loadToolsVersionRegistryIndex() (toolsVersionRegistryIndexDoc, error) {
	content, err := os.ReadFile(toolsVersionRegistryIndex)
	if err != nil {
		if os.IsNotExist(err) {
			return toolsVersionRegistryIndexDoc{SchemaVersion: 1, Versions: []string{}}, nil
		}
		return toolsVersionRegistryIndexDoc{}, err
	}
	var index toolsVersionRegistryIndexDoc
	if err := json.Unmarshal(content, &index); err != nil {
		return toolsVersionRegistryIndexDoc{}, fmt.Errorf("invalid tools version registry index: %w", err)
	}
	if index.SchemaVersion != 1 {
		return toolsVersionRegistryIndexDoc{}, fmt.Errorf("unsupported tools version registry index schema version %d", index.SchemaVersion)
	}
	return index, nil
}

func saveToolsVersionRegistryIndex(index toolsVersionRegistryIndexDoc) error {
	index.SchemaVersion = 1
	sort.Strings(index.Versions)
	content, err := json.MarshalIndent(index, "", "  ")
	if err != nil {
		return err
	}
	return atomicWrite(toolsVersionRegistryIndex, append(content, '\n'), 0644)
}

func loadToolsVersionRegistryEntry(version string) (toolsVersionRegistryEntry, error) {
	version = strings.TrimSpace(version)
	if err := validateToolsVersionLabel(version); err != nil {
		return toolsVersionRegistryEntry{}, err
	}
	content, err := os.ReadFile(toolsVersionRegistryEntryPath(version))
	if err != nil {
		return toolsVersionRegistryEntry{}, err
	}
	var entry toolsVersionRegistryEntry
	if err := json.Unmarshal(content, &entry); err != nil {
		return toolsVersionRegistryEntry{}, fmt.Errorf("invalid tools version registry entry: %w", err)
	}
	return validateToolsVersionRegistryEntry(entry)
}

func validateToolsVersionRegistryEntry(entry toolsVersionRegistryEntry) (toolsVersionRegistryEntry, error) {
	if entry.SchemaVersion != 1 {
		return toolsVersionRegistryEntry{}, fmt.Errorf("unsupported tools version registry schema version %d", entry.SchemaVersion)
	}
	entry.ToolsVersion = strings.TrimSpace(entry.ToolsVersion)
	if err := validateToolsVersionLabel(entry.ToolsVersion); err != nil {
		return toolsVersionRegistryEntry{}, err
	}
	if err := validateToolsAssignment(entry.Assignment); err != nil {
		return toolsVersionRegistryEntry{}, fmt.Errorf("tools version registry entry is invalid: %w", err)
	}
	if entry.Assignment.ToolsVersion != entry.ToolsVersion {
		return toolsVersionRegistryEntry{}, fmt.Errorf(
			"tools assignment version %q does not match registry entry %q",
			entry.Assignment.ToolsVersion,
			entry.ToolsVersion,
		)
	}
	entry.RolloutState = strings.TrimSpace(entry.RolloutState)
	if entry.RolloutState == "" {
		entry.RolloutState = "ready"
	}
	if _, ok := validRegistryRolloutStates[entry.RolloutState]; !ok {
		return toolsVersionRegistryEntry{}, fmt.Errorf("tools version rollout_state %q is invalid", entry.RolloutState)
	}
	return entry, nil
}

func saveToolsVersionRegistryEntry(entry toolsVersionRegistryEntry) error {
	validated, err := validateToolsVersionRegistryEntry(entry)
	if err != nil {
		return err
	}
	content, err := json.MarshalIndent(validated, "", "  ")
	if err != nil {
		return err
	}
	if err := os.MkdirAll(filepath.Dir(toolsVersionRegistryEntryPath(validated.ToolsVersion)), 0755); err != nil {
		return err
	}
	if err := atomicWrite(toolsVersionRegistryEntryPath(validated.ToolsVersion), append(content, '\n'), 0644); err != nil {
		return err
	}
	index, err := loadToolsVersionRegistryIndex()
	if err != nil {
		return err
	}
	if !containsString(index.Versions, validated.ToolsVersion) {
		index.Versions = append(index.Versions, validated.ToolsVersion)
	}
	return saveToolsVersionRegistryIndex(index)
}

func registryImportToolsRelease(args []string) error {
	if err := requireSupervisorRole(); err != nil {
		return err
	}
	releaseDir := ""
	version := ""
	for i := 0; i < len(args); i++ {
		switch args[i] {
		case "--dir":
			if i+1 >= len(args) {
				return errors.New("missing value for --dir")
			}
			releaseDir = args[i+1]
			i++
		case "--version":
			if i+1 >= len(args) {
				return errors.New("missing value for --version")
			}
			version = args[i+1]
			i++
		default:
			return fmt.Errorf("unknown import-tools-release option %q", args[i])
		}
	}
	if releaseDir == "" {
		return errors.New("import-tools-release requires --dir")
	}
	if version == "" {
		version = filepath.Base(filepath.Clean(releaseDir))
	}
	if err := validateToolsVersionLabel(version); err != nil {
		return err
	}

	binaryPath := filepath.Join(releaseDir, toolsArtifactBasename)
	info, err := os.Stat(binaryPath)
	if err != nil {
		return fmt.Errorf("tools release binary is missing: %w", err)
	}
	if !info.Mode().IsRegular() {
		return errors.New("tools release binary is not a regular file")
	}
	digest, err := hashFileAtPath(binaryPath, info.Size())
	if err != nil {
		return err
	}
	if err := verifyToolsArtifactDigestMatchesChecksums(releaseDir, digest); err != nil {
		return err
	}

	assignment := toolsAssignment{
		SchemaVersion: 1,
		ToolsVersion:  version,
		ArtifactURL:   fmt.Sprintf("https://%s/foldingos-tools/%s/%s", toolsApprovedOrigin, version, toolsArtifactBasename),
		ArtifactSize:  info.Size(),
		SHA256:        digest,
	}
	entry := toolsVersionRegistryEntry{
		SchemaVersion:   1,
		ToolsVersion:    version,
		Assignment:      assignment,
		RolloutState:    "ready",
		ImportTimestamp: time.Now().UTC().Format(time.RFC3339),
	}
	if err := saveToolsVersionRegistryEntry(entry); err != nil {
		return err
	}
	fmt.Printf("Imported foldingosctl tools release %q into the supervisor registry.\n", version)
	return nil
}

func verifyToolsArtifactDigestMatchesChecksums(releaseDir, digest string) error {
	checksumsPath := filepath.Join(releaseDir, "SHA256SUMS")
	content, err := os.ReadFile(checksumsPath)
	if err != nil {
		return fmt.Errorf("read SHA256SUMS: %w", err)
	}
	scanner := bufio.NewScanner(strings.NewReader(string(content)))
	for scanner.Scan() {
		line := strings.TrimSpace(scanner.Text())
		if line == "" {
			continue
		}
		fields := strings.Fields(line)
		if len(fields) != 2 {
			continue
		}
		if fields[1] != toolsArtifactBasename {
			continue
		}
		if fields[0] != digest {
			return errors.New("tools release binary SHA-256 does not match SHA256SUMS")
		}
		return nil
	}
	if err := scanner.Err(); err != nil {
		return err
	}
	return fmt.Errorf("SHA256SUMS does not contain %q", toolsArtifactBasename)
}

func listToolsVersionRegistry() error {
	if err := requireSupervisorRole(); err != nil {
		return err
	}
	index, err := loadToolsVersionRegistryIndex()
	if err != nil {
		return err
	}
	if len(index.Versions) == 0 {
		fmt.Println("No foldingosctl tools releases in registry.")
		return nil
	}
	versions := append([]string(nil), index.Versions...)
	sort.Strings(versions)
	for _, version := range versions {
		entry, err := loadToolsVersionRegistryEntry(version)
		if err != nil {
			return err
		}
		fmt.Printf("%s\trollout=%s\tsize=%s\n", entry.ToolsVersion, entry.RolloutState, strconv.FormatInt(entry.Assignment.ArtifactSize, 10))
	}
	return nil
}
