package main

import (
	"crypto/sha256"
	"encoding/hex"
	"errors"
	"fmt"
	"io"
	"net/http"
	"os"
	"path/filepath"
	"strings"
	"time"
)

var (
	foldOpsHTTPClient                    = defaultFoldOpsHTTPClient()
	foldOpsCheckAcquisitionPrerequisites = requireFoldOpsAcquisitionPrerequisites
	foldOpsHasVerifiedActiveRelease      = hasVerifiedActiveFoldOpsRelease
	foldOpsNTPSynchronized               = fahNTPSynchronizedFromTimedatectl
)

func foldOpsAcquire() error {
	manifest, err := resolveEffectiveFoldOpsManifest()
	if err != nil {
		return err
	}
	if err := validateFoldingOSCompatibility(manifest.MinimumFoldingOSVersion); err != nil {
		return err
	}
	role, err := readActiveInstallationRole()
	if err != nil {
		return err
	}
	packages, err := foldOpsPackagesForRole(manifest, role)
	if err != nil {
		return err
	}
	if foldOpsHasVerifiedActiveRelease(manifest.ManifestRelease, role, packages) {
		if err := clearFoldOpsAcquireState(); err != nil {
			return err
		}
		fmt.Printf(
			"Verified FoldOps release %s is already active for role %s; acquisition not required.\n",
			manifest.ManifestRelease,
			role,
		)
		return startFoldOpsProvisionService()
	}

	state, err := loadFoldOpsAcquireState()
	if err != nil {
		return err
	}
	if deferred, remaining, err := deferFoldOpsAcquisitionAttempt(state); err != nil {
		return err
	} else if deferred {
		fmt.Printf(
			"FoldOps acquisition deferred for %s (next attempt at %s).\n",
			remaining.Round(time.Second),
			time.Unix(state.NextAttemptUnix, 0).UTC().Format(time.RFC3339),
		)
		return nil
	}

	if err := foldOpsCheckAcquisitionPrerequisites(); err != nil {
		return recordFoldOpsAcquisitionFailure(err)
	}
	if err := downloadAndStageFoldOpsPackages(manifest.ArtifactFormat, packages); err != nil {
		return recordFoldOpsAcquisitionFailure(err)
	}
	releaseDir, err := extractAndInstallFoldOpsPackages(manifest.ManifestRelease, manifest.ArtifactFormat, packages)
	if err != nil {
		return recordFoldOpsAcquisitionFailure(err)
	}
	fmt.Printf("Installed and verified FoldOps release %s at %s.\n", manifest.ManifestRelease, releaseDir)

	if err := foldOpsActivate(manifest.ManifestRelease); err != nil {
		return recordFoldOpsAcquisitionFailure(err)
	}
	if err := clearFoldOpsAcquireState(); err != nil {
		return err
	}
	return startFoldOpsProvisionService()
}

func defaultFoldOpsHTTPClient() *http.Client {
	return &http.Client{
		CheckRedirect: func(req *http.Request, via []*http.Request) error {
			return errors.New("artifact download redirects are not allowed")
		},
	}
}

func requireFoldOpsAcquisitionPrerequisites() error {
	if err := run("systemctl", "is-active", "--quiet", "network-online.target"); err != nil {
		return errors.New("network is not online")
	}
	synchronized, err := foldOpsNTPSynchronized()
	if err != nil {
		return fmt.Errorf("check time synchronization: %w", err)
	}
	if !synchronized {
		return errors.New("system time is not synchronized")
	}
	return nil
}

func hasVerifiedActiveFoldOpsRelease(release, role string, packages []foldOpsPackage) bool {
	currentRelease, err := readFoldOpsCurrentRelease()
	if err != nil {
		return false
	}
	if currentRelease != release {
		return false
	}
	return foldOpsInstallationVerified(release, role, packages)
}

func foldOpsInstallationVerified(release, role string, packages []foldOpsPackage) bool {
	markerPath := filepath.Join(foldOpsAppsRoot, release, foldOpsVerifiedMarkerName)
	content, err := os.ReadFile(markerPath)
	if err != nil {
		return false
	}
	values := parseKeyValueLines(string(content))
	if values["manifest_release"] != release {
		return false
	}
	if values["installation_role"] != role {
		return false
	}
	for _, pkg := range packages {
		if values["package_"+pkg.Name+"_sha256"] != pkg.SHA256 {
			return false
		}
	}
	for _, pkg := range packages {
		if err := verifyFoldOpsPackageTreeAtRoot(filepath.Join(foldOpsAppsRoot, release), pkg); err != nil {
			return false
		}
	}
	return true
}

func downloadAndStageFoldOpsPackages(artifactFormat string, packages []foldOpsPackage) error {
	if err := os.MkdirAll(foldOpsDownloadsDir, 0755); err != nil {
		return fmt.Errorf("create downloads directory: %w", err)
	}
	for _, pkg := range packages {
		if err := downloadAndStageFoldOpsPackage(artifactFormat, pkg); err != nil {
			return err
		}
	}
	return nil
}

