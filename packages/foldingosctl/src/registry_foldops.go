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
	foldopsManifestRegistryDirDefault   = "/data/registry/foldops"
	foldopsManifestRegistryIndexDefault = "/data/registry/foldops/index.json"
)

var (
	foldopsManifestRegistryDir   = foldopsManifestRegistryDirDefault
	foldopsManifestRegistryIndex = foldopsManifestRegistryIndexDefault
)

type foldopsManifestRegistryEntry struct {
	SchemaVersion   int    `json:"schema_version"`
	ManifestRelease string `json:"manifest_release"`
	ManifestTOML    string `json:"manifest_toml"`
	RolloutState    string `json:"rollout_state"`
	ImportTimestamp string `json:"import_timestamp"`
}

type foldopsManifestRegistryIndexDoc struct {
	SchemaVersion int      `json:"schema_version"`
	Releases      []string `json:"releases"`
}

func foldopsManifestRegistryEntryPath(release string) string {
	return filepath.Join(foldopsManifestRegistryDir, "releases", release+".json")
}

func loadFoldOpsManifestRegistryIndex() (foldopsManifestRegistryIndexDoc, error) {
	content, err := os.ReadFile(foldopsManifestRegistryIndex)
	if err != nil {
		if os.IsNotExist(err) {
			return foldopsManifestRegistryIndexDoc{SchemaVersion: 1, Releases: []string{}}, nil
		}
		return foldopsManifestRegistryIndexDoc{}, err
	}
	var index foldopsManifestRegistryIndexDoc
	if err := json.Unmarshal(content, &index); err != nil {
		return foldopsManifestRegistryIndexDoc{}, fmt.Errorf("invalid foldops manifest registry index: %w", err)
	}
	if index.SchemaVersion != 1 {
		return foldopsManifestRegistryIndexDoc{}, fmt.Errorf("unsupported foldops manifest registry index schema version %d", index.SchemaVersion)
	}
	return index, nil
}

func saveFoldOpsManifestRegistryIndex(index foldopsManifestRegistryIndexDoc) error {
	index.SchemaVersion = 1
	sort.Strings(index.Releases)
	content, err := json.MarshalIndent(index, "", "  ")
	if err != nil {
		return err
	}
	return atomicWrite(foldopsManifestRegistryIndex, append(content, '\n'), 0644)
}

func loadFoldOpsManifestRegistryEntry(release string) (foldopsManifestRegistryEntry, error) {
	release = strings.TrimSpace(release)
	if err := validateFoldOpsReleaseLabel(release); err != nil {
		return foldopsManifestRegistryEntry{}, err
	}
	content, err := os.ReadFile(foldopsManifestRegistryEntryPath(release))
	if err != nil {
		return foldopsManifestRegistryEntry{}, err
	}
	var entry foldopsManifestRegistryEntry
	if err := json.Unmarshal(content, &entry); err != nil {
		return foldopsManifestRegistryEntry{}, fmt.Errorf("invalid foldops manifest registry entry: %w", err)
	}
	return validateFoldOpsManifestRegistryEntry(entry)
}

func validateFoldOpsManifestRegistryEntry(entry foldopsManifestRegistryEntry) (foldopsManifestRegistryEntry, error) {
	if entry.SchemaVersion != 1 {
		return foldopsManifestRegistryEntry{}, fmt.Errorf("unsupported foldops manifest registry schema version %d", entry.SchemaVersion)
	}
	entry.ManifestRelease = strings.TrimSpace(entry.ManifestRelease)
	if err := validateFoldOpsReleaseLabel(entry.ManifestRelease); err != nil {
		return foldopsManifestRegistryEntry{}, err
	}
	entry.ManifestTOML = strings.TrimSpace(entry.ManifestTOML)
	if entry.ManifestTOML == "" {
		return foldopsManifestRegistryEntry{}, errors.New("foldops manifest registry entry is missing manifest_toml")
	}
	manifest, err := parseFoldOpsManifest(entry.ManifestTOML)
	if err != nil {
		return foldopsManifestRegistryEntry{}, fmt.Errorf("foldops manifest registry entry is invalid: %w", err)
	}
	if err := validateFoldOpsManifest(manifest); err != nil {
		return foldopsManifestRegistryEntry{}, fmt.Errorf("foldops manifest registry entry is invalid: %w", err)
	}
	if manifest.ManifestRelease != entry.ManifestRelease {
		return foldopsManifestRegistryEntry{}, fmt.Errorf(
			"foldops manifest release %q does not match registry entry %q",
			manifest.ManifestRelease,
			entry.ManifestRelease,
		)
	}
	entry.RolloutState = strings.TrimSpace(entry.RolloutState)
	if entry.RolloutState == "" {
		entry.RolloutState = "ready"
	}
	if _, ok := validRegistryRolloutStates[entry.RolloutState]; !ok {
		return foldopsManifestRegistryEntry{}, fmt.Errorf("foldops manifest rollout_state %q is invalid", entry.RolloutState)
	}
	return entry, nil
}

