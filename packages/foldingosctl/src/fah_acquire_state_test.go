package main

import (
	"errors"
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"
)

func TestFAHAcquisitionRetryDelaySchedule(t *testing.T) {
	cases := []struct {
		failures int
		want     time.Duration
	}{
		{1, 1 * time.Minute},
		{2, 5 * time.Minute},
		{3, 15 * time.Minute},
		{4, 1 * time.Hour},
		{5, 6 * time.Hour},
		{9, 6 * time.Hour},
	}
	for _, testCase := range cases {
		if got := fahAcquisitionRetryDelay(testCase.failures); got != testCase.want {
			t.Fatalf("failures=%d delay=%s want=%s", testCase.failures, got, testCase.want)
		}
	}
}

func TestDeferFAHAcquisitionAttemptHonorsPersistedState(t *testing.T) {
	now := time.Unix(1_700_000_000, 0)
	restoreNow := setFAHNow(func() time.Time { return now })
	defer restoreNow()

	state := fahAcquireState{NextAttemptUnix: now.Add(5 * time.Minute).Unix()}
	deferred, remaining, err := deferFAHAcquisitionAttempt(state)
	if err != nil {
		t.Fatal(err)
	}
	if !deferred {
		t.Fatal("acquisition was not deferred")
	}
	if remaining != 5*time.Minute {
		t.Fatalf("remaining = %s", remaining)
	}
}

func TestRecordFAHAcquisitionFailurePersistsNextAttempt(t *testing.T) {
	stateDir := t.TempDir()
	statePath := filepath.Join(stateDir, "fah-acquire.state")
	restoreStatePath := setFAHAcquireStatePath(statePath)
	defer restoreStatePath()

	now := time.Unix(1_700_000_000, 0)
	restoreNow := setFAHNow(func() time.Time { return now })
	defer restoreNow()

	err := recordFAHAcquisitionFailure(errors.New("download artifact: connection reset"))
	if err == nil {
		t.Fatal("recorded failure did not return error")
	}

	state, err := loadFAHAcquireState()
	if err != nil {
		t.Fatal(err)
	}
	if state.ConsecutiveFailures != 1 {
		t.Fatalf("consecutive_failures = %d", state.ConsecutiveFailures)
	}
	if state.NextAttemptUnix != now.Add(1*time.Minute).Unix() {
		t.Fatalf("next_attempt_unix = %d", state.NextAttemptUnix)
	}
	if !strings.Contains(state.LastFailureReason, "connection reset") {
		t.Fatalf("last_failure_reason = %q", state.LastFailureReason)
	}
}

func TestRecordFAHAcquisitionFailureCapsDelayAtSixHours(t *testing.T) {
	stateDir := t.TempDir()
	statePath := filepath.Join(stateDir, "fah-acquire.state")
	restoreStatePath := setFAHAcquireStatePath(statePath)
	defer restoreStatePath()

	now := time.Unix(1_700_000_000, 0)
	restoreNow := setFAHNow(func() time.Time { return now })
	defer restoreNow()

	if err := saveFAHAcquireState(fahAcquireState{ConsecutiveFailures: 4}); err != nil {
		t.Fatal(err)
	}
	if err := recordFAHAcquisitionFailure(errors.New("network is not online")); err == nil {
		t.Fatal("recorded failure did not return error")
	}

	state, err := loadFAHAcquireState()
	if err != nil {
		t.Fatal(err)
	}
	if state.ConsecutiveFailures != 5 {
		t.Fatalf("consecutive_failures = %d", state.ConsecutiveFailures)
	}
	if state.NextAttemptUnix != now.Add(6*time.Hour).Unix() {
		t.Fatalf("next_attempt_unix = %d", state.NextAttemptUnix)
	}
}

func TestFAHAcquireDefersWhenRetryWindowNotElapsed(t *testing.T) {
	stateDir := t.TempDir()
	statePath := filepath.Join(stateDir, "fah-acquire.state")
	restoreStatePath := setFAHAcquireStatePath(statePath)
	defer restoreStatePath()

	now := time.Unix(1_700_000_000, 0)
	restoreNow := setFAHNow(func() time.Time { return now })
	defer restoreNow()

	if err := saveFAHAcquireState(fahAcquireState{
		ConsecutiveFailures: 2,
		NextAttemptUnix:   now.Add(4 * time.Minute).Unix(),
	}); err != nil {
		t.Fatal(err)
	}

	restoreManifest := setFAHApprovedManifestLoader(testFAHManifestLoader(t))
	defer restoreManifest()
	restoreCompatibility := setFAHFoldingOSCompatibilityCheck(func(string) error { return nil })
	defer restoreCompatibility()

	calledPrereqs := false
	restorePrereqs := setFAHAcquisitionPrerequisitesCheck(func() error {
		calledPrereqs = true
		return nil
	})
	defer restorePrereqs()

	if err := fahAcquire(); err != nil {
		t.Fatal(err)
	}
	if calledPrereqs {
		t.Fatal("prerequisites were checked while acquisition was deferred")
	}
}

func TestFAHAcquireClearsStateWhenVerifiedClientActive(t *testing.T) {
	stateDir := t.TempDir()
	statePath := filepath.Join(stateDir, "fah-acquire.state")
	if err := os.WriteFile(statePath, []byte("consecutive_failures=2\nnext_attempt_unix=1\n"), 0644); err != nil {
		t.Fatal(err)
	}
	restoreStatePath := setFAHAcquireStatePath(statePath)
	defer restoreStatePath()

	appsRoot := t.TempDir()
	versionDir := filepath.Join(appsRoot, "8.5.6")
	if err := os.MkdirAll(filepath.Join(versionDir, "usr", "bin"), 0755); err != nil {
		t.Fatal(err)
	}
	executable := filepath.Join(versionDir, "usr", "bin", "fah-client")
	if err := os.WriteFile(executable, []byte("binary"), 0755); err != nil {
		t.Fatal(err)
	}
	if err := os.Symlink("8.5.6", filepath.Join(appsRoot, "current")); err != nil {
		t.Fatal(err)
	}
	marker := "client_version=8.5.6\nartifact_sha256=643de04033a1cb972a81e3a193d710e919a4f34634a987f11adc4cee61fdaefe\n"
	if err := os.WriteFile(filepath.Join(versionDir, fahVerifiedMarkerName), []byte(marker), 0644); err != nil {
		t.Fatal(err)
	}

	restoreApps := setFAHAppsRoot(appsRoot)
	defer restoreApps()
	restoreManifest := setFAHApprovedManifestLoader(testFAHManifestLoader(t))
	defer restoreManifest()
	restoreCompatibility := setFAHFoldingOSCompatibilityCheck(func(string) error { return nil })
	defer restoreCompatibility()

	if err := fahAcquire(); err != nil {
		t.Fatal(err)
	}
	if _, err := os.Stat(statePath); !os.IsNotExist(err) {
		t.Fatal("acquisition retry state was not cleared")
	}
}

func setFAHNow(now func() time.Time) func() {
	previous := fahNow
	fahNow = now
	return func() {
		fahNow = previous
	}
}

func setFAHAcquireStatePath(path string) func() {
	previous := fahAcquireStatePath
	fahAcquireStatePath = path
	return func() {
		fahAcquireStatePath = previous
	}
}

func setFAHAcquisitionPrerequisitesCheck(check func() error) func() {
	previous := fahCheckAcquisitionPrerequisites
	fahCheckAcquisitionPrerequisites = check
	return func() {
		fahCheckAcquisitionPrerequisites = previous
	}
}