func foldOpsStagedArtifactPath(artifactFormat string, pkg foldOpsPackage) string {
	suffix := ".deb"
	if artifactFormat == foldOpsManifestArtifactFormatLayout {
		suffix = ".tar.zst"
	}
	return filepath.Join(foldOpsDownloadsDir, pkg.Name+"_"+pkg.Version+suffix)
}

func foldOpsStagedDebPath(pkg foldOpsPackage) string {
	return foldOpsStagedArtifactPath(foldOpsManifestArtifactFormatDeb, pkg)
}

func downloadAndStageFoldOpsPackage(artifactFormat string, pkg foldOpsPackage) error {
	if err := os.MkdirAll(foldOpsDownloadsDir, 0755); err != nil {
		return fmt.Errorf("create downloads directory: %w", err)
	}
	partialPath := foldOpsStagedArtifactPath(artifactFormat, pkg) + ".partial"
	stagedPath := foldOpsStagedArtifactPath(artifactFormat, pkg)

	if err := os.Remove(partialPath); err != nil && !os.IsNotExist(err) {
		return fmt.Errorf("remove stale partial download: %w", err)
	}
	if err := os.Remove(stagedPath); err != nil && !os.IsNotExist(err) {
		return fmt.Errorf("remove stale staged artifact: %w", err)
	}

	if err := downloadFoldOpsPackage(pkg, partialPath); err != nil {
		_ = os.Remove(partialPath)
		return err
	}
	if err := verifyFoldOpsArtifactFile(partialPath, pkg); err != nil {
		_ = os.Remove(partialPath)
		return err
	}
	if err := os.Rename(partialPath, stagedPath); err != nil {
		_ = os.Remove(partialPath)
		return fmt.Errorf("stage verified artifact: %w", err)
	}
	fmt.Printf("Staged verified %s %s artifact at %s.\n", pkg.Name, pkg.Version, stagedPath)
	return nil
}

func downloadFoldOpsPackage(pkg foldOpsPackage, destination string) error {
	request, err := http.NewRequest(http.MethodGet, pkg.ArtifactURL, nil)
	if err != nil {
		return err
	}

	response, err := foldOpsHTTPClient.Do(request)
	if err != nil {
		return fmt.Errorf("download %s artifact: %w", pkg.Name, err)
	}
	defer response.Body.Close()

	if response.Request.URL.String() != pkg.ArtifactURL {
		return fmt.Errorf("%s artifact download resolved to an unexpected URL", pkg.Name)
	}
	if response.StatusCode != http.StatusOK {
		return fmt.Errorf("%s artifact download failed with status %s", pkg.Name, response.Status)
	}

	file, err := os.OpenFile(destination, os.O_CREATE|os.O_WRONLY|os.O_TRUNC, 0644)
	if err != nil {
		return fmt.Errorf("open partial download: %w", err)
	}
	defer file.Close()

	limited := io.LimitReader(response.Body, pkg.ArtifactSize+1)
	written, err := io.Copy(file, limited)
	if err != nil {
		return fmt.Errorf("write partial download: %w", err)
	}
	if written > pkg.ArtifactSize {
		return fmt.Errorf(
			"%s artifact download exceeded expected size %d bytes",
			pkg.Name,
			pkg.ArtifactSize,
		)
	}
	if written != pkg.ArtifactSize {
		return fmt.Errorf(
			"%s artifact download size %d does not match expected size %d",
			pkg.Name,
			written,
			pkg.ArtifactSize,
		)
	}
	if err := file.Sync(); err != nil {
		return fmt.Errorf("sync partial download: %w", err)
	}
	return nil
}

func verifyFoldOpsArtifactFile(path string, pkg foldOpsPackage) error {
	file, err := os.Open(path)
	if err != nil {
		return err
	}
	defer file.Close()

	hasher := sha256.New()
	limited := io.LimitReader(file, pkg.ArtifactSize+1)
	written, err := io.Copy(hasher, limited)
	if err != nil {
		return fmt.Errorf("hash artifact: %w", err)
	}
	if written != pkg.ArtifactSize {
		return fmt.Errorf(
			"%s artifact size %d does not match expected size %d",
			pkg.Name,
			written,
			pkg.ArtifactSize,
		)
	}
	digest := hex.EncodeToString(hasher.Sum(nil))
	if digest != pkg.SHA256 {
		return fmt.Errorf("%s artifact SHA-256 digest does not match approved manifest", pkg.Name)
	}
	return nil
}

