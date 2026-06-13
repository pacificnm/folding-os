package main

import (
	"debug/elf"
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"syscall"
)

var (
	fahSharedLibrarySearchPaths = []string{
		"/lib/x86_64-linux-gnu",
		"/usr/lib/x86_64-linux-gnu",
		"/lib64",
		"/usr/lib64",
		"/lib",
		"/usr/lib",
	}
	fahRequiredELFMachine = elf.EM_X86_64
	fahRequiredELFType    = elf.ET_DYN
	fahRequiredInterpreter  = "/lib64/ld-linux-x86-64.so.2"
)

func fahVerifyInstall(version string) error {
	if err := validateFAHVersionLabel(version); err != nil {
		return err
	}
	manifest, err := loadFAHManifest(embeddedFAHManifestPath)
	if err != nil {
		return err
	}
	if err := validateFoldingOSCompatibility(manifest.MinimumFoldingOSVersion); err != nil {
		return err
	}
	if version != manifest.ClientVersion {
		return fmt.Errorf(
			"version %s does not match approved manifest client %s",
			version,
			manifest.ClientVersion,
		)
	}
	if err := verifyFAHInstalledVersion(version, manifest); err != nil {
		return err
	}
	if err := writeFAHVerifiedMarker(version, manifest); err != nil {
		return err
	}
	fmt.Printf("Verified Folding@home %s installation at %s.\n", version, filepath.Join(fahAppsRoot, version))
	return nil
}

func extractAndInstallFAHArtifact(manifest fahManifest) (string, error) {
	version := manifest.ClientVersion
	stagingDir := filepath.Join(fahAppsRoot, version+".staging")
	versionDir := filepath.Join(fahAppsRoot, version)
	stagedDeb := filepath.Join(fahDownloadsDir, version+".deb")

	if fahInstallationVerified(version, manifest) {
		if err := verifyFAHInstalledVersion(version, manifest); err != nil {
			return "", err
		}
		if err := writeFAHVerifiedMarker(version, manifest); err != nil {
			return "", err
		}
		return versionDir, nil
	}

	if err := os.RemoveAll(stagingDir); err != nil {
		return "", fmt.Errorf("remove stale staging directory: %w", err)
	}
	if info, err := os.Stat(versionDir); err == nil {
		if info.IsDir() {
			if err := removeFAHPath(versionDir); err != nil {
				return "", err
			}
		} else {
			return "", fmt.Errorf("%s exists but is not a directory", versionDir)
		}
	} else if !os.IsNotExist(err) {
		return "", fmt.Errorf("inspect existing version directory: %w", err)
	}

	if _, err := os.Stat(stagedDeb); err != nil {
		return "", fmt.Errorf("staged deb artifact is missing: %w", err)
	}
	if err := fahExtractDebData(stagedDeb, stagingDir); err != nil {
		_ = removeFAHPath(stagingDir)
		return "", err
	}
	if err := normalizeFAHInstallTree(stagingDir); err != nil {
		_ = removeFAHPath(stagingDir)
		return "", err
	}
	if err := verifyFAHInstallTree(stagingDir, manifest); err != nil {
		_ = removeFAHPath(stagingDir)
		return "", err
	}
	if err := os.Rename(stagingDir, versionDir); err != nil {
		_ = removeFAHPath(stagingDir)
		return "", fmt.Errorf("promote verified installation: %w", err)
	}
	if err := verifyFAHInstalledVersion(version, manifest); err != nil {
		_ = removeFAHPath(versionDir)
		return "", err
	}
	if err := writeFAHVerifiedMarker(version, manifest); err != nil {
		_ = removeFAHPath(versionDir)
		return "", err
	}
	return versionDir, nil
}

func verifyFAHInstalledVersion(version string, manifest fahManifest) error {
	versionDir := filepath.Join(fahAppsRoot, version)
	info, err := os.Stat(versionDir)
	if err != nil {
		return fmt.Errorf("installed version directory is missing: %w", err)
	}
	if !info.IsDir() {
		return errors.New("installed version path is not a directory")
	}
	return verifyFAHInstallTree(versionDir, manifest)
}

func verifyFAHInstallTree(root string, manifest fahManifest) error {
	executable, err := fahExecutableInRoot(root, manifest.ExecutablePath)
	if err != nil {
		return err
	}
	if err := verifyFAHInstallLayout(root, executable); err != nil {
		return err
	}
	return verifyFAHExecutableELF(executable)
}

