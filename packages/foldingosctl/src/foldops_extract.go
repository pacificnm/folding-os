package main

import (
	"archive/tar"
	"bytes"
	"errors"
	"fmt"
	"io"
	"os"

	"github.com/klauspost/compress/zstd"
	"github.com/ulikunitz/xz"
)

var foldOpsExtractDebData = extractFoldOpsDebData

func extractFoldOpsDebData(debPath, destination string) error {
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
		switch memberName {
		case "data.tar.xz":
			if err := extractFoldOpsDataTarXZ(reader, destination); err != nil {
				return err
			}
		case "data.tar.zst":
			if err := extractFoldOpsDataTarZST(reader, destination); err != nil {
				return err
			}
		default:
			if _, err := io.Copy(io.Discard, reader); err != nil {
				return fmt.Errorf("consume deb member %q: %w", memberName, err)
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

func extractFoldOpsDataTarXZ(reader io.Reader, destination string) error {
	xzReader, err := xz.NewReader(reader)
	if err != nil {
		return fmt.Errorf("open data.tar.xz stream: %w", err)
	}
	return extractFoldOpsTarArchive(xzReader, destination)
}

func extractFoldOpsDataTarZST(reader io.Reader, destination string) error {
	zstdReader, err := zstd.NewReader(reader)
	if err != nil {
		return fmt.Errorf("open data.tar.zst stream: %w", err)
	}
	defer zstdReader.Close()
	return extractFoldOpsTarArchive(zstdReader, destination)
}

func extractFoldOpsTarArchive(reader io.Reader, destination string) error {
	tarReader := tar.NewReader(reader)
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

func buildTestDebArchiveWithDataMember(memberName string, dataPayload []byte) []byte {
	var buffer bytes.Buffer
	buffer.WriteString(arMagic)
	for _, member := range []struct {
		name string
		data []byte
	}{
		{name: "debian-binary", data: []byte("2.0\n")},
		{name: "control.tar.xz", data: []byte{0xfd, 0x37, 0x7a, 0x58, 0x5a, 0x00}},
		{name: memberName, data: dataPayload},
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

func buildTestTarZSTArchive(entries map[string][]byte) ([]byte, error) {
	tarPayload, err := buildTestTarXZArchive(entries)
	if err != nil {
		return nil, err
	}
	xzReader, err := xz.NewReader(bytes.NewReader(tarPayload))
	if err != nil {
		return nil, err
	}
	tarBytes, err := io.ReadAll(xzReader)
	if err != nil {
		return nil, err
	}

	var zstdBuffer bytes.Buffer
	writer, err := zstd.NewWriter(&zstdBuffer)
	if err != nil {
		return nil, err
	}
	if _, err := writer.Write(tarBytes); err != nil {
		return nil, err
	}
	if err := writer.Close(); err != nil {
		return nil, err
	}
	return zstdBuffer.Bytes(), nil
}

func writeTestDebArtifact(path, memberName string, dataPayload []byte) error {
	return os.WriteFile(path, buildTestDebArchiveWithDataMember(memberName, dataPayload), 0644)
}
