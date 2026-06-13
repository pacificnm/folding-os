package main

import (
	"bytes"
	"errors"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"regexp"
	"strconv"
	"strings"
)

const (
	dataPartitionNumber = "3"
	dataPartitionName   = "FOLDINGOS_DATA"
	dataPartitionGUID   = "464f4c44-494e-474f-5344-415441000001"
	dataPartitionStart  = uint64(5244928)
	minimumDiskSectors  = uint64(8388608)
	sectorAlignment     = uint64(2048)
)

var (
	lastUsablePattern = regexp.MustCompile(`last usable sector is ([0-9]+)`)
	partitionPattern  = regexp.MustCompile(`(?m)^\s*([0-9]+)\s+([0-9]+)\s+([0-9]+)\s+`)
)

var execCommand = exec.Command

func main() {
	if err := dispatch(os.Args[1:]); err != nil {
		fmt.Fprintf(os.Stderr, "foldingosctl: %v\n", err)
		os.Exit(1)
	}
}

func dispatch(args []string) error {
	if len(args) == 2 && args[0] == "storage" && args[1] == "expand-data" {
		return expandData()
	}
	if len(args) == 2 && args[0] == "identity" && args[1] == "ensure" {
		return ensureIdentity()
	}
	if len(args) == 2 && args[0] == "provision" && args[1] == "ssh" {
		return provisionSSH()
	}
	if len(args) == 2 && args[0] == "provision" && args[1] == "role" {
		return provisionRole()
	}
	if len(args) == 2 && args[0] == "provision" && args[1] == "serve" {
		return provisionServe()
	}
	if len(args) == 2 && args[0] == "provision" && args[1] == "enroll" {
		return provisionEnroll()
	}
	if len(args) == 2 && args[0] == "provision" && args[1] == "check-version" {
		return provisionCheckVersion()
	}
	if len(args) == 2 && args[0] == "provision" && args[1] == "list-enrollments" {
		return provisionListEnrollments()
	}
	if len(args) >= 3 && args[0] == "provision" && args[1] == "assign" {
		return provisionAssign(args[2:])
	}
	if len(args) >= 2 && args[0] == "provision" && args[1] == "install" {
		return provisionInstall(args[2:])
	}
	if len(args) == 2 && args[0] == "registry" && args[1] == "import-bootstrap" {
		return registryImportBootstrap()
	}
	if len(args) == 2 && args[0] == "registry" && args[1] == "poll" {
		return registryPoll()
	}
	if len(args) == 2 && args[0] == "registry" && args[1] == "list" {
		return listRegistry()
	}
	if len(args) == 3 && args[0] == "registry" && args[1] == "show" {
		return showRegistry(args[2])
	}
	if len(args) == 2 && args[0] == "boot" && args[1] == "status" {
		return bootStatus()
	}
	if len(args) == 2 && args[0] == "fah" && args[1] == "validate-manifest" {
		return validateFAHManifestEmbedded()
	}
	if len(args) == 2 && args[0] == "fah" && args[1] == "acquire" {
		return fahAcquire()
	}
	if len(args) == 3 && args[0] == "fah" && args[1] == "verify-install" {
		return fahVerifyInstall(args[2])
	}
	if len(args) == 3 && args[0] == "fah" && args[1] == "activate" {
		return fahActivate(args[2])
	}
	if len(args) == 2 && args[0] == "fah" && args[1] == "prepare" {
		return fahPrepare()
	}
	if len(args) == 2 && args[0] == "fah" && args[1] == "run" {
		return fahRun()
	}
	if len(args) == 3 && args[0] == "config" && args[1] == "validate" {
		return validateConfig(args[2])
	}
	if len(args) == 3 && args[0] == "config" && args[1] == "effective" {
		return printEffectiveConfig(args[2])
	}
	if len(args) == 4 && args[0] == "config" && args[1] == "activate" {
		return activateConfig(args[2], args[3])
	}

	fmt.Fprintln(os.Stderr, "usage: foldingosctl <boot|config|fah|identity|provision|registry|storage> <command> [arguments]")
	os.Exit(2)
	return nil
}

