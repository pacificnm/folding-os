# FoldingOS Installer

Version: 0.1
Status: Draft

## Purpose

This document defines what “installation” means for FoldingOS.

FoldingOS is an appliance operating system. The preferred installation method should be simple, reproducible, and reliable.

## Installation Philosophy

For early releases, installation means flashing a complete disk image.

FoldingOS should not initially use a traditional interactive installer.

The preferred workflow is:

```text
Download image
↓
Flash image
↓
Boot target machine
↓
Complete first-boot configuration
↓
Start Folding
```

## Supported Installation Methods

### Primary: Flashable Image

The primary installation method is a complete bootable image written to storage.

Examples:

- USB flash drive
- SSD
- NVMe drive
- SD card for Raspberry Pi

This model is simple, predictable, and consistent with appliance-style systems.

The v0.1.0 image is 4 GiB. On boot, its final persistent data partition and
filesystem automatically expand to the maximum usable capacity of the target
device. Devices smaller than the release image are unsupported. Expansion
behavior and failure safety are defined by
[ADR-0008](adr/0008-raw-image-size-and-data-expansion.md).

### Future: Interactive Installer

A future release may provide a bootable installer ISO for x86_64 systems.

That installer may:

- Detect disks
- Partition storage
- Format filesystems
- Copy FoldingOS
- Install bootloader
- Configure first boot

This is not required for initial releases.

## Installer Scope

The installer or image process should provide:

- Boot partition
- Root filesystem
- Persistent data area
- Bootloader configuration
- Kernel
- Required system services
- First-boot configuration hook

## Non-Goals

The installer should not provide:

- Desktop environment selection
- Package selection
- Development tool selection
- General Linux customization
- Multi-purpose server roles

FoldingOS is not a general-purpose distribution.

## First-Boot Relationship

Installation and first-boot configuration are separate concerns.

Installation places the operating system on storage.

First boot handles node-specific configuration such as:

- Hostname
- Network settings
- FoldOps supervisor address
- Folding@home identity/configuration
- SSH access

For v0.1.0, the administrator prepares SSH access after flashing and before
booting the target node by writing public keys to:

```text
/foldingos/provision/authorized_keys
```

This path is relative to the EFI System Partition. When FoldingOS is running,
the file appears under `/boot/efi`.

On boot, FoldingOS validates and imports the keys into persistent configuration.
The complete workflow is defined by
[ADR-0007](adr/0007-first-boot-administrator-and-ssh-provisioning.md).

## Data Preservation

Initial flashing may overwrite the target device.

Future installer versions should support preserving:

- Configuration
- Node identity
- Folding@home work/checkpoints
- Logs where appropriate

The storage layout is defined by
[ADR-0004](adr/0004-partition-and-persistence-layout.md).

## Safety Requirements

Installer tooling should:

- Clearly identify target disks
- Avoid accidental overwrite where practical
- Require explicit confirmation before destructive actions
- Produce clear logs
- Fail safely

## Unresolved Decisions

The following decisions require ADRs:

- Whether v1.0 ships only flashable images
- Whether x86_64 needs an installer ISO
- Raspberry Pi image generation process

## Summary

For early FoldingOS releases, installation means flashing a complete image.

A traditional installer may come later, but it should not block the first working release.

The priority is a reliable appliance workflow:

```text
Flash. Boot. Configure. Fold.
```
