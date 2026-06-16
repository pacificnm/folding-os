package main

import (
	"crypto/tls"
	"errors"
	"fmt"
	"net/http"
	"net/http/httputil"
	"net/url"
	"time"
)

const foldOpsHTTPSListenAddress = ":3443"

var (
	foldOpsHTTPSUpstreamURL     = fmt.Sprintf("http://127.0.0.1:%d", foldOpsSupervisorLoopbackPort)
	foldOpsListenAndServeTLSFn  = defaultFoldOpsListenAndServeTLS
)

func foldOpsServeHTTPS() error {
	role, err := readActiveInstallationRole()
	if err != nil {
		return err
	}
	if role != "supervisor" {
		return errors.New("foldops serve-https is supported only on supervisor role")
	}
	if err := validateFoldOpsTLSReady(); err != nil {
		return err
	}
	certPath, keyPath, err := loadFoldOpsTLSCertificate()
	if err != nil {
		return err
	}

	upstream, err := url.Parse(foldOpsHTTPSUpstreamURL)
	if err != nil {
		return fmt.Errorf("parse upstream URL: %w", err)
	}
	proxy := httputil.NewSingleHostReverseProxy(upstream)
	proxy.ErrorHandler = func(writer http.ResponseWriter, request *http.Request, proxyErr error) {
		http.Error(writer, "foldops upstream unavailable", http.StatusBadGateway)
	}

	server := &http.Server{
		Addr:              foldOpsHTTPSListenAddress,
		Handler:           proxy,
		ReadHeaderTimeout: 15 * time.Second,
		TLSConfig: &tls.Config{
			MinVersion: tls.VersionTLS12,
		},
	}
	fmt.Printf(
		"FoldOps HTTPS front end listening on https://0.0.0.0%s -> %s\n",
		foldOpsHTTPSListenAddress,
		foldOpsHTTPSUpstreamURL,
	)
	return foldOpsListenAndServeTLSFn(server, certPath, keyPath)
}

func defaultFoldOpsListenAndServeTLS(server *http.Server, certPath, keyPath string) error {
	return server.ListenAndServeTLS(certPath, keyPath)
}
