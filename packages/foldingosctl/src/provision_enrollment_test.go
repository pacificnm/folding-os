package main

import (
	"bytes"
	"encoding/json"
	"io"
	"net/http"
	"net/http/httptest"
	"os"
	"path/filepath"
	"strings"
	"testing"
)

const testAgentNodeID = "550e8400-e29b-41d4-a716-446655440000"

func TestRegisterAgentStoresEnrollmentRecord(t *testing.T) {
	root := t.TempDir()
	restore := setProvisionPaths(root)
	defer restore()
	writeEnrollmentToken(t, root, "test-enrollment-token")

	record, err := registerAgent(sampleRegistrationRequest("test-enrollment-token"))
	if err != nil {
		t.Fatal(err)
	}
	if record.NodeID != testAgentNodeID || record.DesiredImageVersion != "current" {
		t.Fatalf("record: %+v", record)
	}
}

func TestRegisterAgentRejectsInvalidToken(t *testing.T) {
	root := t.TempDir()
	restore := setProvisionPaths(root)
	defer restore()
	writeEnrollmentToken(t, root, "expected-token")

	if _, err := registerAgent(sampleRegistrationRequest("wrong-token")); err == nil {
		t.Fatal("invalid enrollment token was accepted")
	}
}

func TestDesiredVersionReturnsCurrentForRegisteredAgent(t *testing.T) {
	root := t.TempDir()
	restore := setProvisionPaths(root)
	defer restore()
	writeEnrollmentToken(t, root, "test-enrollment-token")
	if _, err := registerAgent(sampleRegistrationRequest("test-enrollment-token")); err != nil {
		t.Fatal(err)
	}

	response, err := desiredVersionForNode(testAgentNodeID)
	if err != nil {
		t.Fatal(err)
	}
	if response.DesiredVersion != "current" {
		t.Fatalf("desired version = %q", response.DesiredVersion)
	}
}

func TestDesiredVersionRejectsUnregisteredAgent(t *testing.T) {
	root := t.TempDir()
	restore := setProvisionPaths(root)
	defer restore()

	if _, err := desiredVersionForNode(testAgentNodeID); err == nil {
		t.Fatal("unregistered agent was accepted")
	}
}

func TestAssignDesiredVersionRequiresRegisteredAgent(t *testing.T) {
	root := t.TempDir()
	restore := setProvisionPaths(root)
	defer restore()
	restoreRole := setInstallationRolePaths(
		filepath.Join(root, "provision-role"),
		filepath.Join(root, "config", "installation-role"),
	)
	defer restoreRole()
	if err := os.MkdirAll(filepath.Join(root, "config"), 0755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(root, "config", "installation-role"), []byte("supervisor"), 0644); err != nil {
		t.Fatal(err)
	}

	if _, err := assignDesiredVersion("node", testAgentNodeID, "current"); err == nil {
		t.Fatal("assignment to unregistered agent was accepted")
	}
}

func TestAssignDesiredVersionUpdatesRegisteredAgents(t *testing.T) {
	root := t.TempDir()
	restore := setProvisionPaths(root)
	defer restore()
	restoreRegistry := setRegistryPaths(root)
	defer restoreRegistry()
	restoreRole := setInstallationRolePaths(
		filepath.Join(root, "provision-role"),
		filepath.Join(root, "config", "installation-role"),
	)
	defer restoreRole()
	if err := os.MkdirAll(filepath.Join(root, "config"), 0755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(root, "config", "installation-role"), []byte("supervisor"), 0644); err != nil {
		t.Fatal(err)
	}
	writeEnrollmentToken(t, root, "test-enrollment-token")
	if _, err := registerAgent(sampleRegistrationRequest("test-enrollment-token")); err != nil {
		t.Fatal(err)
	}
	if err := saveRegistryEntry(registryEntry{
		SchemaVersion:      1,
		FoldingOSVersion:   "0.2.0",
		GitRevision:        "abc",
		ImageSHA256:        strings.Repeat("d", 64),
		ImageSizeBytes:     1024,
		VerificationMethod: "sha256",
		ImportTimestamp:    "2026-06-13T20:00:00Z",
		RolloutState:       "ready",
		LocalImagePath:     filepath.Join(root, "images", "foldingos-x86_64-0.2.0.img"),
	}); err != nil {
		t.Fatal(err)
	}

	updated, err := assignDesiredVersion("node", testAgentNodeID, "0.2.0")
	if err != nil {
		t.Fatal(err)
	}
	if updated != 1 {
		t.Fatalf("updated = %d", updated)
	}
	record, err := loadEnrollmentRecord(testAgentNodeID)
	if err != nil {
		t.Fatal(err)
	}
	if record.DesiredImageVersion != "0.2.0" {
		t.Fatalf("desired version = %q", record.DesiredImageVersion)
	}
}

