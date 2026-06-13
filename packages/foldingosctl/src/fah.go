package main

import (
	"errors"
	"fmt"
	"net/url"
	"os"
	"path/filepath"
	"regexp"
	"strconv"
	"strings"
)

const (
	embeddedFAHManifestPath   = "/usr/share/foldingos/manifests/fah.toml"
	fahVerifiedMarkerName     = ".foldingos-verified"
	fahManifestPlaceholder    = "REQUIRED_BEFORE_RELEASE"
	fahApprovedArtifactOrigin    = "download.foldingathome.org"
	fahExecutablePathPrefix      = "/data/apps/fah/current/"
	fahManifestSchemaVersion     = 1
	fahManifestArchitecture      = "x86_64"
	fahManifestArtifactFormat    = "deb"
	fahManifestMinimumVersion    = "0.1.0"
	fahClientVersionMajorPattern = `^8\.5\.[0-9]+$`
)

var (
	fahAppsRoot     = "/data/apps/fah"
	fahDownloadsDir = "/data/apps/fah/.downloads"
)

var (
	fahSHA256Pattern        = regexp.MustCompile(`^[0-9a-f]{64}$`)
	fahClientVersionPattern = regexp.MustCompile(fahClientVersionMajorPattern)
)

type fahManifest struct {
	SchemaVersion           int
	ClientVersion           string
	Architecture            string
	ArtifactURL             string
	ArtifactSize            int64
	SHA256                  string
	ArtifactFormat          string
	MinimumFoldingOSVersion string
	TermsURL                string
	ExecutablePath          string
	Arguments               []string
}

func validateFAHManifestEmbedded() error {
	manifest, err := loadFAHManifest(embeddedFAHManifestPath)
	if err != nil {
		return err
	}
	if err := validateFoldingOSCompatibility(manifest.MinimumFoldingOSVersion); err != nil {
		return err
	}
	fmt.Printf(
		"Approved Folding@home manifest %s is valid for FoldingOS %s.\n",
		manifest.ClientVersion,
		manifest.MinimumFoldingOSVersion,
	)
	return nil
}

func loadFAHManifest(path string) (fahManifest, error) {
	if path != embeddedFAHManifestPath {
		return fahManifest{}, errors.New("v0.1.0 accepts only the embedded approved manifest")
	}
	content, err := os.ReadFile(path)
	if err != nil {
		return fahManifest{}, fmt.Errorf("read manifest: %w", err)
	}
	if strings.Contains(string(content), fahManifestPlaceholder) {
		return fahManifest{}, fmt.Errorf("manifest contains unresolved placeholder %q", fahManifestPlaceholder)
	}
	manifest, err := parseFAHManifest(string(content))
	if err != nil {
		return fahManifest{}, err
	}
	if err := validateFAHManifest(manifest); err != nil {
		return fahManifest{}, err
	}
	return manifest, nil
}

