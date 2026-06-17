package main

import (
	"crypto/rand"
	"crypto/subtle"
	"encoding/hex"
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
	provisionEnrollmentsDirDefault   = "/data/provision/enrollments"
	provisionEnrollmentsIndexDefault = "/data/provision/enrollments/index.json"
	provisionListenURLPathDefault    = "/data/config/provision/listen.url"
	supervisorURLPathDefault         = "/data/config/provision/supervisor.url"
	enrollmentTokenPathDefault       = "/data/config/provision/enrollment-token"
	agentEnrollmentStatePathDefault  = "/data/state/provision/enrolled"
)

var (
	provisionEnrollmentsDir   = provisionEnrollmentsDirDefault
	provisionEnrollmentsIndex = provisionEnrollmentsIndexDefault
	provisionListenURLPath    = provisionListenURLPathDefault
	supervisorURLPath         = supervisorURLPathDefault
	enrollmentTokenPath       = enrollmentTokenPathDefault
	agentEnrollmentStatePath  = agentEnrollmentStatePathDefault
)

type agentRegistrationRequest struct {
	SchemaVersion       int      `json:"schema_version"`
	NodeID              string   `json:"node_id"`
	EnrollmentToken     string   `json:"enrollment_token"`
	InstallationRole    string   `json:"installation_role"`
	CurrentImageVersion string   `json:"current_image_version"`
	FoldingOSVersion    string   `json:"foldingos_version"`
	Hostname            string   `json:"hostname"`
	MACAddresses        []string `json:"mac_addresses"`
	FAHActive           bool     `json:"fah_active,omitempty"`
}

type enrollmentRecord struct {
	SchemaVersion       int      `json:"schema_version"`
	NodeID              string   `json:"node_id"`
	InstallationRole    string   `json:"installation_role"`
	RegisteredAt        string   `json:"registered_at"`
	LastSeenAt          string   `json:"last_seen_at"`
	MACAddresses        []string `json:"mac_addresses"`
	CurrentImageVersion string   `json:"current_image_version"`
	FoldingOSVersion    string   `json:"foldingos_version"`
	Hostname            string   `json:"hostname"`
	FAHActive           bool     `json:"fah_active,omitempty"`
	DesiredImageVersion           string   `json:"desired_image_version"`
	DesiredFoldOpsManifestRelease string   `json:"desired_foldops_manifest_release,omitempty"`
	DesiredToolsVersion           string   `json:"desired_tools_version,omitempty"`
	LastUpdateStatus              string   `json:"last_update_status,omitempty"`
	LastUpdateVersion   string   `json:"last_update_version,omitempty"`
	LastUpdateMessage   string   `json:"last_update_message,omitempty"`
	LastUpdateAt        string   `json:"last_update_at,omitempty"`
}

type enrollmentIndex struct {
	SchemaVersion int      `json:"schema_version"`
	NodeIDs       []string `json:"node_ids"`
}

type desiredVersionResponse struct {
	SchemaVersion                 int              `json:"schema_version"`
	NodeID                        string           `json:"node_id"`
	CurrentImageVersion           string           `json:"current_image_version"`
	DesiredVersion                string           `json:"desired_version"`
	DesiredFoldOpsManifestRelease string           `json:"desired_foldops_manifest_release,omitempty"`
	DesiredFoldOpsManifest        string           `json:"desired_foldops_manifest,omitempty"`
	DesiredToolsVersion             string           `json:"desired_tools_version,omitempty"`
	DesiredToolsAssignment        *toolsAssignment `json:"desired_tools_assignment,omitempty"`
}

type rolloutAssignRequest struct {
	SchemaVersion               int    `json:"schema_version"`
	EnrollmentToken             string `json:"enrollment_token"`
	Scope                       string `json:"scope"`
	NodeID                      string `json:"node_id,omitempty"`
	Version                     string `json:"version,omitempty"`
	FoldOpsManifestRelease      string `json:"foldops_manifest_release,omitempty"`
	ToolsVersion                string `json:"tools_version,omitempty"`
}

func enrollmentRecordPath(nodeID string) string {
	return filepath.Join(provisionEnrollmentsDir, nodeID+".json")
}