func TestProvisionAPIRegisterAndDesiredVersion(t *testing.T) {
	root := t.TempDir()
	restore := setProvisionPaths(root)
	defer restore()
	writeEnrollmentToken(t, root, "test-enrollment-token")

	server := httptest.NewServer(http.HandlerFunc(func(writer http.ResponseWriter, request *http.Request) {
		switch request.URL.Path {
		case "/v1/agents/register":
			handleAgentRegister(writer, request)
		case "/v1/agents/desired-version":
			handleDesiredVersion(writer, request)
		default:
			http.NotFound(writer, request)
		}
	}))
	defer server.Close()

	body, err := json.Marshal(sampleRegistrationRequest("test-enrollment-token"))
	if err != nil {
		t.Fatal(err)
	}
	registerResponse, err := http.Post(server.URL+"/v1/agents/register", "application/json", bytes.NewReader(body))
	if err != nil {
		t.Fatal(err)
	}
	defer registerResponse.Body.Close()
	if registerResponse.StatusCode != http.StatusOK {
		t.Fatalf("register status = %s", registerResponse.Status)
	}

	request, err := http.NewRequest(http.MethodGet, server.URL+"/v1/agents/desired-version?node_id="+testAgentNodeID, nil)
	if err != nil {
		t.Fatal(err)
	}
	request.Header.Set("X-FoldingOS-Enrollment-Token", "test-enrollment-token")
	desiredResponse, err := http.DefaultClient.Do(request)
	if err != nil {
		t.Fatal(err)
	}
	defer desiredResponse.Body.Close()
	responseBody, err := io.ReadAll(desiredResponse.Body)
	if err != nil {
		t.Fatal(err)
	}
	if desiredResponse.StatusCode != http.StatusOK {
		t.Fatalf("desired-version status = %s body = %s", desiredResponse.Status, responseBody)
	}
	if !strings.Contains(string(responseBody), `"desired_version": "current"`) {
		t.Fatalf("response body = %s", responseBody)
	}
}

func sampleRegistrationRequest(token string) agentRegistrationRequest {
	return agentRegistrationRequest{
		SchemaVersion:       1,
		NodeID:              testAgentNodeID,
		EnrollmentToken:     token,
		InstallationRole:    "agent",
		CurrentImageVersion: "0.1.0",
		FoldingOSVersion:    "0.1.0",
		Hostname:            "folding-test",
		MACAddresses:        []string{"52:54:00:12:34:56"},
	}
}

func writeEnrollmentToken(t *testing.T, root, token string) {
	t.Helper()
	if err := os.MkdirAll(filepath.Join(root, "config", "provision"), 0755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(root, "config", "provision", "enrollment-token"), []byte(token+"\n"), 0600); err != nil {
		t.Fatal(err)
	}
}

func setProvisionPaths(root string) func() {
	previous := struct {
		enrollmentsDir, enrollmentsIndex, listenURL, supervisorURL, tokenPath, enrolledPath string
	}{
		provisionEnrollmentsDir,
		provisionEnrollmentsIndex,
		provisionListenURLPath,
		supervisorURLPath,
		enrollmentTokenPath,
		agentEnrollmentStatePath,
	}
	provisionEnrollmentsDir = filepath.Join(root, "enrollments")
	provisionEnrollmentsIndex = filepath.Join(root, "enrollments", "index.json")
	provisionListenURLPath = filepath.Join(root, "config", "provision", "listen.url")
	supervisorURLPath = filepath.Join(root, "config", "provision", "supervisor.url")
	enrollmentTokenPath = filepath.Join(root, "config", "provision", "enrollment-token")
	agentEnrollmentStatePath = filepath.Join(root, "state", "enrolled")
	return func() {
		provisionEnrollmentsDir = previous.enrollmentsDir
		provisionEnrollmentsIndex = previous.enrollmentsIndex
		provisionListenURLPath = previous.listenURL
		supervisorURLPath = previous.supervisorURL
		enrollmentTokenPath = previous.tokenPath
		agentEnrollmentStatePath = previous.enrolledPath
	}
}
