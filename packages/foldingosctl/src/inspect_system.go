package main

import (
	"bufio"
	"fmt"
	"os"
	"strconv"
	"strings"
)

type inspectSystemMemory struct {
	TotalBytes  uint64  `json:"total_bytes"`
	UsedBytes   uint64  `json:"used_bytes"`
	FreeBytes   uint64  `json:"free_bytes"`
	UsedPercent float64 `json:"used_percent"`
}

type inspectSystemFilesystem struct {
	Mountpoint  string  `json:"mountpoint"`
	TotalBytes  uint64  `json:"total_bytes"`
	UsedBytes   uint64  `json:"used_bytes"`
	FreeBytes   uint64  `json:"free_bytes"`
	UsedPercent float64 `json:"used_percent"`
}

type inspectSystemNetwork struct {
	Interface string `json:"interface"`
	RXBytes   uint64 `json:"rx_bytes"`
	TXBytes   uint64 `json:"tx_bytes"`
}

type inspectSystemData struct {
	UptimeSeconds float64                  `json:"uptime_seconds"`
	LoadAverage   [3]float64               `json:"load_average"`
	Memory        inspectSystemMemory      `json:"memory"`
	RootFilesystem inspectSystemFilesystem `json:"root_filesystem"`
	PrimaryNetwork *inspectSystemNetwork   `json:"primary_network,omitempty"`
	CPUTempCelsius *float64                 `json:"cpu_temp_celsius,omitempty"`
	ChassisTempCelsius *float64             `json:"chassis_temp_celsius,omitempty"`
}

func inspectSystem() error {
	uptime, loadAverage, err := readUptimeAndLoad()
	if err != nil {
		return err
	}
	memory, err := readMemoryUsage()
	if err != nil {
		return err
	}
	total, used, free, percent, err := readRootFilesystemUsage()
	if err != nil {
		return err
	}
	data := inspectSystemData{
		UptimeSeconds: uptime,
		LoadAverage:   loadAverage,
		Memory:        memory,
		RootFilesystem: inspectSystemFilesystem{
			Mountpoint:  "/",
			TotalBytes:  total,
			UsedBytes:   used,
			FreeBytes:   free,
			UsedPercent: percent,
		},
	}
	if network, networkErr := readPrimaryNetworkCounters(); networkErr == nil {
		data.PrimaryNetwork = &network
	}
	cpuTemp, chassisTemp := readTemperaturesFromSysfs()
	data.CPUTempCelsius = cpuTemp
	data.ChassisTempCelsius = chassisTemp

	return automationOrHumanSuccess(data, func() error {
		fmt.Printf(
			"uptime_seconds=%.0f load=%.2f %.2f %.2f memory_used_percent=%.1f root_used_percent=%.1f\n",
			data.UptimeSeconds,
			data.LoadAverage[0],
			data.LoadAverage[1],
			data.LoadAverage[2],
			data.Memory.UsedPercent,
			data.RootFilesystem.UsedPercent,
		)
		return nil
	})
}

func readUptimeAndLoad() (float64, [3]float64, error) {
	uptimeContent, err := os.ReadFile("/proc/uptime")
	if err != nil {
		return 0, [3]float64{}, err
	}
	fields := strings.Fields(string(uptimeContent))
	if len(fields) == 0 {
		return 0, [3]float64{}, fmt.Errorf("invalid /proc/uptime")
	}
	uptime, err := strconv.ParseFloat(fields[0], 64)
	if err != nil {
		return 0, [3]float64{}, err
	}

	loadContent, err := os.ReadFile("/proc/loadavg")
	if err != nil {
		return 0, [3]float64{}, err
	}
	loadFields := strings.Fields(string(loadContent))
	if len(loadFields) < 3 {
		return 0, [3]float64{}, fmt.Errorf("invalid /proc/loadavg")
	}
	var load [3]float64
	for index := 0; index < 3; index++ {
		load[index], err = strconv.ParseFloat(loadFields[index], 64)
		if err != nil {
			return 0, [3]float64{}, err
		}
	}
	return uptime, load, nil
}

func readMemoryUsage() (inspectSystemMemory, error) {
	file, err := os.Open("/proc/meminfo")
	if err != nil {
		return inspectSystemMemory{}, err
	}
	defer file.Close()

	var totalKB, availableKB uint64
	scanner := bufio.NewScanner(file)
	for scanner.Scan() {
		line := scanner.Text()
		switch {
		case strings.HasPrefix(line, "MemTotal:"):
			totalKB, err = parseMeminfoKB(line)
		case strings.HasPrefix(line, "MemAvailable:"):
			availableKB, err = parseMeminfoKB(line)
		}
		if err != nil {
			return inspectSystemMemory{}, err
		}
	}
	if err := scanner.Err(); err != nil {
		return inspectSystemMemory{}, err
	}
	if totalKB == 0 {
		return inspectSystemMemory{}, fmt.Errorf("memory total is unavailable")
	}
	totalBytes := totalKB * 1024
	freeBytes := availableKB * 1024
	usedBytes := totalBytes
	if freeBytes < totalBytes {
		usedBytes = totalBytes - freeBytes
	}
	percent := float64(int64((float64(usedBytes)/float64(totalBytes))*1000+0.5)) / 10.0
	return inspectSystemMemory{
		TotalBytes:  totalBytes,
		UsedBytes:   usedBytes,
		FreeBytes:   freeBytes,
		UsedPercent: percent,
	}, nil
}

