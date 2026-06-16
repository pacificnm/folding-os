package main

import (
	"net/http"
	"net/http/httptest"
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestBootClientEligibleWithEnrollmentToken(t *testing.T) {
	root := t.TempDir()
	restore := setProvisionBootPaths(root)
	defer restore()
	writeEnrollmentTokenForStreamTest(root, "boot-token")

	if err := isBootClientEligible("52:54:00:12:34:56", "boot-token"); err != nil {
		t.Fatal(err)
	}
}

func TestBootClientEligibleWithAllowlist(t *testing.T) {
	root := t.TempDir()
	restore := setProvisionBootPaths(root)
	defer restore()
	if err := os.MkdirAll(filepath.Join(root, "config", "provision"), 0755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(
		filepath.Join(root, "config", "provision", "boot-allowlist"),
		[]byte("52:54:00:12:34:56\n"),
		0644,
	); err != nil {
		t.Fatal(err)
	}

	if err := isBootClientEligible("52:54:00:12:34:56", ""); err != nil {
		t.Fatal(err)
	}
}

func TestBootClientRejectedWithoutEnrollment(t *testing.T) {
	root := t.TempDir()
	restore := setProvisionBootPaths(root)
	defer restore()

	if err := isBootClientEligible("52:54:00:12:34:56", ""); err == nil {
		t.Fatal("unenrolled client was accepted")
	}
}

func TestRenderIPXEInstallScriptUsesHTTPAssets(t *testing.T) {
	root := t.TempDir()
	restore := setProvisionBootPaths(root)
	defer restore()
	writeEnrollmentTokenForStreamTest(root, "boot-token")
	if err := os.MkdirAll(filepath.Join(root, "config", "provision"), 0755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(
		filepath.Join(root, "config", "provision", "boot-allowlist"),
		[]byte("52:54:00:12:34:56\n"),
		0644,
	); err != nil {
		t.Fatal(err)
	}

	script, err := renderIPXEInstallScript("http://192.168.4.12:8743", "52:54:00:12:34:56", "", "")
	if err != nil {
		t.Fatal(err)
	}
	for _, expected := range []string{
		"http://192.168.4.12:8743/boot/vmlinuz",
		"http://192.168.4.12:8743/boot/install-initramfs.cpio.gz",
		"foldingos.install=1",
		"foldingos.enrollment-token=boot-token",
	} {
		if !strings.Contains(script, expected) {
			t.Fatalf("script missing %q:\n%s", expected, script)
		}
	}
	if strings.Contains(script, "tftp://") {
		t.Fatalf("install script must not use TFTP for image transfer:\n%s", script)
	}
}

func TestProvisionBootHTTPHandlersGateEnrollment(t *testing.T) {
	root := t.TempDir()
	restore := setProvisionBootPaths(root)
	defer restore()

	server := httptest.NewServer(http.HandlerFunc(func(writer http.ResponseWriter, request *http.Request) {
		switch request.URL.Path {
		case "/boot/ipxe/bootstrap.ipxe":
			handleIPXEBootstrap(writer, request)
		case "/boot/ipxe/script.ipxe":
			handleIPXEInstallScript(writer, request)
		default:
			http.NotFound(writer, request)
		}
	}))
	defer server.Close()

	bootstrapResponse, err := http.Get(server.URL + "/boot/ipxe/bootstrap.ipxe")
	if err != nil {
		t.Fatal(err)
	}
	defer bootstrapResponse.Body.Close()
	if bootstrapResponse.StatusCode != http.StatusOK {
		t.Fatalf("bootstrap status = %s", bootstrapResponse.Status)
	}

	rejectedResponse, err := http.Get(server.URL + "/boot/ipxe/script.ipxe?mac=52:54:00:12:34:56")
	if err != nil {
		t.Fatal(err)
	}
	defer rejectedResponse.Body.Close()
	if rejectedResponse.StatusCode != http.StatusForbidden {
		t.Fatalf("rejected status = %s", rejectedResponse.Status)
	}

	writeEnrollmentTokenForStreamTest(root, "boot-token")
	allowedResponse, err := http.Get(server.URL + "/boot/ipxe/script.ipxe?mac=52:54:00:12:34:56&token=boot-token")
	if err != nil {
		t.Fatal(err)
	}
	defer allowedResponse.Body.Close()
	if allowedResponse.StatusCode != http.StatusOK {
		t.Fatalf("allowed status = %s", allowedResponse.Status)
	}
}

func TestRenderDnsmasqConfigUsesIsolatedDHCPWhenEnabled(t *testing.T) {
	root := t.TempDir()
	restore := setProvisionBootPaths(root)
	defer restore()
	if err := os.MkdirAll(filepath.Join(root, "config", "provision"), 0755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(provisionBootIsolatedNetworkPath, []byte("\n"), 0644); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(root, "config", "provision", "boot.interface"), []byte("eth0\n"), 0644); err != nil {
		t.Fatal(err)
	}
	restoreHost := setProvisionBootHostResolver(func(string) (string, error) {
		return "192.168.4.12", nil
	})
	defer restoreHost()
	restoreSubnet := setProvisionBootSubnetResolver(func(string) (string, error) {
		return "192.168.4.0", nil
	})
	defer restoreSubnet()

	config, err := renderDnsmasqConfig()
	if err != nil {
		t.Fatal(err)
	}
	if !strings.Contains(config, "dhcp-range=192.168.4.10,192.168.4.200,255.255.255.0,12h") {
		t.Fatalf("config missing isolated dhcp range:\n%s", config)
	}
	if strings.Contains(config, ",proxy,") {
		t.Fatalf("isolated config must not use proxy DHCP:\n%s", config)
	}
}

func TestRenderDnsmasqConfigUsesProxyDHCPAndTFTPBootstrap(t *testing.T) {
	root := t.TempDir()
	restore := setProvisionBootPaths(root)
	defer restore()
	if err := os.MkdirAll(filepath.Join(root, "config", "provision"), 0755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(root, "config", "provision", "boot.interface"), []byte("eth0\n"), 0644); err != nil {
		t.Fatal(err)
	}
	restoreHost := setProvisionBootHostResolver(func(string) (string, error) {
		return "192.168.4.12", nil
	})
	defer restoreHost()
	restoreSubnet := setProvisionBootSubnetResolver(func(string) (string, error) {
		return "192.168.4.0", nil
	})
	defer restoreSubnet()

	config, err := renderDnsmasqConfig()
	if err != nil {
		t.Fatal(err)
	}
	for _, expected := range []string{
		"dhcp-range=192.168.4.0,proxy",
		"enable-tftp",
		"tftp-root=",
		"dhcp-match=set:ipxe,175",
		"dhcp-boot=tag:ipxe,http://192.168.4.12:8743/boot/ipxe/bootstrap.ipxe,,192.168.4.12",
		"dhcp-boot=tag:efi,tag:!ipxe,ipxe.efi,192.168.4.12,192.168.4.12",
		"dhcp-boot=tag:!ipxe,ipxe.efi,192.168.4.12,192.168.4.12",
		"pxe-service=tag:!ipxe,X86-64_EFI",
	} {
		if !strings.Contains(config, expected) {
			t.Fatalf("config missing %q:\n%s", expected, config)
		}
	}
}

func TestRenderIPXEBootstrapScriptUsesAbsoluteHTTPChain(t *testing.T) {
	script := renderIPXEBootstrapScript("http://192.168.4.17:8743")
	for _, expected := range []string{
		"set foldingos-server http://192.168.4.17:8743",
		"chain http://192.168.4.17:8743/boot/ipxe/script.ipxe?mac=${net0/mac}&arch=${buildarch}",
	} {
		if !strings.Contains(script, expected) {
			t.Fatalf("bootstrap script missing %q:\n%s", expected, script)
		}
	}
	if strings.Contains(script, "http://http://") {
		t.Fatalf("bootstrap script must not double-prefix http://:\n%s", script)
	}
}

func TestPrepareProvisionBootAssetsStagesAutoexecScript(t *testing.T) {
	root := t.TempDir()
	restore := setProvisionBootPaths(root)
	defer restore()

	env := provisionBootEnvironment{
		bootBase: "http://192.168.4.12:8743",
	}
	if err := prepareProvisionBootAssets(env); err != nil {
		t.Fatal(err)
	}
	content, err := os.ReadFile(filepath.Join(root, "boot", "tftp", ipxeAutoexecFilename))
	if err != nil {
		t.Fatal(err)
	}
	if !strings.Contains(string(content), "chain http://192.168.4.12:8743/boot/ipxe/script.ipxe") {
		t.Fatalf("autoexec script missing chain:\n%s", content)
	}
	if strings.Contains(string(content), "http://http://") {
		t.Fatalf("autoexec script must not double-prefix http://:\n%s", content)
	}
}

func TestAddBootAllowlistMAC(t *testing.T) {
	root := t.TempDir()
	restore := setProvisionBootPaths(root)
	defer restore()
	writeSupervisorRole(t, root)

	if err := provisionAllowBoot("00:BE:43:E7:59:5E", ""); err != nil {
		t.Fatal(err)
	}
	content, err := os.ReadFile(filepath.Join(root, "config", "provision", "boot-allowlist"))
	if err != nil {
		t.Fatal(err)
	}
	if string(content) != "00:be:43:e7:59:5e\n" {
		t.Fatalf("allowlist = %q", content)
	}
	if err := isBootClientEligible("00:be:43:e7:59:5e", ""); err != nil {
		t.Fatal(err)
	}
}

func TestAddBootAllowlistMACIsIdempotent(t *testing.T) {
	root := t.TempDir()
	restore := setProvisionBootPaths(root)
	defer restore()
	writeSupervisorRole(t, root)

	if err := provisionAllowBoot("52:54:00:12:34:56", ""); err != nil {
		t.Fatal(err)
	}
	if err := provisionAllowBoot("52-54-00-12-34-56", ""); err != nil {
		t.Fatal(err)
	}
	content, err := os.ReadFile(filepath.Join(root, "config", "provision", "boot-allowlist"))
	if err != nil {
		t.Fatal(err)
	}
	if string(content) != "52:54:00:12:34:56\n" {
		t.Fatalf("allowlist = %q", content)
	}
}

func TestAddBootAllowlistMACRejectsInvalidAddress(t *testing.T) {
	root := t.TempDir()
	restore := setProvisionBootPaths(root)
	defer restore()
	writeSupervisorRole(t, root)

	if err := provisionAllowBoot("not-a-mac", ""); err == nil {
		t.Fatal("invalid MAC was accepted")
	}
}

func TestAddBootAllowlistMACRequiresSupervisorRole(t *testing.T) {
	root := t.TempDir()
	restore := setProvisionBootPaths(root)
	defer restore()

	if err := provisionAllowBoot("52:54:00:12:34:56", ""); err == nil {
		t.Fatal("missing supervisor role was accepted")
	}
}

func TestAddBootAllowlistMACWithInstallDisk(t *testing.T) {
	root := t.TempDir()
	restore := setProvisionBootPaths(root)
	defer restore()
	writeSupervisorRole(t, root)

	if err := provisionAllowBoot("52:54:00:12:34:56", "/dev/sda"); err != nil {
		t.Fatal(err)
	}
	allowlist, err := os.ReadFile(filepath.Join(root, "config", "provision", "boot-allowlist"))
	if err != nil {
		t.Fatal(err)
	}
	if string(allowlist) != "52:54:00:12:34:56\n" {
		t.Fatalf("allowlist = %q", allowlist)
	}
	diskAllowlist, err := os.ReadFile(filepath.Join(root, "config", "provision", "boot-install-disk-allowlist"))
	if err != nil {
		t.Fatal(err)
	}
	if string(diskAllowlist) != "52:54:00:12:34:56 /dev/sda\n" {
		t.Fatalf("install disk allowlist = %q", diskAllowlist)
	}
}

func TestRenderIPXEInstallScriptPinsInstallDiskFromQuery(t *testing.T) {
	root := t.TempDir()
	restore := setProvisionBootPaths(root)
	defer restore()
	writeEnrollmentTokenForStreamTest(root, "boot-token")
	if err := os.MkdirAll(filepath.Join(root, "config", "provision"), 0755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(
		filepath.Join(root, "config", "provision", "boot-allowlist"),
		[]byte("52:54:00:12:34:56\n"),
		0644,
	); err != nil {
		t.Fatal(err)
	}

	script, err := renderIPXEInstallScript(
		"http://192.168.4.12:8743",
		"52:54:00:12:34:56",
		"",
		"/dev/sda",
	)
	if err != nil {
		t.Fatal(err)
	}
	if !strings.Contains(script, "foldingos.install-disk=/dev/sda") {
		t.Fatalf("script missing pinned install disk:\n%s", script)
	}
}

func TestRenderIPXEInstallScriptPinsInstallDiskFromAllowlist(t *testing.T) {
	root := t.TempDir()
	restore := setProvisionBootPaths(root)
	defer restore()
	writeEnrollmentTokenForStreamTest(root, "boot-token")
	if err := os.MkdirAll(filepath.Join(root, "config", "provision"), 0755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(
		filepath.Join(root, "config", "provision", "boot-allowlist"),
		[]byte("52:54:00:12:34:56\n"),
		0644,
	); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(
		filepath.Join(root, "config", "provision", "boot-install-disk-allowlist"),
		[]byte("52:54:00:12:34:56 /dev/nvme0n1\n"),
		0644,
	); err != nil {
		t.Fatal(err)
	}

	script, err := renderIPXEInstallScript("http://192.168.4.12:8743", "52:54:00:12:34:56", "", "")
	if err != nil {
		t.Fatal(err)
	}
	if !strings.Contains(script, "foldingos.install-disk=/dev/nvme0n1") {
		t.Fatalf("script missing allowlist install disk:\n%s", script)
	}
}

func writeSupervisorRole(t *testing.T, root string) {
	t.Helper()
	path := filepath.Join(root, "config", "installation-role")
	if err := os.MkdirAll(filepath.Dir(path), 0755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(path, []byte("supervisor"), 0644); err != nil {
		t.Fatal(err)
	}
	activeInstallationRole = path
	t.Cleanup(func() {
		activeInstallationRole = activeInstallationRoleDefault
	})
}

func setProvisionBootPaths(root string) func() {
	restoreProvision := setProvisionPaths(root)
	provisionBootTFTPRoot = filepath.Join(root, "boot", "tftp")
	provisionBootAllowlistPath = filepath.Join(root, "config", "provision", "boot-allowlist")
	provisionBootInstallDiskAllowlistPath = filepath.Join(root, "config", "provision", "boot-install-disk-allowlist")
	provisionBootInterfacePath = filepath.Join(root, "config", "provision", "boot.interface")
	provisionBootDnsmasqConfig = filepath.Join(root, "config", "provision", "dnsmasq.conf")
	provisionBootIsolatedNetworkPath = filepath.Join(root, "config", "provision", "boot-isolated-network")
	provisionBootAssetsDir = filepath.Join(root, "share", "boot")
	if err := os.MkdirAll(filepath.Join(provisionBootAssetsDir, "ipxe"), 0755); err != nil {
		panic(err)
	}
	if err := os.WriteFile(filepath.Join(provisionBootAssetsDir, "ipxe", ipxeBootstrapFilename), []byte("ipxe"), 0644); err != nil {
		panic(err)
	}
	return func() {
		provisionBootTFTPRoot = provisionBootTFTPRootDefault
		provisionBootAllowlistPath = provisionBootAllowlistPathDefault
		provisionBootInstallDiskAllowlistPath = provisionBootInstallDiskAllowlistPathDefault
		provisionBootInterfacePath = provisionBootInterfacePathDefault
		provisionBootDnsmasqConfig = provisionBootDnsmasqConfigDefault
		provisionBootAssetsDir = provisionBootAssetsDirDefault
		restoreProvision()
	}
}

func setProvisionBootHostResolver(fn func(string) (string, error)) func() {
	previous := resolveProvisionBootHost
	resolveProvisionBootHost = fn
	return func() {
		resolveProvisionBootHost = previous
	}
}

func setProvisionBootSubnetResolver(fn func(string) (string, error)) func() {
	previous := interfaceIPv4Subnet
	interfaceIPv4Subnet = fn
	return func() {
		interfaceIPv4Subnet = previous
	}
}
