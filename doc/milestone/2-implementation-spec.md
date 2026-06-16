# FoldingOS Milestone 2 Implementation Specification

## Status

Draft — **superseded for package integration** by
[ADR-0018](../adr/0018-foldops-package-acquisition-and-update-model.md), which
defines runtime acquisition of official artifacts instead of Buildroot-embedded
packages. **Superseded for source location** by
[ADR-0022](../adr/0022-foldops-rust-source-in-foldingos-monorepo.md), which
places FoldOps Rust source in `packages/foldops/`. Enrollment, heartbeat, and
service requirements below remain directional input for later milestones.

## Overview

Milestone 2 implements managed-node functionality through integration with the FoldOps project while preserving the deterministic operating system architecture established in Milestone 1.

---

# Repository Relationship

FoldOps Rust source for FoldingOS appliances lives in `packages/foldops/` per
[ADR-0022](../adr/0022-foldops-rust-source-in-foldingos-monorepo.md). Runtime
binaries are still acquired on `/data` at boot, not embedded in the OS image.

The legacy Node.js repository at `https://github.com/pacificnm/foldops` is
deprecated for appliance work.

FoldingOS consumes approved, pinned FoldOps releases from official HTTPS
publication channels.

---

# FoldOps Agent Package

~~A Buildroot package shall be created for the FoldOps Agent.~~

FoldOps packages are acquired at runtime on deployed appliances per
[ADR-0018](../adr/0018-foldops-package-acquisition-and-update-model.md). The
Buildroot integration approach below is retained only as historical design
context.

Implementation shall (via runtime acquisition):

* use approved pinned sources
* verify acquisition
* build deterministically
* install through Buildroot
* integrate with systemd

---

# Systemd Integration

Required services include:

* foldops-agent.service

Service requirements:

* automatic startup
* restart on failure
* dependency ordering
* journald logging
* graceful shutdown

---

# Enrollment Flow

First boot sequence:

1. generate persistent identity
2. initialize local configuration
3. obtain enrollment information
4. securely enroll
5. persist assigned identity
6. begin heartbeat reporting

---

# Heartbeat Reporting

Periodic heartbeat shall include:

* node identity
* FoldingOS version
* FoldOps Agent version
* uptime
* health status
* Folding@home status

---

# Hardware Collection

Collected information:

* CPU
* logical processors
* memory
* storage
* interfaces
* MAC addresses
* operating system version
* kernel version

---

# Folding@home Collection

Collected information:

* client version
* slot status
* project
* progress
* ETA
* PPD
* completion history
* error state

---

# Health Collection

Collected information:

* CPU usage
* memory usage
* storage usage
* temperatures
* load average
* service status

---

# Diagnostics

Supported diagnostics:

* configuration retrieval
* service status
* log retrieval
* version reporting
* health reporting

Remote shell functionality is prohibited.

---

# Configuration Synchronization

Configuration updates shall:

* be authenticated
* be validated
* be persisted
* support rollback where applicable
* avoid destructive modification

---

# Testing Requirements

Validation shall include:

* successful enrollment
* reboot persistence
* heartbeat verification
* inventory verification
* Folding@home reporting verification
* health reporting verification
* service restart testing
* supervisor communication testing

---

# Milestone Completion

Implementation is complete when all engineering requirements defined in `2-engineering-spec.md` have been satisfied and validated through documented testing.
