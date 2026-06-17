package main

import (
	"bytes"
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"testing"
)

const testAutomationPolicyTOML = `schema_version = 1
service_user = "foldops"
installation_role = "supervisor"

[[commands]]
group = "provision"
name = "assign"

[[commands]]
group = "provision"
name = "allow-boot"
`

func TestParseFoldOpsSupervisorAutomationPolicy(t *testing.T) {
	policy, err := parseFoldOpsSupervisorAutomationPolicy(testAutomationPolicyTOML)
	if err != nil {
		t.Fatal(err)
	}
	if policy.ServiceUser != "foldops" || policy.InstallationRole != "supervisor" {
		t.Fatalf("policy: %+v", policy)
	}
	if len(policy.Commands) != 2 {
		t.Fatalf("commands: %+v", policy.Commands)
	}
}

func TestRequireSupervisorAutomationMutationAllowsOperatorUser(t *testing.T) {
	root := t.TempDir()
	policyPath := filepath.Join(root, "automation-policy.toml")
	if err := os.WriteFile(policyPath, []byte(testAutomationPolicyTOML), 0644); err != nil {
		t.Fatal(err)
	}
	previousPolicyPath := foldOpsSupervisorAutomationPolicyPath
	previousPolicyCache := cachedAutomationPolicy
	foldOpsSupervisorAutomationPolicyPath = policyPath
	cachedAutomationPolicy = nil
	t.Cleanup(func() {
		foldOpsSupervisorAutomationPolicyPath = previousPolicyPath
		cachedAutomationPolicy = previousPolicyCache
	})

	restoreUser := stubAutomationUser("foldingos-admin")
	defer restoreUser()

	if err := requireSupervisorAutomationMutation("provision", "assign"); err != nil {
		t.Fatalf("operator user should bypass automation policy: %v", err)
	}
}

func TestRequireSupervisorAutomationMutationDeniesUnlistedFoldOpsCommand(t *testing.T) {
	root := t.TempDir()
	policyPath := filepath.Join(root, "automation-policy.toml")
	if err := os.WriteFile(policyPath, []byte(testAutomationPolicyTOML), 0644); err != nil {
		t.Fatal(err)
	}
	restoreRole := setInstallationRolePaths(
		filepath.Join(root, "efi", "installation-role"),
		filepath.Join(root, "config", "installation-role"),
	)
	defer restoreRole()
	if err := os.MkdirAll(filepath.Join(root, "config"), 0755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(root, "config", "installation-role"), []byte("supervisor\n"), 0644); err != nil {
		t.Fatal(err)
	}

	previousPolicyPath := foldOpsSupervisorAutomationPolicyPath
	previousPolicyCache := cachedAutomationPolicy
	foldOpsSupervisorAutomationPolicyPath = policyPath
	cachedAutomationPolicy = nil
	t.Cleanup(func() {
		foldOpsSupervisorAutomationPolicyPath = previousPolicyPath
		cachedAutomationPolicy = previousPolicyCache
	})

	restoreUser := stubAutomationUser("foldops")
	defer restoreUser()

	if err := requireSupervisorAutomationMutation("provision", "install"); err == nil {
		t.Fatal("expected automation policy denial")
	}
	if err := requireSupervisorAutomationMutation("provision", "assign"); err != nil {
		t.Fatalf("assign should be authorized: %v", err)
	}
}

func TestAutomationFailureClassifiesPolicyDenial(t *testing.T) {
	automationCtx = automationContext{
		format:  formatJSON,
		command: "provision assign",
	}
	read, write, err := os.Pipe()
	if err != nil {
		t.Fatal(err)
	}
	previous := os.Stdout
	os.Stdout = write
	t.Cleanup(func() {
		os.Stdout = previous
	})

	err = writeAutomationFailure(fmt.Errorf("automation policy does not authorize provision assign for the foldops user"))
	write.Close()
	if err == nil {
		t.Fatal("expected failure")
	}

	var stdout bytes.Buffer
	stdout.ReadFrom(read)
	var document automationFailureDocument
	if err := json.Unmarshal(stdout.Bytes(), &document); err != nil {
		t.Fatal(err)
	}
	if document.Error.Code != "automation_denied" {
		t.Fatalf("document: %+v", document)
	}
}
