package main

import (
	"errors"
	"fmt"
	"net/url"
	"os"
	"path/filepath"
	"sort"
	"strconv"
	"strings"
)

const (
	embeddedFoldOpsManifestPathDefault = "/usr/share/foldingos/manifests/foldops.toml"
	assignedFoldOpsManifestPathDefault = "/data/config/foldops/assigned-manifest.toml"
	foldOpsVerifiedMarkerName          = ".foldingos-verified"
	foldOpsApprovedDebOrigin           = "deb.folding-os.com"
	foldOpsApprovedPackagesOrigin      = "packages.folding-os.com"
	foldOpsManifestSchemaVersionV1     = 1
	foldOpsManifestSchemaVersionV2     = 2
	foldOpsManifestArtifactFormatDeb   = "deb"
	foldOpsManifestArtifactFormatLayout = "layout-tar-zst"
	foldOpsManifestArchitecture        = "x86_64"
	foldOpsManifestMinimumVersion      = "0.1.0"
	foldOpsVerificationPathPrefix      = "/data/apps/foldops/current/"
)

var (
	foldOpsEmbeddedManifestPath = embeddedFoldOpsManifestPathDefault
	foldOpsAssignedManifestPath = assignedFoldOpsManifestPathDefault
)

var (
	foldOpsAppsRoot     = "/data/apps/foldops"
	foldOpsDownloadsDir = "/data/apps/foldops/.downloads"
)

var foldOpsRequiredPackageRoles = map[string][]string{
	"foldops-agent":      {"agent", "supervisor"},
	"foldops-supervisor": {"supervisor"},
	"foldops-web":        {"supervisor"},
}

type foldOpsPackage struct {
	Name             string
	Version          string
	Roles            []string
	InstallPrefix    string
	ArtifactURL      string
	ArtifactSize     int64
	SHA256           string
	VerificationPath string
}

type foldOpsManifest struct {
	SchemaVersion           int
	ManifestRelease         string
	Architecture            string
	ArtifactFormat          string
	MinimumFoldingOSVersion string
	Packages                []foldOpsPackage
}

func validateFoldOpsManifestEmbedded() error {
	manifest, err := loadFoldOpsManifestFromFile(foldOpsEmbeddedManifestPath)
	if err != nil {
		return err
	}
	if err := validateFoldingOSCompatibility(manifest.MinimumFoldingOSVersion); err != nil {
		return err
	}
	fmt.Printf(
		"Approved FoldOps bootstrap manifest %s is valid for FoldingOS %s.\n",
		manifest.ManifestRelease,
		manifest.MinimumFoldingOSVersion,
	)
	if _, err := os.Stat(foldOpsAssignedManifestPath); err == nil {
		assigned, err := loadFoldOpsManifestFromFile(foldOpsAssignedManifestPath)
		if err != nil {
			return fmt.Errorf("assigned manifest: %w", err)
		}
		if err := validateFoldingOSCompatibility(assigned.MinimumFoldingOSVersion); err != nil {
			return fmt.Errorf("assigned manifest: %w", err)
		}
		fmt.Printf(
			"Supervisor-assigned FoldOps manifest %s is valid for FoldingOS %s.\n",
			assigned.ManifestRelease,
			assigned.MinimumFoldingOSVersion,
		)
	} else if !os.IsNotExist(err) {
		return err
	}
	return nil
}

func resolveEffectiveFoldOpsManifest() (foldOpsManifest, error) {
	if _, err := os.Stat(foldOpsAssignedManifestPath); err == nil {
		manifest, err := loadFoldOpsManifestFromFile(foldOpsAssignedManifestPath)
		if err != nil {
			return foldOpsManifest{}, fmt.Errorf("assigned manifest: %w", err)
		}
		return manifest, nil
	} else if !os.IsNotExist(err) {
		return foldOpsManifest{}, err
	}
	return loadFoldOpsManifestFromFile(foldOpsEmbeddedManifestPath)
}

func loadFoldOpsManifest(_ string) (foldOpsManifest, error) {
	return resolveEffectiveFoldOpsManifest()
}

