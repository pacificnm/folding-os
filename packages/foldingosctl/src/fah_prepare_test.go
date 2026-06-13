package main

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestRenderFAHConfigXMLDeterministic(t *testing.T) {
	config := domainConfig{
		"identity.username": {kind: "string", text: "Test User"},
		"identity.team":     {kind: "int", ival: 12345},
		"resources.cpus":    {kind: "int", ival: 0},
	}
	expected := strings.Join([]string{
		"<config>",
		`  <user v="Test User"/>`,
		`  <team v="12345"/>`,
		`  <cpus v="0"/>`,
		"</config>",
		"",
	}, "\n")
	if got := renderFAHConfigXML(config, ""); got != expected {
		t.Fatalf("rendered config:\n%s", got)
	}
}

func TestRenderFAHConfigXMLIncludesPasskey(t *testing.T) {
	config := domainConfig{
		"identity.username": {kind: "string", text: "Anonymous"},
		"identity.team":     {kind: "int", ival: 0},
		"resources.cpus":    {kind: "int", ival: 2},
	}
	passkey := "abcdef0123456789abcdef0123456789"
	rendered := renderFAHConfigXML(config, passkey)
	if !strings.Contains(rendered, `<passkey v="abcdef0123456789abcdef0123456789"/>`) {
		t.Fatalf("rendered config missing passkey:\n%s", rendered)
	}
}

func TestRenderFAHConfigXMLEscapesAttributeValues(t *testing.T) {
	config := domainConfig{
		"identity.username": {kind: "string", text: `User "A" & <B>`},
		"identity.team":     {kind: "int", ival: 0},
		"resources.cpus":    {kind: "int", ival: 1},
	}
	rendered := renderFAHConfigXML(config, "")
	if !strings.Contains(rendered, `User &quot;A&quot; &amp; &lt;B&gt;`) {
		t.Fatalf("rendered config did not escape attribute values:\n%s", rendered)
	}
}

func TestReadFAHPasskeyRejectsInvalidSecret(t *testing.T) {
	secretsDir := filepath.Join(t.TempDir(), "secrets")
	if err := os.MkdirAll(secretsDir, 0700); err != nil {
		t.Fatal(err)
	}
	secretPath := filepath.Join(secretsDir, "fah-passkey")
	if err := os.WriteFile(secretPath, []byte("not-a-valid-passkey\n"), 0640); err != nil {
		t.Fatal(err)
	}

	restoreConfigDir := setConfigDir(filepath.Dir(secretsDir))
	defer restoreConfigDir()
	restoreSecretValidation := setValidateSecretReference(func(string) error { return nil })
	defer restoreSecretValidation()

	if _, err := readFAHPasskey("fah-passkey"); err == nil {
		t.Fatal("invalid passkey secret was accepted")
	}
}

func TestLoadFAHRuntimeConfigurationRejectsInvalidActiveConfig(t *testing.T) {
	root := t.TempDir()
	defaultsDir := filepath.Join(root, "defaults")
	configRoot := filepath.Join(root, "config")
	if err := os.MkdirAll(defaultsDir, 0755); err != nil {
		t.Fatal(err)
	}
	if err := os.MkdirAll(configRoot, 0755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(defaultsDir, "foldinghome.toml"), []byte(validFoldingHomeDefaultTOML()), 0644); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(configRoot, "foldinghome.toml"), []byte(`schema_version = 1

[identity]
username = "Anonymous"
team = 0
passkey_secret = ""

[resources]
cpus = 0
gpus = true
`), 0644); err != nil {
		t.Fatal(err)
	}

	restoreDefaults := setDefaultsDir(defaultsDir)
	defer restoreDefaults()
	restoreConfig := setConfigDir(configRoot)
	defer restoreConfig()

	if _, _, err := loadFAHRuntimeConfiguration(); err == nil {
		t.Fatal("invalid active configuration was accepted")
	}
}

