package main

import (
	"bytes"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"net/http"
	"os"
	"strings"
)

var provisionHTTPClient = &http.Client{}

func provisionEnroll() error {
	if err := requireAgentRole(); err != nil {
		return err
	}

	if err := ensureIdentity(); err != nil {
		return fmt.Errorf("ensure node identity before enrollment: %w", err)
	}

	supervisorURL, err := readSupervisorBaseURL()
	if err != nil {
		return err
	}
	if supervisorURL == "" {
		if _, tokenErr := readEnrollmentToken(); tokenErr == nil {
			return errors.New("supervisor URL is not configured for network-provisioned agent")
		}
		fmt.Println("Supervisor URL is not configured; agent enrollment skipped.")
		return nil
	}

	nodeID, err := readNodeID()
	if err != nil {
		return err
	}
	if enrolledID, err := agentEnrollmentNodeID(); err == nil {
		if enrolledID == nodeID {
			fmt.Printf("Agent %s is already enrolled.\n", nodeID)
			return nil
		}
		return fmt.Errorf("local enrollment state %q does not match node identity %q", enrolledID, nodeID)
	} else if !os.IsNotExist(err) {
		return err
	}

	token, err := readEnrollmentToken()
	if err != nil {
		return fmt.Errorf("agent enrollment token is not configured: %w", err)
	}
	version, err := installedFoldingOSVersionReader()
	if err != nil {
		return err
	}
	hostname, err := readHostname()
	if err != nil {
		return err
	}
	macAddresses, err := collectMACAddresses()
	if err != nil {
		return err
	}

	request := agentRegistrationRequest{
		SchemaVersion:       1,
		NodeID:              nodeID,
		EnrollmentToken:     token,
		InstallationRole:    "agent",
		CurrentImageVersion: version,
		FoldingOSVersion:    version,
		Hostname:            hostname,
		MACAddresses:        macAddresses,
		FAHActive:           fahServiceActive(),
	}
	endpoint, err := joinSupervisorURL(supervisorURL, "/v1/agents/register")
	if err != nil {
		return err
	}
	body, err := json.Marshal(request)
	if err != nil {
		return err
	}
	httpRequest, err := http.NewRequest(http.MethodPost, endpoint, bytes.NewReader(body))
	if err != nil {
		return err
	}
	httpRequest.Header.Set("Content-Type", "application/json")
	response, err := provisionHTTPClient.Do(httpRequest)
	if err != nil {
		return err
	}
	defer response.Body.Close()
	responseBody, err := io.ReadAll(io.LimitReader(response.Body, 1<<20))
	if err != nil {
		return err
	}
	if response.StatusCode != http.StatusOK {
		return fmt.Errorf(
			"supervisor registration failed with status %s: %s",
			response.Status,
			strings.TrimSpace(string(responseBody)),
		)
	}
	if err := markAgentEnrolled(nodeID); err != nil {
		return err
	}
	fmt.Printf("Agent %s enrolled with supervisor %s.\n", nodeID, supervisorURL)
	return nil
}