func parseFAHManifest(content string) (fahManifest, error) {
	allowedKeys := map[string]bool{
		"schema_version":            true,
		"client_version":            true,
		"architecture":              true,
		"artifact_url":              true,
		"artifact_size":             true,
		"sha256":                    true,
		"artifact_format":           true,
		"minimum_foldingos_version": true,
		"terms_url":                 true,
		"executable_path":           true,
		"arguments":                 true,
	}

	manifest := fahManifest{}
	seen := make(map[string]bool)
	lines := strings.Split(content, "\n")
	inArguments := false
	var argumentLines []string

	for number, raw := range lines {
		line := strings.TrimSpace(raw)
		if line == "" || strings.HasPrefix(line, "#") {
			continue
		}
		if strings.HasPrefix(line, "[") {
			return fahManifest{}, fmt.Errorf("line %d: manifest tables are not supported", number+1)
		}
		if inArguments {
			argumentLines = append(argumentLines, line)
			if strings.Contains(line, "]") {
				arguments, err := parseFAHManifestArguments(strings.Join(argumentLines, "\n"))
				if err != nil {
					return fahManifest{}, fmt.Errorf("line %d: %w", number+1, err)
				}
				if seen["arguments"] {
					return fahManifest{}, fmt.Errorf("line %d: duplicate key %q", number+1, "arguments")
				}
				manifest.Arguments = arguments
				seen["arguments"] = true
				inArguments = false
				argumentLines = nil
			}
			continue
		}
		if strings.HasPrefix(line, "arguments = [") {
			if seen["arguments"] {
				return fahManifest{}, fmt.Errorf("line %d: duplicate key %q", number+1, "arguments")
			}
			if strings.HasSuffix(line, "]") && !strings.HasSuffix(line, "[") {
				arguments, err := parseFAHManifestArguments(line[len("arguments = "):])
				if err != nil {
					return fahManifest{}, fmt.Errorf("line %d: %w", number+1, err)
				}
				manifest.Arguments = arguments
				seen["arguments"] = true
				continue
			}
			inArguments = true
			argumentLines = []string{line[len("arguments = "):]}
			if strings.Contains(line, "]") {
				arguments, err := parseFAHManifestArguments(strings.Join(argumentLines, "\n"))
				if err != nil {
					return fahManifest{}, fmt.Errorf("line %d: %w", number+1, err)
				}
				manifest.Arguments = arguments
				seen["arguments"] = true
				inArguments = false
				argumentLines = nil
			}
			continue
		}

		parts := strings.SplitN(line, "=", 2)
		if len(parts) != 2 {
			return fahManifest{}, fmt.Errorf("line %d: expected key = value", number+1)
		}
		key := strings.TrimSpace(parts[0])
		if !allowedKeys[key] {
			return fahManifest{}, fmt.Errorf("line %d: unknown key %q", number+1, key)
		}
		if seen[key] {
			return fahManifest{}, fmt.Errorf("line %d: duplicate key %q", number+1, key)
		}
		seen[key] = true
		value := strings.TrimSpace(parts[1])

		switch key {
		case "schema_version":
			parsed, err := strconv.Atoi(value)
			if err != nil {
				return fahManifest{}, fmt.Errorf("line %d: schema_version must be an integer", number+1)
			}
			manifest.SchemaVersion = parsed
		case "artifact_size":
			parsed, err := strconv.ParseInt(value, 10, 64)
			if err != nil || parsed <= 0 {
				return fahManifest{}, fmt.Errorf("line %d: artifact_size must be a positive integer", number+1)
			}
			manifest.ArtifactSize = parsed
		case "client_version":
			parsed, err := parseQuotedString(value)
			if err != nil {
				return fahManifest{}, fmt.Errorf("line %d: client_version must be a quoted string", number+1)
			}
			manifest.ClientVersion = parsed
		case "architecture":
			parsed, err := parseQuotedString(value)
			if err != nil {
				return fahManifest{}, fmt.Errorf("line %d: architecture must be a quoted string", number+1)
			}
			manifest.Architecture = parsed
		case "artifact_url":
			parsed, err := parseQuotedString(value)
			if err != nil {
				return fahManifest{}, fmt.Errorf("line %d: artifact_url must be a quoted string", number+1)
			}
			manifest.ArtifactURL = parsed
		case "sha256":
			parsed, err := parseQuotedString(value)
			if err != nil {
				return fahManifest{}, fmt.Errorf("line %d: sha256 must be a quoted string", number+1)
			}
			manifest.SHA256 = parsed
		case "artifact_format":
			parsed, err := parseQuotedString(value)
			if err != nil {
				return fahManifest{}, fmt.Errorf("line %d: artifact_format must be a quoted string", number+1)
			}
			manifest.ArtifactFormat = parsed
		case "minimum_foldingos_version":
			parsed, err := parseQuotedString(value)
			if err != nil {
				return fahManifest{}, fmt.Errorf("line %d: minimum_foldingos_version must be a quoted string", number+1)
			}
			manifest.MinimumFoldingOSVersion = parsed
		case "terms_url":
			parsed, err := parseQuotedString(value)
			if err != nil {
				return fahManifest{}, fmt.Errorf("line %d: terms_url must be a quoted string", number+1)
			}
			manifest.TermsURL = parsed
		case "executable_path":
			parsed, err := parseQuotedString(value)
			if err != nil {
				return fahManifest{}, fmt.Errorf("line %d: executable_path must be a quoted string", number+1)
			}
			manifest.ExecutablePath = parsed
		default:
			return fahManifest{}, fmt.Errorf("line %d: unknown key %q", number+1, key)
		}
	}

	if inArguments {
		return fahManifest{}, errors.New("manifest arguments array is not closed")
	}
	for key := range allowedKeys {
		if !seen[key] {
			return fahManifest{}, fmt.Errorf("missing required key %q", key)
		}
	}
	return manifest, nil
}

