package main

import (
	"archive/tar"
	"bytes"
	"errors"
	"fmt"
	"io"
	"os"
	"path/filepath"
	"strconv"
	"strings"

	"github.com/ulikunitz/xz"
)

const arMagic = "!<arch>\n"

var (
	fahExtractDebData = extractFAHDebData
)

func extractFAHDebData(debPath, destination string) error {
	if err := os.MkdirAll(destination, 0755); err != nil {
		return fmt.Errorf("create extraction directory: %w", err)
	}

	file, err := os.Open(debPath)
	if err != nil {
		return fmt.Errorf("open deb artifact: %w", err)
	}
	defer file.Close()

	magic := make([]byte, len(arMagic))
	if _, err := io.ReadFull(file, magic); err != nil {
		return fmt.Errorf("read deb archive header: %w", err)
	}
	if string(magic) != arMagic {
		return errors.New("deb artifact is not a valid ar archive")
	}

	for {
		header, err := readARMemberHeader(file)
		if err != nil {
			if errors.Is(err, io.EOF) {
				break
			}
			return err
		}
		memberName := normalizeARMemberName(header.name)
		reader := io.LimitReader(file, header.size)
		if memberName == "data.tar.xz" {
			if err := extractFAHDataTarXZ(reader, destination); err != nil {
				return err
			}
		}
		if _, err := io.Copy(io.Discard, reader); err != nil {
			return fmt.Errorf("consume deb member %q: %w", memberName, err)
		}
		if header.size%2 == 1 {
			if _, err := file.Read(make([]byte, 1)); err != nil {
				return fmt.Errorf("read deb member padding: %w", err)
			}
		}
	}
	return nil
}

type arMemberHeader struct {
	name string
	size int64
}

func readARMemberHeader(reader io.Reader) (arMemberHeader, error) {
	raw := make([]byte, 60)
	if _, err := io.ReadFull(reader, raw); err != nil {
		if errors.Is(err, io.EOF) {
			return arMemberHeader{}, io.EOF
		}
		return arMemberHeader{}, fmt.Errorf("read deb member header: %w", err)
	}
	if !bytes.Equal(raw[58:60], []byte{'`', '\n'}) {
		return arMemberHeader{}, errors.New("deb archive member header is invalid")
	}
	size, err := strconv.ParseInt(strings.TrimSpace(string(raw[48:58])), 10, 64)
	if err != nil || size < 0 {
		return arMemberHeader{}, errors.New("deb archive member size is invalid")
	}
	return arMemberHeader{
		name: strings.TrimSpace(string(raw[0:16])),
		size: size,
	}, nil
}

func normalizeARMemberName(name string) string {
	name = strings.TrimSpace(name)
	name = strings.TrimRight(name, "\x00")
	if idx := strings.Index(name, "/"); idx >= 0 {
		name = name[:idx]
	}
	return name
}

func extractFAHDataTarXZ(reader io.Reader, destination string) error {
	xzReader, err := xz.NewReader(reader)
	if err != nil {
		return fmt.Errorf("open data.tar.xz stream: %w", err)
	}
	tarReader := tar.NewReader(xzReader)
	for {
		header, err := tarReader.Next()
		if err == io.EOF {
			return nil
		}
		if err != nil {
			return fmt.Errorf("read data archive entry: %w", err)
		}
		if err := extractFAHTarEntry(destination, header, tarReader); err != nil {
			return err
		}
	}
}

func extractFAHTarEntry(destination string, header *tar.Header, reader io.Reader) error {
	relative, err := sanitizeFAHTarPath(header.Name)
	if err != nil {
		return err
	}
	if relative == "" {
		return nil
	}
	target := filepath.Join(destination, relative)
	cleaned := filepath.Clean(target)
	destinationRoot := filepath.Clean(destination)
	if cleaned != destinationRoot && !strings.HasPrefix(cleaned, destinationRoot+string(os.PathSeparator)) {
		return fmt.Errorf("archive entry escapes staging directory: %q", header.Name)
	}

	switch header.Typeflag {
	case tar.TypeDir, tar.TypeReg, tar.TypeRegA:
		if header.Typeflag == tar.TypeDir {
			return os.MkdirAll(cleaned, 0755)
		}
		if err := os.MkdirAll(filepath.Dir(cleaned), 0755); err != nil {
			return fmt.Errorf("create parent directory for %q: %w", relative, err)
		}
		mode := os.FileMode(header.Mode & 0777)
		if mode == 0 {
			mode = 0644
		}
		file, err := os.OpenFile(cleaned, os.O_CREATE|os.O_WRONLY|os.O_TRUNC, mode)
		if err != nil {
			return fmt.Errorf("create archive file %q: %w", relative, err)
		}
		limited := io.LimitReader(reader, header.Size+1)
		written, err := io.Copy(file, limited)
		closeErr := file.Close()
		if err != nil {
			return fmt.Errorf("write archive file %q: %w", relative, err)
		}
		if closeErr != nil {
			return fmt.Errorf("close archive file %q: %w", relative, closeErr)
		}
		if written != header.Size {
			return fmt.Errorf("archive file %q size %d does not match header size %d", relative, written, header.Size)
		}
		return nil
	default:
		return fmt.Errorf("unsupported archive entry type %q for %q", string(header.Typeflag), header.Name)
	}
}

