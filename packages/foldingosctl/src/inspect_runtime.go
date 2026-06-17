package main

import (
	"fmt"
	"os"
	"path/filepath"
	"strings"
)

type inspectFoldOpsPackage struct {
	Name             string `json:"name"`
	Version          string `json:"version"`
	Verified         bool   `json:"verified"`
	VerificationPath string `json:"verification_path"`
}

type inspectFoldOpsAcquireState struct {
	ConsecutiveFailures int     `json:"consecutive_failures"`
	NextAttemptUnix     int64   `json:"next_attempt_unix,omitempty"`
	LastFailureReason   *string `json:"last_failure_reason,omitempty"`
}

type inspectFoldOpsData struct {
	BootstrapManifestRelease *string                     `json:"bootstrap_manifest_release,omitempty"`
	AssignedManifestRelease  *string                     `json:"assigned_manifest_release,omitempty"`
	ActiveManifestRelease    *string                     `json:"active_manifest_release,omitempty"`
	EffectiveManifestRelease *string                     `json:"effective_manifest_release,omitempty"`
	PackagesAcquired         bool                        `json:"packages_acquired"`
	Provisioned              bool                        `json:"provisioned"`
	Packages                 []inspectFoldOpsPackage     `json:"packages,omitempty"`
	AcquireState             *inspectFoldOpsAcquireState `json:"acquire_state,omitempty"`
}

type inspectToolsBinary struct {
	Path        string `json:"path"`
	SizeBytes   int64  `json:"size_bytes"`
	ModTimeUnix int64  `json:"mod_time_unix"`
}

type inspectToolsAcquireState struct {
	ConsecutiveFailures int     `json:"consecutive_failures"`
	NextAttemptUnix     int64   `json:"next_attempt_unix,omitempty"`
	LastFailureReason   *string `json:"last_failure_reason,omitempty"`
}

type inspectToolsData struct {
	BootstrapToolsVersion *string                   `json:"bootstrap_tools_version,omitempty"`
	AssignedToolsVersion  *string                   `json:"assigned_tools_version,omitempty"`
	ActiveToolsVersion    *string                   `json:"active_tools_version,omitempty"`
	EffectiveToolsVersion *string                   `json:"effective_tools_version,omitempty"`
	Verified              bool                      `json:"verified"`
	Binary                inspectToolsBinary        `json:"binary"`
	AcquireState          *inspectToolsAcquireState `json:"acquire_state,omitempty"`
}

func inspectFoldOps() error {
	data, err := collectInspectFoldOpsData()
	if err != nil {
		return err
	}
	return automationOrHumanSuccess(data, func() error {
		fmt.Printf(
			"bootstrap=%v assigned=%v active=%v effective=%v packages_acquired=%t provisioned=%t\n",
			stringOrDash(data.BootstrapManifestRelease),
			stringOrDash(data.AssignedManifestRelease),
			stringOrDash(data.ActiveManifestRelease),
			stringOrDash(data.EffectiveManifestRelease),
			data.PackagesAcquired,
			data.Provisioned,
		)
		return nil
	})
}

func inspectTools() error {
	data, err := collectInspectToolsData()
	if err != nil {
		return err
	}
	return automationOrHumanSuccess(data, func() error {
		fmt.Printf(
			"bootstrap=%v assigned=%v active=%v effective=%v verified=%t binary=%s\n",
			stringOrDash(data.BootstrapToolsVersion),
			stringOrDash(data.AssignedToolsVersion),
			stringOrDash(data.ActiveToolsVersion),
			stringOrDash(data.EffectiveToolsVersion),
			data.Verified,
			data.Binary.Path,
		)
		return nil
	})
}

