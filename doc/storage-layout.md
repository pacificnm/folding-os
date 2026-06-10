# FoldingOS Storage Layout

## Purpose

This document defines the logical storage architecture of FoldingOS.

The objective is to provide a storage model that is:

- Simple
- Reliable
- Recoverable
- Easy to maintain
- Future-proof
- Suitable for both single-node and fleet deployments

This document describes the logical architecture rather than a specific partition implementation.

The v0.1.0 partition implementation, image size, and automatic data-partition
expansion behavior are defined by
[ADR-0004](adr/0004-partition-and-persistence-layout.md) and
[ADR-0008](adr/0008-raw-image-size-and-data-expansion.md).

---

# Design Goals

The storage architecture should:

- Minimize complexity
- Preserve Folding@home work across reboots
- Support future image-based updates
- Support future rollback capabilities
- Protect configuration data
- Separate operating system components from runtime data

---

# Design Philosophy

The operating system should be considered disposable.

Configuration and scientific work data should not.

Whenever practical:

- The operating system can be replaced.
- Configuration should survive.
- Folding checkpoints should survive.
- Logs should survive according to retention policy.

---

# Logical Layout

+--------------------------------------+
| Boot                                 |
+--------------------------------------+

+--------------------------------------+
| Operating System                     |
| (Root Filesystem)                    |
+--------------------------------------+

+--------------------------------------+
| Persistent Data                      |
|                                      |
| Configuration                        |
| Folding Checkpoints                  |
| Logs                                 |
| Runtime State                        |
+--------------------------------------+

Future versions may separate these areas into dedicated partitions.

---

# Boot Area

Responsibilities:

- Boot loader
- Kernel
- Boot configuration
- Early startup assets

Users should rarely need to modify this area.

---

# Root Filesystem

Contains:

- Operating system
- System binaries
- Runtime libraries
- Required services
- Core utilities

The root filesystem should remain as static as practical.

Future immutable operation remains a project objective.

---

# Persistent Data

Persistent storage may contain:

## Configuration

Examples:

- node identity
- network configuration
- FoldOps registration
- local settings

Configuration should survive operating system replacement.

---

## Folding@home Data

Examples:

- work units
- checkpoints
- runtime state

Unexpected shutdown should not result in unnecessary work loss.

---

## Logs

Local logs should support:

- diagnostics
- troubleshooting
- recovery

Retention policies should minimize unnecessary storage consumption.

Future centralized log collection may reduce local retention requirements.

---

## Runtime Data

Examples:

- temporary runtime files
- service state
- transient metadata

Temporary runtime information should not unnecessarily consume persistent storage.

---

# Update Philosophy

Future update systems should replace operating system components without unnecessarily affecting:

- configuration
- Folding checkpoints
- logs
- persistent state

Image replacement should be preferred over uncontrolled in-place modification.

---

# Recovery Philosophy

Recovery operations should preserve:

- configuration
- Folding progress
- operational state whenever practical

Operating system recovery should not require complete system reconfiguration.

---

# Future Architecture

Future releases may support:

- A/B root filesystems
- read-only operating system partitions
- signed images
- automatic rollback
- snapshot-based recovery

Implementation details will be documented separately.

---

# Hardware Independence

The logical storage architecture should remain consistent across:

- x86_64
- Raspberry Pi
- future supported platforms

Implementation details may differ while preserving the same logical model.

---

# Summary

Storage should support the primary mission of FoldingOS:

Provide a reliable, maintainable platform that preserves scientific work while minimizing operational complexity.

The operating system should be replaceable.

Scientific contribution should not be.
