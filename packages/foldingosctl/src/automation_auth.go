package main

import (
	"fmt"
	"os/user"
)

var currentUnixUsername = func() string {
	info, err := user.Current()
	if err != nil {
		return ""
	}
	return info.Username
}

func isFoldOpsAutomationUser() bool {
	return currentUnixUsername() == "foldops"
}

func requireInspectableRole() error {
	if isFoldOpsAutomationUser() {
		return nil
	}
	role, err := readActiveInstallationRole()
	if err != nil {
		return err
	}
	if role == "agent" || role == "supervisor" {
		return nil
	}
	return fmt.Errorf("operation requires agent or supervisor role, found %q", role)
}

func requireAutomationConfigReadAccess() error {
	return requireInspectableRole()
}
