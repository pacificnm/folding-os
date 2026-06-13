# FoldingOS Architecture

## Overview

FoldingOS is a purpose-built operating system designed exclusively for running Folding@home compute nodes.

The architecture emphasizes:

- Simplicity
- Reliability
- Security
- Reproducibility
- Fleet management
- Minimal resource overhead

The operating system is designed as an appliance rather than a general-purpose Linux distribution.

Its primary responsibility is to provide a stable execution environment for Folding@home while integrating seamlessly with centralized management through FoldOps.

---

# High-Level Architecture

                         +----------------------+
                         |      FoldOps         |
                         | Management Platform  |
                         +----------+-----------+
                                    |
                             Secure HTTPS/API
                                    |
            -------------------------------------------------
            |                       |                       |
            |                       |                       |
      +------------+         +------------+         +------------+
      | FoldingOS  |         | FoldingOS  |         | FoldingOS  |
      |   Node 1   |         |   Node 2   |         |   Node N   |
      +------------+         +------------+         +------------+
            |                       |                       |
            +-----------+-----------+-----------------------+
                        |
                 Folding@home Network

---

# Architectural Philosophy

Every component within FoldingOS must justify its existence.

The preferred architecture is:

- small
- understandable
- maintainable
- secure
- deterministic

Features that do not directly contribute to the mission should be excluded.

---

# Major Components

## Linux Kernel

Responsibilities:

- hardware abstraction
- process scheduling
- memory management
- networking
- filesystem support
- hardware drivers

The kernel should be configured specifically for supported hardware targets to minimize unnecessary complexity.

---

## Root Filesystem

Provides:

- minimal userland
- required runtime libraries
- configuration
- startup services

The root filesystem should remain as small as practical.

Future versions may adopt an immutable architecture.

---

## Init System

Responsible for:

- system startup
- service supervision
- dependency ordering
- restart policies
- shutdown

FoldingOS uses systemd for init and service supervision, as defined by
[ADR-0002](adr/0002-init-and-service-supervision.md).

---

## Networking

Provides:

- Ethernet support
- DHCP
- static IP configuration
- DNS
- NTP
- planned first-class IPv6 support

Networking should require minimal configuration while remaining predictable and reliable.

The first bootable x86_64 implementation may support IPv4 only. IPv6 support
should be added without changing the intended networking architecture.

---

## Secure Remote Access

Primary administration interface:

- SSH

Remote administration should be:

- authenticated
- encrypted
- minimal
- secure by default

Initial administrator and SSH-key provisioning is defined by
[ADR-0007](adr/0007-first-boot-administrator-and-ssh-provisioning.md).

Installer-capable releases are superseded by supervisor-led network
provisioning. Deployment architecture is defined by
[ADR-0016](adr/0016-network-provisioning-via-supervisor.md).

Installations use one of the fixed `agent` or `supervisor` roles defined by
[ADR-0014](adr/0014-fixed-installation-roles.md). Roles cannot be changed in
place.

No local desktop environment is planned.

No local browser interface is planned.

Production nodes do not keep a monitor or keyboard attached. During
commissioning, a temporarily attached monitor shows boot messages and a final
ready message with the DHCP IPv4 address and SSH entry point, as defined by
[ADR-0015](adr/0015-local-commissioning-display.md).

---

## Folding@home Runtime

The primary workload of the operating system.

Responsibilities:

- execute Folding@home client
- monitor execution
- persist checkpoints
- recover after reboot
- recover after power failure

The Folding@home client represents the primary computational purpose of the operating system.

The client is acquired after deployment from an approved official upstream
origin and is not contained in the FoldingOS release image. FoldingOS verifies
and activates a pinned version according to
[ADR-0009](adr/0009-fah-acquisition-and-update-model.md).

---

## FoldOps Agent

The FoldOps Agent serves as the management interface between FoldingOS and centralized infrastructure.

Potential responsibilities include:

- node registration
- health reporting
- metrics reporting
- version reporting
- remote configuration
- update coordination
- approved Folding@home client-version rollout coordination
- inventory reporting

Communication should occur using authenticated and encrypted protocols.

---

## FoldOps Supervisor And Web

A supervisor-role installation runs the FoldOps agent, supervisor, and web
services from the same FoldingOS image used by agent-role installations.

The web interface is enabled by default for the supervisor role but must not
become remotely available until initial administrator and TLS provisioning
succeeds.

Whether supervisor-role installations also run the Folding@home workload
remains unresolved.

---

## Logging

Logging should prioritize:

- reliability
- diagnostics
- minimal storage usage

Logs should support:

- local troubleshooting
- remote collection
- operational diagnostics

Retention policies should minimize unnecessary disk usage.

FoldingOS uses `systemd-journald` with bounded persistent storage under
`/data/logs/journal`. Persistent logging failure degrades to volatile logging
and does not stop Folding@home. See
[ADR-0010](adr/0010-persistent-logging-and-retention.md).

---

## Update System

The update architecture must emphasize:

- reliability
- rollback capability
- authenticated and integrity-verified update artifacts
- reproducibility

The initial update implementation is planned for a later milestone. Once
production updates are enabled, unsigned update artifacts must not be accepted.

The implementation may include:

- image-based updates
- A/B partitions
- automatic rollback
- staged deployments

The concrete implementation will be defined in the
[update system specification](update-system.md).

---

# Boot Sequence

Conceptual startup sequence:

Power On

↓

Firmware

↓

Bootloader

↓

Linux Kernel

↓

Init System

↓

Networking

↓

Time Synchronization

↓

FoldOps Agent

↓

Folding@home Acquisition

↓

Folding@home Startup

↓

Operational State

Each stage should fail gracefully whenever possible.

---

# Storage Model

Persistent storage should be limited to:

- configuration
- logs
- Folding@home checkpoints
- update metadata
- optional diagnostics

The operating system itself should remain as static as practical.

Future immutable image support remains a project objective.

---

# Security Model

Security principles include:

- least privilege
- minimal attack surface
- authenticated remote management
- encrypted communications
- signed updates
- reproducible builds

Security architecture is defined in the [security model](security.md).

---

# Fleet-Oriented Design

Although suitable for a single node, FoldingOS is designed to scale naturally to:

- home laboratories
- educational environments
- research clusters
- enterprise deployments

Centralized management through FoldOps should eliminate the need for per-node administration wherever practical.

---

# Hardware Targets

Platform sequence:

- first implementation target: x86_64 UEFI systems
- next planned target: Raspberry Pi 5

Future support will be based on engineering requirements rather than broad hardware compatibility.

---

# Non-Goals

The architecture intentionally excludes:

- desktop environments
- office software
- browsers
- multimedia applications
- gaming
- development toolchains
- unnecessary package management
- unrelated background services

The architecture remains intentionally focused on scientific contribution.

---

# Future Evolution

The architecture is expected to evolve through documented Architecture Decision Records (ADRs).

Significant architectural changes should always be accompanied by:

- documented rationale
- considered alternatives
- implementation consequences
- migration strategy

The long-term objective is not to maximize features, but to maximize reliability, maintainability, and scientific contribution.
