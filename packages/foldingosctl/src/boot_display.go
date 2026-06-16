package main

import (
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"time"
	"unicode/utf8"
)

const (
	commissioningDisplayWidth = 62
	bootServiceWaitTimeout    = 3 * time.Minute
	bootServicePollInterval   = 2 * time.Second
)

type commissioningCheck struct {
	Label string
	Ready bool
}

func writeCommissioningDisplay(waitForServices bool) error {
	prettyName, err := osReleaseValue("PRETTY_NAME")
	if err != nil {
		return err
	}
	if prettyName == "" {
		prettyName, err = osReleaseValue("VERSION")
		if err != nil {
			return err
		}
	}
	if prettyName == "" {
		prettyName = "FoldingOS"
	}
	version, err := osReleaseValue("VERSION_ID")
	if err != nil {
		return err
	}
	if version == "" {
		version, _ = osReleaseValue("VERSION")
	}

	var address string
	var networkErr error
	for attempt := 0; attempt < bootStatusRetryAttempts; attempt++ {
		address, networkErr = routableIPv4Address()
		if networkErr == nil {
			break
		}
		time.Sleep(time.Second)
	}
	if networkErr != nil {
		message := failureDisplayMessage(prettyName, networkErr)
		if err := clearConsole(); err != nil {
			return err
		}
		if err := writeConsole(message); err != nil {
			return err
		}
		fmt.Fprintln(os.Stderr, networkErr.Error())
		return nil
	}

	role := readInstallationRoleForDisplay()
	checks := evaluateCommissioningChecks(role)
	if waitForServices && commissioningChecksPending(checks) {
		deadline := time.Now().Add(bootServiceWaitTimeout)
		for time.Now().Before(deadline) {
			checks = evaluateCommissioningChecks(role)
			if !commissioningChecksPending(checks) {
				break
			}
			time.Sleep(bootServicePollInterval)
		}
	}

	message := formatCommissioningDisplay(prettyName, version, role, address, checks)
	if err := clearConsole(); err != nil {
		return err
	}
	if err := writeConsole(message); err != nil {
		return err
	}
	printCommissioningStatusSummary(checks)
	fmt.Println("Wrote FoldingOS commissioning display status.")
	return nil
}

func refreshCommissioningDisplay() {
	if err := writeCommissioningDisplay(false); err != nil {
		fmt.Fprintf(os.Stderr, "foldingosctl: refresh commissioning display: %v\n", err)
	}
}

func readInstallationRoleForDisplay() string {
	role, err := readActiveInstallationRole()
	if err != nil {
		return "unknown"
	}
	return role
}

func evaluateCommissioningChecks(role string) []commissioningCheck {
	checks := []commissioningCheck{
		{Label: "Network online", Ready: true},
		checkSystemdUnit("SSH administrator provisioned", "foldingos-ssh-provision.service"),
		checkInstallationRole(role),
		checkFoldOpsPackages(),
		checkFoldOpsProvisioned(),
	}
	switch role {
	case "supervisor":
		checks = append(checks,
			checkSystemdUnit("FoldOps HTTPS (port 3443)", foldOpsServeHTTPSServiceName),
			checkSystemdUnit("FoldOps supervisor (loopback)", foldOpsSupervisorServiceName),
			checkSystemdUnit("Provisioning control plane", "foldingos-provision.service"),
		)
	}
	checks = append(checks,
		checkSystemdUnit("FoldOps agent", foldOpsAgentServiceName),
		checkSystemdUnit("Folding@home client", "folding-at-home.service"),
	)
	return checks
}

func commissioningChecksPending(checks []commissioningCheck) bool {
	for _, check := range checks {
		if !check.Ready {
			return true
		}
	}
	return false
}

func checkSystemdUnit(label, unit string) commissioningCheck {
	return commissioningCheck{
		Label: label,
		Ready: systemdUnitIsActive(unit),
	}
}

func checkInstallationRole(role string) commissioningCheck {
	ready := role == "supervisor" || role == "agent"
	return commissioningCheck{
		Label: "Installation role active",
		Ready: ready,
	}
}

func checkFoldOpsPackages() commissioningCheck {
	currentPath := filepath.Join(foldOpsAppsRoot, "current")
	ready := pathExists(currentPath)
	return commissioningCheck{
		Label: "FoldOps packages acquired",
		Ready: ready,
	}
}

func checkFoldOpsProvisioned() commissioningCheck {
	return commissioningCheck{
		Label: "FoldOps provisioned",
		Ready: pathExists(foldOpsProvisionedMarkerPath),
	}
}

func systemdUnitIsActive(unit string) bool {
	return run("systemctl", "is-active", "--quiet", unit) == nil
}

func pathExists(path string) bool {
	_, err := os.Stat(path)
	return err == nil
}

func formatCommissioningDisplay(prettyName, version, role, address string, checks []commissioningCheck) string {
	readyLines := formatReadyDisplay(prettyName, address)
	allReady := !commissioningChecksPending(checks)
	statusLine := "System ready"
	if !allReady {
		statusLine = "Some services are still starting"
	}

	var b strings.Builder
	b.WriteString(renderCommissioningBox(prettyName, statusLine))
	b.WriteString("\n")
	b.WriteString(readyLines)
	b.WriteString("\n")
	if version != "" {
		b.WriteString(fmt.Sprintf("Version       : %s\n", version))
	}
	if role != "" && role != "unknown" {
		b.WriteString(fmt.Sprintf("Role          : %s\n", role))
	}
	b.WriteString("\n")
	for _, check := range checks {
		b.WriteString(formatCommissioningCheckLine(check))
		b.WriteString("\n")
	}
	b.WriteString("\nhttps://folding-os.com\n")
	return b.String()
}

func renderCommissioningBox(title, statusLine string) string {
	lines := []string{
		centerDisplayText(title, commissioningDisplayWidth-2),
		centerDisplayText(statusLine, commissioningDisplayWidth-2),
	}
	var b strings.Builder
	b.WriteString("╔")
	b.WriteString(strings.Repeat("═", commissioningDisplayWidth-2))
	b.WriteString("╗\n")
	for _, line := range lines {
		b.WriteString("║")
		b.WriteString(line)
		b.WriteString("║\n")
	}
	b.WriteString("╚")
	b.WriteString(strings.Repeat("═", commissioningDisplayWidth-2))
	b.WriteString("╝")
	return b.String()
}

func centerDisplayText(text string, width int) string {
	textLen := utf8.RuneCountInString(text)
	if textLen >= width {
		return truncateDisplayRunes(text, width)
	}
	padding := width - textLen
	left := padding / 2
	right := padding - left
	return strings.Repeat(" ", left) + text + strings.Repeat(" ", right)
}

func truncateDisplayRunes(text string, width int) string {
	if width <= 0 {
		return ""
	}
	count := 0
	for index := range text {
		if count == width {
			return text[:index]
		}
		count++
	}
	return text
}

func formatCommissioningCheckLine(check commissioningCheck) string {
	marker := "✗"
	if check.Ready {
		marker = "✓"
	}
	return fmt.Sprintf("%s %s", marker, check.Label)
}

func printCommissioningStatusSummary(checks []commissioningCheck) {
	fmt.Println("Commissioning service status:")
	for _, check := range checks {
		state := "pending"
		if check.Ready {
			state = "ready"
		}
		fmt.Printf("  %s: %s\n", check.Label, state)
	}
}
