package main

import (
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"testing"
)

func TestParseTable(t *testing.T) {
	table := `Disk /dev/vda: 8388608 sectors
First usable sector is 34, last usable sector is 8388574
Number  Start (sector)    End (sector)  Size       Code  Name
   1            2048         1050623   512.0 MiB   EF00  FOLDINGOS_EFI
   2         1050624         5244927   2.0 GiB     8304  FOLDINGOS_ROOT
   3         5244928         8386559   1.5 GiB     8300  FOLDINGOS_DATA
`

	lastUsable, partitions, err := parseTable(table)
	if err != nil {
		t.Fatal(err)
	}
	if lastUsable != 8388574 {
		t.Fatalf("last usable sector = %d", lastUsable)
	}
	if partitions[3].start != dataPartitionStart || partitions[3].end != 8386559 {
		t.Fatalf("unexpected data partition: %+v", partitions[3])
	}
}

func TestAlignedEnd(t *testing.T) {
	if got := alignedEnd(8388574); got != 8386559 {
		t.Fatalf("aligned end = %d", got)
	}
	if got := alignedEnd(12582878); got != 12580863 {
		t.Fatalf("larger aligned end = %d", got)
	}
}

func TestPartitionDevice(t *testing.T) {
	tests := map[string]string{
		"/dev/vda":     "/dev/vda3",
		"/dev/sda":     "/dev/sda3",
		"/dev/nvme0n1": "/dev/nvme0n1p3",
		"/dev/mmcblk0": "/dev/mmcblk0p3",
	}
	for disk, expected := range tests {
		if got := partitionDevice(disk, dataPartitionNumber); got != expected {
			t.Fatalf("partitionDevice(%q) = %q, want %q", disk, got, expected)
		}
	}
}

func TestParseAndRenderSystemConfig(t *testing.T) {
	config := "schema_version = 1\n\n[identity]\nhostname = \"folding-node\"\n"
	values, err := parseDomain("system", config, true)
	if err != nil {
		t.Fatal(err)
	}
	if got := renderDomain("system", values); got != config {
		t.Fatalf("rendered config:\n%s", got)
	}
}

func TestRejectUnknownConfigKey(t *testing.T) {
	config := "schema_version = 1\n\n[ethernet]\ndhcp = true\nrequired_for_online = true\naddress = \"192.0.2.1\"\n"
	if _, err := parseDomain("network", config, true); err == nil {
		t.Fatal("unknown network key was accepted")
	}
}

func TestRejectUnsupportedFoldingHomeConfig(t *testing.T) {
	config := domainConfig{
		"schema_version":          {kind: "int", ival: 1},
		"identity.username":       {kind: "string", text: "Anonymous"},
		"identity.team":           {kind: "int", ival: 0},
		"identity.passkey_secret": {kind: "string", text: "../secret"},
		"resources.cpus":          {kind: "int", ival: 0},
		"resources.gpus":          {kind: "bool", bval: false},
	}
	if err := validateDomain("foldinghome", config); err == nil {
		t.Fatal("unsafe secret reference was accepted")
	}
}

func TestNewUUID(t *testing.T) {
	value, err := newUUID()
	if err != nil {
		t.Fatal(err)
	}
	if !uuidPattern.MatchString(value) {
		t.Fatalf("invalid UUIDv4: %s", value)
	}
}

func TestValidateAuthorizedKeysAcceptsCompleteSupportedKeySet(t *testing.T) {
	ed25519 := generateTestPublicKey(t, "ed25519", "")
	ecdsa := generateTestPublicKey(t, "ecdsa", "256")
	content := []byte("# complete replacement set\n" + ed25519 + "\n\n" + ecdsa + "\n")

	got, err := validateAuthorizedKeys(content)
	if err != nil {
		t.Fatal(err)
	}
	want := ed25519 + "\n" + ecdsa + "\n"
	if string(got) != want {
		t.Fatalf("validated keys = %q, want %q", got, want)
	}
}

func TestValidateAuthorizedKeysRejectsPrivateKeyMaterial(t *testing.T) {
	privateKey := generateTestPrivateKey(t)
	if _, err := validateAuthorizedKeys(privateKey); err == nil {
		t.Fatal("private key material was accepted")
	}
}

func TestValidateAuthorizedKeysRejectsOptions(t *testing.T) {
	key := generateTestPublicKey(t, "ed25519", "")
	if _, err := validateAuthorizedKeys([]byte("no-port-forwarding " + key + "\n")); err == nil {
		t.Fatal("option-prefixed key was accepted")
	}
}

func generateTestPublicKey(t *testing.T, keyType, bits string) string {
	t.Helper()
	privatePath := filepath.Join(t.TempDir(), "key")
	args := []string{"-q", "-t", keyType, "-N", "", "-f", privatePath}
	if bits != "" {
		args = append(args, "-b", bits)
	}
	if output, err := exec.Command("ssh-keygen", args...).CombinedOutput(); err != nil {
		t.Fatalf("generate %s key: %v: %s", keyType, err, output)
	}
	content, err := os.ReadFile(privatePath + ".pub")
	if err != nil {
		t.Fatal(err)
	}
	return strings.TrimSpace(string(content))
}

func generateTestPrivateKey(t *testing.T) []byte {
	t.Helper()
	privatePath := filepath.Join(t.TempDir(), "key")
	if output, err := exec.Command("ssh-keygen", "-q", "-t", "ed25519", "-N", "", "-f", privatePath).CombinedOutput(); err != nil {
		t.Fatalf("generate private key: %v: %s", err, output)
	}
	content, err := os.ReadFile(privatePath)
	if err != nil {
		t.Fatal(err)
	}
	return content
}