func parseMeminfoKB(line string) (uint64, error) {
	fields := strings.Fields(line)
	if len(fields) < 2 {
		return 0, fmt.Errorf("invalid meminfo line %q", line)
	}
	value, err := strconv.ParseUint(fields[1], 10, 64)
	if err != nil {
		return 0, err
	}
	return value, nil
}

func readPrimaryNetworkCounters() (inspectSystemNetwork, error) {
	interfaceName, err := selectNetworkInterface()
	if err != nil {
		return inspectSystemNetwork{}, err
	}
	rxBytes, txBytes, err := readInterfaceCounters(interfaceName)
	if err != nil {
		return inspectSystemNetwork{}, err
	}
	return inspectSystemNetwork{
		Interface: interfaceName,
		RXBytes:   rxBytes,
		TXBytes:   txBytes,
	}, nil
}

func readInterfaceCounters(interfaceName string) (uint64, uint64, error) {
	file, err := os.Open("/proc/net/dev")
	if err != nil {
		return 0, 0, err
	}
	defer file.Close()

	scanner := bufio.NewScanner(file)
	for scanner.Scan() {
		line := strings.TrimSpace(scanner.Text())
		if !strings.HasPrefix(line, interfaceName+":") {
			continue
		}
		parts := strings.Fields(strings.TrimPrefix(line, interfaceName+":"))
		if len(parts) < 9 {
			return 0, 0, fmt.Errorf("invalid /proc/net/dev entry for %s", interfaceName)
		}
		rxBytes, err := strconv.ParseUint(parts[0], 10, 64)
		if err != nil {
			return 0, 0, err
		}
		txBytes, err := strconv.ParseUint(parts[8], 10, 64)
		if err != nil {
			return 0, 0, err
		}
		return rxBytes, txBytes, nil
	}
	return 0, 0, fmt.Errorf("network interface %s not found in /proc/net/dev", interfaceName)
}

func readTemperaturesFromSysfs() (*float64, *float64) {
	cpuTemp, chassisTemp := readHwmonTemperatures()
	if cpuTemp == nil {
		cpuTemp = readThermalZoneTemperature("x86_pkg_temp")
	}
	if chassisTemp == nil {
		chassisTemp = readThermalZoneTemperature("acpitz")
	}
	return cpuTemp, chassisTemp
}

func readHwmonTemperatures() (*float64, *float64) {
	entries, err := os.ReadDir("/sys/class/hwmon")
	if err != nil {
		return nil, nil
	}
	var cpuTemp *float64
	var chassisTemp *float64
	for _, entry := range entries {
		base := "/sys/class/hwmon/" + entry.Name()
		nameBytes, _ := os.ReadFile(base + "/name")
		name := strings.TrimSpace(string(nameBytes))
		for index := 1; index <= 16; index++ {
			inputPath := fmt.Sprintf("%s/temp%d_input", base, index)
			content, err := os.ReadFile(inputPath)
			if err != nil {
				break
			}
			temp, ok := parseTemperatureInput(strings.TrimSpace(string(content)))
			if !ok {
				continue
			}
			labelBytes, _ := os.ReadFile(fmt.Sprintf("%s/temp%d_label", base, index))
			label := strings.TrimSpace(string(labelBytes))
			labelLower := strings.ToLower(label)
			nameLower := strings.ToLower(name)
			switch {
			case cpuTemp == nil && (strings.Contains(labelLower, "package") || strings.Contains(labelLower, "cpu") || strings.Contains(nameLower, "k10temp") || strings.Contains(nameLower, "coretemp")):
				cpuTemp = &temp
			case chassisTemp == nil && (strings.Contains(labelLower, "syst") || strings.Contains(labelLower, "board") || strings.Contains(labelLower, "chassis") || strings.Contains(nameLower, "acpitz")):
				chassisTemp = &temp
			}
		}
	}
	return cpuTemp, chassisTemp
}

func readThermalZoneTemperature(match string) *float64 {
	entries, err := os.ReadDir("/sys/class/thermal")
	if err != nil {
		return nil
	}
	matchLower := strings.ToLower(match)
	for _, entry := range entries {
		if !strings.HasPrefix(entry.Name(), "thermal_zone") {
			continue
		}
		base := "/sys/class/thermal/" + entry.Name()
		typeBytes, _ := os.ReadFile(base + "/type")
		zoneType := strings.TrimSpace(string(typeBytes))
		if !strings.Contains(strings.ToLower(zoneType), matchLower) {
			continue
		}
		content, err := os.ReadFile(base + "/temp")
		if err != nil {
			continue
		}
		if temp, ok := parseTemperatureInput(strings.TrimSpace(string(content))); ok {
			return &temp
		}
	}
	return nil
}

func parseTemperatureInput(raw string) (float64, bool) {
	value, err := strconv.ParseFloat(raw, 64)
	if err != nil || value <= 0 {
		return 0, false
	}
	if value > 200 {
		value = float64(int64((value/1000.0)*10+0.5)) / 10.0
	} else {
		value = float64(int64(value*10+0.5)) / 10.0
	}
	return value, true
}
