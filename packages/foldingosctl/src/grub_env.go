package main

import (
	"bytes"
	"fmt"
	"os"
	"sort"
	"strings"
)

const grubEnvBlockSize = 1024

func setGrubEnvVar(path, key, value string) error {
	content, err := os.ReadFile(path)
	if err != nil {
		return err
	}
	vars, err := parseGrubEnvBlock(content)
	if err != nil {
		return err
	}
	vars[key] = value
	updated, err := formatGrubEnvBlock(vars)
	if err != nil {
		return err
	}
	return atomicWrite(path, updated, 0644)
}

func parseGrubEnvBlock(content []byte) (map[string]string, error) {
	if len(content) != grubEnvBlockSize {
		return nil, fmt.Errorf("grub environment has invalid size %d", len(content))
	}
	vars := make(map[string]string)
	for _, line := range strings.Split(string(content), "\n") {
		line = strings.TrimSpace(line)
		if line == "" || strings.HasPrefix(line, "#") {
			continue
		}
		key, value, ok := strings.Cut(line, "=")
		if !ok || strings.TrimSpace(key) == "" {
			continue
		}
		vars[strings.TrimSpace(key)] = value
	}
	return vars, nil
}

func formatGrubEnvBlock(vars map[string]string) ([]byte, error) {
	block := bytes.Repeat([]byte("#"), grubEnvBlockSize)
	header := []byte("# GRUB Environment Block\n")
	if len(header) >= grubEnvBlockSize {
		return nil, fmt.Errorf("grub environment header is too large")
	}
	copy(block, header)
	offset := len(header)

	keys := make([]string, 0, len(vars))
	for key := range vars {
		keys = append(keys, key)
	}
	sort.Strings(keys)

	for _, key := range keys {
		line := key + "=" + vars[key] + "\n"
		if offset+len(line) > grubEnvBlockSize {
			return nil, fmt.Errorf("grub environment block overflow")
		}
		copy(block[offset:], []byte(line))
		offset += len(line)
	}
	return block, nil
}
