package main

import (
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"net/http"
	"os"
	"path/filepath"
	"regexp"
	"strings"
	"time"
)

var (
	registryHTTPClient = defaultRegistryHTTPClient()
	sha256Pattern      = regexp.MustCompile(`^[0-9a-f]{64}$`)
)

func defaultRegistryHTTPClient() *http.Client {
	return &http.Client{
		CheckRedirect: func(req *http.Request, via []*http.Request) error {
			return errors.New("registry download redirects are not allowed")
		},
	}
}

var registryFetchUpstreamManifest = fetchUpstreamReleasesManifest

func registryPoll() error {
	if err := requireSupervisorRole(); err != nil {
		return err
	}

	upstreamURL, err := readUpstreamReleasesURL()
	if err != nil {
		return err
	}
	if upstreamURL == "" {
		fmt.Println("Upstream releases URL is not configured; polling skipped.")
		return nil
	}

	manifest, err := registryFetchUpstreamManifest(upstreamURL)
	if err != nil {
		return err
	}

	imported := 0
	for _, release := range manifest.Releases {
		added, err := importUpstreamRelease(release)
		if err != nil {
			return err
		}
		if added {
			imported++
		}
	}
	if imported == 0 {
		fmt.Println("Upstream poll completed; no new verified images were imported.")
		return nil
	}
	fmt.Printf("Upstream poll imported %d verified image(s).\n", imported)
	return nil
}

func fetchUpstreamReleasesManifest(url string) (upstreamReleasesManifest, error) {
	request, err := http.NewRequest(http.MethodGet, url, nil)
	if err != nil {
		return upstreamReleasesManifest{}, err
	}
	response, err := registryHTTPClient.Do(request)
	if err != nil {
		return upstreamReleasesManifest{}, err
	}
	defer response.Body.Close()
	if response.StatusCode != http.StatusOK {
		return upstreamReleasesManifest{}, fmt.Errorf("upstream manifest request failed with status %s", response.Status)
	}
	body, err := io.ReadAll(io.LimitReader(response.Body, 1<<20))
	if err != nil {
		return upstreamReleasesManifest{}, err
	}
	var manifest upstreamReleasesManifest
	if err := json.Unmarshal(body, &manifest); err != nil {
		return upstreamReleasesManifest{}, fmt.Errorf("invalid upstream releases manifest: %w", err)
	}
	if manifest.SchemaVersion != 1 {
		return upstreamReleasesManifest{}, fmt.Errorf("unsupported upstream manifest schema version %d", manifest.SchemaVersion)
	}
	return manifest, nil
}

var registryDownloadVerifiedImage = downloadVerifiedRegistryImage

func importUpstreamRelease(release upstreamRelease) (bool, error) {
	release.FoldingOSVersion = strings.TrimSpace(release.FoldingOSVersion)
	release.GitRevision = strings.TrimSpace(release.GitRevision)
	release.ImageURL = strings.TrimSpace(release.ImageURL)
	release.ImageSHA256 = strings.ToLower(strings.TrimSpace(release.ImageSHA256))
	if release.FoldingOSVersion == "" || release.GitRevision == "" || release.ImageURL == "" {
		return false, errors.New("upstream release entry is incomplete")
	}
	if !strings.HasPrefix(release.ImageURL, "https://") {
		return false, fmt.Errorf("upstream image URL must use HTTPS: %q", release.ImageURL)
	}
	if !sha256Pattern.MatchString(release.ImageSHA256) {
		return false, errors.New("upstream release image_sha256 is invalid")
	}
	if release.ImageSizeBytes <= 0 {
		return false, errors.New("upstream release image_size_bytes must be positive")
	}

	if existing, err := loadRegistryEntry(release.FoldingOSVersion); err == nil {
		if existing.ImageSHA256 == release.ImageSHA256 {
			return false, nil
		}
		return false, fmt.Errorf(
			"registry already contains version %s with a different image digest",
			release.FoldingOSVersion,
		)
	} else if !os.IsNotExist(err) {
		return false, err
	}

	imagePath := registryImagePath(release.FoldingOSVersion)
	if err := os.MkdirAll(filepath.Dir(imagePath), 0755); err != nil {
		return false, err
	}
	if err := registryDownloadVerifiedImage(release, imagePath); err != nil {
		return false, err
	}

	entry := registryEntry{
		SchemaVersion:      1,
		FoldingOSVersion:   release.FoldingOSVersion,
		GitRevision:        release.GitRevision,
		ImageSHA256:        release.ImageSHA256,
		ImageSizeBytes:     release.ImageSizeBytes,
		RetrievalURL:       release.ImageURL,
		VerificationMethod: "sha256",
		ImportTimestamp:    time.Now().UTC().Format(time.RFC3339),
		RolloutState:       "ready",
		LocalImagePath:     imagePath,
	}
	if err := saveRegistryEntry(entry); err != nil {
		os.Remove(imagePath)
		return false, err
	}
	fmt.Printf("Imported verified upstream image for FoldingOS %s.\n", release.FoldingOSVersion)
	return true, nil
}

func downloadVerifiedRegistryImage(release upstreamRelease, destination string) error {
	temp, err := os.CreateTemp(filepath.Dir(destination), ".registry-download.tmp-")
	if err != nil {
		return err
	}
	tempPath := temp.Name()
	cleanup := true
	defer func() {
		if cleanup {
			os.Remove(tempPath)
		}
	}()

	request, err := http.NewRequest(http.MethodGet, release.ImageURL, nil)
	if err != nil {
		temp.Close()
		return err
	}
	response, err := registryHTTPClient.Do(request)
	if err != nil {
		temp.Close()
		return err
	}
	defer response.Body.Close()
	if response.StatusCode != http.StatusOK {
		temp.Close()
		return fmt.Errorf("image download failed with status %s", response.Status)
	}

	hasher := sha256.New()
	writer := io.MultiWriter(temp, hasher)
	written, err := io.CopyN(writer, response.Body, release.ImageSizeBytes)
	if err != nil {
		temp.Close()
		return fmt.Errorf("download image: %w", err)
	}
	if written != release.ImageSizeBytes {
		temp.Close()
		return fmt.Errorf("downloaded image size %d does not match expected %d", written, release.ImageSizeBytes)
	}
	var extra [1]byte
	if n, _ := response.Body.Read(extra[:]); n > 0 {
		temp.Close()
		return errors.New("downloaded image exceeds declared size")
	}
	if err := temp.Sync(); err != nil {
		temp.Close()
		return err
	}
	if err := temp.Close(); err != nil {
		return err
	}
	actual := hex.EncodeToString(hasher.Sum(nil))
	if actual != release.ImageSHA256 {
		return errors.New("downloaded image failed SHA-256 verification")
	}
	if err := os.Rename(tempPath, destination); err != nil {
		return err
	}
	cleanup = false
	return nil
}
