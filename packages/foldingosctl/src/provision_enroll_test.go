package main

import (
	"net/http"
	"net/http/httptest"
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestProvisionEnrollEnsuresNodeIdentity(t *testing.T) {
	root := t.TempDir()
	restoreProvision := setProvisionPaths(root)
	defer restoreProvision()
	restoreRole := setInstallationRolePaths(
		filepath.Join(root, "efi", "installation-role"),
		filepath.Join(root, "config", "installation-role"),
	)
	defer restoreRole()
	restoreConfig := setConfigTestPaths(root)
	defer restoreConfig()

	writeEnrollmentToken(t, root, "test-enrollment-token")
	if err := os.WriteFile(filepath.Join(root, "config", "installation-role"), []byte("agent\n"), 0644); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(
		filepath.Join(root, "config", "provision", "supervisor.url"),
		[]byte("http://127.0.0.1:8743\n"),
		0644,
	); err != nil {
		t.Fatal(err)
	}

	server := httptest.NewServer(http.HandlerFunc(handleAgentRegister))
	defer server.Close()

	if err := os.WriteFile(
		filepath.Join(root, "config", "provision", "supervisor.url"),
		[]byte(server.URL+"\n"),
		0644,
	); err != nil {
		t.Fatal(err)
	}

	previousClient := provisionHTTPClient
	provisionHTTPClient = server.Client()
	defer func() {
		provisionHTTPClient = previousClient
	}()

	if err := provisionEnroll(); err != nil {
		t.Fatal(err)
	}
	if _, err := os.Stat(filepath.Join(root, "config", "node-id")); err != nil {
		t.Fatalf("node-id was not created: %v", err)
	}
}

func TestProvisionEnrollFailsWhenSupervisorURLMissingForNetworkAgent(t *testing.T) {
	root := t.TempDir()
	restoreProvision := setProvisionPaths(root)
	defer restoreProvision()
	restoreRole := setInstallationRolePaths(
		filepath.Join(root, "efi", "installation-role"),
		filepath.Join(root, "config", "installation-role"),
	)
	defer restoreRole()
	restoreConfig := setConfigTestPaths(root)
	defer restoreConfig()

	writeEnrollmentToken(t, root, "test-enrollment-token")
	if err := os.WriteFile(filepath.Join(root, "config", "installation-role"), []byte("agent\n"), 0644); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(root, "config", "node-id"), []byte(testAgentNodeID+"\n"), 0644); err != nil {
		t.Fatal(err)
	}

	if err := provisionEnroll(); err == nil {
		t.Fatal("provisionEnroll() succeeded without supervisor URL for network-provisioned agent")
	}
}

func TestProvisionEnrollRegistersWithSupervisor(t *testing.T) {
	root := t.TempDir()
	restoreProvision := setProvisionPaths(root)
	defer restoreProvision()
	restoreRole := setInstallationRolePaths(
		filepath.Join(root, "efi", "installation-role"),
		filepath.Join(root, "config", "installation-role"),
	)
	defer restoreRole()
	restoreConfig := setConfigTestPaths(root)
	defer restoreConfig()

	writeEnrollmentToken(t, root, "test-enrollment-token")
	if err := os.WriteFile(filepath.Join(root, "config", "installation-role"), []byte("agent\n"), 0644); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(root, "config", "node-id"), []byte(testAgentNodeID+"\n"), 0644); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(
		filepath.Join(root, "config", "system.toml"),
		[]byte("schema_version = 1\n\n[identity]\nhostname = \"folding-test\"\n"),
		0644,
	); err != nil {
		t.Fatal(err)
	}

	server := httptest.NewServer(http.HandlerFunc(handleAgentRegister))
	defer server.Close()

	if err := os.WriteFile(
		filepath.Join(root, "config", "provision", "supervisor.url"),
		[]byte(server.URL+"\n"),
		0644,
	); err != nil {
		t.Fatal(err)
	}

	previousClient := provisionHTTPClient
	provisionHTTPClient = server.Client()
	defer func() {
		provisionHTTPClient = previousClient
	}()

	if err := provisionEnroll(); err != nil {
		t.Fatal(err)
	}
	if _, err := agentEnrollmentNodeID(); err != nil {
		t.Fatalf("local enrollment marker missing: %v", err)
	}
}

func TestAgentRegisterServiceWaitsForIdentity(t *testing.T) {
	unitPath := filepath.Join(
		"..", "..", "..",
		"overlay", "usr", "lib", "systemd", "system",
		"foldingos-agent-register.service",
	)
	content, err := os.ReadFile(unitPath)
	if err != nil {
		t.Fatalf("read agent register unit: %v", err)
	}
	text := string(content)
	for _, required := range []string{
		"Requires=foldingos-installation-role.service foldingos-identity.service foldingos-config-validate.service",
		"After=foldingos-installation-role.service foldingos-identity.service foldingos-config-validate.service network-online.target",
	} {
		if !strings.Contains(text, required) {
			t.Fatalf("agent register unit missing %q:\n%s", required, text)
		}
	}
}

func TestIdentityServiceRunsBeforeAgentRegister(t *testing.T) {
	unitPath := filepath.Join(
		"..", "..", "..",
		"overlay", "usr", "lib", "systemd", "system",
		"foldingos-identity.service",
	)
	content, err := os.ReadFile(unitPath)
	if err != nil {
		t.Fatalf("read identity unit: %v", err)
	}
	if !strings.Contains(string(content), "Before=foldingos-config-validate.service foldingos-agent-register.service") {
		t.Fatalf("identity unit must run before agent registration:\n%s", content)
	}
}

func setConfigTestPaths(root string) func() {
	previous := struct {
		configDir, effectiveDir, defaultsDir string
	}{
		configDir,
		effectiveDir,
		defaultsDir,
	}
	configDir = filepath.Join(root, "config")
	effectiveDir = filepath.Join(root, "effective")
	defaultsDir = filepath.Join(root, "defaults")
	for _, directory := range []string{configDir, effectiveDir, defaultsDir} {
		if err := os.MkdirAll(directory, 0755); err != nil {
			panic(err)
		}
	}
	return func() {
		configDir = previous.configDir
		effectiveDir = previous.effectiveDir
		defaultsDir = previous.defaultsDir
	}
}
