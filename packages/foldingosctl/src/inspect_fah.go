package main

import (
	"fmt"
	"os"
	"regexp"
	"strconv"
	"strings"
)

var (
	fahProjectLinePattern = regexp.MustCompile(`(?i)Project:\s*(\d+)\s*\(\s*Run\s*(\d+)\s*,\s*Clone\s*(\d+)\s*,\s*Gen\s*(\d+)\s*\)`)
	fahProgressPattern    = regexp.MustCompile(`(?i)Progress:\s*([\d.]+)\s*%`)
	fahStepsPattern       = regexp.MustCompile(`Completed\s+(\d+)\s+out\s+of\s+(\d+)\s+steps\s+\(([\d.]+)%\)`)
	fahPPDPattern         = regexp.MustCompile(`(?i)PPD[:\s]+([\d,.]+)`)
	fahTPFPattern         = regexp.MustCompile(`(?i)TPF[:\s]+([\d:]+(?:\.\d+)?)`)
	fahErrorPattern       = regexp.MustCompile(`(?i)\b(ERROR|FATAL|Exception|failed)\b`)
)

type inspectFAHRuntime struct {
	Project      *string   `json:"project,omitempty"`
	Run          *int      `json:"run,omitempty"`
	Clone        *int      `json:"clone,omitempty"`
	Gen          *int      `json:"gen,omitempty"`
	Progress     *float64  `json:"progress,omitempty"`
	PPD          *float64  `json:"ppd,omitempty"`
	TPF          *string   `json:"tpf,omitempty"`
	RecentErrors []string  `json:"recent_errors,omitempty"`
}

type inspectFAHData struct {
	ActiveClientVersion *string           `json:"active_client_version,omitempty"`
	ServiceActive       bool              `json:"service_active"`
	Verified            bool              `json:"verified"`
	Runtime             inspectFAHRuntime `json:"runtime"`
	LogPath             string            `json:"log_path"`
}

func inspectFAH() error {
	data := inspectFAHData{
		LogPath: fahInspectLogPath,
	}
	data.ServiceActive = systemdUnitIsActive("folding-at-home.service")

	if version, err := readFAHCurrentVersion(); err == nil {
		data.ActiveClientVersion = &version
		manifest, manifestErr := fahLoadApprovedManifest(embeddedFAHManifestPath)
		if manifestErr == nil {
			data.Verified = fahInstallationVerified(version, manifest)
		}
	}

	data.Runtime = parseFAHLogState(fahInspectLogPath)

	return automationOrHumanSuccess(data, func() error {
		version := "unknown"
		if data.ActiveClientVersion != nil {
			version = *data.ActiveClientVersion
		}
		fmt.Printf(
			"active_client_version=%s service_active=%t verified=%t\n",
			version,
			data.ServiceActive,
			data.Verified,
		)
		if data.Runtime.Project != nil {
			fmt.Printf("project=%s progress=%v ppd=%v\n", *data.Runtime.Project, data.Runtime.Progress, data.Runtime.PPD)
		}
		return nil
	})
}

func parseFAHLogState(path string) inspectFAHRuntime {
	state := inspectFAHRuntime{
		RecentErrors: []string{},
	}
	content, err := os.ReadFile(path)
	if err != nil {
		return state
	}
	lines := strings.Split(string(content), "\n")
	start := 0
	if len(lines) > 500 {
		start = len(lines) - 500
	}
	for _, line := range lines[start:] {
		line = strings.TrimSpace(line)
		if line == "" {
			continue
		}
		if matches := fahProjectLinePattern.FindStringSubmatch(line); len(matches) == 5 {
			project := matches[1]
			state.Project = &project
			if run, err := strconv.Atoi(matches[2]); err == nil {
				state.Run = &run
			}
			if clone, err := strconv.Atoi(matches[3]); err == nil {
				state.Clone = &clone
			}
			if gen, err := strconv.Atoi(matches[4]); err == nil {
				state.Gen = &gen
			}
		}
		if matches := fahProgressPattern.FindStringSubmatch(line); len(matches) == 2 {
			if progress, err := strconv.ParseFloat(matches[1], 64); err == nil {
				state.Progress = &progress
			}
		}
		if matches := fahStepsPattern.FindStringSubmatch(line); len(matches) == 4 {
			if progress, err := strconv.ParseFloat(matches[3], 64); err == nil {
				state.Progress = &progress
			}
		}
		if matches := fahPPDPattern.FindStringSubmatch(line); len(matches) == 2 {
			raw := strings.ReplaceAll(matches[1], ",", "")
			if ppd, err := strconv.ParseFloat(raw, 64); err == nil {
				state.PPD = &ppd
			}
		}
		if matches := fahTPFPattern.FindStringSubmatch(line); len(matches) == 2 {
			tpf := matches[1]
			state.TPF = &tpf
		}
		if fahErrorPattern.MatchString(line) {
			state.RecentErrors = append(state.RecentErrors, line)
		}
	}
	if len(state.RecentErrors) > 10 {
		state.RecentErrors = state.RecentErrors[len(state.RecentErrors)-10:]
	}
	return state
}
