# FoldingOS Milestone 7 Raspberry Pi 5 Implementation Specification

**Version:** 0.1

**Status:** Proposed

**Target Milestone:** Milestone 7, Raspberry Pi Support

**Issue:** [#153](https://github.com/pacificnm/folding-os/issues/153)

---

# Purpose

This document defines the implementation scope for Milestone 7.

Milestone 7 ports FoldingOS to Raspberry Pi 5 as the first ARM64 platform while
preserving the existing appliance model. It adds a reproducible, flashable Pi 5
image, architecture-aware runtime bundles, and Pi-specific validation without
changing the x86_64 UEFI boot and provisioning contracts.

Concrete platform design and verification details are in
[7-engineering-spec.md](7-engineering-spec.md) and
[raspberry-pi-5-platform.md](../raspberry-pi-5-platform.md).

Proposed ADRs:

- [ADR-0034](../adr/0034-raspberry-pi-5-boot-and-image-format.md)
- [ADR-0035](../adr/0035-arm64-release-artifacts-and-runtime-bundles.md)

---

# Milestone Goal

```text
Build host

↓

Buildroot Raspberry Pi 5 ARM64 target

↓

Reproducible flashable image

↓

Pi 5 boots from supported media

↓

Persistent data expands safely

↓

Fixed agent or supervisor role initializes

↓

FoldOps, foldingosctl, and Folding@home runtime acquisition select ARM64
artifacts

↓

Pi 5 contributes Folding@home work when upstream ARM64 support is verified
```

---

# Scope

## In scope

- Raspberry Pi 5 ARM64 Buildroot board support
- `configs/foldingos_raspberrypi5_defconfig`
- flashable Pi 5 raw image generation
- Pi boot partition contents, kernel, device tree, overlays, and boot
  configuration
- SD-card boot validation
- NVMe boot validation with approved Pi 5 M.2 hardware when available
- Ethernet DHCP validation
- persistent data partition expansion on Pi boot media
- fixed `agent` and `supervisor` role support unless ADR review narrows scope
- `aarch64` FoldOps layout bundles
- `aarch64` `foldingosctl` tools bundles
- architecture-aware release and package metadata
- Folding@home ARM64 client and core availability verification
- Pi-specific hardware validation records
- operator documentation for flashing, first boot, and validation

## Out of scope

- Raspberry Pi models before Pi 5
- 32-bit ARM
- broad ARM SBC support
- Kubernetes or a generic container runtime
- changing the x86_64 UEFI boot architecture
- replacing supervisor-led x86_64 network provisioning
- A/B root filesystem updates
- Wi-Fi as a required deployment path
- GPU, camera, GPIO, desktop, or media workloads

---

# Prerequisites

Before implementation merges that change boot or release format:

1. ADR-0034 and ADR-0035 must be accepted or superseded.
2. The accepted boot chain must identify every boot-stage artifact copied into
   the Pi image.
3. The release metadata format must include architecture and platform fields.
4. The Folding@home ARM64 acquisition question must have a documented answer:
   supported, unsupported, or temporarily blocked.

---

# Implementation Sequence

## Phase 1 - Architecture acceptance

1. Review and accept the Milestone 7 ADR set.
2. Confirm the Pi boot chain and image layout.
3. Confirm artifact naming and release-index changes.
4. Create child implementation issues from this specification.

**Exit criteria:** implementation agents have accepted ADRs and no unresolved
architecture question blocks board support work.

## Phase 2 - Board and image foundation

1. Add the Raspberry Pi 5 board directory.
2. Add the ARM64 defconfig.
3. Add boot partition generation assets.
4. Produce a raw image artifact.
5. Confirm the image has the expected boot, root, and data partitions.

**Exit criteria:** a build produces a Pi 5 image artifact with documented
partition contents.

## Phase 3 - Boot and storage validation

1. Flash the image to SD card.
2. Boot on Raspberry Pi 5 with wired Ethernet.
3. Validate systemd startup, local commissioning output, and SSH access.
4. Validate persistent data mount and expansion.
5. Repeat boot to confirm idempotent expansion.
6. Validate NVMe boot if approved hardware is available.

**Exit criteria:** Pi 5 boots reliably from required media and preserves
persistent state across reboot.

## Phase 4 - Runtime acquisition

1. Build FoldOps `aarch64` layout bundles.
2. Build `foldingosctl` `aarch64` tools bundles.
3. Add architecture fields to package indexes.
4. Verify supervisor assignment rejects mismatched architecture artifacts.
5. Verify Pi nodes acquire the correct runtime artifacts.

**Exit criteria:** Pi nodes do not install x86_64 runtime bundles and can
activate ARM64 bundles.

## Phase 5 - Folding@home verification

1. Verify upstream ARM64 Folding@home client availability.
2. Verify approved download URL, digest, and activation flow.
3. Verify required cores run on Pi 5 under FoldingOS.
4. Record any upstream limitation in the readiness review.

**Exit criteria:** Pi 5 can complete the Folding@home acquisition path and begin
work, or the milestone is explicitly blocked from claiming Folding support.

## Phase 6 - Role and FoldOps validation

1. Validate `agent` role first boot and registration.
2. Validate `supervisor` role first boot and dashboard availability.
3. Validate FoldOps ingest from Pi agent to supervisor.
4. Validate update and recovery surfaces do not offer incompatible artifacts.

**Exit criteria:** fixed roles work on Pi 5 or a documented ADR update narrows
the supported role set.

## Phase 7 - Documentation and readiness

1. Update operations, hardware support, and build documentation.
2. Commit validation records.
3. Complete a Milestone 7 readiness review.
4. Link child issues and evidence from the readiness review.

---

# Proposed Child Issues

| Issue | Title | Dependencies | Verification |
| --- | --- | --- | --- |
| TBD | Accept Milestone 7 ADRs | #153 | ADR review committed and statuses updated |
| TBD | Add Raspberry Pi 5 Buildroot board target | ADR-0034 | Build produces Pi image |
| TBD | Implement Pi boot partition generation | ADR-0034 | Image contains accepted boot assets |
| TBD | Add ARM64 release metadata fields | ADR-0035 | Index validates platform/arch fields |
| TBD | Build FoldOps `aarch64` layout bundles | ADR-0035 | Bundle smoke test on Pi 5 |
| TBD | Build `foldingosctl` `aarch64` tools bundles | ADR-0035 | `foldingosctl --version` on Pi 5 |
| TBD | Verify Folding@home ARM64 acquisition | ADR-0009, ADR-0035 | Client acquisition and service start |
| TBD | Validate Pi 5 SD boot | board target | validation record committed |
| TBD | Validate Pi 5 NVMe boot | board target, hardware | validation record committed or scoped out |
| TBD | Validate Pi 5 supervisor role | runtime bundles | dashboard and API smoke test |
| TBD | Validate Pi 5 agent role | runtime bundles | supervisor ingest shows Pi node |
| TBD | Complete Milestone 7 readiness review | all above | readiness review committed |

---

# Component Ownership

| Component | Owner path |
| --- | --- |
| Pi board support | `board/foldingos/raspberrypi5/` |
| Pi defconfig | `configs/foldingos_raspberrypi5_defconfig` |
| Build orchestration | `scripts/`, `BUILD_COMMANDS.md` |
| Release metadata | release and package publication scripts |
| FoldOps bundles | `packages/foldops/` |
| `foldingosctl` tools bundles | `packages/foldingosctl/` |
| Platform docs | `doc/`, `ROADMAP.md` |
| Validation records | `validation/` |

---

# Acceptance Criteria

- Milestone 7 ADRs are accepted before implementation relies on them
- Pi 5 board target builds from the documented Buildroot workflow
- Pi 5 raw image artifact is named and published with platform and architecture
  metadata
- image boots on Raspberry Pi 5 from SD card
- persistent data expands safely and idempotently
- Ethernet DHCP works on first boot
- local commissioning output works on attached HDMI display
- SSH access follows the existing administrator provisioning model or an
  accepted Pi-specific amendment
- `agent` and `supervisor` roles work, or an accepted ADR narrows the supported
  role set
- FoldOps and `foldingosctl` runtime acquisition select `aarch64` artifacts
- incompatible architecture updates are rejected
- Folding@home ARM64 acquisition is verified before Pi Folding support is
  claimed
- NVMe boot is validated or explicitly deferred with rationale
- Pi network provisioning scope is explicitly documented
- validation records and readiness review are committed

---

# Unknowns To Resolve

- final Pi boot chain: native firmware handoff, U-Boot, optional UEFI, or other
- whether GRUB participates in any Pi boot mode
- exact Pi boot partition contents and update ownership
- minimum supported boot media for the first Pi release
- NVMe HAT model and firmware baseline for validation
- whether supervisor role performance and storage behavior are acceptable on
  Pi 5
- Folding@home `aarch64` client/core support and upstream release cadence
- whether Pi network provisioning should use Raspberry Pi network boot, HTTP
  boot, direct flash only, or a later FoldOps workflow
- whether release reproducibility for Pi firmware assets requires additional
  pinning or mirroring policy

---

# Related Documents

- [Milestone 7 engineering specification](7-engineering-spec.md)
- [Raspberry Pi 5 platform design](../raspberry-pi-5-platform.md)
- [ROADMAP.md](../../ROADMAP.md)
- [hardware-support.md](../hardware-support.md)
- [build-system.md](../build-system.md)
