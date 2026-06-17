package main

import (
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"net/http"
	"os"
	"path/filepath"
	"strings"
)

func writeAssignedFoldOpsManifest(content string) error {
	content = strings.TrimSpace(content)
	if content == "" {
		return clearAssignedFoldOpsManifest()
	}
	manifest, err := parseFoldOpsManifest(content)
	if err != nil {
		return err
	}
	if err := validateFoldOpsManifest(manifest); err != nil {
		return err
	}
	if err := os.MkdirAll(filepath.Dir(foldOpsAssignedManifestPath), 0755); err != nil {
		return err
	}
	return atomicWrite(foldOpsAssignedManifestPath, []byte(content+"\n"), 0644)
}

func clearAssignedFoldOpsManifest() error {
	if err := os.Remove(foldOpsAssignedManifestPath); err != nil && !os.IsNotExist(err) {
		return fmt.Errorf("clear assigned foldops manifest: %w", err)
	}
	return nil
}

func writeAssignedToolsVersion(assignment toolsAssignment) error {
	if err := validateToolsAssignment(assignment); err != nil {
		return err
	}
	content, err := json.MarshalIndent(assignment, "", "  ")
	if err != nil {
		return err
	}
	if err := os.MkdirAll(filepath.Dir(toolsAssignedVersionPath), 0755); err != nil {
		return err
	}
	return atomicWrite(toolsAssignedVersionPath, append(content, '\n'), 0644)
}

func clearAssignedToolsVersion() error {
	if err := os.Remove(toolsAssignedVersionPath); err != nil && !os.IsNotExist(err) {
		return fmt.Errorf("clear assigned tools version: %w", err)
	}
	return nil
}

func applyLocalSoftwareAssignments(record enrollmentRecord) error {
	if err := applyAssignedFoldOpsManifestForRelease(record.DesiredFoldOpsManifestRelease); err != nil {
		return err
	}
	return applyAssignedToolsVersionForRelease(record.DesiredToolsVersion)
}

func applyAssignedFoldOpsManifestForRelease(release string) error {
	release = strings.TrimSpace(release)
	if isBootstrapAssignmentLabel(release) {
		return clearAssignedFoldOpsManifest()
	}
	entry, err := loadFoldOpsManifestRegistryEntry(release)
	if err != nil {
		return fmt.Errorf("assigned foldops manifest %q is not in registry: %w", release, err)
	}
	if entry.RolloutState != "ready" {
		return fmt.Errorf("assigned foldops manifest %q is not ready for rollout", release)
	}
	return writeAssignedFoldOpsManifest(entry.ManifestTOML)
}

func applyAssignedToolsVersionForRelease(version string) error {
	version = strings.TrimSpace(version)
	if isBootstrapAssignmentLabel(version) {
		return clearAssignedToolsVersion()
	}
	entry, err := loadToolsVersionRegistryEntry(version)
	if err != nil {
		return fmt.Errorf("assigned tools version %q is not in registry: %w", version, err)
	}
	if entry.RolloutState != "ready" {
		return fmt.Errorf("assigned tools version %q is not ready for rollout", version)
	}
	return writeAssignedToolsVersion(entry.Assignment)
}

func syncLocalSoftwareAssignmentsFromSupervisor(supervisorURL, nodeID, token string) error {
	response, err := queryDesiredSoftwareAssignments(supervisorURL, nodeID, token)
	if err != nil {
		return err
	}
	if strings.TrimSpace(response.DesiredFoldOpsManifest) != "" {
		if err := writeAssignedFoldOpsManifest(response.DesiredFoldOpsManifest); err != nil {
			return err
		}
	} else {
		if err := clearAssignedFoldOpsManifest(); err != nil {
			return err
		}
	}
	if response.DesiredToolsAssignment != nil {
		if err := writeAssignedToolsVersion(*response.DesiredToolsAssignment); err != nil {
			return err
		}
	} else {
		if err := clearAssignedToolsVersion(); err != nil {
			return err
		}
	}
	return nil
}

func queryDesiredSoftwareAssignments(supervisorURL, nodeID, token string) (desiredVersionResponse, error) {
	endpoint, err := joinSupervisorURL(supervisorURL, "/v1/agents/desired-version?node_id="+nodeID)
	if err != nil {
		return desiredVersionResponse{}, err
	}
	request, err := http.NewRequest(http.MethodGet, endpoint, nil)
	if err != nil {
		return desiredVersionResponse{}, err
	}
	request.Header.Set("X-FoldingOS-Enrollment-Token", token)
	response, err := provisionHTTPClient.Do(request)
	if err != nil {
		return desiredVersionResponse{}, err
	}
	defer response.Body.Close()
	body, err := io.ReadAll(io.LimitReader(response.Body, 1<<20))
	if err != nil {
		return desiredVersionResponse{}, err
	}
	if response.StatusCode != http.StatusOK {
		return desiredVersionResponse{}, fmt.Errorf(
			"desired-version query failed with status %s: %s",
			response.Status,
			strings.TrimSpace(string(body)),
		)
	}
	var result desiredVersionResponse
	if err := json.Unmarshal(body, &result); err != nil {
		return desiredVersionResponse{}, err
	}
	return result, nil
}

func shouldApplyLocalSupervisorAssignments(scope, targetNodeID string) (bool, error) {
	if err := requireSupervisorRole(); err != nil {
		return false, nil
	}
	localNodeID, err := readNodeID()
	if err != nil {
		return false, nil
	}
	if scope == "fleet" {
		return true, nil
	}
	return strings.TrimSpace(targetNodeID) == localNodeID, nil
}

func applySupervisorLocalAssignmentsIfNeeded(scope, targetNodeID string, record enrollmentRecord) error {
	apply, err := shouldApplyLocalSupervisorAssignments(scope, targetNodeID)
	if err != nil {
		return err
	}
	if !apply {
		return nil
	}
	return applyLocalSoftwareAssignments(record)
}

var errNoAssignmentFields = errors.New("assignment requires at least one of --version, --foldops-manifest, or --tools-version")
