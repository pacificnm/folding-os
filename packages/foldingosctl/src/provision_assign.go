package main

import (
	"errors"
	"fmt"
	"sort"
)

func provisionAssign(args []string) error {
	var nodeID string
	var version string
	all := false

	for i := 0; i < len(args); i++ {
		switch args[i] {
		case "--node":
			if i+1 >= len(args) {
				return errors.New("missing value for --node")
			}
			nodeID = args[i+1]
			i++
		case "--all":
			all = true
		case "--version":
			if i+1 >= len(args) {
				return errors.New("missing value for --version")
			}
			version = args[i+1]
			i++
		default:
			return fmt.Errorf("unknown assign option %q", args[i])
		}
	}
	if version == "" {
		return errors.New("assigned version is required")
	}
	if all && nodeID != "" {
		return errors.New("use either --all or --node, not both")
	}
	if !all && nodeID == "" {
		return errors.New("assignment requires --all or --node")
	}

	scope := "node"
	if all {
		scope = "fleet"
	}
	updated, err := assignDesiredVersion(scope, nodeID, version)
	if err != nil {
		return err
	}
	fmt.Printf("Assigned desired image version %q to %d enrolled agent(s).\n", version, updated)
	return nil
}

func provisionListEnrollments() error {
	if err := requireSupervisorRole(); err != nil {
		return err
	}
	index, err := loadEnrollmentIndex()
	if err != nil {
		return err
	}
	if len(index.NodeIDs) == 0 {
		fmt.Println("No enrolled agents.")
		return nil
	}
	nodeIDs := append([]string(nil), index.NodeIDs...)
	sort.Strings(nodeIDs)
	for _, nodeID := range nodeIDs {
		record, err := loadEnrollmentRecord(nodeID)
		if err != nil {
			return err
		}
		fmt.Printf(
			"%s\t%s\tcurrent=%s\tdesired=%s\n",
			record.NodeID,
			record.Hostname,
			record.CurrentImageVersion,
			record.DesiredImageVersion,
		)
	}
	return nil
}
