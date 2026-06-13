package main

import (
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"net"
	"net/http"
	"net/url"
	"os"
	"os/exec"
	"strings"
)

var provisionListenAndServe = func(server *http.Server) error {
	return server.ListenAndServe()
}

func provisionServe() error {
	if err := requireSupervisorRole(); err != nil {
		return err
	}
	token, err := ensureEnrollmentToken()
	if err != nil {
		return err
	}
	listenHost, err := readProvisionListenHost()
	if err != nil {
		return err
	}

	mux := http.NewServeMux()
	mux.HandleFunc("/v1/agents/register", handleAgentRegister)
	mux.HandleFunc("/v1/agents/desired-version", handleDesiredVersion)
	mux.HandleFunc("/v1/rollouts/assign", handleRolloutAssign)

	server := &http.Server{
		Addr:    listenHost,
		Handler: mux,
	}
	fmt.Printf("Supervisor provisioning API listening on http://%s\n", listenHost)
	fmt.Printf("Enrollment token is stored at %s\n", enrollmentTokenPath)
	fmt.Printf("Generated or loaded enrollment token prefix: %s...\n", token[:8])
	return provisionListenAndServe(server)
}

func readProvisionListenHost() (string, error) {
	content, err := os.ReadFile(provisionListenURLPath)
	if err != nil {
		if os.IsNotExist(err) {
			return "0.0.0.0:8743", nil
		}
		return "", err
	}
	raw := strings.TrimSpace(string(content))
	if raw == "" {
		return "0.0.0.0:8743", nil
	}
	parsed, err := url.Parse(raw)
	if err != nil {
		return "", fmt.Errorf("invalid provision listen url: %w", err)
	}
	if parsed.Scheme != "http" {
		return "", fmt.Errorf("provision listen url must use http for Milestone 3 step 3: %q", raw)
	}
	if parsed.Host == "" {
		return "", errors.New("provision listen url missing host")
	}
	return parsed.Host, nil
}

func handleAgentRegister(writer http.ResponseWriter, request *http.Request) {
	if request.Method != http.MethodPost {
		http.Error(writer, "method not allowed", http.StatusMethodNotAllowed)
		return
	}
	body, err := io.ReadAll(io.LimitReader(request.Body, 1<<20))
	if err != nil {
		http.Error(writer, "invalid request body", http.StatusBadRequest)
		return
	}
	var registration agentRegistrationRequest
	if err := json.Unmarshal(body, &registration); err != nil {
		http.Error(writer, "invalid registration payload", http.StatusBadRequest)
		return
	}
	record, err := registerAgent(registration)
	if err != nil {
		status := http.StatusBadRequest
		if strings.Contains(err.Error(), "enrollment token") {
			status = http.StatusUnauthorized
		}
		http.Error(writer, err.Error(), status)
		return
	}
	writeJSON(writer, http.StatusOK, record)
}

func handleDesiredVersion(writer http.ResponseWriter, request *http.Request) {
	if request.Method != http.MethodGet {
		http.Error(writer, "method not allowed", http.StatusMethodNotAllowed)
		return
	}
	nodeID := strings.TrimSpace(request.URL.Query().Get("node_id"))
	if nodeID == "" {
		http.Error(writer, "node_id is required", http.StatusBadRequest)
		return
	}
	if err := validateEnrollmentToken(strings.TrimSpace(request.Header.Get("X-FoldingOS-Enrollment-Token"))); err != nil {
		http.Error(writer, err.Error(), http.StatusUnauthorized)
		return
	}
	response, err := desiredVersionForNode(nodeID)
	if err != nil {
		status := http.StatusBadRequest
		if strings.Contains(err.Error(), "not registered") {
			status = http.StatusForbidden
		}
		http.Error(writer, err.Error(), status)
		return
	}
	writeJSON(writer, http.StatusOK, response)
}