func loadEnrollmentIndex() (enrollmentIndex, error) {
	content, err := os.ReadFile(provisionEnrollmentsIndex)
	if err != nil {
		if os.IsNotExist(err) {
			return enrollmentIndex{SchemaVersion: 1, NodeIDs: []string{}}, nil
		}
		return enrollmentIndex{}, err
	}
	var index enrollmentIndex
	if err := json.Unmarshal(content, &index); err != nil {
		return enrollmentIndex{}, fmt.Errorf("invalid enrollment index: %w", err)
	}
	if index.SchemaVersion != 1 {
		return enrollmentIndex{}, fmt.Errorf("unsupported enrollment index schema version %d", index.SchemaVersion)
	}
	return index, nil
}

func saveEnrollmentIndex(index enrollmentIndex) error {
	index.SchemaVersion = 1
	sort.Strings(index.NodeIDs)
	content, err := json.MarshalIndent(index, "", "  ")
	if err != nil {
		return err
	}
	return atomicWrite(provisionEnrollmentsIndex, append(content, '\n'), 0644)
}

func loadEnrollmentRecord(nodeID string) (enrollmentRecord, error) {
	content, err := os.ReadFile(enrollmentRecordPath(nodeID))
	if err != nil {
		return enrollmentRecord{}, err
	}
	var record enrollmentRecord
	if err := json.Unmarshal(content, &record); err != nil {
		return enrollmentRecord{}, fmt.Errorf("invalid enrollment record for %s: %w", nodeID, err)
	}
	return validateEnrollmentRecord(record)
}

func saveEnrollmentRecord(record enrollmentRecord) error {
	validated, err := validateEnrollmentRecord(record)
	if err != nil {
		return err
	}
	content, err := json.MarshalIndent(validated, "", "  ")
	if err != nil {
		return err
	}
	if err := atomicWrite(enrollmentRecordPath(validated.NodeID), append(content, '\n'), 0644); err != nil {
		return err
	}
	index, err := loadEnrollmentIndex()
	if err != nil {
		return err
	}
	if !containsString(index.NodeIDs, validated.NodeID) {
		index.NodeIDs = append(index.NodeIDs, validated.NodeID)
	}
	return saveEnrollmentIndex(index)
}

func validateEnrollmentRecord(record enrollmentRecord) (enrollmentRecord, error) {
	if record.SchemaVersion != 1 {
		return enrollmentRecord{}, fmt.Errorf("unsupported enrollment schema version %d", record.SchemaVersion)
	}
	record.NodeID = strings.TrimSpace(record.NodeID)
	if !uuidPattern.MatchString(record.NodeID) {
		return enrollmentRecord{}, errors.New("enrollment record node_id is invalid")
	}
	record.InstallationRole = strings.TrimSpace(record.InstallationRole)
	if record.InstallationRole != "agent" {
		return enrollmentRecord{}, fmt.Errorf("enrollment record role must be agent, found %q", record.InstallationRole)
	}
	record.CurrentImageVersion = strings.TrimSpace(record.CurrentImageVersion)
	if record.CurrentImageVersion == "" {
		return enrollmentRecord{}, errors.New("enrollment record missing current_image_version")
	}
	record.FoldingOSVersion = strings.TrimSpace(record.FoldingOSVersion)
	if record.FoldingOSVersion == "" {
		return enrollmentRecord{}, errors.New("enrollment record missing foldingos_version")
	}
	record.Hostname = strings.TrimSpace(record.Hostname)
	if record.Hostname == "" {
		return enrollmentRecord{}, errors.New("enrollment record missing hostname")
	}
	if len(record.MACAddresses) == 0 {
		return enrollmentRecord{}, errors.New("enrollment record missing mac_addresses")
	}
	record.DesiredImageVersion = strings.TrimSpace(record.DesiredImageVersion)
	if record.DesiredImageVersion == "" {
		record.DesiredImageVersion = "current"
	}
	if record.DesiredImageVersion != "current" {
		entry, err := loadRegistryEntry(record.DesiredImageVersion)
		if err != nil {
			return enrollmentRecord{}, fmt.Errorf("desired image version %q is not in registry: %w", record.DesiredImageVersion, err)
		}
		if entry.RolloutState != "ready" {
			return enrollmentRecord{}, fmt.Errorf("desired image version %q is not ready for rollout", record.DesiredImageVersion)
		}
	}
	record.DesiredFoldOpsManifestRelease = strings.TrimSpace(record.DesiredFoldOpsManifestRelease)
	if record.DesiredFoldOpsManifestRelease != "" && !isBootstrapAssignmentLabel(record.DesiredFoldOpsManifestRelease) {
		entry, err := loadFoldOpsManifestRegistryEntry(record.DesiredFoldOpsManifestRelease)
		if err != nil {
			return enrollmentRecord{}, fmt.Errorf("desired foldops manifest %q is not in registry: %w", record.DesiredFoldOpsManifestRelease, err)
		}
		if entry.RolloutState != "ready" {
			return enrollmentRecord{}, fmt.Errorf("desired foldops manifest %q is not ready for rollout", record.DesiredFoldOpsManifestRelease)
		}
	}
	record.DesiredToolsVersion = strings.TrimSpace(record.DesiredToolsVersion)
	if record.DesiredToolsVersion != "" && !isBootstrapAssignmentLabel(record.DesiredToolsVersion) {
		entry, err := loadToolsVersionRegistryEntry(record.DesiredToolsVersion)
		if err != nil {
			return enrollmentRecord{}, fmt.Errorf("desired tools version %q is not in registry: %w", record.DesiredToolsVersion, err)
		}
		if entry.RolloutState != "ready" {
			return enrollmentRecord{}, fmt.Errorf("desired tools version %q is not ready for rollout", record.DesiredToolsVersion)
		}
	}
	if record.RegisteredAt == "" {
		record.RegisteredAt = time.Now().UTC().Format(time.RFC3339)
	}
	if record.LastSeenAt == "" {
		record.LastSeenAt = record.RegisteredAt
	}
	return record, nil
}

