package main

import (
	"errors"
	"fmt"
	"os"
	"strings"
)

const (
	provisionedInstallationRoleDefault = "/boot/efi/foldingos/provision/installation-role"
	activeInstallationRoleDefault      = "/data/config/installation-role"
)

var (
	provisionedInstallationRole = provisionedInstallationRoleDefault
	activeInstallationRole      = activeInstallationRoleDefault
)

var validInstallationRoles = map[string]struct{}{
	"agent":      {},
	"supervisor": {},
}

func provisionRole() error {
	provisioned, provisionedErr := os.ReadFile(provisionedInstallationRole)
	if provisionedErr != nil && !os.IsNotExist(provisionedErr) {
		return provisionedErr
	}

	activeContent, activeErr := os.ReadFile(activeInstallationRole)
	if activeErr != nil && !os.IsNotExist(activeErr) {
		return activeErr
	}

	if provisionedErr == nil {
		role, err := parseInstallationRole(provisioned)
		if err != nil {
			return fmt.Errorf("provisioned installation role is invalid: %w", err)
		}
		if activeErr == nil {
			activeRole, err := parseInstallationRole(activeContent)
			if err != nil {
				return fmt.Errorf("persistent installation role is invalid: %w", err)
			}
			if activeRole != role {
				return fmt.Errorf(
					"provisioned installation role %q conflicts with persisted role %q",
					role,
					activeRole,
				)
			}
			if err := os.Remove(provisionedInstallationRole); err != nil {
				return err
			}
			fmt.Printf("Installation role %q is already persisted.\n", role)
			return nil
		}
		if err := atomicWrite(activeInstallationRole, []byte(role), 0644); err != nil {
			return err
		}
		if err := os.Remove(provisionedInstallationRole); err != nil {
			return err
		}
		fmt.Printf("Activated provisioned installation role %q.\n", role)
		return nil
	}

	if activeErr != nil {
		return errors.New("installation role is not provisioned")
	}

	role, err := parseInstallationRole(activeContent)
	if err != nil {
		return fmt.Errorf("persistent installation role is invalid: %w", err)
	}
	fmt.Printf("Validated installation role %q.\n", role)
	return nil
}

func parseInstallationRole(content []byte) (string, error) {
	role := strings.TrimSpace(string(content))
	if role == "" {
		return "", errors.New("installation role is empty")
	}
	if strings.Contains(role, "\n") {
		return "", errors.New("installation role must be a single line")
	}
	if _, ok := validInstallationRoles[role]; !ok {
		return "", fmt.Errorf("unsupported installation role %q", role)
	}
	return role, nil
}
