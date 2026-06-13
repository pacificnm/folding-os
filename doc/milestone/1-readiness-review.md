# Milestone 1 Readiness Review

**Version:** 1.0

**Status:** Approved

**Review date:** 2026-06-12

**Target release:** v0.1.0 foundation scope

---

# Purpose

This document records the Milestone 1 Bootable Appliance Foundation readiness
review required by issue #12.

It reconciles implemented foundation behavior with approved ADRs and Milestone 1
specifications, records validation evidence, and states what remains before a
public v0.1.0 release.

---

# Completion Status

```text
Milestone 1 foundation implementation: COMPLETE
Milestone 1 foundation validation:      COMPLETE
Public v0.1.0 release eligibility:      BLOCKED
```

Milestone 1 foundation work is complete when issues #1 through #11 are closed
with matching implementation and validation evidence. Milestone 2 Folding@home
runtime work and the remaining v0.1.0 release gates are separate scope.

---

# Issue Closure Matrix

| Issue | Title | State | Primary evidence |
| --- | --- | --- | --- |
| #1 | Establish reproducible Buildroot build foundation | Closed | `scripts/build`, `scripts/fetch-sources`, `configs/foldingos_x86_64_defconfig` |
| #2 | Complete deterministic x86_64 UEFI image and GRUB boot | Closed | `board/foldingos/x86_64/`, `scripts/test-qemu` image-structure checks |
| #3 | Complete systemd appliance boot graph and failure behavior | Closed | `scripts/verify-systemd-graph`, `scripts/test-qemu` |
| #4 | Complete persistent storage layout and safe data expansion | Closed | `foldingosctl storage expand-data`, `scripts/test-qemu` larger-disk tests |
| #5 | Complete DHCP networking, DNS, and time synchronization | Closed | `overlay/etc/systemd/network/`, `scripts/test-qemu`, `scripts/run-physical-acceptance` |
| #6 | Complete persistent directories and bounded journald logging | Closed | `scripts/verify-persistent-logging`, `scripts/test-qemu` journal tests |
| #7 | Complete node identity and TOML configuration management | Closed | `packages/foldingosctl/`, `scripts/verify-config`, `scripts/test-qemu` config tests |
| #8 | Complete secure administrator account and EFI SSH-key provisioning | Closed | [ADR-0007](../adr/0007-first-boot-administrator-and-ssh-provisioning.md), `scripts/test-qemu` SSH tests |
| #9 | Complete automated QEMU/OVMF foundation acceptance suite | Closed | `scripts/test-qemu` |
| #10 | Prove reproducible foundation builds and verify release artifacts | Closed | `scripts/build-a`, `scripts/build-b`, `scripts/verify-reproducible` |
| #11 | Validate bootable appliance foundation on physical x86_64 UEFI hardware | Closed | [validation/appliance-physical-0.1.0.json](../../validation/appliance-physical-0.1.0.json), `scripts/make-bootable-usb`, `scripts/run-physical-acceptance` |

No Milestone 1 release-blocking issue remains open or deferred through an
approved document change.

---

# Documentation Reconciliation

The following operator and engineering documents now match implemented
foundation behavior:

| Topic | Document |
| --- | --- |
| Build workflow | [operations.md](../operations.md), [build-system.md](../build-system.md), [milestone/1-engineering-spec.md](1-engineering-spec.md) |
| Direct flash and USB preparation | [operations.md](../operations.md), [physical-validation.md](../physical-validation.md), [installer.md](../installer.md) |
| SSH provisioning | [operations.md](../operations.md), [ADR-0007](../adr/0007-first-boot-administrator-and-ssh-provisioning.md), [security.md](../security.md) |
| Diagnostics and recovery | [operations.md](../operations.md), [boot-process.md](../boot-process.md) |
| Automated and physical validation | [testing-strategy.md](../testing-strategy.md), [physical-validation.md](../physical-validation.md) |
| Validated hardware | [hardware-support.md](../hardware-support.md) |