func validateRegistrationRequest(request agentRegistrationRequest) (agentRegistrationRequest, error) {
	if request.SchemaVersion != 1 {
		return agentRegistrationRequest{}, fmt.Errorf("unsupported registration schema version %d", request.SchemaVersion)
	}
	request.NodeID = strings.TrimSpace(request.NodeID)
	if !uuidPattern.MatchString(request.NodeID) {
		return agentRegistrationRequest{}, errors.New("registration node_id is invalid")
	}
	request.EnrollmentToken = strings.TrimSpace(request.EnrollmentToken)
	if request.EnrollmentToken == "" {
		return agentRegistrationRequest{}, errors.New("registration enrollment_token is required")
	}
	request.InstallationRole = strings.TrimSpace(request.InstallationRole)
	if request.InstallationRole != "agent" {
		return agentRegistrationRequest{}, fmt.Errorf("registration role must be agent, found %q", request.InstallationRole)
	}
	request.CurrentImageVersion = strings.TrimSpace(request.CurrentImageVersion)
	if request.CurrentImageVersion == "" {
		return agentRegistrationRequest{}, errors.New("registration current_image_version is required")
	}
	request.FoldingOSVersion = strings.TrimSpace(request.FoldingOSVersion)
	if request.FoldingOSVersion == "" {
		return agentRegistrationRequest{}, errors.New("registration foldingos_version is required")
	}
	request.Hostname = strings.TrimSpace(request.Hostname)
	if request.Hostname == "" {
		return agentRegistrationRequest{}, errors.New("registration hostname is required")
	}
	if len(request.MACAddresses) == 0 {
		return agentRegistrationRequest{}, errors.New("registration mac_addresses is required")
	}
	return request, nil
}

