# FoldingOS Milestone 2 Implementation Specification

## Status

Draft

## Overview

Milestone 2 implements managed-node functionality through integration with the FoldOps project while preserving the deterministic operating system architecture established in Milestone 1.

---

# Repository Relationship

FoldOps remains an independent repository.

Repository:

* https://github.com/pacificnm/foldops

FoldingOS consumes approved, pinned FoldOps releases.

The FoldOps source tree is not merged into the FoldingOS repository.

---

# FoldOps Agent Package

A Buildroot package shall be created for the FoldOps Agent.

Implementation shall:

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
