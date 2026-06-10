# FoldingOS Boot Process

## Purpose

This document defines the expected startup sequence for FoldingOS.

The objective is to ensure that every node boots quickly, predictably, securely, and with minimal administrative intervention while preparing the system to contribute computational resources to Folding@home.

This document describes the logical boot sequence rather than implementation-specific details.

---

# Design Goals

The FoldingOS boot process should be:

- deterministic
- reliable
- observable
- recoverable
- secure
- simple

A successful boot should require no user interaction.

---

# High-Level Boot Sequence

```text
Power On
    │
    ▼

Firmware (BIOS / UEFI)

    │
    ▼

Boot Loader

    │
    ▼

Linux Kernel

    │
    ▼

Initial Root Filesystem

    │
    ▼

Init System

    │
    ▼

Core System Services

    │
    ▼

Network Initialization

    │
    ▼

Time Synchronization

    │
    ▼

Configuration Validation

    │
    ▼

FoldOps Agent Startup

    │
    ▼

Folding@home Startup

    │
    ▼

Operational State
```

---

# Stage 1 - Firmware

System firmware performs:

- hardware initialization
- memory initialization
- CPU initialization
- device discovery
- boot device selection

Firmware should transfer execution to the configured boot loader.

FoldingOS does not modify firmware behavior.

---

# Stage 2 - Boot Loader

Responsibilities include:

- locating the operating system
- loading the Linux kernel
- loading the initial ramdisk if required
- passing kernel parameters

Boot loader implementation is intentionally unspecified at this stage.

Selection will be documented through an Architecture Decision Record.

---

# Stage 3 - Linux Kernel

The Linux kernel initializes:

- scheduler
- memory management
- storage
- networking
- device drivers
- process management

Kernel configuration should remain as minimal as practical.

Only required functionality should be enabled.

---

# Stage 4 - Root Filesystem

The root filesystem becomes available and provides:

- runtime libraries
- configuration
- startup scripts
- required system binaries

Future immutable filesystem support remains a project objective.

---

# Stage 5 - Init System

The init system becomes responsible for:

- service startup
- dependency ordering
- restart management
- shutdown sequencing
- system supervision

The implementation remains intentionally unspecified pending ADR approval.

---

# Stage 6 - Core Services

Core operating system services initialize.

Examples include:

- logging
- hostname
- persistent storage
- runtime directories
- local configuration

Only required services should execute.

---

# Stage 7 - Networking

Networking is initialized.

Typical responsibilities:

- Ethernet initialization
- DHCP or static configuration
- DNS configuration
- IPv6 (where enabled)
- route establishment

Network initialization failures should be detectable and recoverable.

---

# Stage 8 - Time Synchronization

Accurate system time is required for:

- TLS
- certificates
- logging
- diagnostics
- secure communications

Time synchronization should occur automatically whenever possible.

---

# Stage 9 - Configuration Validation

Before application startup, FoldingOS validates:

- local configuration
- required directories
- storage availability
- required runtime state

Recoverable problems should be corrected automatically where practical.

Fatal errors should be clearly logged.

---

# Stage 10 - FoldOps Agent

The FoldOps Agent initializes.

Potential responsibilities include:

- node registration
- health reporting
- metrics reporting
- inventory reporting
- remote management
- update coordination

Failure of the FoldOps Agent should not necessarily prevent Folding@home from operating.

---

# Stage 11 - Folding@home

The Folding@home client starts.

Responsibilities include:

- loading configuration
- resuming checkpoints
- obtaining work units
- executing scientific workloads
- periodically saving progress

Automatic recovery after reboot is a primary design objective.

---

# Operational State

A node is considered operational when:

- operating system startup has completed
- networking is functional
- Folding@home is executing normally
- health monitoring is active
- required services are healthy

At this point the system should require no user interaction.

---

# Failure Philosophy

Failures should be:

- detected
- logged
- recoverable whenever practical

Single-service failures should not require full system reboots unless absolutely necessary.

Graceful degradation is preferred over complete failure.

---

# Future Considerations

Future enhancements may include:

- secure boot integration
- TPM support
- measured boot
- A/B boot environments
- automatic rollback
- boot health verification

These capabilities should be introduced only when they improve reliability or security without unnecessary complexity.

---

# Summary

The FoldingOS boot process is designed around a single objective:

Bring the node into a reliable operational state as quickly and predictably as possible so it can begin contributing computational resources to Folding@home with minimal administrative overhead.