func expandData() error {
	rootSource, err := output("findmnt", "-n", "-o", "SOURCE", "/")
	if err != nil {
		return err
	}
	rootSource = strings.TrimSpace(rootSource)
	if !strings.HasPrefix(rootSource, "/dev/") {
		return fmt.Errorf("root source is not a block device: %q", rootSource)
	}

	parentName, err := output("lsblk", "-n", "-o", "PKNAME", rootSource)
	if err != nil {
		return err
	}
	parentName = strings.TrimSpace(parentName)
	if parentName == "" || strings.Contains(parentName, "/") {
		return fmt.Errorf("could not resolve boot disk from %q", rootSource)
	}
	disk := filepath.Join("/dev", parentName)

	table, err := output("sgdisk", "--print", disk)
	if err != nil {
		return err
	}
	lastUsable, partitions, err := parseTable(table)
	if err != nil {
		return err
	}
	if lastUsable+34 < minimumDiskSectors {
		return errors.New("boot disk is smaller than the release image")
	}
	if err := validateLayout(disk, partitions); err != nil {
		return err
	}

	data := partitions[3]
	dataDevice := partitionDevice(disk, dataPartitionNumber)
	if mounted(dataDevice) {
		return fmt.Errorf("refusing to resize mounted data filesystem %s", dataDevice)
	}
	if err := checkFilesystem(dataDevice, false); err != nil {
		return err
	}

	diskSizeText, err := output("lsblk", "-b", "-d", "-n", "-o", "SIZE", disk)
	if err != nil {
		return err
	}
	diskBytes, err := strconv.ParseUint(strings.TrimSpace(diskSizeText), 10, 64)
	if err != nil {
		return fmt.Errorf("could not determine boot disk size: %w", err)
	}
	diskSectors := diskBytes / 512
	if diskSectors < minimumDiskSectors {
		return errors.New("boot disk is smaller than the release image")
	}
	if diskSectors > lastUsable+34 {
		if err := run("sgdisk", "--move-second-header", disk); err != nil {
			return err
		}
		table, err = output("sgdisk", "--print", disk)
		if err != nil {
			return err
		}
		lastUsable, _, err = parseTable(table)
		if err != nil {
			return err
		}
	}

	targetEnd := alignedEnd(lastUsable)
	if data.end > targetEnd {
		return fmt.Errorf("refusing to shrink data partition from sector %d to %d", data.end, targetEnd)
	}
	if data.end == targetEnd {
		fmt.Println("Data partition already occupies available aligned capacity.")
		return nil
	}

	if err := checkFilesystem(dataDevice, true); err != nil {
		return err
	}

	if err := run(
		"sgdisk",
		"--delete=3",
		fmt.Sprintf("--new=3:%d:%d", dataPartitionStart, targetEnd),
		"--typecode=3:8300",
		"--change-name=3:"+dataPartitionName,
		"--partition-guid=3:"+dataPartitionGUID,
		disk,
	); err != nil {
		return err
	}
	if err := run("partx", "--update", "--nr", dataPartitionNumber, disk); err != nil {
		return err
	}

	if err := run("resize2fs", dataDevice); err != nil {
		return err
	}

	fmt.Printf("Expanded %s to sector %d.\n", dataDevice, targetEnd)
	return nil
}

func checkFilesystem(device string, force bool) error {
	args := []string{"-p"}
	if force {
		args = append([]string{"-f"}, args...)
	}
	args = append(args, device)

	cmd := exec.Command("fsck.ext4", args...)
	cmd.Stdout = os.Stdout
	cmd.Stderr = os.Stderr
	if err := cmd.Run(); err != nil {
		var exitErr *exec.ExitError
		if !errors.As(err, &exitErr) || exitErr.ExitCode() != 1 {
			return fmt.Errorf("fsck.ext4 failed: %w", err)
		}
	}
	if force {
		fmt.Printf("Force-checked %s before data expansion.\n", device)
	} else {
		fmt.Printf("Checked %s before data mount.\n", device)
	}
	return nil
}

type partition struct {
	start uint64
	end   uint64
}

func parseTable(table string) (uint64, map[int]partition, error) {
	lastMatch := lastUsablePattern.FindStringSubmatch(table)
	if len(lastMatch) != 2 {
		return 0, nil, errors.New("could not determine GPT last usable sector")
	}
	lastUsable, err := strconv.ParseUint(lastMatch[1], 10, 64)
	if err != nil {
		return 0, nil, err
	}

	partitions := make(map[int]partition)
	for _, match := range partitionPattern.FindAllStringSubmatch(table, -1) {
		number, _ := strconv.Atoi(match[1])
		start, _ := strconv.ParseUint(match[2], 10, 64)
		end, _ := strconv.ParseUint(match[3], 10, 64)
		partitions[number] = partition{start: start, end: end}
	}
	return lastUsable, partitions, nil
}

func validateLayout(disk string, partitions map[int]partition) error {
	if len(partitions) != 3 {
		return fmt.Errorf("expected exactly three GPT partitions, found %d", len(partitions))
	}
	data, ok := partitions[3]
	if !ok || data.start != dataPartitionStart {
		return fmt.Errorf("unexpected data partition start sector")
	}

	expectedNames := map[int]string{
		1: "FOLDINGOS_EFI",
		2: "FOLDINGOS_ROOT",
		3: dataPartitionName,
	}
	for number, name := range expectedNames {
		info, err := output("sgdisk", fmt.Sprintf("--info=%d", number), disk)
		if err != nil {
			return err
		}
		if !strings.Contains(info, name) {
			return fmt.Errorf("partition %d name does not match approved layout", number)
		}
		if number == 3 && !strings.Contains(strings.ToUpper(info), strings.ToUpper(dataPartitionGUID)) {
			return errors.New("data partition identity does not match approved layout")
		}
	}
	return nil
}

func partitionDevice(disk, number string) string {
	base := filepath.Base(disk)
	if base[len(base)-1] >= '0' && base[len(base)-1] <= '9' {
		return disk + "p" + number
	}
	return disk + number
}

func mounted(device string) bool {
	return exec.Command("findmnt", "-n", device).Run() == nil
}

func alignedEnd(lastUsable uint64) uint64 {
	return ((lastUsable + 1) / sectorAlignment * sectorAlignment) - 1
}

func output(name string, args ...string) (string, error) {
	cmd := exec.Command(name, args...)
	var stdout, stderr bytes.Buffer
	cmd.Stdout = &stdout
	cmd.Stderr = &stderr
	if err := cmd.Run(); err != nil {
		return "", fmt.Errorf("%s failed: %s", name, strings.TrimSpace(stderr.String()))
	}
	return stdout.String(), nil
}

func run(name string, args ...string) error {
	cmd := exec.Command(name, args...)
	cmd.Stdout = os.Stdout
	cmd.Stderr = os.Stderr
	if err := cmd.Run(); err != nil {
		return fmt.Errorf("%s failed: %w", name, err)
	}
	return nil
}
