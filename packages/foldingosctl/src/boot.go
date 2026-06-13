package main

import (
	"bufio"
	"errors"
	"fmt"
	"net"
	"os"
	"regexp"
	"strings"
	"time"
)

const (
	adminSSHUser           = "foldingos-admin"
	osReleasePath          = "/usr/lib/os-release"
	consoleDeviceTTY1      = "/dev/tty1"
	consoleDevice          = "/dev/console"
	bootStatusRetryAttempts = 90
)

var ipv4AddressPattern = regexp.MustCompile(`\b(?:[0-9]{1,3}\.){3}[0-9]{1,3}\b`)

func bootStatus() error {
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

	var displayErr error
	var message string
	for attempt := 0; attempt < bootStatusRetryAttempts; attempt++ {
		message, displayErr = readyDisplayMessage(prettyName)
		if displayErr == nil {
			break
		}
		time.Sleep(time.Second)
	}
	if displayErr != nil {
		message = failureDisplayMessage(prettyName, displayErr)
		fmt.Fprintln(os.Stderr, displayErr.Error())
	}

	if err := writeConsole(message); err != nil {
		return err
	}
	fmt.Println("Wrote FoldingOS commissioning display status.")
	return nil
}

func readyDisplayMessage(prettyName string) (string, error) {
	address, err := routableIPv4Address()
	if err != nil {
		return "", err
	}
	return formatReadyDisplay(prettyName, address), nil
}

func formatReadyDisplay(prettyName, address string) string {
	return strings.Join([]string{
		prettyName + " ready",
		"Address: " + address,
		"SSH: " + adminSSHUser + "@" + address,
	}, "\n") + "\n"
}

func failureDisplayMessage(prettyName string, err error) string {
	return strings.Join([]string{
		prettyName,
		"Network: " + err.Error(),
	}, "\n") + "\n"
}

func osReleaseValue(key string) (string, error) {
	file, err := os.Open(osReleasePath)
	if err != nil {
		return "", err
	}
	defer file.Close()

	prefix := key + "="
	scanner := bufio.NewScanner(file)
	for scanner.Scan() {
		line := strings.TrimSpace(scanner.Text())
		if !strings.HasPrefix(line, prefix) {
			continue
		}
		value := strings.TrimPrefix(line, prefix)
		return strings.Trim(value, `"`), nil
	}
	if err := scanner.Err(); err != nil {
		return "", err
	}
	return "", nil
}

func routableIPv4Address() (string, error) {
	listing, err := output("networkctl", "--no-legend", "--no-pager", "list")
	if err != nil {
		return "", err
	}

	interfaces, err := candidateNetworkInterfaces(listing)
	if err != nil {
		return "", err
	}

	var lastErr error
	for _, interfaceName := range interfaces {
		status, err := output("networkctl", "--no-legend", "--no-pager", "status", interfaceName)
		if err != nil {
			lastErr = err
			continue
		}
		address, err := parseIPv4Address(status)
		if err != nil {
			lastErr = err
			continue
		}
		return address, nil
	}
	if lastErr != nil {
		return "", lastErr
	}
	return "", errors.New("no routable IPv4 address available")
}

func candidateNetworkInterfaces(listing string) ([]string, error) {
	lines := strings.Split(strings.TrimSpace(listing), "\n")
	var routable []string
	var fallback []string
	for _, line := range lines {
		fields := strings.Fields(line)
		if len(fields) < 2 || fields[0] == "lo" {
			continue
		}
		if fields[1] == "routable" {
			routable = append(routable, fields[0])
			continue
		}
		fallback = append(fallback, fields[0])
	}
	if len(routable) > 0 {
		return routable, nil
	}
	if len(fallback) > 0 {
		return fallback, nil
	}
	return nil, errors.New("no wired network interface found")
}

func selectNetworkInterfaceFromListing(listing string) (string, error) {
	interfaces, err := candidateNetworkInterfaces(listing)
	if err != nil {
		return "", err
	}
	return interfaces[0], nil
}

func parseIPv4Address(status string) (string, error) {
	var dhcpAddress string
	var globalAddress string
	for _, line := range strings.Split(status, "\n") {
		trimmed := strings.TrimSpace(line)
		if !strings.HasPrefix(trimmed, "Address:") {
			continue
		}
		for _, field := range strings.Fields(trimmed)[1:] {
			candidate := strings.TrimSuffix(field, "(DHCP)")
			candidate = strings.TrimSuffix(candidate, "(Router)")
			if !ipv4AddressPattern.MatchString(candidate) || !isRoutableIPv4(candidate) {
				continue
			}
			if strings.Contains(field, "(DHCP)") {
				dhcpAddress = candidate
			}
			if globalAddress == "" {
				globalAddress = candidate
			}
		}
	}
	if dhcpAddress != "" {
		return dhcpAddress, nil
	}
	if globalAddress != "" {
		return globalAddress, nil
	}
	return "", errors.New("no routable IPv4 address available")
}

func isRoutableIPv4(address string) bool {
	ip := net.ParseIP(address)
	if ip == nil || ip.To4() == nil {
		return false
	}
	return !ip.IsLoopback() && !ip.IsUnspecified() && !ip.IsLinkLocalUnicast()
}

func writeConsole(message string) error {
	var firstErr error
	for _, device := range []string{consoleDeviceTTY1, consoleDevice} {
		file, err := os.OpenFile(device, os.O_WRONLY|os.O_APPEND, 0)
		if err != nil {
			if firstErr == nil {
				firstErr = fmt.Errorf("open %s: %w", device, err)
			}
			continue
		}
		if _, err := file.WriteString(message); err != nil && firstErr == nil {
			firstErr = fmt.Errorf("write %s: %w", device, err)
		}
		if err := file.Close(); err != nil && firstErr == nil {
			firstErr = fmt.Errorf("close %s: %w", device, err)
		}
	}
	return firstErr
}
