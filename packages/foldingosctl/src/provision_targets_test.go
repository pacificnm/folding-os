package main

import (
	"os"
	"path/filepath"
	"testing"
)

func TestParseProvisionInstallDiskPathAcceptsWholeDiskDevices(t *testing.T) {
	for _, path := range []string{"/dev/sda", "/dev/vdb", "/dev/nvme0n1"} {
		got, err := parseProvisionInstallDiskPath(path)
		if err != nil {
			t.Fatalf("parseProvisionInstallDiskPath(%q) = %v", path, err)
		}
		if got != path {
			t.Fatalf("parseProvisionInstallDiskPath(%q) = %q", path, got)
		}
	}
}

func TestParseProvisionInstallDiskPathRejectsPartitions(t *testing.T) {
	if _, err := parseProvisionInstallDiskPath("/dev/sda1"); err == nil {
		t.Fatal("partition path was accepted")
	}
}

func TestIsEligibleProvisionTransportAcceptsInternalSATAWithEmptyTRAN(t *testing.T) {
	disk := provisionTargetDisk{
		Path:      "/dev/sda",
		Transport: "",
	}
	if !isEligibleProvisionTransport(disk) {
		t.Fatal("internal sd* disk with empty TRAN was rejected")
	}
}

func TestIsEligibleProvisionTransportRejectsUSB(t *testing.T) {
	disk := provisionTargetDisk{
		Path:      "/dev/sda",
		Transport: "usb",
	}
	if isEligibleProvisionTransport(disk) {
		t.Fatal("USB disk was accepted")
	}
}

func TestReadProvisionTargetDiskSerialFromSysfs(t *testing.T) {
	root := t.TempDir()
	if err := os.MkdirAll(filepath.Join(root, "block", "sda", "device"), 0755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(
		filepath.Join(root, "block", "sda", "device", "serial"),
		[]byte("CT1000XYZ\n"),
		0644,
	); err != nil {
		t.Fatal(err)
	}
	previous := provisionTargetSysfsRoot
	provisionTargetSysfsRoot = root
	t.Cleanup(func() {
		provisionTargetSysfsRoot = previous
	})

	got := readProvisionTargetDiskSerialFromSysfs("/dev/sda")
	if got != "CT1000XYZ" {
		t.Fatalf("serial = %q", got)
	}
}

func TestReadProvisionTargetDiskSerialFromSysfsNVMe(t *testing.T) {
	root := t.TempDir()
	if err := os.MkdirAll(filepath.Join(root, "class", "nvme", "nvme0"), 0755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(
		filepath.Join(root, "class", "nvme", "nvme0", "serial"),
		[]byte("SN512345678\n"),
		0644,
	); err != nil {
		t.Fatal(err)
	}
	previous := provisionTargetSysfsRoot
	provisionTargetSysfsRoot = root
	t.Cleanup(func() {
		provisionTargetSysfsRoot = previous
	})

	got := readProvisionTargetDiskSerialFromSysfs("/dev/nvme0n1")
	if got != "SN512345678" {
		t.Fatalf("serial = %q", got)
	}
}