func collectInspectFoldOpsData() (inspectFoldOpsData, error) {
	data := inspectFoldOpsData{
		PackagesAcquired: pathExists(filepath.Join(foldOpsAppsRoot, "current")),
		Provisioned:      pathExists(foldOpsProvisionedMarkerPath),
	}

	if manifest, err := loadFoldOpsManifestFromFile(foldOpsEmbeddedManifestPath); err == nil {
		data.BootstrapManifestRelease = &manifest.ManifestRelease
	}
	if _, err := os.Stat(foldOpsAssignedManifestPath); err == nil {
		if manifest, err := loadFoldOpsManifestFromFile(foldOpsAssignedManifestPath); err == nil {
			data.AssignedManifestRelease = &manifest.ManifestRelease
		}
	} else if !os.IsNotExist(err) {
		return inspectFoldOpsData{}, err
	}
	if release, err := readFoldOpsCurrentRelease(); err == nil {
		data.ActiveManifestRelease = &release
	}
	if manifest, err := resolveEffectiveFoldOpsManifest(); err == nil {
		data.EffectiveManifestRelease = &manifest.ManifestRelease
		role, roleErr := readActiveInstallationRole()
		if roleErr == nil {
			packages, packageErr := foldOpsPackagesForRole(manifest, role)
			if packageErr == nil {
				data.Packages = buildInspectFoldOpsPackages(releasePackagesDir(data.ActiveManifestRelease), packages)
			}
		}
	}
	if state, err := loadFoldOpsAcquireState(); err == nil && (state.ConsecutiveFailures > 0 || state.NextAttemptUnix > 0 || state.LastFailureReason != "") {
		data.AcquireState = &inspectFoldOpsAcquireState{
			ConsecutiveFailures: state.ConsecutiveFailures,
			NextAttemptUnix:     state.NextAttemptUnix,
			LastFailureReason:   optionalStringValue(state.LastFailureReason),
		}
	} else if err != nil {
		return inspectFoldOpsData{}, err
	}
	return data, nil
}

func collectInspectToolsData() (inspectToolsData, error) {
	data := inspectToolsData{
		Binary: inspectToolsBinary{Path: toolsBinaryPath},
	}
	if info, err := os.Stat(toolsBinaryPath); err == nil {
		data.Binary.SizeBytes = info.Size()
		data.Binary.ModTimeUnix = info.ModTime().Unix()
	} else if !os.IsNotExist(err) {
		return inspectToolsData{}, err
	}
	if assignment, err := loadToolsAssignmentFromFile(toolsBootstrapManifestPath); err == nil {
		data.BootstrapToolsVersion = &assignment.ToolsVersion
	} else if !os.IsNotExist(err) {
		return inspectToolsData{}, err
	}
	if _, err := os.Stat(toolsAssignedVersionPath); err == nil {
		if assignment, err := loadToolsAssignmentFromFile(toolsAssignedVersionPath); err == nil {
			data.AssignedToolsVersion = &assignment.ToolsVersion
		}
	} else if !os.IsNotExist(err) {
		return inspectToolsData{}, err
	}
	if state, err := loadToolsActiveState(); err == nil && state.ToolsVersion != "" {
		data.ActiveToolsVersion = &state.ToolsVersion
	} else if err != nil {
		return inspectToolsData{}, err
	}
	if assignment, ok, err := resolveEffectiveToolsAssignment(); err != nil {
		return inspectToolsData{}, err
	} else if ok {
		data.EffectiveToolsVersion = &assignment.ToolsVersion
		data.Verified = toolsInstallationVerified(assignment)
	}
	if state, err := loadToolsAcquireState(); err == nil && (state.ConsecutiveFailures > 0 || state.NextAttemptUnix > 0 || state.LastFailureReason != "") {
		data.AcquireState = &inspectToolsAcquireState{
			ConsecutiveFailures: state.ConsecutiveFailures,
			NextAttemptUnix:     state.NextAttemptUnix,
			LastFailureReason:   optionalStringValue(state.LastFailureReason),
		}
	} else if err != nil {
		return inspectToolsData{}, err
	}
	return data, nil
}

func releasePackagesDir(release *string) string {
	if release == nil || strings.TrimSpace(*release) == "" {
		return filepath.Join(foldOpsAppsRoot, "current")
	}
	return filepath.Join(foldOpsAppsRoot, *release)
}

func buildInspectFoldOpsPackages(releaseDir string, packages []foldOpsPackage) []inspectFoldOpsPackage {
	results := make([]inspectFoldOpsPackage, 0, len(packages))
	for _, pkg := range packages {
		verified := false
		if pkg.VerificationPath != "" {
			_, err := os.Stat(pkg.VerificationPath)
			verified = err == nil
		}
		results = append(results, inspectFoldOpsPackage{
			Name:             pkg.Name,
			Version:          pkg.Version,
			Verified:         verified,
			VerificationPath: pkg.VerificationPath,
		})
	}
	_ = releaseDir
	return results
}

func stringOrDash(value *string) string {
	if value == nil {
		return "-"
	}
	return *value
}
