# FoldingOS Networking

Version: 0.1

Status: Living Document

---

# Purpose

This document defines the networking philosophy and expected behavior of
FoldingOS.

Reliable networking is essential for:

- Folding@home
- FoldOps communication
- updates
- diagnostics
- remote administration

---

# Design Goals

Networking should be:

- reliable
- predictable
- simple
- secure
- observable

---

# Default Configuration

Default preference:

- Ethernet
- DHCP
- automatic DNS
- automatic route configuration

The default experience should require minimal user intervention.

---

# Static Configuration

Where required, administrators should be able to configure:

- static IP
- subnet
- gateway
- DNS
- hostname

Configuration should remain explicit and documented.

---

# IPv6

IPv6 support should be considered a first-class capability.

Operation should remain functional in:

- IPv4-only
- IPv6-only
- dual-stack

environments where practical.

---

# Time Synchronization

Accurate time is required for:

- TLS
- certificates
- logging
- diagnostics

Time synchronization should occur automatically.

---

# DNS

Reliable DNS resolution is required for:

- Folding@home
- FoldOps
- update infrastructure

DNS failures should be detectable and diagnosable.

---

# Failure Handling

Temporary network failures should not require reboot.

Automatic recovery should occur whenever practical.

Graceful degradation is preferred.

---

# Remote Access

SSH remains the primary remote administration interface.

Future management functionality should integrate through FoldOps rather than
introducing unnecessary local services.

---

# Future Considerations

Potential future capabilities:

- VLAN awareness

- multiple interfaces

- bonded interfaces

- enterprise deployments

These should be evaluated based on project requirements rather than feature
requests alone.

---

# Summary

Networking should be simple, reliable, secure, and require minimal ongoing
administration.