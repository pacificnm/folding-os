package main

import (
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"os"
	"strings"
)

const (
	installSessionHeader = "X-FoldingOS-Install-Session"
)

func handleProvisionAuthorize(writer http.ResponseWriter, request *http.Request) {
	if request.Method != http.MethodPost {
		http.Error(writer, "method not allowed", http.StatusMethodNotAllowed)
		return
	}
	body, err := io.ReadAll(io.LimitReader(request.Body, 1<<20))
	if err != nil {
		http.Error(writer, "invalid request body", http.StatusBadRequest)
		return
	}
	var authorizeRequest provisionAuthorizeRequest
	if err := json.Unmarshal(body, &authorizeRequest); err != nil {
		http.Error(writer, "invalid authorize payload", http.StatusBadRequest)
		return
	}
	response, err := authorizeProvisionInstall(authorizeRequest)
	if err != nil {
		status := http.StatusBadRequest
		if strings.Contains(err.Error(), "enrollment token") {
			status = http.StatusUnauthorized
		}
		http.Error(writer, err.Error(), status)
		return
	}
	writeJSON(writer, http.StatusOK, response)
}

func handleProvisionImageStream(writer http.ResponseWriter, request *http.Request) {
	if request.Method != http.MethodGet {
		http.Error(writer, "method not allowed", http.StatusMethodNotAllowed)
		return
	}
	version := strings.TrimSpace(strings.TrimPrefix(request.URL.Path, "/v1/provision/images/"))
	version = strings.TrimSuffix(version, "/stream")
	if version == "" || strings.Contains(version, "/") {
		http.Error(writer, "image version is required", http.StatusBadRequest)
		return
	}
	sessionID := strings.TrimSpace(request.Header.Get(installSessionHeader))
	enrollmentToken := strings.TrimSpace(request.Header.Get("X-FoldingOS-Enrollment-Token"))
	if sessionID == "" {
		http.Error(writer, "install session is required", http.StatusBadRequest)
		return
	}
	session, entry, err := validateInstallStreamAccess(sessionID, version, enrollmentToken)
	if err != nil {
		status := http.StatusBadRequest
		if strings.Contains(err.Error(), "enrollment token") || strings.Contains(err.Error(), "install session") {
			status = http.StatusUnauthorized
		}
		http.Error(writer, err.Error(), status)
		return
	}

	file, err := os.Open(entry.LocalImagePath)
	if err != nil {
		http.Error(writer, "registry image is unavailable", http.StatusInternalServerError)
		return
	}
	defer file.Close()
	info, err := file.Stat()
	if err != nil {
		http.Error(writer, "registry image is unavailable", http.StatusInternalServerError)
		return
	}
	if info.Size() != entry.ImageSizeBytes {
		http.Error(writer, "registry image size mismatch", http.StatusInternalServerError)
		return
	}

	writer.Header().Set("Content-Type", "application/octet-stream")
	writer.Header().Set("Content-Length", fmt.Sprintf("%d", entry.ImageSizeBytes))
	writer.Header().Set("X-FoldingOS-Image-SHA256", entry.ImageSHA256)
	writer.Header().Set("X-FoldingOS-Target-Disk", session.TargetDisk)
	writer.WriteHeader(http.StatusOK)
	if _, err := io.Copy(writer, file); err != nil {
		return
	}
}