Approved architectural requirements for the foundation appliance are documented
in accepted ADRs and the Milestone 1 implementation and engineering
specifications. No reviewed contradiction was found between those sources and
the implemented image, scripts, or validation records.

---

# Non-Goals Verification

The foundation image and documentation were reviewed against the explicit
non-goals in [1-implementation-spec.md](1-implementation-spec.md).

Confirmed absent from the Milestone 1 foundation implementation:

- desktop environment, browser, GUI installer
- general-purpose package manager
- Docker, Kubernetes
- OTA updates, A/B root, rollback, snapshot support
- FoldOps agent, supervisor, or placeholder services
- embedded Folding@home client binaries
- GPU Folding@home support
- TPM integration and Secure Boot enforcement
- static-only networking

Folding@home acquisition and runtime remain approved v0.1.0 scope, but they are
Milestone 2 implementation work and are not claimed as complete in this review.

---

# Validation Evidence

## QEMU/OVMF reference platform

```bash
./scripts/test-qemu
```

## Reproducible foundation artifacts

```bash
./scripts/build-a
./scripts/build-b
./scripts/verify-reproducible build/verification/build-a build/verification/build-b
```

## Physical foundation acceptance

Validated system:

```text
Dell OptiPlex Micro
```

Committed record:

```text
validation/appliance-physical-0.1.0.json
```

Verification:

```bash
./scripts/verify-physical-validation-record \
  validation/appliance-physical-0.1.0.json \
  build/output/images/foldingos-x86_64-0.1.0.img
```

Remote acceptance command:

```bash
./scripts/run-physical-acceptance <host> <ssh-private-key>
```

---

# Release Gate Status

| Gate | Milestone 1 foundation | Public v0.1.0 release |
| --- | --- | --- |
| QEMU/OVMF acceptance | Satisfied | Required |
| Foundation physical validation | Satisfied | Required |
| Reproducible required artifacts | Satisfied | Required |
| Approved Folding@home manifest | Not in Milestone 1 scope | Still blocked |
| Folding@home acquisition and runtime validation | Not in Milestone 1 scope | Still blocked |
| Documentation matches implementation | Satisfied by this review | Required |

Until the Folding@home manifest and runtime gates are satisfied, release
metadata must continue to record:

```json
{
  "approved_fah_manifest_sha256": null,
  "build_type": "development",
  "physical_validation_complete": false,
  "release_eligible": false
}
```

Foundation physical validation is complete, but public release metadata must
not claim `physical_validation_complete: true` until the approved release
metadata workflow is implemented for the full v0.1.0 gate set.

---

# Known Limitations

- Local display output is not guaranteed on physical hardware without
  framebuffer or GPU drivers.
- Only hardware listed in [hardware-support.md](../hardware-support.md) is
  validated for v0.1.0 foundation testing.
- USB-source installation to internal SATA or NVMe targets remains Milestone 3
  scope even though direct flash and USB boot testing are documented.
- Full-image reflashing remains the supported update path in v0.1.0.

---

# Milestone Boundary

Milestone 1 foundation work ends with a bootable, reproducible, headless
appliance that:

- boots through UEFI and GRUB
- mounts persistent storage
- acquires network, DNS, and time synchronization
- provisions SSH administrator access
- preserves identity, configuration, and bounded journal storage

Milestone 2 work may proceed only after this readiness review is committed.
Milestone 2 does not retroactively change the Milestone 1 foundation contract.

---

# Review Outcome

| Acceptance criterion | Result |
| --- | --- |
| Documentation accurately matches implemented foundation behavior | Pass |
| No approved architectural requirement is undocumented or contradicted | Pass |
| No out-of-scope services or capabilities are included | Pass |
| All Milestone 1 release-blocking issues are closed or explicitly deferred | Pass |
| Milestone 1 completion status is recorded | Pass |

**Milestone 1 Bootable Appliance Foundation readiness review: PASS**

---

# Related Documents

- [v0.1.0 scope specification](1-implementation-spec.md)
- [v0.1.0 engineering specification](1-engineering-spec.md)
- [Operations](../operations.md)
- [Documentation index](../README.md)
