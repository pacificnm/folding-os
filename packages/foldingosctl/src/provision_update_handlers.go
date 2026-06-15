package main

import (
	"encoding/json"
	"errors"
	"io"
	"net/http"
	"os"
	"strings"
)

func handleUpdateAuthorize(writer http.ResponseWriter, request *http.Request) {
	if request.Method != http.MethodPost {
		http.Error(writer, "method not allowed", http.StatusMethodNotAllowed)
		return
	}
	body, err := io.ReadAll(io.LimitReader(request.Body, 1<<20))
	if err != nil {
		http.Error(writer, "invalid request body", http.StatusBadRequest)
		return
	}
	var authorizeRequest updateAuthorizeRequest
	if err := json.Unmarshal(body, &authorizeRequest); err != nil {
		http.Error(writer, "invalid update authorize payload", http.StatusBadRequest)
		return
	}
	response, err := authorizeAgentUpdate(authorizeRequest)
	if err != nil {
		status := http.StatusBadRequest
		if strings.Contains(err.Error(), "enrollment token") {
			status = http.StatusUnauthorized
		}
		if strings.Contains(err.Error(), "not registered") {
			status = http.StatusNotFound
		}
		http.Error(writer, err.Error(), status)
		return
	}
	writeJSON(writer, http.StatusOK, response)
}

func handleUpdateStatus(writer http.ResponseWriter, request *http.Request) {
	if request.Method != http.MethodPost {
		http.Error(writer, "method not allowed", http.StatusMethodNotAllowed)
		return
	}
	body, err := io.ReadAll(io.LimitReader(request.Body, 1<<20))
	if err != nil {
		http.Error(writer, "invalid request body", http.StatusBadRequest)
		return
	}
	var statusRequest updateStatusRequest
	if err := json.Unmarshal(body, &statusRequest); err != nil {
		http.Error(writer, "invalid update status payload", http.StatusBadRequest)
		return
	}
	if statusRequest.SchemaVersion != 1 {
		http.Error(writer, "unsupported update status schema version", http.StatusBadRequest)
		return
	}
	if err := validateEnrollmentToken(strings.TrimSpace(statusRequest.EnrollmentToken)); err != nil {
		http.Error(writer, err.Error(), http.StatusUnauthorized)
		return
	}
	nodeID := strings.TrimSpace(statusRequest.NodeID)
	if !uuidPattern.MatchString(nodeID) {
		http.Error(writer, "node_id is invalid", http.StatusBadRequest)
		return
	}
	if err := recordAgentUpdateStatus(
		nodeID,
		statusRequest.ImageVersion,
		statusRequest.Status,
		statusRequest.Message,
	); err != nil {
		status := http.StatusBadRequest
		if errors.Is(err, os.ErrNotExist) || strings.Contains(err.Error(), "not registered") {
			status = http.StatusNotFound
		}
		http.Error(writer, err.Error(), status)
		return
	}
	writeJSON(writer, http.StatusOK, map[string]string{"status": "ok"})
}
