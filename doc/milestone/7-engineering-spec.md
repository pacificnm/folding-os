# FoldingOS Milestone 7 Raspberry Pi 5 Engineering Specification

**Version:** 0.1

**Status:** Proposed

**Target Milestone:** Milestone 7, Raspberry Pi Support

**Issue:** [#153](https://github.com/pacificnm/folding-os/issues/153)

---

# Purpose

This document defines the engineering contract for the Raspberry Pi 5 ARM64
port. It is the concrete design companion to
[7-implementation-spec.md](7-implementation-spec.md).

Milestone 7 must produce a flashable Pi 5 image and supporting ARM64 runtime
artifacts without changing the accepted x86_64 UEFI architecture.

---

# Governing Decisions

| Document | Role |
| --- | --- |
| [ADR-0034](../adr/0034-raspberry-pi-5-boot-and-image-format.md) | Pi 5 boot and image architecture |
| [ADR-0035](../adr/0035-arm64-release-artifacts-and-runtime-bundles.md) | ARM64 release artifacts and runtime bundles |
| [ADR-0001](../adr/0001-use-buildroot.md) | Buildroot remains the build system |
| [ADR-0004](../adr/0004-partition-and-persistence-layout.md) | Partition and persistence model |
| [ADR-0008](../adr/0008-raw-image-size-and-data-expansion.md) | data-partition expansion safety |
| [ADR-0014](../adr/0014-fixed-installation-roles.md) | fixed `agent` and `supervisor` roles |
| [ADR-0018](../adr/0018-foldops-package-acquisition-and-update-model.md) | runtime FoldOps acquisition |
| [ADR-0023](../adr/0023-runtime-foldops-and-foldingosctl-updates-without-os-reimage.md) | layout-bundle transport |

ADR-0034 and ADR-0035 are proposed until an acceptance review is committed.
Implementation may prototype against them, but release behavior is not binding
until accepted.

---

# Board Target

Milestone 7 adds one new supported platform target:

```text
Platform: Raspberry Pi 5
Architecture: aarch64
Buildroot defconfig: configs/foldingos_raspberrypi5_defconfig
Board directory: board/foldingos/raspberrypi5/
```

The board target must define:

- Linux kernel configuration for Raspberry Pi 5
- device tree and overlay handling
- firmware and boot files required by the accepted boot chain
- root filesystem integration with existing FoldingOS overlays
- image generation steps for boot, root, and persistent data partitions
- validation metadata emitted by the build

The board target must share common FoldingOS overlay behavior where possible.
Platform differences belong in the Pi board directory or documented build
conditionals.

---

# Boot Architecture

## Required boot-chain contract

The accepted implementation must document this exact sequence:

```text
Raspberry Pi 5 power-on

↓

Raspberry Pi ROM / EEPROM bootloader

↓

Accepted FoldingOS Pi boot partition contents

↓

Linux kernel + device tree

↓

root filesystem

↓

systemd

↓

FoldingOS services
```

If U-Boot, UEFI firmware, or GRUB is inserted between the Pi firmware and Linux,
the implementation must document:

- why it is needed
- how it is built or acquired reproducibly
- where it is stored in the image
- how failures are diagnosed
- how it affects future image update work

## Boot partition

The Pi boot partition is platform-specific. It may contain firmware, boot
configuration, kernel images, device trees, overlays, and bootloader files
required by the accepted boot chain.

The boot partition must not become an operator configuration dumping ground.
Operator-managed configuration remains under `/data/config` unless a future ADR
explicitly changes ownership.

## Diagnostics

Validation must capture:

- boot media type
- Pi firmware or EEPROM version when available
- bootloader path selected
- kernel command line
- service startup state
- local commissioning output
- network address acquisition

---

# Image Layout

The Pi image must preserve the logical storage model:

| Area | Purpose | Notes |
| --- | --- | --- |
| Boot | Pi boot assets | platform-specific contents |
| Root filesystem | FoldingOS OS image | disposable OS state |
| Persistent data | `/data` | final partition; expands on first boot |

The initial image size should remain as small as practical while large enough
for boot assets, root filesystem, and a valid initial data filesystem.

The data partition expansion implementation must:

- identify the boot device without assuming `/dev/mmcblk0` or `/dev/nvme0n1`
- confirm the data partition is final
- never shrink or recreate persistent data
- preserve filesystem identity when expanding
- log failures clearly
- allow safe continuation with original data capacity when possible

---

# Release Artifacts

Required image artifact:

```text
foldingos-raspberrypi5-aarch64-<version>.img
```

Required associated metadata:

- SHA-256 checksum
- signature reference when signing is enabled
- release notes entry
- platform identifier
- architecture identifier
- Buildroot version
- kernel version
- boot firmware or bootloader source version where applicable
- validation status

Required runtime bundle artifact families:

```text
foldops-agent-aarch64-<version>.tar.zst
foldops-supervisor-aarch64-<version>.tar.zst
foldingos-tools-aarch64-<version>.tar.zst
```

Release and package indexes must let a supervisor distinguish x86_64 and ARM64
artifacts before assignment or activation.

---

# Runtime Behavior

## Roles

The Pi image supports the existing fixed-role model:

- `agent`
- `supervisor`

Roles are selected and persisted by the existing role mechanism unless an
accepted ADR amends that behavior for Pi.

## FoldOps

FoldOps binaries remain runtime-acquired. Pi images must not embed FoldOps
release payloads.

Expected behavior:

- Pi `agent` acquires `foldops-agent-aarch64`
- Pi `supervisor` acquires `foldops-supervisor-aarch64`
- mismatched architecture bundles are rejected
- supervisor update UI and APIs expose only compatible assignments

## foldingosctl

`foldingosctl` runtime tools bundles must be available for `aarch64` where the
Milestone 5 and Milestone 6 update workflows require them.

The on-image bootstrap version and runtime-updated version must report
architecture clearly in machine-readable output.

## Folding@home

The Pi release cannot claim Folding support until the ARM64 Folding@home client
and required cores are verified through the ADR-0009 acquisition path.

Validation must record:

- upstream package or archive selected
- digest verification result
- activation path
- service start result
- first observed work assignment or documented upstream blocker

---

# Provisioning

Milestone 7 initial support is direct-flash unless ADR acceptance selects a
Pi-specific network provisioning path.

Direct-flash workflow:

1. Download Pi image.
2. Verify digest and signature when available.
3. Flash to SD card or validated NVMe media.
4. Boot Pi 5 with wired Ethernet.
5. Complete existing role and administrator bootstrap.

x86_64 UEFI network provisioning remains governed by ADR-0016. Pi support must
not change x86 provisioning behavior.

Pi network boot, HTTP boot, or supervisor-led Pi provisioning are future design
items unless explicitly added by an accepted ADR.

---

# Hardware Validation Matrix

Minimum required validation for Milestone 7:

| Area | Required evidence |
| --- | --- |
| Board | Raspberry Pi 5 model and RAM size |
| Power | power supply model and rating |
| Firmware | EEPROM or firmware version when available |
| Boot media | SD card model and capacity |
| Network | wired Ethernet DHCP |
| Storage | data partition expansion and persistence |
| Display | local commissioning output on HDMI |
| Role | agent and supervisor role tests |
| Runtime | FoldOps and `foldingosctl` ARM64 acquisition |
| Folding | ARM64 Folding@home acquisition and service start |

Conditional validation:

| Area | Required evidence |
| --- | --- |
| NVMe | M.2 HAT model, SSD model, boot success, expansion success |
| USB storage | adapter model, boot success, expansion success |
| Pi network boot | boot method, server config, security boundary |

If conditional validation is not complete, the readiness review must state
whether the feature is unsupported, experimental, or deferred.

---

# Verification Commands

Exact commands may change as implementation lands. The issue work must provide
project-specific commands for:

```bash
# build Pi image
make foldingos_raspberrypi5_defconfig
make

# inspect image partitions
sgdisk -p output/images/foldingos-raspberrypi5-aarch64-<version>.img

# build FoldOps ARM64 bundles
cargo build --release --target aarch64-unknown-linux-gnu

# run web/dashboard checks when supervisor role changes affect FoldOps
npm run build
npm test
```

The final implementation must replace placeholders with the repository's actual
Buildroot wrapper, release, package, and validation commands.

---

# Failure Handling

Pi-specific failures must not silently fall back to x86 assumptions.

Required failure behavior:

- boot-chain validation failure blocks release readiness
- data expansion failure must not format persistent storage
- missing ARM64 runtime bundle blocks Pi FoldOps activation
- architecture mismatch blocks update activation
- missing Folding@home ARM64 support blocks Pi Folding support claims
- supervisor role instability blocks supervisor support or requires ADR update

---

# Unknowns

The following are intentionally unresolved until ADR acceptance or validation:

- exact Pi boot chain
- whether U-Boot, UEFI, or GRUB is used
- Pi boot partition size and contents
- Pi firmware pinning and reproducibility policy
- exact release image size
- first supported NVMe HAT and SSD combination
- whether Pi 5 supervisor role meets reliability expectations
- Folding@home ARM64 client/core availability
- Pi network provisioning strategy

---

# Related Documents

- [Milestone 7 implementation specification](7-implementation-spec.md)
- [Raspberry Pi 5 platform design](../raspberry-pi-5-platform.md)
- [hardware-support.md](../hardware-support.md)
- [build-system.md](../build-system.md)
- [storage-layout.md](../storage-layout.md)
