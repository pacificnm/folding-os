package main

import (
	"errors"
	"testing"
)

func TestFormatReadyDisplay(t *testing.T) {
	got := formatReadyDisplay("FoldingOS 0.1.0", "192.168.4.32")
	want := "FoldingOS 0.1.0 ready\nAddress: 192.168.4.32\nSSH: foldingos-admin@192.168.4.32\n"
	if got != want {
		t.Fatalf("formatReadyDisplay():\n%q\nwant:\n%q", got, want)
	}
}

func TestParseIPv4Address(t *testing.T) {
	status := `●  2: enp0s2
     State: routable
   Address: 192.168.4.32 (DHCP)
`
	got, err := parseIPv4Address(status)
	if err != nil {
		t.Fatal(err)
	}
	if got != "192.168.4.32" {
		t.Fatalf("parseIPv4Address() = %q", got)
	}
}

func TestParseIPv4AddressPrefersDHCPAndSkipsLoopback(t *testing.T) {
	status := `Address: 127.0.0.1
   Address: 192.168.4.32 (DHCP)
`
	got, err := parseIPv4Address(status)
	if err != nil {
		t.Fatal(err)
	}
	if got != "192.168.4.32" {
		t.Fatalf("parseIPv4Address() = %q", got)
	}
}

func TestIsRoutableIPv4RejectsLoopback(t *testing.T) {
	if isRoutableIPv4("127.0.0.1") {
		t.Fatal("loopback address was accepted")
	}
	if !isRoutableIPv4("192.168.4.32") {
		t.Fatal("routable address was rejected")
	}
}

func TestSelectNetworkInterfacePrefersRoutable(t *testing.T) {
	listing := "lo no-carrier unmanaged\nenp0s2 routable configured\n"
	iface, err := selectNetworkInterfaceFromListing(listing)
	if err != nil {
		t.Fatal(err)
	}
	if iface != "enp0s2" {
		t.Fatalf("selectNetworkInterfaceFromListing() = %q", iface)
	}
}

func TestFailureDisplayMessage(t *testing.T) {
	got := failureDisplayMessage("FoldingOS 0.1.0", errors.New("no routable IPv4 address available"))
	if got != "FoldingOS 0.1.0\nNetwork: no routable IPv4 address available\n" {
		t.Fatalf("failureDisplayMessage() = %q", got)
	}
}