func registerAgent(request agentRegistrationRequest) (enrollmentRecord, error) {
	validated, err := validateRegistrationRequest(request)
	if err != nil {
		return enrollmentRecord{}, err
	}
	if err := validateEnrollmentToken(validated.EnrollmentToken); err != nil {
		return enrollmentRecord{}, err
	}

	now := time.Now().UTC().Format(time.RFC3339)
	record := enrollmentRecord{
		SchemaVersion:       1,
		NodeID:              validated.NodeID,
		InstallationRole:    validated.InstallationRole,
		RegisteredAt:        now,
		LastSeenAt:          now,
		MACAddresses:        validated.MACAddresses,
		CurrentImageVersion: validated.CurrentImageVersion,
		FoldingOSVersion:    validated.FoldingOSVersion,
		Hostname:            validated.Hostname,
		FAHActive:           validated.FAHActive,
		DesiredImageVersion: "current",
	}

	if existing, err := loadEnrollmentRecord(validated.NodeID); err == nil {
		record.RegisteredAt = existing.RegisteredAt
		record.DesiredImageVersion = existing.DesiredImageVersion
		record.DesiredFoldOpsManifestRelease = existing.DesiredFoldOpsManifestRelease
		record.DesiredToolsVersion = existing.DesiredToolsVersion
	} else if !os.IsNotExist(err) {
		return enrollmentRecord{}, err
	}

	if err := saveEnrollmentRecord(record); err != nil {
		return enrollmentRecord{}, err
	}
	return record, nil
}

func desiredVersionForNode(nodeID string) (desiredVersionResponse, error) {
	record, err := loadEnrollmentRecord(nodeID)
	if err != nil {
		if os.IsNotExist(err) {
			return desiredVersionResponse{}, errors.New("agent is not registered")
		}
		return desiredVersionResponse{}, err
	}

	record.LastSeenAt = time.Now().UTC().Format(time.RFC3339)
	if err := saveEnrollmentRecord(record); err != nil {
		return desiredVersionResponse{}, err
	}

	desired := record.DesiredImageVersion
	if desired == record.CurrentImageVersion {
		desired = "current"
	}

	return desiredVersionResponse{
		SchemaVersion:                 2,
		NodeID:                        record.NodeID,
		CurrentImageVersion:           record.CurrentImageVersion,
		DesiredVersion:                desired,
		DesiredFoldOpsManifestRelease: record.DesiredFoldOpsManifestRelease,
		DesiredFoldOpsManifest:        resolveDesiredFoldOpsManifestTOML(record.DesiredFoldOpsManifestRelease),
		DesiredToolsVersion:           record.DesiredToolsVersion,
		DesiredToolsAssignment:        resolveDesiredToolsAssignment(record.DesiredToolsVersion),
	}, nil
}

func resolveDesiredFoldOpsManifestTOML(release string) string {
	release = strings.TrimSpace(release)
	if isBootstrapAssignmentLabel(release) {
		return ""
	}
	entry, err := loadFoldOpsManifestRegistryEntry(release)
	if err != nil {
		return ""
	}
	return entry.ManifestTOML
}

func resolveDesiredToolsAssignment(version string) *toolsAssignment {
	version = strings.TrimSpace(version)
	if isBootstrapAssignmentLabel(version) {
		return nil
	}
	entry, err := loadToolsVersionRegistryEntry(version)
	if err != nil {
		return nil
	}
	assignment := entry.Assignment
	return &assignment
}

type softwareAssignmentUpdate struct {
	imageVersion           *string
	foldOpsManifestRelease *string
	toolsVersion           *string
}

func assignDesiredVersion(scope, nodeID, version string) (int, error) {
	update := softwareAssignmentUpdate{}
	if version != "" {
		update.imageVersion = &version
	}
	return assignSoftwareVersions(scope, nodeID, update)
}

