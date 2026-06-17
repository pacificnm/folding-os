package main

import (
	"encoding/json"
	"errors"
	"fmt"
	"net/url"
	"os"
	"path/filepath"
	"strings"
)

const (
	toolsBootstrapManifestPathDefault = "/usr/share/foldingos/manifests/tools.json"
	toolsAssignedVersionPathDefault   = "/data/config/tools/assigned-version.json"
	toolsActiveStatePathDefault       = "/data/state/tools/active.json"
	toolsBinaryPathDefault            = "/usr/bin/foldingosctl"
	toolsApprovedOrigin               = "packages.folding-os.com"
	toolsArtifactBasename             = "foldingosctl-x86_64"
	toolsAssignmentSchemaVersion      = 1
)

var (
	toolsBootstrapManifestPath = toolsBootstrapManifestPathDefault
	toolsAssignedVersionPath   = toolsAssignedVersionPathDefault
	toolsActiveStatePath       = toolsActiveStatePathDefault
	toolsBinaryPath            = toolsBinaryPathDefault
)

type toolsAssignment struct {
	SchemaVersion int    `json:"schema_version"`
	ToolsVersion  string `json:"tools_version"`
	ArtifactURL   string `json:"artifact_url"`
	ArtifactSize  int64  `json:"artifact_size"`
	SHA256        string `json:"sha256"`
}

type toolsActiveState struct {
	SchemaVersion   int    `json:"schema_version"`
	ToolsVersion    string `json:"tools_version"`
	SHA256          string `json:"sha256"`
	InstalledAtUnix int64  `json:"installed_at_unix"`
}

func resolveEffectiveToolsAssignment() (toolsAssignment, bool, error) {
	if assignment, err := loadToolsAssignmentFromFile(toolsAssignedVersionPath); err == nil {
		return assignment, true, nil
	} else if !os.IsNotExist(err) {
		return toolsAssignment{}, false, err
	}
	if assignment, err := loadToolsAssignmentFromFile(toolsBootstrapManifestPath); err == nil {
		return assignment, true, nil
	} else if !os.IsNotExist(err) {
		return toolsAssignment{}, false, fmt.Errorf("bootstrap tools manifest: %w", err)
	}
	return toolsAssignment{}, false, nil
}

func loadToolsAssignmentFromFile(path string) (toolsAssignment, error) {
	if path != toolsAssignedVersionPath && path != toolsBootstrapManifestPath {
		return toolsAssignment{}, errors.New("tools assignment path is not allowed")
	}
	content, err := os.ReadFile(path)
	if err != nil {
		return toolsAssignment{}, err
	}
	assignment, err := parseToolsAssignment(content)
	if err != nil {
		return toolsAssignment{}, err
	}
	if err := validateToolsAssignment(assignment); err != nil {
		return toolsAssignment{}, err
	}
	return assignment, nil
}

func parseToolsAssignment(content []byte) (toolsAssignment, error) {
	var assignment toolsAssignment
	if err := json.Unmarshal(content, &assignment); err != nil {
		return toolsAssignment{}, fmt.Errorf("parse tools assignment JSON: %w", err)
	}
	return assignment, nil
}

func validateToolsAssignment(assignment toolsAssignment) error {
	if assignment.SchemaVersion != toolsAssignmentSchemaVersion {
		return errors.New("tools assignment schema_version must be 1")
	}
	if err := validateToolsVersionLabel(assignment.ToolsVersion); err != nil {
		return fmt.Errorf("tools_version: %w", err)
	}
	if !fahSHA256Pattern.MatchString(assignment.SHA256) {
		return errors.New("sha256 must be a 64-character lowercase hex digest")
	}
	if assignment.ArtifactSize <= 0 {
		return errors.New("artifact_size must be positive")
	}

	artifactURL, err := url.Parse(assignment.ArtifactURL)
	if err != nil {
		return fmt.Errorf("artifact_url is invalid: %w", err)
	}
	if artifactURL.Scheme != "https" {
		return errors.New("artifact_url must use HTTPS")
	}
	if artifactURL.Host != toolsApprovedOrigin {
		return fmt.Errorf("artifact_url must use HTTPS from the approved official origin: %s", toolsApprovedOrigin)
	}
	if !strings.Contains(artifactURL.Path, "/foldingos-tools/"+assignment.ToolsVersion+"/") {
		return errors.New("artifact_url must reference the assigned tools version directory")
	}
	if !strings.HasSuffix(artifactURL.Path, "/"+toolsArtifactBasename) &&
		!strings.HasSuffix(artifactURL.Path, toolsArtifactBasename) {
		return errors.New("artifact_url must reference the foldingosctl-x86_64 artifact")
	}
	return nil
}

func validateToolsVersionLabel(version string) error {
	version = strings.TrimSpace(version)
	if version == "" {
		return errors.New("tools version must be non-empty")
	}
	if version != filepath.Clean(version) || strings.Contains(version, "..") || strings.ContainsAny(version, `/\`) {
		return errors.New("tools version must not contain path separators or traversal")
	}
	return nil
}

func loadToolsActiveState() (toolsActiveState, error) {
	content, err := os.ReadFile(toolsActiveStatePath)
	if err != nil {
		if os.IsNotExist(err) {
			return toolsActiveState{}, nil
		}
		return toolsActiveState{}, fmt.Errorf("read active tools state: %w", err)
	}
	var state toolsActiveState
	if err := json.Unmarshal(content, &state); err != nil {
		return toolsActiveState{}, fmt.Errorf("parse active tools state: %w", err)
	}
	if state.SchemaVersion != toolsAssignmentSchemaVersion {
		return toolsActiveState{}, errors.New("active tools state schema_version is unsupported")
	}
	if err := validateToolsVersionLabel(state.ToolsVersion); err != nil {
		return toolsActiveState{}, err
	}
	if !fahSHA256Pattern.MatchString(state.SHA256) {
		return toolsActiveState{}, errors.New("active tools state sha256 is invalid")
	}
	return state, nil
}

func saveToolsActiveState(state toolsActiveState) error {
	state.SchemaVersion = toolsAssignmentSchemaVersion
	content, err := json.Marshal(state)
	if err != nil {
		return err
	}
	content = append(content, '\n')
	return atomicWrite(toolsActiveStatePath, content, 0644)
}

func toolsInstallationVerified(assignment toolsAssignment) bool {
	state, err := loadToolsActiveState()
	if err != nil {
		return false
	}
	if state.ToolsVersion != assignment.ToolsVersion || state.SHA256 != assignment.SHA256 {
		return false
	}
	digest, err := hashFileAtPath(toolsBinaryPath, assignment.ArtifactSize)
	if err != nil {
		return false
	}
	return digest == assignment.SHA256
}
