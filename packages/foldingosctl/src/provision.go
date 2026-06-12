package main

import (
	"bufio"
	"bytes"
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"strconv"
	"strings"
)

const (
	provisionedKeys = "/boot/efi/foldingos/provision/authorized_keys"
	activeKeys      = "/data/config/ssh/authorized_keys"
	hostKey         = "/data/config/ssh/host-keys/ssh_host_ed25519_key"
)

func provisionSSH() error {
	if err := ensureHostKey(); err != nil {
		return err
	}
	content, err := os.ReadFile(provisionedKeys)
	if err != nil {
		if os.IsNotExist(err) {
			if existing, readErr := os.ReadFile(activeKeys); readErr == nil {
				if _, validateErr := validateAuthorizedKeys(existing); validateErr != nil {
					return fmt.Errorf("persistent SSH authorized keys are invalid: %w", validateErr)
				}
			} else if !os.IsNotExist(readErr) {
				return readErr
			}
			fmt.Println("No SSH provisioning file present; SSH remains unavailable without persistent authorized keys.")
			return nil
		}
		return err
	}
	keys, err := validateAuthorizedKeys(content)
	if err != nil {
		return err
	}
	if err := atomicWrite(activeKeys, keys, 0644); err != nil {
		return err
	}
	if err := os.Remove(provisionedKeys); err != nil {
		return err
	}
	fmt.Println("Activated provisioned SSH administrator keys.")
	return nil
}

func ensureHostKey() error {
	if info, err := os.Stat(hostKey); err == nil && info.Mode().IsRegular() {
		if err := os.Chmod(hostKey, 0600); err != nil {
			return err
		}
		publicKey, validateErr := commandInput("", "ssh-keygen", "-y", "-f", hostKey)
		if validateErr == nil {
			return atomicWrite(hostKey+".pub", []byte(strings.TrimSpace(publicKey)+"\n"), 0644)
		}
	} else if err != nil && !os.IsNotExist(err) {
		return err
	}
	if err := os.MkdirAll(filepath.Dir(hostKey), 0700); err != nil {
		return err
	}
	if err := os.Chmod(filepath.Dir(hostKey), 0700); err != nil {
		return err
	}
	temp, err := os.CreateTemp(filepath.Dir(hostKey), ".ssh_host_ed25519_key.tmp-")
	if err != nil {
		return err
	}
	tempName := temp.Name()
	if err := temp.Close(); err != nil {
		return err
	}
	if err := os.Remove(tempName); err != nil {
		return err
	}
	defer os.Remove(tempName)
	defer os.Remove(tempName + ".pub")

	if err := run("ssh-keygen", "-q", "-t", "ed25519", "-N", "", "-f", tempName); err != nil {
		return err
	}
	if err := os.Chmod(tempName, 0600); err != nil {
		return err
	}
	if err := os.Chmod(tempName+".pub", 0644); err != nil {
		return err
	}
	if err := os.Rename(tempName+".pub", hostKey+".pub"); err != nil {
		return err
	}
	if err := os.Rename(tempName, hostKey); err != nil {
		return err
	}
	return nil
}

func validateAuthorizedKeys(content []byte) ([]byte, error) {
	var accepted bytes.Buffer
	scanner := bufio.NewScanner(bytes.NewReader(content))
	count := 0
	for scanner.Scan() {
		line := strings.TrimSpace(scanner.Text())
		if line == "" || strings.HasPrefix(line, "#") {
			continue
		}
		if strings.Contains(line, "PRIVATE KEY") {
			return nil, errors.New("SSH provisioning file contains private-key material")
		}
		fields := strings.Fields(line)
		if len(fields) < 2 {
			return nil, errors.New("SSH provisioning file contains a malformed key")
		}
		switch fields[0] {
		case "ssh-ed25519", "ecdsa-sha2-nistp256", "ssh-rsa":
		default:
			return nil, fmt.Errorf("unsupported or option-prefixed SSH key type %q", fields[0])
		}
		output, err := commandInput(line+"\n", "ssh-keygen", "-lf", "-")
		if err != nil {
			return nil, err
		}
		if fields[0] == "ssh-rsa" {
			details := strings.Fields(output)
			if len(details) == 0 {
				return nil, errors.New("ssh-keygen returned no RSA key details")
			}
			bits, err := strconv.Atoi(details[0])
			if err != nil || bits < 3072 {
				return nil, errors.New("SSH RSA keys must contain at least 3072 bits")
			}
		}
		accepted.WriteString(line)
		accepted.WriteByte('\n')
		count++
	}
	if err := scanner.Err(); err != nil {
		return nil, err
	}
	if count == 0 {
		return nil, errors.New("SSH provisioning file contains no supported public keys")
	}
	return accepted.Bytes(), nil
}

func commandInput(input, name string, args ...string) (string, error) {
	cmd := execCommand(name, args...)
	cmd.Stdin = strings.NewReader(input)
	var stdout, stderr bytes.Buffer
	cmd.Stdout = &stdout
	cmd.Stderr = &stderr
	if err := cmd.Run(); err != nil {
		return "", fmt.Errorf("%s failed: %s", name, strings.TrimSpace(stderr.String()))
	}
	return stdout.String(), nil
}
