package main

import (
	"fmt"
	"os"
	"runtime"
	"strings"
	"syscall"
)

const (
	fahLogPathDefault    = "/data/fah/log.txt"
	fahDBPathDefault     = "/data/fah/client.db"
	fahWorkDirDefault    = "/data/fah/work"
	rebootRequiredPath   = "/run/reboot-required"
)

var (
	fahInspectLogPath = fahLogPathDefault
	fahInspectDBPath  = fahDBPathDefault
	fahInspectWorkDir = fahWorkDirDefault
)

func inspectCommand(subcommand string, args []string) error {
	if len(args) > 0 {
		return automationUsageError(fmt.Sprintf("unknown inspect option %q", args[0]))
	}
	if err := requireInspectableRole(); err != nil {
		return err
	}
	switch subcommand {
	case "node":
		return inspectNode()
	case "system":
		return inspectSystem()
	case "fah":
		return inspectFAH()
	case "commissioning":
		return inspectCommissioning()
	case "update":
		return inspectUpdate()
	case "foldops":
		return inspectFoldOps()
	case "tools":
		return inspectTools()
	default:
		return automationUsageError(fmt.Sprintf("unknown inspect subcommand %q", subcommand))
	}
}

type inspectNodeData struct {
	NodeID            string   `json:"node_id"`
	Hostname          string   `json:"hostname"`
	InstallationRole  string   `json:"installation_role"`
	FoldingOSVersion  string   `json:"foldingos_version"`
	KernelVersion     string   `json:"kernel_version"`
	PrimaryIPv4       *string  `json:"primary_ipv4,omitempty"`
	MACAddresses      []string `json:"mac_addresses"`
}

func inspectNode() error {
	nodeID, err := readNodeID()
	if err != nil {
		return err
	}
	hostname, err := readHostname()
	if err != nil {
		return err
	}
	role, err := readActiveInstallationRole()
	if err != nil {
		return err
	}
	version, err := installedFoldingOSVersionReader()
	if err != nil {
		return err
	}
	macAddresses, err := collectMACAddresses()
	if err != nil {
		return err
	}
	var primaryIPv4 *string
	if address, addressErr := routableIPv4Address(); addressErr == nil {
		primaryIPv4 = &address
	}
	data := inspectNodeData{
		NodeID:           nodeID,
		Hostname:         hostname,
		InstallationRole: role,
		FoldingOSVersion: version,
		KernelVersion:    runtime.Version(),
		PrimaryIPv4:      primaryIPv4,
		MACAddresses:     macAddresses,
	}
	return automationOrHumanSuccess(data, func() error {
		fmt.Printf(
			"node_id=%s hostname=%s role=%s foldingos_version=%s kernel=%s\n",
			data.NodeID,
			data.Hostname,
			data.InstallationRole,
			data.FoldingOSVersion,
			data.KernelVersion,
		)
		if data.PrimaryIPv4 != nil {
			fmt.Printf("primary_ipv4=%s\n", *data.PrimaryIPv4)
		}
		fmt.Printf("mac_addresses=%s\n", strings.Join(data.MACAddresses, ","))
		return nil
	})
}

type inspectCommissioningCheck struct {
	Label string `json:"label"`
	Ready bool   `json:"ready"`
}

type inspectCommissioningData struct {
	InstallationRole string                      `json:"installation_role"`
	AllReady         bool                        `json:"all_ready"`
	Checks           []inspectCommissioningCheck `json:"checks"`
}

func inspectCommissioning() error {
	role := readInstallationRoleForDisplay()
	checks := evaluateCommissioningChecks(role)
	payloadChecks := make([]inspectCommissioningCheck, 0, len(checks))
	allReady := true
	for _, check := range checks {
		if !check.Ready {
			allReady = false
		}
		payloadChecks = append(payloadChecks, inspectCommissioningCheck{
			Label: check.Label,
			Ready: check.Ready,
		})
	}
	data := inspectCommissioningData{
		InstallationRole: role,
		AllReady:         allReady,
		Checks:           payloadChecks,
	}
	return automationOrHumanSuccess(data, func() error {
		printCommissioningStatusSummary(checks)
		return nil
	})
}

func readRootFilesystemUsage() (total, used, free uint64, percent float64, err error) {
	var stat syscall.Statfs_t
	if err = syscall.Statfs("/", &stat); err != nil {
		return 0, 0, 0, 0, err
	}
	blockSize := uint64(stat.Bsize)
	total = stat.Blocks * blockSize
	free = stat.Bavail * blockSize
	if total <= free {
		used = 0
	} else {
		used = total - free
	}
	if total > 0 {
		percent = float64(int64((float64(used)/float64(total))*1000+0.5)) / 10.0
	}
	return total, used, free, percent, nil
}

func rebootRequired() bool {
	_, err := os.Stat(rebootRequiredPath)
	return err == nil
}
