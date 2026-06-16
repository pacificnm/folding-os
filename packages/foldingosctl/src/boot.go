package main

import (
	"bufio"
	"errors"
	"fmt"
	"net"
	"os"
	"regexp"
	"strconv"
	"strings"
)

const (
	adminSSHUser            = "foldingos-admin"
	osReleasePath           = "/usr/lib/os-release"
	consoleDeviceTTY1       = "/dev/tty1"
	consoleDevice           = "/dev/console"
	consoleClearScreen      = "\033[2J\033[3J\033[H"
	bootStatusRetryAttempts = 90
)

var (
	ipv4AddressPattern        = regexp.MustCompile(`\b(?:[0-9]{1,3}\.){3}[0-9]{1,3}\b`)
	networkctlStatusIfaceName = regexp.MustCompile(`^\S+\s+\d+:\s*(\S+)`)
)

func bootStatus() error {
	return writeCommissioningDisplay(true)
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
		name, isRoutable, skip := parseNetworkctlListLine(line)
		if skip {
			continue
		}
		if isRoutable {
			routable = append(routable, name)
			continue
		}
		fallback = append(fallback, name)
	}
	if len(routable) > 0 {
		return routable, nil
	}
	if len(fallback) > 0 {
		return fallback, nil
	}
	return nil, errors.New("no wired network interface found")
}

func parseNetworkctlListLine(line string) (name string, routable bool, skip bool) {
	fields := strings.Fields(strings.TrimSpace(line))
	if len(fields) < 2 {
		return "", false, true
	}
	if _, err := strconv.Atoi(fields[0]); err == nil && len(fields) >= 4 {
		name = fields[1]
		if name == "lo" {
			return "", false, true
		}
		for _, field := range fields[2:] {
			if field == "routable" {
				return name, true, false
			}
		}
		return name, false, false
	}
	name = fields[0]
	if name == "lo" {
		return "", false, true
	}
	if len(fields) > 1 && fields[1] == "routable" {
		return name, true, false
	}
	return name, false, false
}

func selectNetworkInterfaceFromListing(listing string) (string, error) {
	interfaces, err := candidateNetworkInterfaces(listing)
	if err != nil {
		return "", err
	}
	return interfaces[0], nil
}

func selectNetworkInterface() (string, error) {
	listing, err := output("networkctl", "--no-legend", "--no-pager", "list")
	if err != nil {
		return "", err
	}
	return selectNetworkInterfaceFromListing(listing)
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
			candidate := normalizeIPv4AddressField(field)
			if candidate == "" {
				continue
			}
			if strings.Contains(strings.ToLower(field), "dhcp") {
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

func normalizeIPv4AddressField(field string) string {
	if field == "on" || field == "via" || strings.HasPrefix(field, "(") {
		return ""
	}
	match := ipv4AddressPattern.FindString(field)
	if match == "" || !isRoutableIPv4(match) {
		return ""
	}
	return match
}

func resolveNetworkInterfaceName(iface string) (string, error) {
	if _, err := strconv.Atoi(iface); err != nil {
		return iface, nil
	}
	status, err := output("networkctl", "--no-legend", "--no-pager", "status", iface)
	if err != nil {
		return "", err
	}
	for _, line := range strings.Split(status, "\n") {
		matches := networkctlStatusIfaceName.FindStringSubmatch(strings.TrimSpace(line))
		if len(matches) == 2 {
			return matches[1], nil
		}
	}
	return "", fmt.Errorf("network interface name is unavailable for index %s", iface)
}

func ipv4AddressFromInterface(iface string) (string, error) {
	resolved, err := resolveNetworkInterfaceName(iface)
	if err != nil {
		return "", err
	}
	ifaceObj, err := net.InterfaceByName(resolved)
	if err != nil {
		return "", err
	}
	addrs, err := ifaceObj.Addrs()
	if err != nil {
		return "", err
	}
	for _, addr := range addrs {
		ipNet, ok := addr.(*net.IPNet)
		if !ok || ipNet.IP.To4() == nil {
			continue
		}
		address := ipNet.IP.String()
		if isRoutableIPv4(address) {
			return address, nil
		}
	}
	return "", fmt.Errorf("no routable IPv4 address on %s", resolved)
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

func clearConsole() error {
	return writeConsole(consoleClearScreen)
}