func loadFoldOpsManifestFromFile(path string) (foldOpsManifest, error) {
	if path != foldOpsEmbeddedManifestPath && path != foldOpsAssignedManifestPath {
		return foldOpsManifest{}, errors.New("manifest path is not allowed")
	}
	content, err := os.ReadFile(path)
	if err != nil {
		return foldOpsManifest{}, fmt.Errorf("read manifest: %w", err)
	}
	if strings.Contains(string(content), fahManifestPlaceholder) {
		return foldOpsManifest{}, fmt.Errorf("manifest contains unresolved placeholder %q", fahManifestPlaceholder)
	}
	manifest, err := parseFoldOpsManifest(string(content))
	if err != nil {
		return foldOpsManifest{}, err
	}
	if err := validateFoldOpsManifest(manifest); err != nil {
		return foldOpsManifest{}, err
	}
	return manifest, nil
}

func parseFoldOpsManifest(content string) (foldOpsManifest, error) {
	allowedHeaderKeys := map[string]bool{
		"schema_version":            true,
		"manifest_release":          true,
		"architecture":              true,
		"artifact_format":           true,
		"minimum_foldingos_version": true,
	}
	allowedPackageKeys := map[string]bool{
		"name":              true,
		"version":           true,
		"roles":             true,
		"install_prefix":    true,
		"artifact_url":      true,
		"artifact_size":     true,
		"sha256":            true,
		"verification_path": true,
	}

	manifest := foldOpsManifest{}
	headerSeen := make(map[string]bool)
	var current foldOpsPackage
	packageSeen := make(map[string]bool)
	inPackage := false
	inRoles := false
	var roleLines []string

	flushPackage := func(lineNumber int) error {
		if !inPackage {
			return nil
		}
		for _, key := range []string{
			"name", "version", "roles", "artifact_url", "artifact_size", "sha256", "verification_path",
		} {
			if !packageSeen[key] {
				return fmt.Errorf("line %d: package is missing required key %q", lineNumber, key)
			}
		}
		manifest.Packages = append(manifest.Packages, current)
		current = foldOpsPackage{}
		packageSeen = make(map[string]bool)
		inPackage = false
		inRoles = false
		roleLines = nil
		return nil
	}

	lines := strings.Split(content, "\n")
	for number, raw := range lines {
		line := strings.TrimSpace(raw)
		if line == "" || strings.HasPrefix(line, "#") {
			continue
		}
		if strings.HasPrefix(line, "[[") {
			if line != "[[packages]]" {
				return foldOpsManifest{}, fmt.Errorf("line %d: unsupported manifest table %q", number+1, line)
			}
			if err := flushPackage(number + 1); err != nil {
				return foldOpsManifest{}, err
			}
			inPackage = true
			continue
		}
		if inRoles {
			roleLines = append(roleLines, line)
			if strings.Contains(line, "]") {
				roles, err := parseFoldOpsRoles(strings.Join(roleLines, "\n"))
				if err != nil {
					return foldOpsManifest{}, fmt.Errorf("line %d: %w", number+1, err)
				}
				if packageSeen["roles"] {
					return foldOpsManifest{}, fmt.Errorf("line %d: duplicate key %q", number+1, "roles")
				}
				current.Roles = roles
				packageSeen["roles"] = true
				inRoles = false
				roleLines = nil
			}
			continue
		}

		parts := strings.SplitN(line, "=", 2)
		if len(parts) != 2 {
			return foldOpsManifest{}, fmt.Errorf("line %d: expected key = value", number+1)
		}
		key := strings.TrimSpace(parts[0])
		value := strings.TrimSpace(parts[1])

		if inPackage {
			if !allowedPackageKeys[key] {
				return foldOpsManifest{}, fmt.Errorf("line %d: unknown package key %q", number+1, key)
			}
			if packageSeen[key] {
				return foldOpsManifest{}, fmt.Errorf("line %d: duplicate key %q", number+1, key)
			}
			if key == "roles" {
				if strings.HasPrefix(value, "[") {
					if strings.HasSuffix(value, "]") && !strings.HasSuffix(value, "[") {
						roles, err := parseFoldOpsRoles(value)
						if err != nil {
							return foldOpsManifest{}, fmt.Errorf("line %d: %w", number+1, err)
						}
						current.Roles = roles
						packageSeen["roles"] = true
						continue
					}
					inRoles = true
					roleLines = []string{value}
					if strings.Contains(value, "]") {
						roles, err := parseFoldOpsRoles(strings.Join(roleLines, "\n"))
						if err != nil {
							return foldOpsManifest{}, fmt.Errorf("line %d: %w", number+1, err)
						}
						current.Roles = roles
						packageSeen["roles"] = true
						inRoles = false
						roleLines = nil
					}
					continue
				}
				return foldOpsManifest{}, fmt.Errorf("line %d: roles must be a TOML array", number+1)
			}
			packageSeen[key] = true
			switch key {
			case "artifact_size":
				parsed, err := strconv.ParseInt(value, 10, 64)
				if err != nil || parsed <= 0 {
					return foldOpsManifest{}, fmt.Errorf("line %d: artifact_size must be a positive integer", number+1)
				}
				current.ArtifactSize = parsed
			case "name", "version", "artifact_url", "sha256", "verification_path", "install_prefix":
				parsed, err := parseQuotedString(value)
				if err != nil {
					return foldOpsManifest{}, fmt.Errorf("line %d: %q must be a quoted string", number+1, key)
				}
				switch key {
				case "name":
					current.Name = parsed
				case "version":
					current.Version = parsed
				case "artifact_url":
					current.ArtifactURL = parsed
				case "sha256":
					current.SHA256 = parsed
				case "verification_path":
					current.VerificationPath = parsed
				case "install_prefix":
					current.InstallPrefix = parsed
				}
			}
			continue
		}

		if !allowedHeaderKeys[key] {
			return foldOpsManifest{}, fmt.Errorf("line %d: unknown key %q", number+1, key)
		}
		if headerSeen[key] {
			return foldOpsManifest{}, fmt.Errorf("line %d: duplicate key %q", number+1, key)
		}
		headerSeen[key] = true
		switch key {
		case "schema_version":
			parsed, err := strconv.Atoi(value)
			if err != nil {
				return foldOpsManifest{}, fmt.Errorf("line %d: schema_version must be an integer", number+1)
			}
			manifest.SchemaVersion = parsed
		case "manifest_release", "architecture", "artifact_format", "minimum_foldingos_version":
			parsed, err := parseQuotedString(value)
			if err != nil {
				return foldOpsManifest{}, fmt.Errorf("line %d: %q must be a quoted string", number+1, key)
			}
			switch key {
			case "manifest_release":
				manifest.ManifestRelease = parsed
			case "architecture":
				manifest.Architecture = parsed
			case "artifact_format":
				manifest.ArtifactFormat = parsed
			case "minimum_foldingos_version":
				manifest.MinimumFoldingOSVersion = parsed
			}
		}
	}

	if inRoles {
		return foldOpsManifest{}, errors.New("manifest roles array is not closed")
	}
	if err := flushPackage(len(lines)); err != nil {
		return foldOpsManifest{}, err
	}
	for key := range allowedHeaderKeys {
		if !headerSeen[key] {
			return foldOpsManifest{}, fmt.Errorf("missing required key %q", key)
		}
	}
	return manifest, nil
}

