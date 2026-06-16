package main

import (
	"encoding/json"
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"regexp"
	"strings"
)

var provisionInstallDiskPathPattern = regexp.MustCompile(`^/dev/(sd[a-z]+|vd[a-z]+|nvme[0-9]+n[0-9]+)$`)

type provisionTargetDisk struct {
	Path        string
	SizeBytes   int64
	Serial      string
	Transport   string
	Removable   bool
	DeviceType  string
}

var (
	inspectProvisionTargetDisk = inspectProvisionTargetDiskFromSystem
	resolveHostBootDisk        = resolveHostBootDiskFromSystem
	listMountedBlockDevices    = listMountedBlockDevicesFromSystem
	provisionTargetSysfsRoot   = "/sys"
)

func validateProvisionTargetDisk(path string) (provisionTargetDisk, error) {
	disk, err := inspectProvisionTargetDisk(path)
	if err != nil {
		return provisionTargetDisk{}, err
	}
	if disk.DeviceType != "disk" {
		return provisionTargetDisk{}, fmt.Errorf("target %q is not a whole disk", path)
	}
	if disk.Removable {
		return provisionTargetDisk{}, fmt.Errorf("target %q is removable and is not eligible for network provisioning", path)
	}
	if !isEligibleProvisionTransport(disk) {
		return provisionTargetDisk{}, fmt.Errorf(
			"target %q uses transport %q; only internal SATA or NVMe targets are eligible",
			path,
			disk.Transport,
		)
	}
	if strings.TrimSpace(disk.Serial) == "" {
		return provisionTargetDisk{}, errors.New("target disk serial number is required")
	}
	if disk.SizeBytes < releaseImageSizeBytes {
		return provisionTargetDisk{}, fmt.Errorf(
			"target %q is too small (%d bytes); release image requires %d bytes",
			path,
			disk.SizeBytes,
			releaseImageSizeBytes,
		)
	}
	bootDisk, err := resolveHostBootDisk()
	if err != nil {
		return provisionTargetDisk{}, err
	}
	if bootDisk != "" && disk.Path == bootDisk {
		return provisionTargetDisk{}, fmt.Errorf("refusing to provision the host boot disk %q", path)
	}
	mounted, err := listMountedBlockDevices()
	if err != nil {
		return provisionTargetDisk{}, err
	}
	for _, device := range mounted {
		if device == disk.Path || strings.HasPrefix(device, disk.Path) {
			return provisionTargetDisk{}, fmt.Errorf("target %q has mounted filesystems", path)
		}
	}
	return disk, nil
}

func isEligibleProvisionTransport(disk provisionTargetDisk) bool {
	transport := strings.ToLower(strings.TrimSpace(disk.Transport))
	name := strings.ToLower(filepath.Base(disk.Path))
	switch {
	case transport == "usb":
		return false
	case transport == "sata", transport == "ata":
		return true
	case transport == "nvme":
		return true
	case strings.Contains(name, "nvme"):
		return true
	case strings.HasPrefix(name, "sd") && len(name) >= 3:
		// Internal SCSI/SATA disks may report an empty TRAN in the install initramfs.
		return transport == "" || transport == "sata" || transport == "ata"
	case strings.HasPrefix(name, "vd") && len(name) >= 3:
		return transport == "" || transport == "sata" || transport == "ata"
	default:
		return false
	}
}

func parseProvisionInstallDiskPath(path string) (string, error) {
	path = strings.TrimSpace(path)
	if path == "" {
		return "", errors.New("install disk path is empty")
	}
	if !provisionInstallDiskPathPattern.MatchString(path) {
		return "", fmt.Errorf(
			"install disk must be a whole-disk device path such as /dev/sda or /dev/nvme0n1: %q",
			path,
		)
	}
	return path, nil
}

