# FoldingOS v0.1.0 Scope Specification

**Version:** 1.0

**Status:** Draft

**Target Release:** v0.1.0

---

# Purpose

This document defines the intended implementation scope for the first working
release of FoldingOS. It combines roadmap Milestone 1, Bootable Base System,
and Milestone 2, Folding@home Integration, into the v0.1.0 release scope.

The objective is not to build a feature-rich operating system.

The objective is to build a stable, reproducible, appliance-style platform
capable of reliably running Folding@home.

Anything not explicitly listed in this document should be considered out of
scope for v0.1.0.

---

# Milestone Goal

The success criterion is simple:

```text
Flash

↓

Boot

↓

Acquire Network

↓

Start Services

↓

Start Folding@home

↓

Remain Operational
```

No GUI.

No package manager.

No desktop.

No unnecessary services.

---

# Target Platforms

Supported:

* x86_64 UEFI

Future:

* Raspberry Pi 5

Only x86_64 is required for v0.1.0.

---

# Build System

Build framework:

Buildroot

Reference:

[ADR-0001](../adr/0001-use-buildroot.md)

Build should produce reproducible release artifacts.

---

# Init System

Init:

systemd

Reference:

[ADR-0002](../adr/0002-init-and-service-supervision.md)

Responsible for:

* boot
* service management
* restart policy
* dependency ordering

---

# Bootloader

Bootloader:

GRUB 2

Firmware:

UEFI

Reference:

[ADR-0003](../adr/0003-x86_64-bootloader-and-image-format.md)

Legacy BIOS support is not required.

---

# Release Artifact

Primary artifact:

```text
foldingos-x86_64-<version>.img
```

The v0.1.0 raw image is exactly 4 GiB.

Additional artifacts:

* SHA256 checksum

* release notes

Future:

* detached signatures

---

# Partition Layout

v0.1.0:

```text
Disk

├── EFI (vfat, 512 MiB)
├── Root (ext4, 2 GiB)
└── Data (ext4, remaining image capacity)
```

The data partition is the final partition. On boot, it and its filesystem must
expand idempotently to the maximum usable aligned capacity of the boot device.
The implementation must never shrink or recreate the persistent data
filesystem.

Reference:

[ADR-0004](../adr/0004-partition-and-persistence-layout.md)

[ADR-0008](../adr/0008-raw-image-size-and-data-expansion.md)

---

# Mount Points

```text
/boot/efi

/

/data
```

Persistent application data resides under:

```text
/data
```

---

# Filesystem Layout

Examples:

```text
/data/config

/data/foldops

/data/fah

/data/logs

/data/state
```

---

# Networking

Default:

* Ethernet

* DHCP

* automatic DNS

IPv6 is not required for v0.1.0 and may be absent from the initial image.

Static configuration support may be added after initial implementation.

---

# SSH

Enabled:

Yes

Authentication:

Public-key only

SSH server:

OpenSSH

Password authentication:

Disabled

Root login:

Disabled

Administrator account:

```text
foldingos-admin
```

The account has no usable password credential and receives passwordless full
administrative privileges through an explicit sudo policy.

Initial and recovery keys are imported from:

```text
/boot/efi/foldingos/provision/authorized_keys
```

Persistent authorized keys reside at:

```text
/data/config/ssh/authorized_keys
```

SSH represents the primary administration interface.

Reference:

[ADR-0007](../adr/0007-first-boot-administrator-and-ssh-provisioning.md)

---

# Time Synchronization

Automatic time synchronization is required.

Correct system time is necessary for:

* TLS

* logging

* certificates

---

# Folding@home Service

Managed through:

systemd

Runs as:

```text
User: fah

Group: fah
```

Does not run as root during normal operation.

The Folding@home client is not included in the release image. After networking
and time synchronization are available, FoldingOS downloads the exact pinned
client artifact directly from an official Folding@home origin, verifies it
against the approved manifest embedded in the image, installs it into
versioned persistent application storage, and starts it.