func parseFoldOpsRoles(arrayLiteral string) ([]string, error) {
	arrayLiteral = strings.TrimSpace(arrayLiteral)
	if !strings.HasPrefix(arrayLiteral, "[") || !strings.HasSuffix(arrayLiteral, "]") {
		return nil, errors.New("roles must be a TOML array")
	}
	inner := strings.TrimSpace(arrayLiteral[1 : len(arrayLiteral)-1])
	if inner == "" {
		return nil, errors.New("roles must be a non-empty array")
	}
	var roles []string
	for _, segment := range splitCommaRespectingQuotes(inner) {
		segment = strings.TrimSpace(segment)
		if segment == "" {
			continue
		}
		role, err := parseQuotedString(segment)
		if err != nil {
			return nil, errors.New("roles must contain only quoted strings")
		}
		if role != "agent" && role != "supervisor" {
			return nil, errors.New("roles must contain only agent or supervisor")
		}
		roles = append(roles, role)
	}
	if len(roles) == 0 {
		return nil, errors.New("roles must be a non-empty array")
	}
	return roles, nil
}

func validateFoldOpsManifest(manifest foldOpsManifest) error {
	switch manifest.SchemaVersion {
	case foldOpsManifestSchemaVersionV1:
		if manifest.ArtifactFormat != foldOpsManifestArtifactFormatDeb {
			return errors.New("manifest schema_version 1 requires artifact_format deb")
		}
	case foldOpsManifestSchemaVersionV2:
		if manifest.ArtifactFormat != foldOpsManifestArtifactFormatLayout &&
			manifest.ArtifactFormat != foldOpsManifestArtifactFormatDeb {
			return fmt.Errorf(
				"manifest schema_version 2 requires artifact_format %q or %q",
				foldOpsManifestArtifactFormatLayout,
				foldOpsManifestArtifactFormatDeb,
			)
		}
	default:
		return errors.New("manifest schema_version must be 1 or 2")
	}
	if manifest.Architecture != foldOpsManifestArchitecture {
		return errors.New("manifest architecture must be x86_64")
	}
	if manifest.MinimumFoldingOSVersion != foldOpsManifestMinimumVersion {
		return errors.New("manifest minimum_foldingos_version must be 0.1.0")
	}
	if strings.TrimSpace(manifest.ManifestRelease) == "" {
		return errors.New("manifest manifest_release must be non-empty")
	}
	if len(manifest.Packages) == 0 {
		return errors.New("manifest packages must be non-empty")
	}

	seenNames := make(map[string]struct{})
	for _, pkg := range manifest.Packages {
		if err := validateFoldOpsPackage(manifest, pkg); err != nil {
			return err
		}
		if _, exists := seenNames[pkg.Name]; exists {
			return fmt.Errorf("duplicate package name in manifest: %s", pkg.Name)
		}
		seenNames[pkg.Name] = struct{}{}
	}
	for name := range foldOpsRequiredPackageRoles {
		if _, ok := seenNames[name]; !ok {
			return fmt.Errorf("manifest is missing required package: %s", name)
		}
	}
	return nil
}

