# Milestone 7 ADR Acceptance Review

**Version:** 0.1

**Status:** Draft

**Review date:** TBD

**Target milestone:** Milestone 7, Raspberry Pi Support

**Issue:** [#153](https://github.com/pacificnm/folding-os/issues/153)

---

# Purpose

This document is the acceptance gate for the Milestone 7 Raspberry Pi 5 ADRs.
Implementation that changes boot architecture, image format, release artifact
metadata, or runtime bundle architecture selection must not be treated as
approved until this review is completed and the relevant ADRs are marked
Accepted.

---

# Completion Status

```text
Milestone 7 ADR review:              NOT STARTED
Milestone 7 ADR acceptance:          NOT APPROVED
Milestone 7 implementation:          NOT AUTHORIZED
```

---

# Governing Documents Under Review

| Document | Role | Review status |
| --- | --- | --- |
| [ADR-0034](../adr/0034-raspberry-pi-5-boot-and-image-format.md) | Raspberry Pi 5 boot and image format | Pending |
| [ADR-0035](../adr/0035-arm64-release-artifacts-and-runtime-bundles.md) | ARM64 release artifacts and runtime bundles | Pending |
| [7-implementation-spec.md](7-implementation-spec.md) | Milestone 7 implementation scope | Pending |
| [7-engineering-spec.md](7-engineering-spec.md) | Milestone 7 engineering contract | Pending |
| [raspberry-pi-5-platform.md](../raspberry-pi-5-platform.md) | Pi 5 platform design summary | Pending |

---

# Required Conflict Checks

The review must confirm that Milestone 7 does not conflict with:

| Existing document | Required check |
| --- | --- |
| [ADR-0001](../adr/0001-use-buildroot.md) | Pi board support remains Buildroot-based and in-tree |
| [ADR-0003](../adr/0003-x86_64-bootloader-and-image-format.md) | Pi boot design does not silently rewrite x86_64 UEFI behavior |
| [ADR-0004](../adr/0004-partition-and-persistence-layout.md) | Pi storage preserves logical boot/root/data separation |
| [ADR-0008](../adr/0008-raw-image-size-and-data-expansion.md) | Pi data expansion remains safe, idempotent, and non-shrinking |
| [ADR-0009](../adr/0009-fah-acquisition-and-update-model.md) | Folding@home ARM64 client acquisition remains upstream-pinned and verified |
| [ADR-0014](../adr/0014-fixed-installation-roles.md) | Pi role support preserves fixed `agent` and `supervisor` semantics or records an accepted exception |
| [ADR-0016](../adr/0016-network-provisioning-via-supervisor.md) | Pi direct-flash or future provisioning does not break x86_64 supervisor provisioning |
| [ADR-0018](../adr/0018-foldops-package-acquisition-and-update-model.md) | FoldOps remains runtime-acquired |
| [ADR-0023](../adr/0023-runtime-foldops-and-foldingosctl-updates-without-os-reimage.md) | layout-bundle transport remains the runtime update model |
| [ADR-0029](../adr/0029-packages-channel-publication-via-rclone.md) | package publication remains compatible with rclone channel publication |

---

# Acceptance Questions

The review must answer these questions explicitly:

1. What exact Pi 5 boot chain is accepted?
2. Does the boot chain use native firmware handoff, U-Boot, UEFI, GRUB, or a
   combination?
3. What Pi boot assets are pinned, built, copied, or acquired?
4. What is the accepted Pi release image name and platform identifier?
5. What architecture identifiers appear in release and package indexes?
6. Can both `agent` and `supervisor` roles proceed for Pi 5?
7. Is initial Pi support direct-flash only?
8. What validation media are required before readiness: SD, NVMe, USB?
9. What is the current Folding@home ARM64 support status?
10. What implementation issues are authorized after acceptance?

---

# Verdict

Pending.

When complete, this section must state one of:

- **Accept:** ADR-0034 and ADR-0035 become Accepted and implementation may
  proceed.
- **Accept with amendments:** listed amendments must be applied before ADR
  statuses change.
- **Reject:** implementation remains blocked pending revised ADRs.

---

# Related Documents

- [Issue #153](https://github.com/pacificnm/folding-os/issues/153)
- [Milestone 7 implementation specification](7-implementation-spec.md)
- [Milestone 7 engineering specification](7-engineering-spec.md)
