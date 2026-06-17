package main

import (
	"errors"
	"fmt"
	"sort"
	"strings"
)

func provisionAssign(args []string) error {
	var nodeID string
	var imageVersion string
	var foldOpsManifestRelease string
	var toolsVersion string
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
			imageVersion = args[i+1]
			i++
		case "--foldops-manifest":
			if i+1 >= len(args) {
				return errors.New("missing value for --foldops-manifest")
			}
			foldOpsManifestRelease = args[i+1]
			i++
		case "--tools-version":
			if i+1 >= len(args) {
				return errors.New("missing value for --tools-version")
			}
			toolsVersion = args[i+1]
			i++
		default:
			return fmt.Errorf("unknown assign option %q", args[i])
		}
	}
	if all && nodeID != "" {
		return errors.New("use either --all or --node, not both")
	}
	if !all && nodeID == "" {
		return errors.New("assignment requires --all or --node")
	}

	update := softwareAssignmentUpdate{}
	if imageVersion != "" {
		update.imageVersion = &imageVersion
	}
	if foldOpsManifestRelease != "" {
		update.foldOpsManifestRelease = &foldOpsManifestRelease
	}
	if toolsVersion != "" {
		update.toolsVersion = &toolsVersion
	}
	if update.imageVersion == nil && update.foldOpsManifestRelease == nil && update.toolsVersion == nil {
		return errNoAssignmentFields
	}

	scope := "node"
	if all {
		scope = "fleet"
	}
	updated, err := assignSoftwareVersions(scope, nodeID, update)
	if err != nil {
		return err
	}

	parts := make([]string, 0, 3)
	if update.imageVersion != nil {
		parts = append(parts, fmt.Sprintf("image=%q", strings.TrimSpace(*update.imageVersion)))
	}
	if update.foldOpsManifestRelease != nil {
		parts = append(parts, fmt.Sprintf("foldops=%q", strings.TrimSpace(*update.foldOpsManifestRelease)))
	}
	if update.toolsVersion != nil {
		parts = append(parts, fmt.Sprintf("tools=%q", strings.TrimSpace(*update.toolsVersion)))
	}
	fmt.Printf("Assigned %s to %d enrolled agent(s).\n", strings.Join(parts, ", "), updated)
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
			"%s\t%s\tcurrent=%s\tdesired=%s\tfoldops=%s\ttools=%s\n",
			record.NodeID,
			record.Hostname,
			record.CurrentImageVersion,
			record.DesiredImageVersion,
			displayAssignmentLabel(record.DesiredFoldOpsManifestRelease),
			displayAssignmentLabel(record.DesiredToolsVersion),
		)
	}
	return nil
}

func displayAssignmentLabel(value string) string {
	value = strings.TrimSpace(value)
	if value == "" {
		return "bootstrap"
	}
	return value
}