func handleRolloutAssign(writer http.ResponseWriter, request *http.Request) {
	if request.Method != http.MethodPost {
		http.Error(writer, "method not allowed", http.StatusMethodNotAllowed)
		return
	}
	body, err := io.ReadAll(io.LimitReader(request.Body, 1<<20))
	if err != nil {
		http.Error(writer, "invalid request body", http.StatusBadRequest)
		return
	}
	var assign rolloutAssignRequest
	if err := json.Unmarshal(body, &assign); err != nil {
		http.Error(writer, "invalid rollout assignment payload", http.StatusBadRequest)
		return
	}
	if assign.SchemaVersion != 1 {
		http.Error(writer, "unsupported rollout assignment schema version", http.StatusBadRequest)
		return
	}
	if err := validateEnrollmentToken(strings.TrimSpace(assign.EnrollmentToken)); err != nil {
		http.Error(writer, err.Error(), http.StatusUnauthorized)
		return
	}
	scope := strings.TrimSpace(assign.Scope)
	updated, err := assignDesiredVersion(scope, strings.TrimSpace(assign.NodeID), strings.TrimSpace(assign.Version))
	if err != nil {
		status := http.StatusBadRequest
		if strings.Contains(err.Error(), "not registered") {
			status = http.StatusForbidden
		}
		http.Error(writer, err.Error(), status)
		return
	}
	writeJSON(writer, http.StatusOK, map[string]any{
		"schema_version": 1,
		"updated_agents": updated,
	})
}

func writeJSON(writer http.ResponseWriter, status int, value any) {
	writer.Header().Set("Content-Type", "application/json")
	writer.WriteHeader(status)
	encoder := json.NewEncoder(writer)
	encoder.SetIndent("", "  ")
	_ = encoder.Encode(value)
}

func readSupervisorBaseURL() (string, error) {
	content, err := os.ReadFile(supervisorURLPath)
	if err != nil {
		if os.IsNotExist(err) {
			return "", nil
		}
		return "", err
	}
	raw := strings.TrimSpace(string(content))
	if raw == "" {
		return "", nil
	}
	parsed, err := url.Parse(raw)
	if err != nil {
		return "", fmt.Errorf("invalid supervisor url: %w", err)
	}
	if parsed.Scheme != "http" && parsed.Scheme != "https" {
		return "", fmt.Errorf("supervisor url must use http or https: %q", raw)
	}
	if parsed.Host == "" {
		return "", errors.New("supervisor url missing host")
	}
	return strings.TrimRight(raw, "/"), nil
}

func joinSupervisorURL(base, path string) (string, error) {
	parsed, err := url.Parse(base)
	if err != nil {
		return "", err
	}
	ref, err := url.Parse(path)
	if err != nil {
		return "", err
	}
	return parsed.ResolveReference(ref).String(), nil
}

func collectMACAddresses() ([]string, error) {
	interfaces, err := net.Interfaces()
	if err != nil {
		return nil, err
	}
	var addresses []string
	for _, iface := range interfaces {
		if iface.Flags&net.FlagLoopback != 0 || iface.Flags&net.FlagUp == 0 {
			continue
		}
		if len(iface.HardwareAddr) == 0 {
			continue
		}
		addresses = append(addresses, iface.HardwareAddr.String())
	}
	sortStrings(addresses)
	if len(addresses) == 0 {
		return nil, errors.New("no active network interface MAC addresses found")
	}
	return addresses, nil
}

func sortStrings(values []string) {
	for i := 1; i < len(values); i++ {
		for j := i; j > 0 && values[j-1] > values[j]; j-- {
			values[j-1], values[j] = values[j], values[j-1]
		}
	}
}

func readNodeID() (string, error) {
	content, err := os.ReadFile("/data/config/node-id")
	if err != nil {
		return "", err
	}
	nodeID := strings.TrimSpace(string(content))
	if !uuidPattern.MatchString(nodeID) {
		return "", errors.New("node identity is invalid")
	}
	return nodeID, nil
}

func readHostname() (string, error) {
	content, err := effectiveConfig("system", false)
	if err != nil {
		return "", err
	}
	values, err := parseDomain("system", content, true)
	if err != nil {
		return "", err
	}
	hostname := strings.TrimSpace(values["identity.hostname"].text)
	if hostname == "" {
		return "", errors.New("hostname is unavailable")
	}
	return hostname, nil
}

func fahServiceActive() bool {
	return exec.Command("systemctl", "is-active", "--quiet", "folding-at-home.service").Run() == nil
}
