package main

import (
	"encoding/json"
	"errors"
	"fmt"
	"os"
	"strings"
)

const automationSchemaVersion = 1

type automationFormat string

const (
	formatHuman automationFormat = "human"
	formatJSON  automationFormat = "json"
)

type automationContext struct {
	format  automationFormat
	command string
}

var automationCtx automationContext

type automationErrorBody struct {
	Code    string `json:"code"`
	Message string `json:"message"`
}

type automationSuccessDocument struct {
	SchemaVersion int    `json:"schema_version"`
	OK            bool   `json:"ok"`
	Command       string `json:"command"`
	Data          any    `json:"data"`
}

type automationFailureDocument struct {
	SchemaVersion int                 `json:"schema_version"`
	OK            bool                `json:"ok"`
	Command       string              `json:"command"`
	Error         automationErrorBody `json:"error"`
}

func stripFormatFlag(args []string) ([]string, automationFormat) {
	format := formatHuman
	clean := make([]string, 0, len(args))
	for index := 0; index < len(args); index++ {
		if args[index] == "--format" {
			if index+1 >= len(args) {
				clean = append(clean, args[index])
				continue
			}
			switch args[index+1] {
			case "json":
				format = formatJSON
			default:
				clean = append(clean, args[index], args[index+1])
			}
			index++
			continue
		}
		clean = append(clean, args[index])
	}
	return clean, format
}

func automationJSONEnabled() bool {
	return automationCtx.format == formatJSON
}

func setAutomationCommand(command string) {
	automationCtx.command = command
}

func writeAutomationSuccess(data any) error {
	if !automationJSONEnabled() {
		return errors.New("writeAutomationSuccess called without JSON format")
	}
	document := automationSuccessDocument{
		SchemaVersion: automationSchemaVersion,
		OK:            true,
		Command:       automationCtx.command,
		Data:          data,
	}
	content, err := json.MarshalIndent(document, "", "  ")
	if err != nil {
		return err
	}
	content = append(content, '\n')
	_, err = os.Stdout.Write(content)
	return err
}

func writeAutomationFailure(err error) error {
	if !automationJSONEnabled() {
		return err
	}
	document := automationFailureDocument{
		SchemaVersion: automationSchemaVersion,
		OK:            false,
		Command:       automationCtx.command,
		Error:         classifyAutomationError(err),
	}
	content, marshalErr := json.MarshalIndent(document, "", "  ")
	if marshalErr != nil {
		return marshalErr
	}
	content = append(content, '\n')
	if _, writeErr := os.Stdout.Write(content); writeErr != nil {
		return writeErr
	}
	return err
}

func classifyAutomationError(err error) automationErrorBody {
	if err == nil {
		return automationErrorBody{Code: "internal", Message: "unknown error"}
	}
	message := err.Error()
	lower := strings.ToLower(message)
	switch {
	case strings.Contains(lower, "requires supervisor role"):
		return automationErrorBody{Code: "role_required", Message: message}
	case strings.Contains(lower, "requires agent role"):
		return automationErrorBody{Code: "role_required", Message: message}
	case strings.Contains(lower, "requires agent or supervisor role"):
		return automationErrorBody{Code: "role_required", Message: message}
	case strings.Contains(lower, "automation policy"):
		return automationErrorBody{Code: "automation_denied", Message: message}
	case strings.Contains(lower, "permission denied"):
		return automationErrorBody{Code: "permission_denied", Message: message}
	case strings.Contains(lower, "unknown configuration domain"),
		strings.Contains(lower, "unknown inspect subcommand"),
		strings.Contains(lower, "missing value for"):
		return automationErrorBody{Code: "invalid_input", Message: message}
	case strings.Contains(lower, "not registered"),
		strings.Contains(lower, "not in registry"),
		os.IsNotExist(err):
		return automationErrorBody{Code: "not_found", Message: message}
	default:
		return automationErrorBody{Code: "internal", Message: message}
	}
}

func automationOrHumanSuccess(data any, human func() error) error {
	if automationJSONEnabled() {
		return writeAutomationSuccess(data)
	}
	return human()
}

func optionalStringValue(value string) *string {
	trimmed := strings.TrimSpace(value)
	if trimmed == "" {
		return nil
	}
	return &trimmed
}

func marshalAutomationData(value any) ([]byte, error) {
	document := automationSuccessDocument{
		SchemaVersion: automationSchemaVersion,
		OK:            true,
		Command:       automationCtx.command,
		Data:          value,
	}
	return json.MarshalIndent(document, "", "  ")
}

func formatAutomationCommand(parts ...string) string {
	return strings.Join(parts, " ")
}

func automationUsageError(message string) error {
	return fmt.Errorf("%s", message)
}
