package main

import (
	"fmt"
	"os"
	"os/user"
	"strconv"
	"strings"
)

const foldOpsSupervisorAutomationPolicyPathDefault = "/usr/share/foldingos/foldops-supervisor-automation.toml"

var foldOpsSupervisorAutomationPolicyPath = foldOpsSupervisorAutomationPolicyPathDefault

type automationPolicyCommand struct {
	Group string
	Name  string
}

type foldOpsSupervisorAutomationPolicy struct {
	SchemaVersion    int
	ServiceUser      string
	InstallationRole string
	Commands         []automationPolicyCommand
}

var cachedAutomationPolicy *foldOpsSupervisorAutomationPolicy

func requireSupervisorAutomationMutation(commandGroup, commandName string) error {
	if !isFoldOpsAutomationUser() {
		return nil
	}
	if err := requireSupervisorRole(); err != nil {
		return err
	}
	policy, err := loadFoldOpsSupervisorAutomationPolicy()
	if err != nil {
		return fmt.Errorf("automation policy is unavailable: %w", err)
	}
	if policy.ServiceUser != "" && policy.ServiceUser != "foldops" {
		return fmt.Errorf("automation policy service_user must be foldops, found %q", policy.ServiceUser)
	}
	if policy.InstallationRole != "" && policy.InstallationRole != "supervisor" {
		return fmt.Errorf("automation policy installation_role must be supervisor, found %q", policy.InstallationRole)
	}
	commandGroup = strings.TrimSpace(commandGroup)
	commandName = strings.TrimSpace(commandName)
	for _, command := range policy.Commands {
		if command.Group == commandGroup && command.Name == commandName {
			return nil
		}
	}
	return fmt.Errorf(
		"automation policy does not authorize %s %s for the foldops user",
		commandGroup,
		commandName,
	)
}

func loadFoldOpsSupervisorAutomationPolicy() (*foldOpsSupervisorAutomationPolicy, error) {
	if cachedAutomationPolicy != nil {
		return cachedAutomationPolicy, nil
	}
	content, err := os.ReadFile(foldOpsSupervisorAutomationPolicyPath)
	if err != nil {
		return nil, err
	}
	policy, err := parseFoldOpsSupervisorAutomationPolicy(string(content))
	if err != nil {
		return nil, err
	}
	cachedAutomationPolicy = policy
	return policy, nil
}

func parseFoldOpsSupervisorAutomationPolicy(content string) (*foldOpsSupervisorAutomationPolicy, error) {
	policy := &foldOpsSupervisorAutomationPolicy{}
	var current automationPolicyCommand
	inCommand := false
	commandSeen := map[string]bool{}

	flushCommand := func(lineNumber int) error {
		if !inCommand {
			return nil
		}
		if !commandSeen["group"] || !commandSeen["name"] {
			return fmt.Errorf("line %d: command entry is missing group or name", lineNumber)
		}
		policy.Commands = append(policy.Commands, current)
		current = automationPolicyCommand{}
		commandSeen = map[string]bool{}
		inCommand = false
		return nil
	}

	lines := strings.Split(content, "\n")
	for number, raw := range lines {
		line := strings.TrimSpace(raw)
		if line == "" || strings.HasPrefix(line, "#") {
			continue
		}
		if strings.HasPrefix(line, "[[") {
			if line != "[[commands]]" {
				return nil, fmt.Errorf("line %d: unsupported automation policy table %q", number+1, line)
			}
			if err := flushCommand(number + 1); err != nil {
				return nil, err
			}
			inCommand = true
			continue
		}

		parts := strings.SplitN(line, "=", 2)
		if len(parts) != 2 {
			return nil, fmt.Errorf("line %d: expected key = value", number+1)
		}
		key := strings.TrimSpace(parts[0])
		value, err := parseAutomationPolicyScalar(parts[1])
		if err != nil {
			return nil, fmt.Errorf("line %d: %w", number+1, err)
		}

		if inCommand {
			switch key {
			case "group", "name":
				if commandSeen[key] {
					return nil, fmt.Errorf("line %d: duplicate key %q", number+1, key)
				}
				commandSeen[key] = true
				if key == "group" {
					current.Group = value
				} else {
					current.Name = value
				}
			default:
				return nil, fmt.Errorf("line %d: unknown command key %q", number+1, key)
			}
			continue
		}

		switch key {
		case "schema_version":
			policy.SchemaVersion, err = strconv.Atoi(value)
			if err != nil {
				return nil, fmt.Errorf("line %d: schema_version must be an integer", number+1)
			}
		case "service_user":
			policy.ServiceUser = value
		case "installation_role":
			policy.InstallationRole = value
		default:
			return nil, fmt.Errorf("line %d: unknown policy key %q", number+1, key)
		}
	}

	if err := flushCommand(len(lines)); err != nil {
		return nil, err
	}
	if policy.SchemaVersion != 1 {
		return nil, fmt.Errorf("unsupported automation policy schema version %d", policy.SchemaVersion)
	}
	if policy.ServiceUser == "" {
		policy.ServiceUser = "foldops"
	}
	if policy.InstallationRole == "" {
		policy.InstallationRole = "supervisor"
	}
	if len(policy.Commands) == 0 {
		return nil, fmt.Errorf("automation policy defines no commands")
	}
	return policy, nil
}

func parseAutomationPolicyScalar(raw string) (string, error) {
	value := strings.TrimSpace(raw)
	if len(value) >= 2 && value[0] == '"' && value[len(value)-1] == '"' {
		return value[1 : len(value)-1], nil
	}
	if value == "" {
		return "", fmt.Errorf("expected quoted string value")
	}
	return value, nil
}

func foldOpsGroupGID() (int, error) {
	group, err := user.LookupGroup("foldops")
	if err != nil {
		return 0, fmt.Errorf("lookup foldops group: %w", err)
	}
	gid, err := strconv.Atoi(group.Gid)
	if err != nil {
		return 0, fmt.Errorf("parse foldops group id: %w", err)
	}
	return gid, nil
}