Reference:

[ADR-0006](../adr/0006-fah-packaging-and-privilege-model.md)

[ADR-0009](../adr/0009-fah-acquisition-and-update-model.md)

---

# FoldOps

v0.1.0:

Not required.

A placeholder service may exist.

Remote management is not required for release.

---

# Logging

Minimum requirements:

* boot diagnostics

* service diagnostics

* Folding service logs

Persistent logs should survive reboot.

---

# Configuration

Persistent configuration resides under:

```text
/data/config
```

Configuration must survive reboot. The architecture keeps configuration
separate so future replacement tooling can preserve it.

Configuration precedence is defined by
[ADR-0005](../adr/0005-configuration-ownership-and-precedence.md).

---

# Update System

v0.1.0:

Full-image reflashing. Preservation of the existing data partition is not
guaranteed in v0.1.0.

OTA updates:

Not implemented.

A/B updates:

Not implemented.

Rollback:

Not implemented.

Architecture should permit future implementation.

---

# Security Defaults

Default principles:

* least privilege

* minimal attack surface

* no unnecessary services

* SSH enabled

* root SSH disabled

* SSH password authentication disabled

* no SSH access until an administrator key is provisioned

* Folding service non-root

No hardcoded credentials.

---

# Included Services

Expected services:

* systemd

* networking

* SSH

* time synchronization

* Folding@home acquisition

Folding@home is installed after deployment and then managed as a system
service.

No additional services should be enabled without explicit justification.

Included administrative tooling:

* sudo

---

# Explicit Non-Goals

v0.1.0 does NOT include:

* desktop environment

* browser

* package manager

* Docker

* Kubernetes

* GUI installer

* OTA updates

* FoldOps management

* TPM integration

* Secure Boot

* A/B partitions

* immutable root enforcement

* snapshot support

---

# Build Output

Successful build should produce:

* bootable image

* version metadata

* checksum

Future releases may additionally produce:

* signatures

* SBOM

* provenance metadata

---

# Open Decisions Blocking Approval

This scope specification remains Draft until the following decisions are
resolved:

1. Select and validate the exact official upstream Folding@home client artifact
   for v0.1.0, then record its version, URL, size, SHA-256 digest, runtime
   compatibility, and upstream terms reference in the approved manifest.
2. Define persistent logging implementation, retention, and disk-full behavior.
3. Define the measurable reproducible-build procedure and success condition.
4. Define the configuration file format and the mechanism that validates and
   applies effective configuration.

---

# Acceptance Criteria

A v0.1.0 build is considered successful if it can:

1. Build reproducibly from source

2. Produce a bootable image

3. Boot successfully on supported x86_64 hardware

4. Mount all expected filesystems

5. Acquire network connectivity

6. Synchronize system time

7. Import a valid administrator key and allow `foldingos-admin` to connect over
   SSH

8. Download the pinned Folding@home client directly from its approved official
   upstream origin and reject an artifact that fails verification

9. Install and start the verified Folding@home client automatically

10. Run Folding@home as the `fah` service account

11. Preserve Folding checkpoints across reboot

12. Preserve configuration across reboot

13. Shut down cleanly

14. Boot successfully after power interruption

15. Keep SSH inaccessible when no administrator key is provisioned

16. Replace administrator keys through the EFI provisioning path

17. Expand the data partition and filesystem to fill a larger boot device

18. Preserve existing persistent data during expansion

19. Make no partition or filesystem changes when expansion is unnecessary or
    has already completed

20. Continue running the last verified Folding@home client when FoldOps is
    unavailable

---

# Out of Scope

Anything not explicitly required by this specification should be deferred to
future milestones unless a documented engineering decision approves its
inclusion.

---

# Definition of Done

v0.1.0 is complete when a user can:

Flash an image to a machine.

Power it on.

Complete minimal configuration.

Watch it automatically join Folding@home.

Leave it unattended with confidence.

Nothing more is required for v0.1.0.

A stable foundation is the objective.