func assignSoftwareVersions(scope, nodeID string, update softwareAssignmentUpdate) (int, error) {
	if err := requireSupervisorRole(); err != nil {
		return 0, err
	}
	if update.imageVersion == nil && update.foldOpsManifestRelease == nil && update.toolsVersion == nil {
		return 0, errNoAssignmentFields
	}
	if update.imageVersion != nil {
		version := strings.TrimSpace(*update.imageVersion)
		if version == "" {
			return 0, errors.New("assigned image version is required")
		}
		if version != "current" {
			entry, err := loadRegistryEntry(version)
			if err != nil {
				return 0, fmt.Errorf("assigned image version %q is not in registry: %w", version, err)
			}
			if entry.RolloutState != "ready" {
				return 0, fmt.Errorf("assigned image version %q is not ready for rollout", version)
			}
		}
	}
	if update.foldOpsManifestRelease != nil {
		release := strings.TrimSpace(*update.foldOpsManifestRelease)
		if release != "" && !isBootstrapAssignmentLabel(release) {
			entry, err := loadFoldOpsManifestRegistryEntry(release)
			if err != nil {
				return 0, fmt.Errorf("assigned foldops manifest %q is not in registry: %w", release, err)
			}
			if entry.RolloutState != "ready" {
				return 0, fmt.Errorf("assigned foldops manifest %q is not ready for rollout", release)
			}
		}
	}
	if update.toolsVersion != nil {
		version := strings.TrimSpace(*update.toolsVersion)
		if version != "" && !isBootstrapAssignmentLabel(version) {
			entry, err := loadToolsVersionRegistryEntry(version)
			if err != nil {
				return 0, fmt.Errorf("assigned tools version %q is not in registry: %w", version, err)
			}
			if entry.RolloutState != "ready" {
				return 0, fmt.Errorf("assigned tools version %q is not ready for rollout", version)
			}
		}
	}

	index, err := loadEnrollmentIndex()
	if err != nil {
		return 0, err
	}
	if len(index.NodeIDs) == 0 {
		return 0, errors.New("no enrolled agents are available")
	}

	targets := index.NodeIDs
	if scope == "node" {
		if !uuidPattern.MatchString(nodeID) {
			return 0, errors.New("node id is invalid")
		}
		if !containsString(index.NodeIDs, nodeID) {
			return 0, errors.New("agent is not registered")
		}
		targets = []string{nodeID}
	} else if scope != "fleet" {
		return 0, fmt.Errorf("unsupported assignment scope %q", scope)
	}

	updated := 0
	for _, target := range targets {
		record, err := loadEnrollmentRecord(target)
		if err != nil {
			return updated, err
		}
		if update.imageVersion != nil {
			version := strings.TrimSpace(*update.imageVersion)
			if version == "" {
				version = "current"
			}
			record.DesiredImageVersion = version
		}
		if update.foldOpsManifestRelease != nil {
			release := strings.TrimSpace(*update.foldOpsManifestRelease)
			if isBootstrapAssignmentLabel(release) {
				release = ""
			}
			record.DesiredFoldOpsManifestRelease = release
		}
		if update.toolsVersion != nil {
			version := strings.TrimSpace(*update.toolsVersion)
			if isBootstrapAssignmentLabel(version) {
				version = ""
			}
			record.DesiredToolsVersion = version
		}
		if err := saveEnrollmentRecord(record); err != nil {
			return updated, err
		}
		if err := applySupervisorLocalAssignmentsIfNeeded(scope, target, record); err != nil {
			return updated, err
		}
		updated++
	}
	return updated, nil
}

func readEnrollmentToken() (string, error) {
	content, err := os.ReadFile(enrollmentTokenPath)
	if err != nil {
		return "", err
	}
	token := strings.TrimSpace(string(content))
	if token == "" {
		return "", errors.New("enrollment token is empty")
	}
	return token, nil
}

func ensureEnrollmentToken() (string, error) {
	token, err := readEnrollmentToken()
	if err == nil {
		return token, nil
	}
	if !os.IsNotExist(err) {
		return "", err
	}
	value := make([]byte, 32)
	if _, err := rand.Read(value); err != nil {
		return "", err
	}
	token = hex.EncodeToString(value)
	if err := atomicWrite(enrollmentTokenPath, []byte(token+"\n"), 0600); err != nil {
		return "", err
	}
	return token, nil
}

func validateEnrollmentToken(provided string) error {
	expected, err := readEnrollmentToken()
	if err != nil {
		return err
	}
	if subtle.ConstantTimeCompare([]byte(provided), []byte(expected)) != 1 {
		return errors.New("enrollment token is invalid")
	}
	return nil
}

func markAgentEnrolled(nodeID string) error {
	content := nodeID + "\n"
	return atomicWrite(agentEnrollmentStatePath, []byte(content), 0644)
}

func agentEnrollmentNodeID() (string, error) {
	content, err := os.ReadFile(agentEnrollmentStatePath)
	if err != nil {
		return "", err
	}
	nodeID := strings.TrimSpace(string(content))
	if !uuidPattern.MatchString(nodeID) {
		return "", errors.New("local enrollment state is invalid")
	}
	return nodeID, nil
}