func TestFAHPrepareWritesRuntimeConfig(t *testing.T) {
	appsRoot := t.TempDir()
	runtimeDir := filepath.Join(t.TempDir(), "fah-runtime")
	configRoot := setupFoldingHomeConfigRoot(t)

	versionDir := filepath.Join(appsRoot, "8.5.6")
	if err := os.MkdirAll(filepath.Join(versionDir, "usr", "bin"), 0755); err != nil {
		t.Fatal(err)
	}
	executable := filepath.Join(versionDir, "usr", "bin", "fah-client")
	if err := copyFile("/tmp/fah-manifest-work/deb-inspect/data/usr/bin/fah-client", executable); err != nil {
		t.Skip("approved FAH executable is unavailable for prepare test")
	}
	if err := os.Chmod(executable, 0755); err != nil {
		t.Fatal(err)
	}
	marker := "client_version=8.5.6\nartifact_sha256=643de04033a1cb972a81e3a193d710e919a4f34634a987f11adc4cee61fdaefe\n"
	if err := os.WriteFile(filepath.Join(versionDir, fahVerifiedMarkerName), []byte(marker), 0644); err != nil {
		t.Fatal(err)
	}
	if err := os.Symlink("8.5.6", filepath.Join(appsRoot, "current")); err != nil {
		t.Fatal(err)
	}

	restoreApps := setFAHAppsRoot(appsRoot)
	defer restoreApps()
	restoreRuntime := setFAHRuntimePaths(runtimeDir)
	defer restoreRuntime()
	restoreDefaults := setDefaultsDir(filepath.Join(configRoot, "defaults"))
	defer restoreDefaults()
	restoreConfig := setConfigDir(filepath.Join(configRoot, "config"))
	defer restoreConfig()
	restoreManifest := setFAHApprovedManifestLoader(testFAHManifestLoader(t))
	defer restoreManifest()
	restoreCompatibility := setFAHFoldingOSCompatibilityCheck(func(string) error { return nil })
	defer restoreCompatibility()

	if err := fahPrepare(); err != nil {
		t.Fatal(err)
	}
	content, err := os.ReadFile(filepath.Join(runtimeDir, "config.xml"))
	if err != nil {
		t.Fatal(err)
	}
	rendered := string(content)
	if !strings.Contains(rendered, `<user v="Anonymous"/>`) {
		t.Fatalf("runtime config missing username:\n%s", rendered)
	}
	if strings.Contains(rendered, "top-secret-passkey-value") {
		t.Fatalf("runtime config leaked unexpected secret value:\n%s", rendered)
	}
}

func validFoldingHomeDefaultTOML() string {
	return `schema_version = 1

[identity]
username = "Anonymous"
team = 0
passkey_secret = ""

[resources]
cpus = 0
gpus = false
`
}

func setupFoldingHomeConfigRoot(t *testing.T) string {
	t.Helper()
	root := t.TempDir()
	defaultsDir := filepath.Join(root, "defaults")
	configDir := filepath.Join(root, "config")
	if err := os.MkdirAll(defaultsDir, 0755); err != nil {
		t.Fatal(err)
	}
	if err := os.MkdirAll(configDir, 0755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(defaultsDir, "foldinghome.toml"), []byte(validFoldingHomeDefaultTOML()), 0644); err != nil {
		t.Fatal(err)
	}
	return root
}

func setDefaultsDir(path string) func() {
	previous := defaultsDir
	defaultsDir = path
	return func() {
		defaultsDir = previous
	}
}

func setConfigDir(path string) func() {
	previous := configDir
	configDir = path
	return func() {
		configDir = previous
	}
}

func setValidateSecretReference(check func(string) error) func() {
	previous := validateSecretReferenceFn
	validateSecretReferenceFn = check
	return func() {
		validateSecretReferenceFn = previous
	}
}

func setFAHRuntimePaths(dir string) func() {
	previousDir := fahRuntimeDir
	previousConfig := fahRuntimeConfigPath
	fahRuntimeDir = dir
	fahRuntimeConfigPath = filepath.Join(dir, "config.xml")
	return func() {
		fahRuntimeDir = previousDir
		fahRuntimeConfigPath = previousConfig
	}
}
