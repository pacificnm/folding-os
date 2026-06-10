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

Selection of the init implementation will be documented through an Architecture Decision Record (ADR).

---

## Networking

Provides:

- Ethernet support
- DHCP
- static IP configuration
- DNS
- NTP
- optional IPv6

Networking should require minimal configuration while remaining predictable and reliable.

---

## Secure Remote Access

Primary administration interface:

- SSH

Remote administration should be:

- authenticated
- encrypted
- minimal
- secure by default

No local desktop environment is planned.

No local browser interface is planned.

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
- inventory reporting

Communication should occur using authenticated and encrypted protocols.

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

---

## Update System

The update architecture should emphasize:

- reliability
- rollback capability
- signed releases
- reproducibility

Future versions may implement:

- image-based updates
- A/B partitions
- automatic rollback
- staged deployments

Final implementation will be documented separately.

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

Security architecture is documented separately in `security.md`.

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

Initial support targets:

- x86_64 UEFI systems
- Raspberry Pi 5

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