func verifyFAHInstallLayout(root, executable string) error {
	executableClean := filepath.Clean(executable)
	rootClean := filepath.Clean(root)
	if executableClean != rootClean && !strings.HasPrefix(executableClean, rootClean+string(os.PathSeparator)) {
		return errors.New("manifest executable is outside the installation directory")
	}
	if _, err := os.Stat(executableClean); err != nil {
		return fmt.Errorf("required executable is missing: %w", err)
	}
	return filepath.Walk(rootClean, func(path string, info os.FileInfo, err error) error {
		if err != nil {
			return err
		}
		relative, err := filepath.Rel(rootClean, path)
		if err != nil {
			return err
		}
		if relative == "." {
			return nil
		}
		if info.Mode()&os.ModeSymlink != 0 {
			return fmt.Errorf("symlinks are not permitted: %s", relative)
		}
		if info.Mode()&os.ModeSetuid != 0 || info.Mode()&os.ModeSetgid != 0 || info.Mode()&os.ModeSticky != 0 {
			return fmt.Errorf("special permission bits are not permitted: %s", relative)
		}
		if requireFAHRootOwnership() && !isRootOwned(info) {
			return fmt.Errorf("installed file is not owned by root:root: %s", relative)
		}
		if info.Mode().Perm()&0002 != 0 {
			return fmt.Errorf("world-writable permissions are not permitted: %s", relative)
		}
		if info.IsDir() {
			if perm := info.Mode().Perm(); perm != 0755 {
				return fmt.Errorf("directory permissions are unsafe: %s (%04o)", relative, perm)
			}
			return nil
		}
		if !info.Mode().IsRegular() {
			return fmt.Errorf("unsupported file type: %s", relative)
		}
		if info.Mode().Perm()&0111 != 0 {
			if perm := info.Mode().Perm(); perm != 0755 {
				return fmt.Errorf("executable permissions are unsafe: %s (%04o)", relative, perm)
			}
			return nil
		}
		if perm := info.Mode().Perm(); perm != 0644 {
			return fmt.Errorf("file permissions are unsafe: %s (%04o)", relative, perm)
		}
		return nil
	})
}

func normalizeFAHInstallTree(root string) error {
	return filepath.Walk(root, func(path string, info os.FileInfo, err error) error {
		if err != nil {
			return err
		}
		if info.Mode()&os.ModeSymlink != 0 {
			return fmt.Errorf("symlinks are not permitted: %s", path)
		}
		if !info.Mode().IsRegular() && !info.IsDir() {
			return fmt.Errorf("unsupported file type: %s", path)
		}
		if requireFAHRootOwnership() {
			if err := os.Chown(path, 0, 0); err != nil {
				return fmt.Errorf("normalize ownership for %s: %w", path, err)
			}
		}
		mode := info.Mode().Perm() &^ 07000
		switch {
		case info.IsDir():
			mode = 0755
		case mode&0111 != 0:
			mode = 0755
		default:
			mode = 0644
		}
		if err := os.Chmod(path, mode); err != nil {
			return fmt.Errorf("normalize permissions for %s: %w", path, err)
		}
		return nil
	})
}

func verifyFAHExecutableELF(path string) error {
	file, err := elf.Open(path)
	if err != nil {
		return fmt.Errorf("read executable ELF header: %w", err)
	}
	defer file.Close()

	if file.Machine != fahRequiredELFMachine {
		return fmt.Errorf("executable architecture %v is not x86_64", file.Machine)
	}
	if file.Type != fahRequiredELFType {
		return fmt.Errorf("executable type %v is not supported", file.Type)
	}
	interp := file.Section(".interp")
	if interp == nil {
		return errors.New("executable is missing the ELF interpreter section")
	}
	interpData, err := interp.Data()
	if err != nil {
		return fmt.Errorf("read ELF interpreter: %w", err)
	}
	interpValue := strings.TrimRight(string(interpData), "\x00")
	if interpValue != fahRequiredInterpreter {
		return fmt.Errorf("executable interpreter %q is not supported", interpValue)
	}

	libraries, err := file.ImportedLibraries()
	if err != nil {
		return fmt.Errorf("read executable shared library requirements: %w", err)
	}
	for _, library := range libraries {
		if !fahSharedLibraryExists(library) {
			return fmt.Errorf("required shared library is unavailable: %s", library)
		}
	}
	return nil
}

func fahSharedLibraryExists(name string) bool {
	for _, directory := range fahSharedLibrarySearchPaths {
		if _, err := os.Stat(filepath.Join(directory, name)); err == nil {
			return true
		}
	}
	return false
}

func requireFAHRootOwnership() bool {
	return os.Geteuid() == 0
}

func isRootOwned(info os.FileInfo) bool {
	stat, ok := info.Sys().(*syscall.Stat_t)
	if !ok {
		return false
	}
	return stat.Uid == 0 && stat.Gid == 0
}

func fahExecutableInRoot(root, manifestExecutablePath string) (string, error) {
	if !strings.HasPrefix(manifestExecutablePath, fahExecutablePathPrefix) {
		return "", errors.New("manifest executable_path is invalid")
	}
	relative := strings.TrimPrefix(manifestExecutablePath, fahExecutablePathPrefix)
	if relative == "" || strings.Contains(relative, "..") {
		return "", errors.New("manifest executable_path is invalid")
	}
	executable := filepath.Join(root, relative)
	cleaned := filepath.Clean(executable)
	rootClean := filepath.Clean(root)
	if cleaned != rootClean && !strings.HasPrefix(cleaned, rootClean+string(os.PathSeparator)) {
		return "", errors.New("resolved executable escapes installation directory")
	}
	return cleaned, nil
}

func writeFAHVerifiedMarker(version string, manifest fahManifest) error {
	markerPath := filepath.Join(fahAppsRoot, version, fahVerifiedMarkerName)
	content := strings.Join([]string{
		"client_version=" + manifest.ClientVersion,
		"artifact_sha256=" + manifest.SHA256,
	}, "\n") + "\n"
	return atomicWrite(markerPath, []byte(content), 0644)
}
