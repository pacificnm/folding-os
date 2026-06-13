package main

import (
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestValidateRegistryEntryRejectsInvalidRolloutState(t *testing.T) {
	_, err := validateRegistryEntry(registryEntry{
		SchemaVersion:      1,
		FoldingOSVersion:   "0.1.0",
		GitRevision:        "abc",
		ImageSHA256:        strings.Repeat("a", 64),
		ImageSizeBytes:     100,
		VerificationMethod: "sha256",
		RolloutState:       "published",
		LocalImagePath:     "/data/registry/images/test.img",
	})
	if err == nil {
		t.Fatal("invalid rollout state was accepted")
	}
}

func TestSaveAndLoadRegistryEntry(t *testing.T) {
	root := t.TempDir()
	restore := setRegistryPaths(root)
	defer restore()

	entry := registryEntry{
		SchemaVersion:      1,
		FoldingOSVersion:   "0.1.0",
		GitRevision:        "deadbeef",
		ImageSHA256:        strings.Repeat("b", 64),
		ImageSizeBytes:     1024,
		VerificationMethod: "sha256",
		ImportTimestamp:    "2026-06-13T18:55:49Z",
		RolloutState:       "ready",
		LocalImagePath:     filepath.Join(root, "images", "foldingos-x86_64-0.1.0.img"),
	}
	if err := saveRegistryEntry(entry); err != nil {
		t.Fatal(err)
	}

	loaded, err := loadRegistryEntry("0.1.0")
	if err != nil {
		t.Fatal(err)
	}
	if loaded.RolloutState != "ready" || loaded.GitRevision != "deadbeef" {
		t.Fatalf("loaded entry: %+v", loaded)
	}
	index, err := loadRegistryIndex()
	if err != nil {
		t.Fatal(err)
	}
	if len(index.Versions) != 1 || index.Versions[0] != "0.1.0" {
		t.Fatalf("index versions: %#v", index.Versions)
	}
}

func TestFormatRegistryCopyProgress(t *testing.T) {
	got := formatRegistryCopyProgress(1024*1024*1024, 4*1024*1024*1024)
	want := "Registry: copying release image 1024 MiB / 4096 MiB (25%)"
	if got != want {
		t.Fatalf("formatRegistryCopyProgress() = %q, want %q", got, want)
	}
}

func TestRegistryImportBootstrapStoresVerifiedImage(t *testing.T) {
	root := t.TempDir()
	restore := setRegistryPaths(root)
	defer restore()
	restoreRole := setInstallationRolePaths(
		filepath.Join(root, "provision-role"),
		filepath.Join(root, "config", "installation-role"),
	)
	defer restoreRole()
	if err := os.MkdirAll(filepath.Join(root, "config"), 0755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(
		filepath.Join(root, "config", "installation-role"),
		[]byte("supervisor"),
		0644,
	); err != nil {
		t.Fatal(err)
	}

	restoreVersion := setInstalledFoldingOSVersionReader(func() (string, error) {
		return "0.1.0", nil
	})
	defer restoreVersion()
	restoreRevision := setEmbeddedBuildRevisionReader(func() string { return "abc123" })
	defer restoreRevision()

	digest, err := writeTestImage(filepath.Join(root, "source-disk"), []byte("foldingos-image-bytes"))
	if err != nil {
		t.Fatal(err)
	}
	restoreSize := setRegistryExpectedImageSize(int64(len("foldingos-image-bytes")))
	defer restoreSize()
	restoreProgress := setRegistryReportCopyProgress(func(int64, int64) {})
	defer restoreProgress()
	restoreDisk := setRegistryBootDiskHooks(
		func() (string, error) { return filepath.Join(root, "source-disk"), nil },
		func(_, destination string, size int64) (string, int64, error) {
			input, err := os.ReadFile(filepath.Join(root, "source-disk"))
			if err != nil {
				return "", 0, err
			}
			if err := os.WriteFile(destination, input, 0644); err != nil {
				return "", 0, err
			}
			return digest, int64(len(input)), nil
		},
	)
	defer restoreDisk()

	if err := registryImportBootstrap(); err != nil {
		t.Fatal(err)
	}
	entry, err := loadRegistryEntry("0.1.0")
	if err != nil {
		t.Fatal(err)
	}
	if entry.RolloutState != "ready" || entry.ImageSHA256 != digest {
		t.Fatalf("registry entry: %+v", entry)
	}
	if err := registryImportBootstrap(); err != nil {
		t.Fatal(err)
	}
}

func TestRegistryPollImportsVerifiedUpstreamRelease(t *testing.T) {
	root := t.TempDir()
	restore := setRegistryPaths(root)
	defer restore()
	restoreRole := setInstallationRolePaths(
		filepath.Join(root, "provision-role"),
		filepath.Join(root, "config", "installation-role"),
	)
	defer restoreRole()
	if err := os.MkdirAll(filepath.Join(root, "config"), 0755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(
		filepath.Join(root, "config", "installation-role"),
		[]byte("supervisor"),
		0644,
	); err != nil {
		t.Fatal(err)
	}
	if err := os.MkdirAll(filepath.Join(root, "config", "provision"), 0755); err != nil {
		t.Fatal(err)
	}

	payload := []byte("verified-upstream-image")
	digest, err := writeTestImage(filepath.Join(root, "upstream.img"), payload)
	if err != nil {
		t.Fatal(err)
	}
	server := httptest.NewTLSServer(http.HandlerFunc(func(writer http.ResponseWriter, request *http.Request) {
		switch request.URL.Path {
		case "/releases.json":
			manifest := upstreamReleasesManifest{
				SchemaVersion: 1,
				Releases: []upstreamRelease{{
					FoldingOSVersion: "0.2.0",
					GitRevision:      "feedface",
					ImageURL:         "https://" + request.Host + "/foldingos-x86_64-0.2.0.img",
					ImageSHA256:      digest,
					ImageSizeBytes:   int64(len(payload)),
				}},
			}
			_ = json.NewEncoder(writer).Encode(manifest)
		case "/foldingos-x86_64-0.2.0.img":
			_, _ = writer.Write(payload)
		default:
			http.NotFound(writer, request)
		}
	}))
	defer server.Close()

	if err := os.WriteFile(
		filepath.Join(root, "config", "provision", "upstream-releases.url"),
		[]byte(server.URL+"/releases.json"),
		0644,
	); err != nil {
		t.Fatal(err)
	}

	restoreClient := setRegistryHTTPClient(server.Client())
	defer restoreClient()

	if err := registryPoll(); err != nil {
		t.Fatal(err)
	}
	entry, err := loadRegistryEntry("0.2.0")
	if err != nil {
		t.Fatal(err)
	}
	if entry.RolloutState != "ready" || entry.ImageSHA256 != digest {
		t.Fatalf("registry entry: %+v", entry)
	}
}

func TestRegistryPollRejectsChecksumMismatch(t *testing.T) {
	root := t.TempDir()
	restore := setRegistryPaths(root)
	defer restore()
	restoreRole := setInstallationRolePaths(
		filepath.Join(root, "provision-role"),
		filepath.Join(root, "config", "installation-role"),
	)
	defer restoreRole()
	if err := os.MkdirAll(filepath.Join(root, "config"), 0755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(
		filepath.Join(root, "config", "installation-role"),
		[]byte("supervisor"),
		0644,
	); err != nil {
		t.Fatal(err)
	}

	payload := []byte("bad-upstream-image")
	server := httptest.NewTLSServer(http.HandlerFunc(func(writer http.ResponseWriter, request *http.Request) {
		switch request.URL.Path {
		case "/releases.json":
			manifest := upstreamReleasesManifest{
				SchemaVersion: 1,
				Releases: []upstreamRelease{{
					FoldingOSVersion: "0.2.0",
					GitRevision:      "feedface",
					ImageURL:         "https://" + request.Host + "/foldingos-x86_64-0.2.0.img",
					ImageSHA256:      strings.Repeat("c", 64),
					ImageSizeBytes:   int64(len(payload)),
				}},
			}
			_ = json.NewEncoder(writer).Encode(manifest)
		case "/foldingos-x86_64-0.2.0.img":
			_, _ = writer.Write(payload)
		default:
			http.NotFound(writer, request)
		}
	}))
	defer server.Close()

	restoreManifest := setRegistryFetchUpstreamManifest(func(url string) (upstreamReleasesManifest, error) {
		return fetchUpstreamReleasesManifest(url)
	})
	defer restoreManifest()
	restoreClient := setRegistryHTTPClient(server.Client())
	defer restoreClient()

	release := upstreamRelease{
		FoldingOSVersion: "0.2.0",
		GitRevision:      "feedface",
		ImageURL:         server.URL + "/foldingos-x86_64-0.2.0.img",
		ImageSHA256:      strings.Repeat("c", 64),
		ImageSizeBytes:   int64(len(payload)),
	}
	if _, err := importUpstreamRelease(release); err == nil {
		t.Fatal("checksum mismatch was accepted")
	}
	if _, err := os.Stat(registryImagePath("0.2.0")); !os.IsNotExist(err) {
		t.Fatal("failed upstream import left an image file behind")
	}
}

func writeTestImage(path string, payload []byte) (string, error) {
	if err := os.WriteFile(path, payload, 0644); err != nil {
		return "", err
	}
	sum := sha256.Sum256(payload)
	return hex.EncodeToString(sum[:]), nil
}

func setRegistryPaths(root string) func() {
	previous := struct {
		dir, images, entries, index, upstream string
	}{
		registryDir,
		registryImagesDir,
		registryEntriesDir,
		registryIndexPath,
		upstreamReleasesURLPath,
	}
	registryDir = root
	registryImagesDir = filepath.Join(root, "images")
	registryEntriesDir = filepath.Join(root, "entries")
	registryIndexPath = filepath.Join(root, "index.json")
	upstreamReleasesURLPath = filepath.Join(root, "config", "provision", "upstream-releases.url")
	return func() {
		registryDir = previous.dir
		registryImagesDir = previous.images
		registryEntriesDir = previous.entries
		registryIndexPath = previous.index
		upstreamReleasesURLPath = previous.upstream
	}
}

func setRegistryReportCopyProgress(report func(int64, int64)) func() {
	previous := registryReportCopyProgress
	registryReportCopyProgress = report
	return func() {
		registryReportCopyProgress = previous
	}
}

func setRegistryExpectedImageSize(size int64) func() {
	previous := registryExpectedImageSizeBytes
	registryExpectedImageSizeBytes = size
	return func() {
		registryExpectedImageSizeBytes = previous
	}
}

func setRegistryBootDiskHooks(
	resolve func() (string, error),
	copyImage func(string, string, int64) (string, int64, error),
) func() {
	previousResolve := registryResolveBootDisk
	previousCopy := registryCopyBootDiskImage
	registryResolveBootDisk = resolve
	registryCopyBootDiskImage = copyImage
	return func() {
		registryResolveBootDisk = previousResolve
		registryCopyBootDiskImage = previousCopy
	}
}

func setRegistryHTTPClient(client *http.Client) func() {
	previous := registryHTTPClient
	registryHTTPClient = client
	return func() {
		registryHTTPClient = previous
	}
}

func setRegistryFetchUpstreamManifest(fetch func(string) (upstreamReleasesManifest, error)) func() {
	previous := registryFetchUpstreamManifest
	registryFetchUpstreamManifest = fetch
	return func() {
		registryFetchUpstreamManifest = previous
	}
}

func setInstalledFoldingOSVersionReader(reader func() (string, error)) func() {
	previous := installedFoldingOSVersionReader
	installedFoldingOSVersionReader = reader
	return func() {
		installedFoldingOSVersionReader = previous
	}
}

func setEmbeddedBuildRevisionReader(reader func() string) func() {
	previous := embeddedBuildRevisionReader
	embeddedBuildRevisionReader = reader
	return func() {
		embeddedBuildRevisionReader = previous
	}
}
