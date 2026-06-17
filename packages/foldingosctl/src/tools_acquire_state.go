package main

import (
	"errors"
	"fmt"
	"os"
	"strconv"
	"strings"
	"time"
)

const toolsAcquireStatePathDefault = "/data/state/tools/acquire.state"

var toolsAcquireStatePath = toolsAcquireStatePathDefault

var (
	toolsAcquisitionRetryDelays = foldOpsAcquisitionRetryDelays
	toolsNow                    = time.Now
)

type toolsAcquireState struct {
	ConsecutiveFailures int
	NextAttemptUnix     int64
	LastFailureReason   string
}

func clearToolsAcquireState() error {
	if err := os.Remove(toolsAcquireStatePath); err != nil && !os.IsNotExist(err) {
		return fmt.Errorf("clear tools acquisition retry state: %w", err)
	}
	return nil
}

func loadToolsAcquireState() (toolsAcquireState, error) {
	content, err := os.ReadFile(toolsAcquireStatePath)
	if err != nil {
		if os.IsNotExist(err) {
			return toolsAcquireState{}, nil
		}
		return toolsAcquireState{}, fmt.Errorf("read tools acquisition retry state: %w", err)
	}
	state, err := parseToolsAcquireState(string(content))
	if err != nil {
		return toolsAcquireState{}, fmt.Errorf("parse tools acquisition retry state: %w", err)
	}
	return state, nil
}

func parseToolsAcquireState(content string) (toolsAcquireState, error) {
	values := parseKeyValueLines(content)
	state := toolsAcquireState{}

	if failures, ok := values["consecutive_failures"]; ok {
		parsed, err := strconv.Atoi(failures)
		if err != nil || parsed < 0 {
			return toolsAcquireState{}, errors.New("consecutive_failures must be a non-negative integer")
		}
		state.ConsecutiveFailures = parsed
	}
	if nextAttempt, ok := values["next_attempt_unix"]; ok {
		parsed, err := strconv.ParseInt(nextAttempt, 10, 64)
		if err != nil || parsed < 0 {
			return toolsAcquireState{}, errors.New("next_attempt_unix must be a non-negative integer")
		}
		state.NextAttemptUnix = parsed
	}
	state.LastFailureReason = values["last_failure_reason"]
	return state, nil
}

func saveToolsAcquireState(state toolsAcquireState) error {
	content := strings.Join([]string{
		"consecutive_failures=" + strconv.Itoa(state.ConsecutiveFailures),
		"next_attempt_unix=" + strconv.FormatInt(state.NextAttemptUnix, 10),
		"last_failure_reason=" + state.LastFailureReason,
	}, "\n") + "\n"
	return atomicWrite(toolsAcquireStatePath, []byte(content), 0644)
}

func toolsAcquisitionRetryDelay(consecutiveFailures int) time.Duration {
	if consecutiveFailures <= 0 {
		return toolsAcquisitionRetryDelays[0]
	}
	index := consecutiveFailures - 1
	if index >= len(toolsAcquisitionRetryDelays) {
		index = len(toolsAcquisitionRetryDelays) - 1
	}
	return toolsAcquisitionRetryDelays[index]
}

func deferToolsAcquisitionAttempt(state toolsAcquireState) (bool, time.Duration, error) {
	if state.NextAttemptUnix == 0 {
		return false, 0, nil
	}
	remaining := time.Unix(state.NextAttemptUnix, 0).Sub(toolsNow())
	if remaining > 0 {
		return true, remaining, nil
	}
	return false, 0, nil
}

func recordToolsAcquisitionFailure(cause error) error {
	state, err := loadToolsAcquireState()
	if err != nil {
		return err
	}
	state.ConsecutiveFailures++
	delay := toolsAcquisitionRetryDelay(state.ConsecutiveFailures)
	state.NextAttemptUnix = toolsNow().Add(delay).Unix()
	state.LastFailureReason = cause.Error()
	if err := saveToolsAcquireState(state); err != nil {
		return err
	}
	return fmt.Errorf(
		"tools acquisition failed; next retry in %s: %w",
		delay.Round(time.Second),
		cause,
	)
}