func extractAndInstallFoldOpsPackages(release, artifactFormat string, packages []foldOpsPackage) (string, error) {
	role, err := readActiveInstallationRole()
	if err != nil {
		return "", err
	}
	if foldOpsInstallationVerified(release, role, packages) {
		return filepath.Join(foldOpsAppsRoot, release), nil
	}

	stagingRoot := filepath.Join(foldOpsAppsRoot, release+".staging")
	releaseDir := filepath.Join(foldOpsAppsRoot, release)

	if err := os.RemoveAll(stagingRoot); err != nil {
		return "", fmt.Errorf("remove stale staging directory: %w", err)
	}
	if info, err := os.Stat(releaseDir); err == nil {
		if info.IsDir() {
			if err := removeFAHPath(releaseDir); err != nil {
				return "", err
			}
		} else {
			return "", fmt.Errorf("%s exists but is not a directory", releaseDir)
		}
	} else if !os.IsNotExist(err) {
		return "", fmt.Errorf("inspect existing release directory: %w", err)
	}

	for _, pkg := range packages {
		if err := extractFoldOpsPackage(stagingRoot, artifactFormat, pkg); err != nil {
			_ = removeFAHPath(stagingRoot)
			return "", err
		}
	}
	if err := writeFoldOpsVerifiedMarker(stagingRoot, release, role, packages); err != nil {
		_ = removeFAHPath(stagingRoot)
		return "", err
	}
	for _, pkg := range packages {
		if err := verifyFoldOpsPackageTreeAtRoot(stagingRoot, pkg); err != nil {
			_ = removeFAHPath(stagingRoot)
			return "", err
		}
	}
	if err := os.Rename(stagingRoot, releaseDir); err != nil {
		_ = removeFAHPath(stagingRoot)
		return "", fmt.Errorf("promote verified installation: %w", err)
	}
	return releaseDir, nil
}

func extractFoldOpsPackage(stagingRoot, artifactFormat string, pkg foldOpsPackage) error {
	switch artifactFormat {
	case foldOpsManifestArtifactFormatDeb:
		stagedArtifact := foldOpsStagedArtifactPath(artifactFormat, pkg)
		if _, err := os.Stat(stagedArtifact); err != nil {
			return fmt.Errorf("staged deb artifact is missing: %w", err)
		}
		packageRoot := filepath.Join(stagingRoot, pkg.Name)
		if err := foldOpsExtractDebData(stagedArtifact, packageRoot); err != nil {
			return fmt.Errorf("extract %s: %w", pkg.Name, err)
		}
		if err := normalizeFAHInstallTree(packageRoot); err != nil {
			return fmt.Errorf("normalize %s install tree: %w", pkg.Name, err)
		}
	case foldOpsManifestArtifactFormatLayout:
		stagedArtifact := foldOpsStagedArtifactPath(artifactFormat, pkg)
		if _, err := os.Stat(stagedArtifact); err != nil {
			return fmt.Errorf("staged layout bundle is missing: %w", err)
		}
		installPrefix := pkg.InstallPrefix
		if installPrefix == "" {
			installPrefix = pkg.Name
		}
		if err := extractFoldOpsLayoutBundle(stagedArtifact, stagingRoot, installPrefix); err != nil {
			return fmt.Errorf("extract %s: %w", pkg.Name, err)
		}
	default:
		return fmt.Errorf("unsupported artifact_format %q", artifactFormat)
	}
	return verifyFoldOpsPackageTreeAtRoot(stagingRoot, pkg)
}

func verifyFoldOpsPackageTreeAtRoot(releaseRoot string, pkg foldOpsPackage) error {
	target, err := foldOpsVerificationTargetAtRoot(releaseRoot, pkg.VerificationPath)
	if err != nil {
		return err
	}
	info, err := os.Stat(target)
	if err != nil {
		return fmt.Errorf("%s verification path is missing: %w", pkg.Name, err)
	}
	if info.IsDir() {
		return fmt.Errorf("%s verification path must be a file", pkg.Name)
	}
	if strings.HasSuffix(target, ".html") || strings.Contains(target, "/web/") {
		return nil
	}
	return verifyFAHExecutableELF(target)
}

func foldOpsVerificationTargetAtRoot(releaseRoot, verificationPath string) (string, error) {
	if !strings.HasPrefix(verificationPath, foldOpsVerificationPathPrefix) {
		return "", errors.New("manifest verification_path is invalid")
	}
	relative := strings.TrimPrefix(verificationPath, foldOpsVerificationPathPrefix)
	if relative == "" || strings.Contains(relative, "..") {
		return "", errors.New("manifest verification_path is invalid")
	}
	target := filepath.Join(releaseRoot, relative)
	cleaned := filepath.Clean(target)
	releaseRootClean := filepath.Clean(releaseRoot)
	if cleaned != releaseRootClean && !strings.HasPrefix(cleaned, releaseRootClean+string(os.PathSeparator)) {
		return "", errors.New("resolved verification path escapes release directory")
	}
	return cleaned, nil
}

func writeFoldOpsVerifiedMarker(root, release, role string, packages []foldOpsPackage) error {
	lines := []string{
		"manifest_release=" + release,
		"installation_role=" + role,
	}
	for _, pkg := range packages {
		lines = append(lines, "package_"+pkg.Name+"_sha256="+pkg.SHA256)
	}
	markerPath := filepath.Join(root, foldOpsVerifiedMarkerName)
	return atomicWrite(markerPath, []byte(strings.Join(lines, "\n")+"\n"), 0644)
}
