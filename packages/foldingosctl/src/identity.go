package main

import (
	"crypto/rand"
	"encoding/hex"
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"regexp"
	"strings"
)

var uuidPattern = regexp.MustCompile(`^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$`)

func ensureIdentity() error {
	const nodeIDPath = "/data/config/node-id"
	content, err := os.ReadFile(nodeIDPath)
	if err != nil {
		if !os.IsNotExist(err) {
			return err
		}
		nodeID, err := newUUID()
		if err != nil {
			return err
		}
		if err := atomicWrite(nodeIDPath, []byte(nodeID+"\n"), 0644); err != nil {
			return err
		}
		content = []byte(nodeID)
	}
	nodeID := strings.TrimSpace(string(content))
	if !uuidPattern.MatchString(nodeID) {
		return errors.New("existing node identity is invalid")
	}

	system, err := effectiveConfig("system", false)
	if err != nil {
		return err
	}
	values, err := parseDomain("system", system, true)
	if err != nil {
		return err
	}
	hostname := values["identity.hostname"].text
	if hostname == "" {
		hostname = "folding-" + strings.ReplaceAll(nodeID, "-", "")[:6]
		generated := "schema_version = 1\n\n[identity]\nhostname = " + fmt.Sprintf("%q", hostname) + "\n"
		if err := atomicWrite(filepath.Join(configDir, "system.toml"), []byte(generated), 0644); err != nil {
			return err
		}
		if _, err := effectiveConfig("system", true); err != nil {
			return err
		}
	}
	if !hostnamePattern.MatchString(hostname) {
		return errors.New("effective hostname is invalid")
	}
	return run("hostnamectl", "set-hostname", "--static", hostname)
}

func newUUID() (string, error) {
	value := make([]byte, 16)
	if _, err := rand.Read(value); err != nil {
		return "", err
	}
	value[6] = (value[6] & 0x0f) | 0x40
	value[8] = (value[8] & 0x3f) | 0x80
	return fmt.Sprintf("%s-%s-%s-%s-%s",
		hex.EncodeToString(value[0:4]),
		hex.EncodeToString(value[4:6]),
		hex.EncodeToString(value[6:8]),
		hex.EncodeToString(value[8:10]),
		hex.EncodeToString(value[10:16])), nil
}
