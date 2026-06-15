package main

import (
	"fmt"
	"os"
	"path/filepath"
	"syscall"
)

const stagedUpdateLockPathDefault = "/data/state/provision/staged-update.lock"

var stagedUpdateLockPath = stagedUpdateLockPathDefault

func withStagedUpdateLock(fn func() error) error {
	if err := os.MkdirAll(filepath.Dir(stagedUpdateLockPath), 0755); err != nil {
		return err
	}
	lock, err := os.OpenFile(stagedUpdateLockPath, os.O_CREATE|os.O_RDWR, 0600)
	if err != nil {
		return fmt.Errorf("open staged update lock: %w", err)
	}
	defer lock.Close()
	if err := syscall.Flock(int(lock.Fd()), syscall.LOCK_EX); err != nil {
		return fmt.Errorf("acquire staged update lock: %w", err)
	}
	defer func() { _ = syscall.Flock(int(lock.Fd()), syscall.LOCK_UN) }()
	return fn()
}