func saveFoldOpsManifestRegistryEntry(entry foldopsManifestRegistryEntry) error {
	validated, err := validateFoldOpsManifestRegistryEntry(entry)
	if err != nil {
		return err
	}
	content, err := json.MarshalIndent(validated, "", "  ")
	if err != nil {
		return err
	}
	if err := os.MkdirAll(filepath.Dir(foldopsManifestRegistryEntryPath(validated.ManifestRelease)), 0755); err != nil {
		return err
	}
	if err := atomicWrite(foldopsManifestRegistryEntryPath(validated.ManifestRelease), append(content, '\n'), 0644); err != nil {
		return err
	}
	index, err := loadFoldOpsManifestRegistryIndex()
	if err != nil {
		return err
	}
	if !containsString(index.Releases, validated.ManifestRelease) {
		index.Releases = append(index.Releases, validated.ManifestRelease)
	}
	return saveFoldOpsManifestRegistryIndex(index)
}

func registryImportFoldOpsManifest(args []string) error {
	if err := requireSupervisorRole(); err != nil {
		return err
	}
	manifestPath := ""
	for i := 0; i < len(args); i++ {
		switch args[i] {
		case "--manifest":
			if i+1 >= len(args) {
				return errors.New("missing value for --manifest")
			}
			manifestPath = args[i+1]
			i++
		default:
			return fmt.Errorf("unknown import-foldops-manifest option %q", args[i])
		}
	}
	if manifestPath == "" {
		return errors.New("import-foldops-manifest requires --manifest")
	}
	content, err := os.ReadFile(manifestPath)
	if err != nil {
		return fmt.Errorf("read manifest: %w", err)
	}
	manifest, err := parseFoldOpsManifest(string(content))
	if err != nil {
		return err
	}
	if err := validateFoldOpsManifest(manifest); err != nil {
		return err
	}
	entry := foldopsManifestRegistryEntry{
		SchemaVersion:   1,
		ManifestRelease: manifest.ManifestRelease,
		ManifestTOML:    strings.TrimSpace(string(content)),
		RolloutState:    "ready",
		ImportTimestamp: time.Now().UTC().Format(time.RFC3339),
	}
	if err := saveFoldOpsManifestRegistryEntry(entry); err != nil {
		return err
	}
	fmt.Printf("Imported FoldOps manifest release %q into the supervisor registry.\n", entry.ManifestRelease)
	return nil
}

func listFoldOpsManifestRegistry() error {
	if err := requireSupervisorRole(); err != nil {
		return err
	}
	index, err := loadFoldOpsManifestRegistryIndex()
	if err != nil {
		return err
	}
	if len(index.Releases) == 0 {
		fmt.Println("No FoldOps manifest releases in registry.")
		return nil
	}
	releases := append([]string(nil), index.Releases...)
	sort.Strings(releases)
	for _, release := range releases {
		entry, err := loadFoldOpsManifestRegistryEntry(release)
		if err != nil {
			return err
		}
		fmt.Printf("%s\trollout=%s\n", entry.ManifestRelease, entry.RolloutState)
	}
	return nil
}

func isBootstrapAssignmentLabel(value string) bool {
	switch strings.TrimSpace(strings.ToLower(value)) {
	case "", "bootstrap", "current":
		return true
	default:
		return false
	}
}
