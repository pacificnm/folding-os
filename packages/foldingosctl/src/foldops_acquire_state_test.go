package main

import (
	"os"
	"path/filepath"
	"testing"
	"time"
)

func TestParseFoldOpsAcquireState(t *testing.T) {
	state, err := parseFoldOpsAcquireState(
		"consecutive_failures=2\nnext_attempt_unix=1700000000\nlast_failure_reason=network is not online\n",
	)
	if err != nil {
		t.Fatal(err)
	}
	if state.ConsecutiveFailures != 2 || state.NextAttemptUnix != 1700000000 {
		t.Fatalf("unexpected state: %+v", state)
	}
}

func TestDeferFoldOpsAcquisitionAttempt(t *testing.T) {
	restoreNow := foldOpsNow
	defer func() { foldOpsNow = restoreNow }()
	foldOpsNow = func() time.Time { return time.Unix(100, 0) }

	deferred, remaining, err := deferFoldOpsAcquisitionAttempt(foldOpsAcquireState{
		NextAttemptUnix: 200,
	})
	if err != nil {
		t.Fatal(err)
	}
	if !deferred || remaining != 100*time.Second {
		t.Fatalf("expected deferred retry, got deferred=%v remaining=%s", deferred, remaining)
	}
}

func TestActivateFoldOpsCurrentSymlink(t *testing.T) {
	appsRoot := filepath.Join(t.TempDir(), "foldops")
	release := "0.1.0-1"
	releaseDir := filepath.Join(appsRoot, release)
	if err := os.MkdirAll(releaseDir, 0755); err != nil {
		t.Fatal(err)
	}

	restoreAppsRoot := setFoldOpsAppsRoot(appsRoot)
	defer restoreAppsRoot()

	if err := activateFoldOpsCurrentSymlink(release); err != nil {
		t.Fatal(err)
	}
	target, err := os.Readlink(filepath.Join(appsRoot, "current"))
	if err != nil {
		t.Fatal(err)
	}
	if target != release {
		t.Fatalf("current = %q, want %q", target, release)
	}
}

func setFoldOpsAppsRoot(path string) func() {
	previous := foldOpsAppsRoot
	foldOpsAppsRoot = path
	return func() {
		foldOpsAppsRoot = previous
	}
}
