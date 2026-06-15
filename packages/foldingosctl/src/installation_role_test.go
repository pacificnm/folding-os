package main

import (
	"os"
	"path/filepath"
	"testing"
)

func TestParseInstallationRoleAcceptsApprovedRoles(t *testing.T) {
	for _, role := range []string{"agent", "supervisor"} {
		got, err := parseInstallationRole([]byte(role))
		if err != nil {
			t.Fatalf("parseInstallationRole(%q): %v", role, err)
		}
		if got != role {
			t.Fatalf("parseInstallationRole(%q) = %q", role, got)
		}
	}
}

func TestParseInstallationRoleTrimsWhitespace(t *testing.T) {
	got, err := parseInstallationRole([]byte(" supervisor \n"))
	if err != nil {
		t.Fatal(err)
	}
	if got != "supervisor" {
		t.Fatalf("parseInstallationRole() = %q", got)
	}
}

func TestParseInstallationRoleRejectsInvalidValues(t *testing.T) {
	cases := []string{
		"",
		" ",
		"admin",
		"role=supervisor",
		"agent\nsupervisor",
	}
	for _, input := range cases {
		if _, err := parseInstallationRole([]byte(input)); err == nil {
			t.Fatalf("parseInstallationRole(%q) was accepted", input)
		}
	}
}

func TestProvisionRoleImportsValidProvisionedRole(t *testing.T) {
	root := t.TempDir()
	restore := setInstallationRolePaths(
		filepath.Join(root, "efi", "installation-role"),
		filepath.Join(root, "data", "installation-role"),
	)
	defer restore()

	if err := os.MkdirAll(filepath.Join(root, "efi"), 0755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(
		filepath.Join(root, "efi", "installation-role"),
		[]byte("supervisor"),
		0644,
	); err != nil {
		t.Fatal(err)
	}

	if err := provisionRole(); err != nil {
		t.Fatal(err)
	}

	active, err := os.ReadFile(filepath.Join(root, "data", "installation-role"))
	if err != nil {
		t.Fatal(err)
	}
	if string(active) != "supervisor" {
		t.Fatalf("active role = %q", active)
	}
	if _, err := os.Stat(filepath.Join(root, "efi", "installation-role")); !os.IsNotExist(err) {
		t.Fatal("provisioned role file was not removed")
	}
}

func TestProvisionRoleValidatesPersistedRole(t *testing.T) {
	root := t.TempDir()
	restore := setInstallationRolePaths(
		filepath.Join(root, "efi", "installation-role"),
		filepath.Join(root, "data", "installation-role"),
	)
	defer restore()

	if err := os.MkdirAll(filepath.Join(root, "data"), 0755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(
		filepath.Join(root, "data", "installation-role"),
		[]byte("agent"),
		0644,
	); err != nil {
		t.Fatal(err)
	}

	if err := provisionRole(); err != nil {
		t.Fatal(err)
	}
}

func TestProvisionRoleFailsWhenMissing(t *testing.T) {
	root := t.TempDir()
	restore := setInstallationRolePaths(
		filepath.Join(root, "efi", "installation-role"),
		filepath.Join(root, "data", "installation-role"),
	)
	defer restore()

	if err := provisionRole(); err == nil {
		t.Fatal("missing installation role was accepted")
	}
}

func TestProvisionRoleFailsWhenInvalid(t *testing.T) {
	root := t.TempDir()
	restore := setInstallationRolePaths(
		filepath.Join(root, "efi", "installation-role"),
		filepath.Join(root, "data", "installation-role"),
	)
	defer restore()

	if err := os.MkdirAll(filepath.Join(root, "data"), 0755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(
		filepath.Join(root, "data", "installation-role"),
		[]byte("invalid"),
		0644,
	); err != nil {
		t.Fatal(err)
	}

	if err := provisionRole(); err == nil {
		t.Fatal("invalid installation role was accepted")
	}
}

func TestProvisionRoleRecoversInvalidPersistentRoleFromEFI(t *testing.T) {
	root := t.TempDir()
	restore := setInstallationRolePaths(
		filepath.Join(root, "efi", "installation-role"),
		filepath.Join(root, "data", "installation-role"),
	)
	defer restore()

	for _, path := range []string{
		filepath.Join(root, "efi"),
		filepath.Join(root, "data"),
	} {
		if err := os.MkdirAll(path, 0755); err != nil {
			t.Fatal(err)
		}
	}
	if err := os.WriteFile(
		filepath.Join(root, "efi", "installation-role"),
		[]byte("agent"),
		0644,
	); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(
		filepath.Join(root, "data", "installation-role"),
		[]byte{0x46, 0x2a, 0x86, 0x57, 0x85, 0xf3, 0x1b, 0x15, 0x9c, 0xce},
		0644,
	); err != nil {
		t.Fatal(err)
	}

	if err := provisionRole(); err != nil {
		t.Fatal(err)
	}

	active, err := os.ReadFile(filepath.Join(root, "data", "installation-role"))
	if err != nil {
		t.Fatal(err)
	}
	if string(active) != "agent" {
		t.Fatalf("active role = %q", active)
	}
	if _, err := os.Stat(filepath.Join(root, "efi", "installation-role")); !os.IsNotExist(err) {
		t.Fatal("provisioned role file was not removed")
	}
}

func TestProvisionRoleRejectsConflictingProvisionedRole(t *testing.T) {
	root := t.TempDir()
	restore := setInstallationRolePaths(
		filepath.Join(root, "efi", "installation-role"),
		filepath.Join(root, "data", "installation-role"),
	)
	defer restore()

	for _, path := range []string{
		filepath.Join(root, "efi"),
		filepath.Join(root, "data"),
	} {
		if err := os.MkdirAll(path, 0755); err != nil {
			t.Fatal(err)
		}
	}
	if err := os.WriteFile(
		filepath.Join(root, "efi", "installation-role"),
		[]byte("agent"),
		0644,
	); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(
		filepath.Join(root, "data", "installation-role"),
		[]byte("supervisor"),
		0644,
	); err != nil {
		t.Fatal(err)
	}

	if err := provisionRole(); err == nil {
		t.Fatal("conflicting installation role was accepted")
	}
}

func TestProvisionRoleRejectsInvalidProvisionedRole(t *testing.T) {
	root := t.TempDir()
	restore := setInstallationRolePaths(
		filepath.Join(root, "efi", "installation-role"),
		filepath.Join(root, "data", "installation-role"),
	)
	defer restore()

	if err := os.MkdirAll(filepath.Join(root, "efi"), 0755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(
		filepath.Join(root, "efi", "installation-role"),
		[]byte("invalid"),
		0644,
	); err != nil {
		t.Fatal(err)
	}

	if err := provisionRole(); err == nil {
		t.Fatal("invalid provisioned installation role was accepted")
	}
}

func setInstallationRolePaths(provisionedPath, activePath string) func() {
	previousProvisioned := provisionedInstallationRole
	previousActive := activeInstallationRole
	provisionedInstallationRole = provisionedPath
	activeInstallationRole = activePath
	return func() {
		provisionedInstallationRole = previousProvisioned
		activeInstallationRole = previousActive
	}
}