func validateFoldOpsPackage(manifest foldOpsManifest, pkg foldOpsPackage) error {
	expectedRoles, ok := foldOpsRequiredPackageRoles[pkg.Name]
	if !ok {
		return fmt.Errorf("unexpected package name in manifest: %s", pkg.Name)
	}
	if strings.TrimSpace(pkg.Version) == "" {
		return fmt.Errorf("package %s version must be non-empty", pkg.Name)
	}
	if !rolesEqual(pkg.Roles, expectedRoles) {
		return fmt.Errorf(
			"package %s roles must be %v; found %v",
			pkg.Name,
			expectedRoles,
			pkg.Roles,
		)
	}
	if !fahSHA256Pattern.MatchString(pkg.SHA256) {
		return fmt.Errorf("package %s sha256 must be a 64-character lowercase hex digest", pkg.Name)
	}
	if pkg.ArtifactSize <= 0 {
		return fmt.Errorf("package %s artifact_size must be positive", pkg.Name)
	}

	installPrefix := strings.TrimSpace(pkg.InstallPrefix)
	if manifest.SchemaVersion == foldOpsManifestSchemaVersionV2 &&
		manifest.ArtifactFormat == foldOpsManifestArtifactFormatLayout {
		if installPrefix == "" {
			return fmt.Errorf("package %s install_prefix is required for layout-tar-zst manifests", pkg.Name)
		}
		if installPrefix != pkg.Name {
			return fmt.Errorf("package %s install_prefix must match package name %q", pkg.Name, pkg.Name)
		}
	} else if installPrefix != "" && installPrefix != pkg.Name {
		return fmt.Errorf("package %s install_prefix must match package name when present", pkg.Name)
	}

	artifactURL, err := url.Parse(pkg.ArtifactURL)
	if err != nil {
		return fmt.Errorf("package %s artifact_url is invalid: %w", pkg.Name, err)
	}
	if artifactURL.Scheme != "https" {
		return fmt.Errorf("package %s artifact_url must use HTTPS", pkg.Name)
	}
	expectedOrigin := foldOpsApprovedArtifactOrigin(manifest.ArtifactFormat)
	if artifactURL.Host != expectedOrigin {
		return fmt.Errorf(
			"package %s artifact_url must use HTTPS from the approved official origin: %s",
			pkg.Name,
			expectedOrigin,
		)
	}
	if strings.HasSuffix(artifactURL.Path, "/latest.deb") || strings.HasSuffix(artifactURL.Path, "latest.deb") {
		return fmt.Errorf("package %s artifact_url must not reference an unpinned latest artifact", pkg.Name)
	}
	if err := validateFoldOpsArtifactURLPath(manifest.ArtifactFormat, artifactURL.Path, pkg.Name); err != nil {
		return fmt.Errorf("package %s: %w", pkg.Name, err)
	}

	expectedPrefix := foldOpsVerificationPathPrefix + pkg.Name + "/"
	if err := validateFoldOpsVerificationPath(pkg.VerificationPath, expectedPrefix); err != nil {
		return fmt.Errorf("package %s: %w", pkg.Name, err)
	}
	return nil
}

