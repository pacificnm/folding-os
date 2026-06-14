package main

import (
	"net/http"
	"os"
	"path/filepath"
	"strings"
)

func handleIPXEBootstrap(writer http.ResponseWriter, request *http.Request) {
	if request.Method != http.MethodGet {
		http.Error(writer, "method not allowed", http.StatusMethodNotAllowed)
		return
	}
	host, err := provisionBootHTTPBase(request)
	if err != nil {
		http.Error(writer, err.Error(), http.StatusInternalServerError)
		return
	}
	script := renderIPXEBootstrapScript(host)
	writer.Header().Set("Content-Type", "text/plain")
	writer.WriteHeader(http.StatusOK)
	_, _ = writer.Write([]byte(script))
}

func handleIPXEInstallScript(writer http.ResponseWriter, request *http.Request) {
	if request.Method != http.MethodGet {
		http.Error(writer, "method not allowed", http.StatusMethodNotAllowed)
		return
	}
	host, err := provisionBootHTTPBase(request)
	if err != nil {
		http.Error(writer, err.Error(), http.StatusInternalServerError)
		return
	}
	mac := strings.TrimSpace(request.URL.Query().Get("mac"))
	token := strings.TrimSpace(request.URL.Query().Get("token"))
	script, err := renderIPXEInstallScript(host, mac, token)
	if err != nil {
		status := http.StatusForbidden
		if strings.Contains(err.Error(), "enrollment token") {
			status = http.StatusUnauthorized
		}
		http.Error(writer, err.Error(), status)
		return
	}
	writer.Header().Set("Content-Type", "text/plain")
	writer.WriteHeader(http.StatusOK)
	_, _ = writer.Write([]byte(script))
}

func handleProvisionBootAsset(filename string) http.HandlerFunc {
	return func(writer http.ResponseWriter, request *http.Request) {
		if request.Method != http.MethodGet {
			http.Error(writer, "method not allowed", http.StatusMethodNotAllowed)
			return
		}
		path := filepath.Join(provisionBootAssetsDir, filename)
		content, err := os.ReadFile(path)
		if err != nil {
			http.Error(writer, "boot asset is unavailable", http.StatusNotFound)
			return
		}
		switch filepath.Ext(filename) {
		case ".gz":
			writer.Header().Set("Content-Type", "application/gzip")
		default:
			writer.Header().Set("Content-Type", "application/octet-stream")
		}
		writer.WriteHeader(http.StatusOK)
		_, _ = writer.Write(content)
	}
}
