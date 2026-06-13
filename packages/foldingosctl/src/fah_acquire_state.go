package main

import (
	"errors"
	"fmt"
	"os"
	"strconv"
	"strings"
	"time"
)

const (
	fahAcquireStatePathDefault = "/data/state/fah-acquire.state"
)

var fahAcquireStatePath = fahAcquireStatePathDefault

var (
	fahAcquisitionRetryDelays = []time.Duration{
		1 * time.Minute,
		5 * time.Minute,
		15 * time.Minute,
		1 * time.Hour,
		6 * time.Hour,
	}
	fahNow = time.Now
)

type fahAcquireState struct {
	ConsecutiveFailures int
	NextAttemptUnix     int64
	LastFailureReason   string
}

func clearFAHAcquireState() error {
	if err := os.Remove(fahAcquireStatePath); err != nil && !os.IsNotExist(err) {
		return fmt.Errorf("clear acquisition retry state: %w", err)
	}
	return nil
}

func loadFAHAcquireState() (fahAcquireState, error) {
	content, err := os.ReadFile(fahAcquireStatePath)
	if err != nil {
		if os.IsNotExist(err) {
			return fahAcquireState{}, nil
		}
		return fahAcquireState{}, fmt.Errorf("read acquisition retry state: %w", err)
	}
	state, err := parseFAHAcquireState(string(content))
	if err != nil {
		return fahAcquireState{}, fmt.Errorf("parse acquisition retry state: %w", err)
	}
	return state, nil
}

func parseFAHAcquireState(content string) (fahAcquireState, error) {
	values := parseKeyValueLines(content)
	state := fahAcquireState{}

	if failures, ok := values["consecutive_failures"]; ok {
		parsed, err := strconv.Atoi(failures)
		if err != nil || parsed < 0 {
			return fahAcquireState{}, errors.New("consecutive_failures must be a non-negative integer")
		}
		state.ConsecutiveFailures = parsed
	}
	if nextAttempt, ok := values["next_attempt_unix"]; ok {
		parsed, err := strconv.ParseInt(nextAttempt, 10, 64)
		if err != nil || parsed < 0 {
			return fahAcquireState{}, errors.New("next_attempt_unix must be a non-negative integer")
		}
		state.NextAttemptUnix = parsed
	}
	state.LastFailureReason = values["last_failure_reason"]
	return state, nil
}

func saveFAHAcquireState(state fahAcquireState) error {
	content := strings.Join([]string{
		"consecutive_failures=" + strconv.Itoa(state.ConsecutiveFailures),
		"next_attempt_unix=" + strconv.FormatInt(state.NextAttemptUnix, 10),
		"last_failure_reason=" + state.LastFailureReason,
	}, "\n") + "\n"
	return atomicWrite(fahAcquireStatePath, []byte(content), 0644)
}

func fahAcquisitionRetryDelay(consecutiveFailures int) time.Duration {
	if consecutiveFailures <= 0 {
		return fahAcquisitionRetryDelays[0]
	}
	index := consecutiveFailures - 1
	if index >= len(fahAcquisitionRetryDelays) {
		index = len(fahAcquisitionRetryDelays) - 1
	}
	return fahAcquisitionRetryDelays[index]
}

func deferFAHAcquisitionAttempt(state fahAcquireState) (bool, time.Duration, error) {
	if state.NextAttemptUnix == 0 {
		return false, 0, nil
	}
	remaining := time.Unix(state.NextAttemptUnix, 0).Sub(fahNow())
	if remaining > 0 {
		return true, remaining, nil
	}
	return false, 0, nil
}

func recordFAHAcquisitionFailure(cause error) error {
	state, err := loadFAHAcquireState()
	if err != nil {
		return err
	}
	state.ConsecutiveFailures++
	delay := fahAcquisitionRetryDelay(state.ConsecutiveFailures)
	state.NextAttemptUnix = fahNow().Add(delay).Unix()
	state.LastFailureReason = cause.Error()
	if err := saveFAHAcquireState(state); err != nil {
		return err
	}
	return fmt.Errorf(
		"acquisition failed; next retry in %s: %w",
		delay.Round(time.Second),
		cause,
	)
}