func inspectProvisionTargetDiskFromSystem(path string) (provisionTargetDisk, error) {
	path = strings.TrimSpace(path)
	if path == "" {
		return provisionTargetDisk{}, errors.New("target disk path is required")
	}
	if !strings.HasPrefix(path, "/dev/") {
		return provisionTargetDisk{}, fmt.Errorf("target disk must be a block device path: %q", path)
	}
	if strings.Contains(filepath.Base(path), "/") {
		return provisionTargetDisk{}, fmt.Errorf("target disk must be a whole-disk device, not a partition: %q", path)
	}
	info, err := os.Stat(path)
	if err != nil {
		return provisionTargetDisk{}, err
	}
	if info.Mode()&os.ModeDevice == 0 {
		return provisionTargetDisk{}, fmt.Errorf("%q is not a block device", path)
	}

	listing, err := output("lsblk", "-J", "-b", "-d", "-o", "NAME,TYPE,TRAN,SIZE,SERIAL,RM,PATH")
	if err != nil {
		return provisionTargetDisk{}, err
	}
	var parsed struct {
		BlockDevices []struct {
			Name   string `json:"name"`
			Type   string `json:"type"`
			Tran   string `json:"tran"`
			Size   int64  `json:"size"`
			Serial string `json:"serial"`
			RM     bool   `json:"rm"`
			Path   string `json:"path"`
		} `json:"blockdevices"`
	}
	if err := json.Unmarshal([]byte(listing), &parsed); err != nil {
		return provisionTargetDisk{}, fmt.Errorf("parse lsblk output: %w", err)
	}
	for _, device := range parsed.BlockDevices {
		devicePath := device.Path
		if devicePath == "" {
			devicePath = "/dev/" + device.Name
		}
		if devicePath != path {
			continue
		}
		serial := strings.TrimSpace(device.Serial)
		if serial == "" {
			serial = readProvisionTargetDiskSerialFromSysfs(devicePath)
		}
		return provisionTargetDisk{
			Path:       devicePath,
			SizeBytes:  device.Size,
			Serial:     serial,
			Transport:  strings.TrimSpace(device.Tran),
			Removable:  device.RM,
			DeviceType: strings.TrimSpace(device.Type),
		}, nil
	}
	return provisionTargetDisk{}, fmt.Errorf("target disk %q was not found", path)
}

func readProvisionTargetDiskSerialFromSysfs(devicePath string) string {
	name := strings.TrimSpace(filepath.Base(devicePath))
	if name == "" {
		return ""
	}
	candidates := []string{
		filepath.Join(provisionTargetSysfsRoot, "block", name, "device", "serial"),
	}
	if strings.HasPrefix(name, "nvme") {
		controller := name
		if idx := strings.LastIndex(name, "n"); idx > len("nvme") {
			controller = name[:idx]
		}
		candidates = append(
			candidates,
			filepath.Join(provisionTargetSysfsRoot, "class", "nvme", controller, "serial"),
		)
	}
	for _, candidate := range candidates {
		content, err := os.ReadFile(candidate)
		if err != nil {
			continue
		}
		serial := strings.TrimSpace(string(content))
		if serial != "" {
			return serial
		}
	}
	return ""
}

func resolveHostBootDiskFromSystem() (string, error) {
	rootSource, err := output("findmnt", "-n", "-o", "SOURCE", "/")
	if err != nil {
		return "", nil
	}
	rootSource = strings.TrimSpace(rootSource)
	if !strings.HasPrefix(rootSource, "/dev/") {
		return "", nil
	}
	parentName, err := output("lsblk", "-n", "-o", "PKNAME", rootSource)
	if err != nil {
		return "", err
	}
	parentName = strings.TrimSpace(parentName)
	if parentName == "" || strings.Contains(parentName, "/") {
		return "", nil
	}
	return filepath.Join("/dev", parentName), nil
}

func listMountedBlockDevicesFromSystem() ([]string, error) {
	listing, err := output("findmnt", "-rn", "-o", "SOURCE")
	if err != nil {
		return nil, err
	}
	var devices []string
	for _, line := range strings.Split(strings.TrimSpace(listing), "\n") {
		line = strings.TrimSpace(line)
		if line == "" || !strings.HasPrefix(line, "/dev/") {
			continue
		}
		devices = append(devices, line)
	}
	return devices, nil
}

func efiPartitionPath(disk string) string {
	return partitionDevice(disk, "1")
}

func selectProvisionInstallDisk() (string, error) {
	listing, err := output("lsblk", "-J", "-b", "-d", "-o", "NAME,TYPE,TRAN,SIZE,SERIAL,RM,PATH")
	if err != nil {
		return "", err
	}
	var parsed struct {
		BlockDevices []struct {
			Name   string `json:"name"`
			Type   string `json:"type"`
			Tran   string `json:"tran"`
			Size   int64  `json:"size"`
			Serial string `json:"serial"`
			RM     bool   `json:"rm"`
			Path   string `json:"path"`
		} `json:"blockdevices"`
	}
	if err := json.Unmarshal([]byte(listing), &parsed); err != nil {
		return "", fmt.Errorf("parse lsblk output: %w", err)
	}
	for _, device := range parsed.BlockDevices {
		path := device.Path
		if path == "" {
			path = "/dev/" + device.Name
		}
		if _, err := validateProvisionTargetDisk(path); err == nil {
			return path, nil
		}
	}
	return "", errors.New("no eligible internal target disk was found")
}
