# FoldingOS Milestone 2 Engineering Specification

## Status

Draft

## Purpose

Milestone 2 extends the deterministic FoldingOS platform established in Milestone 1 by introducing managed-node capabilities through integration with the FoldOps ecosystem.

The primary objective is to enable centralized monitoring and management of FoldingOS nodes while preserving the architectural principles established in Milestone 1:

* deterministic builds
* reproducible artifacts
* documentation-first engineering
* immutable operating system image
* minimal attack surface
* explicit release gates

---

# Scope

Milestone 2 implements:

* FoldOps agent integration
* secure node enrollment
* persistent node identity
* supervisor communication
* hardware inventory reporting
* Folding@home status reporting
* health monitoring
* structured diagnostics
* centralized configuration synchronization
* systemd-managed FoldOps services

---

# Architectural Principles

The FoldOps Agent remains a separate project and repository.

FoldingOS shall consume an approved, pinned FoldOps Agent release.

The operating system shall not depend on mutable online sources without an approved acquisition manifest.

All externally acquired software must be:

* version pinned
* cryptographically verified
* documented
* reproducible

---

# Node Identity

Each FoldingOS node shall possess a persistent identity.

Identity shall survive:

* reboot
* software updates
* Folding@home updates

Identity shall not change unless explicitly reset by an administrator.

---

# Enrollment

Enrollment shall support:

* enrollment tokens
* secure registration
* certificate validation
* authenticated supervisor communication

Enrollment shall never require embedded credentials inside the operating system image.

---

# Supervisor Communication

Communication shall support:

* HTTPS
* authenticated sessions
* periodic heartbeat
* health reporting
* configuration synchronization
* diagnostics

No unauthenticated communication is permitted.

---

# Hardware Inventory

Reported inventory shall include:

* CPU model
* logical processors
* memory
* storage
* network interfaces
* MAC addresses
* FoldingOS version
* kernel version
* FoldOps Agent version

---

# Folding@home Reporting

Reported information shall include:

* client version
* slot information
* active project
* progress
* estimated completion
* PPD
* completion statistics
* client health
* error conditions

---

# Health Monitoring

Health reporting shall include:

* CPU utilization
* memory utilization
* storage utilization
* temperatures
* uptime
* load average
* service status
* network status

---

# Security Requirements

Milestone 2 shall preserve the security principles established in Milestone 1.

No remote shell functionality shall be implemented.

No arbitrary remote command execution shall be implemented.

All communications shall be authenticated and encrypted.

Least-privilege execution is required.

---

# Explicitly Out of Scope

Milestone 2 does not implement:

* GPU management
* OTA operating system updates
* package management
* container runtime
* Docker
* Kubernetes
* remote shell
* arbitrary command execution
* local web administration interface

---

# Milestone Exit Criteria

Milestone 2 is complete when:

* FoldOps Agent is integrated
* secure enrollment functions correctly
* supervisor communication is operational
* hardware inventory reporting functions
* Folding@home reporting functions
* health monitoring functions
* identity persists across reboot
* systemd services recover automatically
* documentation and implementation remain synchronized
* reproducibility guarantees established in Milestone 1 remain intact
