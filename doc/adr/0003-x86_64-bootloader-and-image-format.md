# ADR-0003: x86_64 Bootloader and Image Format

**Status:** Accepted

**Version:** 1.0

**Date:** 2026-06-10

**Authors:** FoldingOS Project Contributors

---

# Context

FoldingOS requires a standardized boot architecture and release artifact
format for x86_64 systems.

The project philosophy emphasizes:

* simplicity
* reproducibility
* reliability
* appliance-style deployment
* minimal operational complexity

The selected solution should be stable, widely supported, and easy to
automate.

---

# Decision

For x86_64 systems, FoldingOS adopts:

* UEFI as the primary boot method
* GRUB 2 as the bootloader
* Raw bootable disk images (`.img`) as the primary release artifact

The preferred deployment workflow is:

Download

↓

Verify

↓

Flash

↓

Boot

↓

Configure

↓

Fold

Interactive installers are explicitly not required for the initial release.

---

# Rationale

The project seeks predictable and repeatable deployments rather than maximum
installation flexibility.

A prebuilt image provides:

* deterministic layout
* reproducible installation
* simpler testing
* easier documentation
* reduced installation complexity
* appliance-like behavior

UEFI is now the standard firmware interface for modern x86_64 systems and
provides excellent compatibility.

GRUB 2 is mature, well-understood, and supported by Buildroot.

---

# Boot Architecture

The expected startup sequence is:

Firmware (UEFI)

↓

EFI System Partition

↓

GRUB 2

↓

Linux Kernel

↓

Initial Root Filesystem

↓

systemd

↓

Core Services

↓

Networking

↓

FoldOps (when enabled)

↓

Folding@home

↓

Operational State

---

Folding@home must not depend on FoldOps availability.

---

# Primary Release Artifact

The official release artifact shall be a complete bootable disk image.

Example:

foldingos-x86_64-0.1.0.img

For v0.1.0, this raw image is exactly 4 GiB. Its final persistent data
partition expands automatically to the boot device's maximum usable capacity
as defined by
[ADR-0008](0008-raw-image-size-and-data-expansion.md).

Associated release artifacts may include:

* SHA-256 checksum
* detached signature
* release notes
* version metadata

Future artifact formats may be added without replacing the primary image.

---

# Image Philosophy

Images should be:

* reproducible
* deterministic
* versioned
* documented
* verifiable

Users should not need to manually assemble operating system components.

---

# Partition Philosophy

The exact partition layout is defined separately by ADR-0004.

Expected logical structure:

EFI System Partition

↓

Operating System

↓

Persistent Data

Future A/B layouts remain possible.

---

# Alternatives Considered

## Interactive Installer

Advantages:

* familiar user experience
* flexible partitioning
* custom installation options

Disadvantages:

* increased complexity
* additional testing burden
* larger implementation effort
* inconsistent deployments

Decision:

Rejected for Milestone 1.

May be reconsidered in the future.

---

## Legacy BIOS Support

Advantages:

* compatibility with older hardware

Disadvantages:

* additional engineering burden
* additional testing complexity
* declining industry relevance

Decision:

Not a design target for initial releases.

Support may be evaluated later if justified.

---

## systemd-boot

Advantages:

* lightweight
* modern UEFI integration
* straightforward configuration

Disadvantages:

* less flexible for future boot scenarios
* narrower deployment familiarity

Decision:

Rejected in favor of GRUB 2.

---

## Custom Bootloader

Advantages:

* complete control

Disadvantages:

* unnecessary engineering effort
* additional maintenance burden
* unnecessary project risk

Decision:

Rejected.

---

# Consequences

## Positive

* deterministic deployment
* simple documentation
* reproducible images
* strong Buildroot compatibility
* mature bootloader ecosystem
* predictable operational behavior

## Negative

* less installation flexibility
* UEFI-first design
* no interactive installer initially

These tradeoffs are acceptable given project objectives.

---

# Security Considerations

Future releases may support:

* Secure Boot
* signed EFI binaries
* measured boot
* TPM integration
* boot verification

These capabilities should extend the existing architecture rather than
replace it.

---

# Operational Philosophy

Boot should require no user interaction.

Once flashed, the node should reliably progress to:

Power On

↓

Boot

↓

Network

↓

Fold

with minimal administrative effort.

---

# Future Review

This decision should be revisited only if:

* UEFI standards materially change
* GRUB becomes unsuitable
* project requirements fundamentally change
* measurable engineering benefits justify an alternative

Technology trends alone are insufficient justification.

---

# Related Documents

* [Project charter](../../PROJECT_CHARTER.md)
* [Engineering principles](../../PRINCIPLES.md)
* [Boot process](../boot-process.md)
* [Storage layout](../storage-layout.md)
* [Build system](../build-system.md)
* [Installer](../installer.md)
* [ADR-0001: Use Buildroot as the FoldingOS Build System](0001-use-buildroot.md)
* [ADR-0008: Raw Image Size and Data-Partition Expansion](0008-raw-image-size-and-data-expansion.md)

---

# Closing Statement

FoldingOS is designed as an appliance rather than a traditional Linux
distribution.

UEFI, GRUB 2, and reproducible disk images provide a mature and predictable
foundation that aligns with the project's goals of simplicity, reliability,
and long-term maintainability.

Deployment should be simple enough that users can confidently:

Download.

Flash.

Boot.

Configure.

Fold.