func sanitizeFAHTarPath(name string) (string, error) {
	name = strings.TrimSpace(name)
	if name == "" || name == "." || name == "./" {
		return "", nil
	}
	name = strings.TrimPrefix(name, "./")
	if filepath.IsAbs(name) || strings.HasPrefix(name, "/") {
		return "", fmt.Errorf("archive entry uses an absolute path: %q", name)
	}
	cleaned := filepath.Clean(name)
	if cleaned == "." {
		return "", nil
	}
	if strings.HasPrefix(cleaned, "..") || strings.Contains(cleaned, string(os.PathSeparator)+"..") {
		return "", fmt.Errorf("archive entry contains path traversal: %q", name)
	}
	return cleaned, nil
}

func validateFAHVersionLabel(version string) error {
	if !fahClientVersionPattern.MatchString(version) {
		return errors.New("version must be a Folding@home 8.5 release")
	}
	if version != filepath.Clean(version) || strings.Contains(version, "..") || strings.ContainsAny(version, `/\`) {
		return errors.New("version must not contain path separators or traversal")
	}
	return nil
}

func removeFAHPath(path string) error {
	if err := os.RemoveAll(path); err != nil {
		return fmt.Errorf("remove %s: %w", path, err)
	}
	return nil
}

func readARMemberHeaderFromBytes(raw []byte) (arMemberHeader, error) {
	if len(raw) != 60 {
		return arMemberHeader{}, errors.New("deb archive member header has invalid length")
	}
	if !bytes.Equal(raw[58:60], []byte{'`', '\n'}) {
		return arMemberHeader{}, errors.New("deb archive member header is invalid")
	}
	size, err := strconv.ParseInt(strings.TrimSpace(string(raw[48:58])), 10, 64)
	if err != nil || size < 0 {
		return arMemberHeader{}, errors.New("deb archive member size is invalid")
	}
	return arMemberHeader{
		name: strings.TrimSpace(string(raw[0:16])),
		size: size,
	}, nil
}

func encodeARMemberHeader(name string, size int64) []byte {
	raw := make([]byte, 60)
	copy(raw[0:16], []byte(fmt.Sprintf("%-16s", name)))
	copy(raw[48:58], []byte(fmt.Sprintf("%-10d", size)))
	raw[58] = '`'
	raw[59] = '\n'
	return raw
}

func buildTestDebArchive(dataPayload []byte) []byte {
	var buffer bytes.Buffer
	buffer.WriteString(arMagic)
	for _, member := range []struct {
		name string
		data []byte
	}{
		{name: "debian-binary", data: []byte("2.0\n")},
		{name: "control.tar.xz", data: []byte{0xfd, 0x37, 0x7a, 0x58, 0x5a, 0x00}},
		{name: "data.tar.xz", data: dataPayload},
	} {
		header := encodeARMemberHeader(member.name, int64(len(member.data)))
		buffer.Write(header)
		buffer.Write(member.data)
		if len(member.data)%2 == 1 {
			buffer.WriteByte(0)
		}
	}
	return buffer.Bytes()
}

func buildTestTarXZArchive(entries map[string][]byte) ([]byte, error) {
	var tarBuffer bytes.Buffer
	tarWriter := tar.NewWriter(&tarBuffer)
	for name, content := range entries {
		header := &tar.Header{
			Name:     "./" + name,
			Mode:     0644,
			Size:     int64(len(content)),
			Typeflag: tar.TypeReg,
		}
		if strings.HasSuffix(name, "/") {
			header.Typeflag = tar.TypeDir
			header.Size = 0
			header.Name = "./" + strings.TrimSuffix(name, "/")
		}
		if err := tarWriter.WriteHeader(header); err != nil {
			return nil, err
		}
		if len(content) > 0 {
			if _, err := tarWriter.Write(content); err != nil {
				return nil, err
			}
		}
	}
	if err := tarWriter.Close(); err != nil {
		return nil, err
	}

	var xzBuffer bytes.Buffer
	writer, err := xz.NewWriter(&xzBuffer)
	if err != nil {
		return nil, err
	}
	if _, err := writer.Write(tarBuffer.Bytes()); err != nil {
		return nil, err
	}
	if err := writer.Close(); err != nil {
		return nil, err
	}
	return xzBuffer.Bytes(), nil
}
