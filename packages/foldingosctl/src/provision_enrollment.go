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
	DesiredImageVersion string   `json:"desired_image_version"`
}

type enrollmentIndex struct {
	SchemaVersion int      `json:"schema_version"`
	NodeIDs       []string `json:"node_ids"`
}

type desiredVersionResponse struct {
	SchemaVersion       int    `json:"schema_version"`
	NodeID              string `json:"node_id"`
	CurrentImageVersion string `json:"current_image_version"`
	DesiredVersion      string `json:"desired_version"`
}

type rolloutAssignRequest struct {
	SchemaVersion int    `json:"schema_version"`
	EnrollmentToken string `json:"enrollment_token"`
	Scope         string `json:"scope"`
	NodeID        string `json:"node_id,omitempty"`
	Version       string `json:"version"`
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
		SchemaVersion:       1,
		NodeID:              record.NodeID,
		CurrentImageVersion: record.CurrentImageVersion,
		DesiredVersion:      desired,
	}, nil
}

func assignDesiredVersion(scope, nodeID, version string) (int, error) {
	if err := requireSupervisorRole(); err != nil {
		return 0, err
	}
	version = strings.TrimSpace(version)
	if version == "" {
		return 0, errors.New("assigned version is required")
	}
	if version != "current" {
		entry, err := loadRegistryEntry(version)
		if err != nil {
			return 0, fmt.Errorf("assigned version %q is not in registry: %w", version, err)
		}
		if entry.RolloutState != "ready" {
			return 0, fmt.Errorf("assigned version %q is not ready for rollout", version)
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
		record.DesiredImageVersion = version
		if err := saveEnrollmentRecord(record); err != nil {
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
