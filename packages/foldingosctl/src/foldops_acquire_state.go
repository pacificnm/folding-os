package main

import (
	"errors"
	"fmt"
	"os"
	"strconv"
	"strings"
	"time"
)

const foldOpsAcquireStatePathDefault = "/data/state/foldops/acquire.state"

var foldOpsAcquireStatePath = foldOpsAcquireStatePathDefault

var (
	foldOpsAcquisitionRetryDelays = []time.Duration{
		1 * time.Minute,
		5 * time.Minute,
		15 * time.Minute,
		1 * time.Hour,
		6 * time.Hour,
	}
	foldOpsNow = time.Now
)

type foldOpsAcquireState struct {
	ConsecutiveFailures int
	NextAttemptUnix     int64
	LastFailureReason   string
}

func clearFoldOpsAcquireState() error {
	if err := os.Remove(foldOpsAcquireStatePath); err != nil && !os.IsNotExist(err) {
		return fmt.Errorf("clear acquisition retry state: %w", err)
	}
	return nil
}

func loadFoldOpsAcquireState() (foldOpsAcquireState, error) {
	content, err := os.ReadFile(foldOpsAcquireStatePath)
	if err != nil {
		if os.IsNotExist(err) {
			return foldOpsAcquireState{}, nil
		}
		return foldOpsAcquireState{}, fmt.Errorf("read acquisition retry state: %w", err)
	}
	state, err := parseFoldOpsAcquireState(string(content))
	if err != nil {
		return foldOpsAcquireState{}, fmt.Errorf("parse acquisition retry state: %w", err)
	}
	return state, nil
}

func parseFoldOpsAcquireState(content string) (foldOpsAcquireState, error) {
	values := parseKeyValueLines(content)
	state := foldOpsAcquireState{}

	if failures, ok := values["consecutive_failures"]; ok {
		parsed, err := strconv.Atoi(failures)
		if err != nil || parsed < 0 {
			return foldOpsAcquireState{}, errors.New("consecutive_failures must be a non-negative integer")
		}
		state.ConsecutiveFailures = parsed
	}
	if nextAttempt, ok := values["next_attempt_unix"]; ok {
		parsed, err := strconv.ParseInt(nextAttempt, 10, 64)
		if err != nil || parsed < 0 {
			return foldOpsAcquireState{}, errors.New("next_attempt_unix must be a non-negative integer")
		}
		state.NextAttemptUnix = parsed
	}
	state.LastFailureReason = values["last_failure_reason"]
	return state, nil
}

func saveFoldOpsAcquireState(state foldOpsAcquireState) error {
	content := strings.Join([]string{
		"consecutive_failures=" + strconv.Itoa(state.ConsecutiveFailures),
		"next_attempt_unix=" + strconv.FormatInt(state.NextAttemptUnix, 10),
		"last_failure_reason=" + state.LastFailureReason,
	}, "\n") + "\n"
	return atomicWrite(foldOpsAcquireStatePath, []byte(content), 0644)
}

func foldOpsAcquisitionRetryDelay(consecutiveFailures int) time.Duration {
	if consecutiveFailures <= 0 {
		return foldOpsAcquisitionRetryDelays[0]
	}
	index := consecutiveFailures - 1
	if index >= len(foldOpsAcquisitionRetryDelays) {
		index = len(foldOpsAcquisitionRetryDelays) - 1
	}
	return foldOpsAcquisitionRetryDelays[index]
}

func deferFoldOpsAcquisitionAttempt(state foldOpsAcquireState) (bool, time.Duration, error) {
	if state.NextAttemptUnix == 0 {
		return false, 0, nil
	}
	remaining := time.Unix(state.NextAttemptUnix, 0).Sub(foldOpsNow())
	if remaining > 0 {
		return true, remaining, nil
	}
	return false, 0, nil
}

func recordFoldOpsAcquisitionFailure(cause error) error {
	state, err := loadFoldOpsAcquireState()
	if err != nil {
		return err
	}
	state.ConsecutiveFailures++
	delay := foldOpsAcquisitionRetryDelay(state.ConsecutiveFailures)
	state.NextAttemptUnix = foldOpsNow().Add(delay).Unix()
	state.LastFailureReason = cause.Error()
	if err := saveFoldOpsAcquireState(state); err != nil {
		return err
	}
	return fmt.Errorf(
		"acquisition failed; next retry in %s: %w",
		delay.Round(time.Second),
		cause,
	)
}