func foldOpsApprovedArtifactOrigin(artifactFormat string) string {
	if artifactFormat == foldOpsManifestArtifactFormatLayout {
		return foldOpsApprovedPackagesOrigin
	}
	return foldOpsApprovedDebOrigin
}

func validateFoldOpsArtifactURLPath(artifactFormat, path, packageName string) error {
	switch artifactFormat {
	case foldOpsManifestArtifactFormatDeb:
		if !strings.Contains(path, "/"+packageName+"/") {
			return fmt.Errorf("artifact_url must reference the %s pool artifact", packageName)
		}
	case foldOpsManifestArtifactFormatLayout:
		if !strings.Contains(path, packageName) {
			return fmt.Errorf("artifact_url must reference the %s layout bundle", packageName)
		}
	default:
		return fmt.Errorf("unsupported artifact_format %q", artifactFormat)
	}
	return nil
}

func validateFoldOpsVerificationPath(path, expectedPrefix string) error {
	if !strings.HasPrefix(path, expectedPrefix) {
		return fmt.Errorf("verification_path must remain under %s", expectedPrefix)
	}
	cleaned := filepath.Clean(path)
	if cleaned != path || strings.Contains(path, "..") {
		return errors.New("verification_path must not contain path traversal")
	}
	if !strings.HasPrefix(cleaned, expectedPrefix) {
		return fmt.Errorf("verification_path must remain under %s", expectedPrefix)
	}
	return nil
}

func rolesEqual(actual, expected []string) bool {
	if len(actual) != len(expected) {
		return false
	}
	actualSorted := append([]string(nil), actual...)
	expectedSorted := append([]string(nil), expected...)
	sort.Strings(actualSorted)
	sort.Strings(expectedSorted)
	for index := range actualSorted {
		if actualSorted[index] != expectedSorted[index] {
			return false
		}
	}
	return true
}

func foldOpsPackagesForRole(manifest foldOpsManifest, role string) ([]foldOpsPackage, error) {
	var packages []foldOpsPackage
	for _, pkg := range manifest.Packages {
		for _, pkgRole := range pkg.Roles {
			if pkgRole == role {
				packages = append(packages, pkg)
				break
			}
		}
	}
	if len(packages) == 0 {
		return nil, fmt.Errorf("manifest defines no FoldOps packages for role %q", role)
	}
	return packages, nil
}
