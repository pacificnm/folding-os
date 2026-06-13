package main

import (
	"archive/tar"
	"bytes"
	"os"
	"path/filepath"
	"testing"

	"github.com/ulikunitz/xz"
)

func TestExtractApprovedFAHDebArtifact(t *testing.T) {
	debPath := "/tmp/fah-manifest-work/fah-client_8.5.6_amd64.deb"
	if _, err := os.Stat(debPath); err != nil {
		t.Skip("approved FAH deb artifact is unavailable for extraction test")
	}

	destination := t.TempDir()
	if err := extractFAHDebData(debPath, destination); err != nil {
		t.Fatal(err)
	}

	executable := filepath.Join(destination, "usr", "bin", "fah-client")
	info, err := os.Stat(executable)
	if err != nil {
		t.Fatal(err)
	}
	if info.IsDir() || info.Mode().Perm()&0111 == 0 {
		t.Fatalf("expected executable fah-client, got mode %o", info.Mode().Perm())
	}
}

func TestRejectFAHTarPathTraversal(t *testing.T) {
	tarXZ, err := buildTestTarXZArchive(map[string][]byte{
		"../escape.txt": []byte("bad"),
	})
	if err != nil {
		t.Fatal(err)
	}
	deb := buildTestDebArchive(tarXZ)
	debPath := filepath.Join(t.TempDir(), "bad.deb")
	if err := os.WriteFile(debPath, deb, 0644); err != nil {
		t.Fatal(err)
	}
	if err := extractFAHDebData(debPath, t.TempDir()); err == nil {
		t.Fatal("path traversal archive was accepted")
	}
}

func TestRejectUnsupportedFAHTarEntryType(t *testing.T) {
	var tarBuffer bytes.Buffer
	tarWriter := tar.NewWriter(&tarBuffer)
	header := &tar.Header{
		Name:     "./usr/bin/link",
		Typeflag: tar.TypeSymlink,
		Linkname: "fah-client",
		Size:     0,
	}
	if err := tarWriter.WriteHeader(header); err != nil {
		t.Fatal(err)
	}
	if err := tarWriter.Close(); err != nil {
		t.Fatal(err)
	}

	var xzBuffer bytes.Buffer
	writer, err := xz.NewWriter(&xzBuffer)
	if err != nil {
		t.Fatal(err)
	}
	if _, err := writer.Write(tarBuffer.Bytes()); err != nil {
		t.Fatal(err)
	}
	if err := writer.Close(); err != nil {
		t.Fatal(err)
	}

	deb := buildTestDebArchive(xzBuffer.Bytes())
	debPath := filepath.Join(t.TempDir(), "symlink.deb")
	if err := os.WriteFile(debPath, deb, 0644); err != nil {
		t.Fatal(err)
	}
	if err := extractFAHDebData(debPath, t.TempDir()); err == nil {
		t.Fatal("symlink archive entry was accepted")
	}
}

func TestSanitizeFAHTarPath(t *testing.T) {
	if _, err := sanitizeFAHTarPath("/etc/passwd"); err == nil {
		t.Fatal("absolute path was accepted")
	}
	if _, err := sanitizeFAHTarPath("../escape"); err == nil {
		t.Fatal("path traversal was accepted")
	}
	relative, err := sanitizeFAHTarPath("./usr/bin/fah-client")
	if err != nil || relative != "usr/bin/fah-client" {
		t.Fatalf("sanitizeFAHTarPath() = %q, %v", relative, err)
	}
}