func parseFAHManifestArguments(arrayLiteral string) ([]string, error) {
	arrayLiteral = strings.TrimSpace(arrayLiteral)
	if !strings.HasPrefix(arrayLiteral, "[") || !strings.HasSuffix(arrayLiteral, "]") {
		return nil, errors.New("arguments must be a TOML array")
	}
	inner := strings.TrimSpace(arrayLiteral[1 : len(arrayLiteral)-1])
	if inner == "" {
		return nil, errors.New("arguments must be a non-empty array")
	}

	var arguments []string
	for _, segment := range splitCommaRespectingQuotes(inner) {
		segment = strings.TrimSpace(segment)
		if segment == "" {
			continue
		}
		argument, err := parseQuotedString(segment)
		if err != nil {
			return nil, errors.New("arguments must contain only quoted strings")
		}
		arguments = append(arguments, argument)
	}
	if len(arguments) == 0 {
		return nil, errors.New("arguments must be a non-empty array")
	}
	return arguments, nil
}

func splitCommaRespectingQuotes(input string) []string {
	var segments []string
	var current strings.Builder
	inString := false
	for index := 0; index < len(input); index++ {
		ch := input[index]
		switch ch {
		case '"':
			inString = !inString
			current.WriteByte(ch)
		case ',':
			if inString {
				current.WriteByte(ch)
				continue
			}
			segments = append(segments, current.String())
			current.Reset()
		default:
			current.WriteByte(ch)
		}
	}
	if inString {
		return nil
	}
	if current.Len() > 0 {
		segments = append(segments, current.String())
	}
	return segments
}

func parseQuotedString(value string) (string, error) {
	parsed, err := strconv.Unquote(value)
	if err != nil || parsed == "" {
		return "", errors.New("expected non-empty quoted string")
	}
	return parsed, nil
}

func validateFAHManifest(manifest fahManifest) error {
	if manifest.SchemaVersion != fahManifestSchemaVersion {
		return errors.New("manifest schema_version must be 1")
	}
	if manifest.Architecture != fahManifestArchitecture {
		return errors.New("manifest architecture must be x86_64")
	}
	if manifest.ArtifactFormat != fahManifestArtifactFormat {
		return errors.New("manifest artifact_format must be deb")
	}
	if manifest.MinimumFoldingOSVersion != fahManifestMinimumVersion {
		return errors.New("manifest minimum_foldingos_version must be 0.1.0")
	}
	if !fahClientVersionPattern.MatchString(manifest.ClientVersion) {
		return errors.New("manifest client_version must be a Folding@home 8.5 release")
	}
	if !fahSHA256Pattern.MatchString(manifest.SHA256) {
		return errors.New("manifest sha256 must be a 64-character lowercase hex digest")
	}

	artifactURL, err := url.Parse(manifest.ArtifactURL)
	if err != nil || artifactURL.Scheme != "https" || artifactURL.Host != fahApprovedArtifactOrigin {
		return fmt.Errorf(
			"manifest artifact_url must use HTTPS from the approved official origin: %s",
			fahApprovedArtifactOrigin,
		)
	}
	if strings.HasSuffix(artifactURL.Path, "/latest.deb") || strings.HasSuffix(artifactURL.Path, "latest.deb") {
		return errors.New("manifest artifact_url must not reference an unpinned latest artifact")
	}

	termsURL, err := url.Parse(manifest.TermsURL)
	if err != nil || termsURL.Scheme != "https" || !strings.HasSuffix(termsURL.Host, "foldingathome.org") {
		return errors.New("manifest terms_url must use HTTPS on foldingathome.org")
	}

	if err := validateFAHExecutablePath(manifest.ExecutablePath); err != nil {
		return err
	}
	for _, argument := range manifest.Arguments {
		if strings.TrimSpace(argument) == "" {
			return errors.New("manifest arguments must contain only non-empty strings")
		}
	}
	return nil
}

func validateFAHExecutablePath(path string) error {
	if !strings.HasPrefix(path, fahExecutablePathPrefix) {
		return errors.New("manifest executable_path must remain under /data/apps/fah/current")
	}
	cleaned := filepath.Clean(path)
	if cleaned != path || strings.Contains(path, "..") {
		return errors.New("manifest executable_path must not contain path traversal")
	}
	if !strings.HasPrefix(cleaned, fahExecutablePathPrefix) {
		return errors.New("manifest executable_path must remain under /data/apps/fah/current")
	}
	return nil
}

func validateFoldingOSCompatibility(minimumVersion string) error {
	currentVersion, err := osReleaseValue("VERSION_ID")
	if err != nil {
		return err
	}
	if currentVersion != minimumVersion {
		return fmt.Errorf(
			"manifest requires FoldingOS %s but image reports %s",
			minimumVersion,
			currentVersion,
		)
	}
	return nil
}
