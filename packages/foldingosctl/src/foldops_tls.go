package main

import (
	"crypto/ecdsa"
	"crypto/elliptic"
	"crypto/rand"
	"crypto/x509"
	"crypto/x509/pkix"
	"encoding/pem"
	"errors"
	"fmt"
	"math/big"
	"net"
	"os"
	"path/filepath"
	"time"
)

const foldOpsTLSCertLifetime = 365 * 24 * time.Hour

var (
	foldOpsGenerateTLSMaterial = generateFoldOpsSelfSignedTLS
	foldOpsTLSNow              = time.Now
)

func ensureFoldOpsTLSMaterial() error {
	certPath := filepath.Join(foldOpsTLSDir, "cert.pem")
	keyPath := filepath.Join(foldOpsTLSDir, "key.pem")
	caPath := filepath.Join(foldOpsTLSDir, "ca.pem")
	if fileExists(certPath) && fileExists(keyPath) && fileExists(caPath) {
		return nil
	}
	hostname, err := readHostname()
	if err != nil {
		return err
	}
	return foldOpsGenerateTLSMaterial(hostname)
}

func generateFoldOpsSelfSignedTLS(hostname string) error {
	if err := os.MkdirAll(foldOpsTLSDir, 0750); err != nil {
		return fmt.Errorf("create TLS directory: %w", err)
	}

	privateKey, err := ecdsa.GenerateKey(elliptic.P256(), rand.Reader)
	if err != nil {
		return fmt.Errorf("generate TLS private key: %w", err)
	}

	serial, err := rand.Int(rand.Reader, new(big.Int).Lsh(big.NewInt(1), 128))
	if err != nil {
		return fmt.Errorf("generate TLS serial number: %w", err)
	}

	notBefore := foldOpsTLSNow().UTC()
	notAfter := notBefore.Add(foldOpsTLSCertLifetime)
	template := x509.Certificate{
		SerialNumber: serial,
		Subject: pkix.Name{
			CommonName: hostname,
		},
		NotBefore:             notBefore,
		NotAfter:              notAfter,
		KeyUsage:              x509.KeyUsageDigitalSignature | x509.KeyUsageKeyEncipherment,
		ExtKeyUsage:           []x509.ExtKeyUsage{x509.ExtKeyUsageServerAuth},
		BasicConstraintsValid: true,
		DNSNames:              []string{hostname},
		IPAddresses:           []net.IP{net.ParseIP("127.0.0.1")},
	}

	der, err := x509.CreateCertificate(rand.Reader, &template, &template, &privateKey.PublicKey, privateKey)
	if err != nil {
		return fmt.Errorf("create TLS certificate: %w", err)
	}

	certPEM := pem.EncodeToMemory(&pem.Block{Type: "CERTIFICATE", Bytes: der})
	keyDER, err := x509.MarshalECPrivateKey(privateKey)
	if err != nil {
		return fmt.Errorf("marshal TLS private key: %w", err)
	}
	keyPEM := pem.EncodeToMemory(&pem.Block{Type: "EC PRIVATE KEY", Bytes: keyDER})

	if err := atomicWrite(filepath.Join(foldOpsTLSDir, "cert.pem"), certPEM, 0644); err != nil {
		return err
	}
	if err := atomicWrite(filepath.Join(foldOpsTLSDir, "key.pem"), keyPEM, 0600); err != nil {
		return err
	}
	if err := atomicWrite(filepath.Join(foldOpsTLSDir, "ca.pem"), certPEM, 0644); err != nil {
		return err
	}
	return nil
}

func loadFoldOpsTLSCertificate() (certPath, keyPath string, err error) {
	certPath = filepath.Join(foldOpsTLSDir, "cert.pem")
	keyPath = filepath.Join(foldOpsTLSDir, "key.pem")
	for _, path := range []string{certPath, keyPath} {
		info, statErr := os.Stat(path)
		if statErr != nil {
			return "", "", fmt.Errorf("TLS material is missing at %s: %w", path, statErr)
		}
		if info.IsDir() {
			return "", "", fmt.Errorf("TLS material path is not a file: %s", path)
		}
	}
	return certPath, keyPath, nil
}

func fileExists(path string) bool {
	info, err := os.Stat(path)
	return err == nil && !info.IsDir()
}

func validateFoldOpsTLSReady() error {
	if !foldOpsProvisioned() {
		return errors.New("FoldOps is not provisioned")
	}
	_, _, err := loadFoldOpsTLSCertificate()
	return err
}
