package main

import (
	"strings"
	"testing"
)

func TestFormatCommissioningDisplayIncludesRequiredReadyLines(t *testing.T) {
	checks := []commissioningCheck{
		{Label: "Network online", Ready: true},
		{Label: "SSH administrator provisioned", Ready: true},
	}
	message := formatCommissioningDisplay(
		"FoldingOS 0.1.0",
		"0.1.0",
		"supervisor",
		"192.168.4.32",
		checks,
	)
	for _, required := range []string{
		"FoldingOS 0.1.0 ready",
		"Address: 192.168.4.32",
		"SSH: foldingos-admin@192.168.4.32",
		"╔",
		"╚",
		"✓ Network online",
		"https://folding-os.com",
	} {
		if !strings.Contains(message, required) {
			t.Fatalf("commissioning display missing %q:\n%s", required, message)
		}
	}
}

func TestCommissioningChecksPending(t *testing.T) {
	if commissioningChecksPending([]commissioningCheck{{Label: "x", Ready: true}}) {
		t.Fatal("all-ready checks reported pending")
	}
	if !commissioningChecksPending([]commissioningCheck{{Label: "x", Ready: false}}) {
		t.Fatal("pending check not detected")
	}
}

func TestCenterDisplayText(t *testing.T) {
	got := centerDisplayText("FoldingOS", 20)
	if len([]rune(got)) != 20 {
		t.Fatalf("centerDisplayText width = %d", len([]rune(got)))
	}
	if strings.TrimSpace(got) != "FoldingOS" {
		t.Fatalf("centerDisplayText() = %q", got)
	}
}
