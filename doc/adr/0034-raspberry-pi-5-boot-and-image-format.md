# ADR-0034: Raspberry Pi 5 Boot And Image Format

**Status:** Proposed

**Date:** 2026-06-22

**Authors:** FoldingOS project

**Depends on:** [ADR-0001](0001-use-buildroot.md),
[ADR-0004](0004-partition-and-persistence-layout.md),
[ADR-0008](0008-raw-image-size-and-data-expansion.md),
[ADR-0014](0014-fixed-installation-roles.md)

**Related:** [Milestone 7 implementation specification](../milestone/7-implementation-spec.md),
[Milestone 7 engineering specification](../milestone/7-engineering-spec.md),
[Raspberry Pi 5 platform design](../raspberry-pi-5-platform.md)

---

## Context

FoldingOS currently ships an x86_64 UEFI image governed by
[ADR-0003](0003-x86_64-bootloader-and-image-format.md). Milestone 7 adds
Raspberry Pi 5 as the first ARM64 target. Extending ADR-0003 silently would
hide a real platform difference: Raspberry Pi 5 uses Raspberry Pi firmware and
EEPROM boot behavior rather than PC UEFI as the native boot path.

The Pi 5 image must preserve the appliance model:

- one flashable operating-system image per supported platform
- fixed `agent` and `supervisor` installation roles
- persistent `/data` layout and expansion behavior consistent with x86_64
- no Folding@home, FoldOps, or `foldingosctl` release binaries embedded when
  existing ADRs require runtime acquisition
- reproducible Buildroot-based image generation

Milestone 7 must also resolve whether the Pi boot path uses native Raspberry Pi
firmware handoff directly, U-Boot, optional UEFI firmware, or another accepted
chain. This ADR records the required decision boundary before implementation.

---

## Decision

FoldingOS will add Raspberry Pi 5 as a **platform-specific ARM64 appliance
image** rather than treating the x86_64 UEFI image as portable.

### 1. Platform-specific board support

Milestone 7 will introduce a Raspberry Pi 5 board target under the existing
Buildroot repository architecture.

Expected implementation paths:

- `board/foldingos/raspberrypi5/`
- `configs/foldingos_raspberrypi5_defconfig`

The board target must remain in-tree. Milestone 7 does not authorize a
Buildroot external tree.

### 2. Boot chain decision

The Pi 5 boot chain is a Milestone 7 architecture decision and must be
implemented only after this ADR is accepted with a concrete selected chain.

The accepted chain must specify:

- firmware entry point
- boot partition contents
- kernel and device-tree loading mechanism
- whether U-Boot or UEFI firmware is used
- whether GRUB participates
- how boot diagnostics are captured for validation
- how the chain preserves future update and recovery compatibility

The initial proposal is to prefer the simplest Pi-native chain that Buildroot
can produce reproducibly and that supports the existing FoldingOS partition and
service model. If U-Boot or UEFI is selected, the acceptance review must explain
why the added layer is required.

### 3. Image format

The primary Raspberry Pi 5 release artifact will be a raw flashable disk image.

Example:

```text
foldingos-raspberrypi5-aarch64-0.1.0.img
```

The image must be suitable for direct flashing to supported Pi boot media. SD
card boot is the minimum required first validation target. NVMe boot through an
approved Raspberry Pi 5 M.2 HAT is a validation target for Milestone 7, but it
may be marked conditional until hardware evidence exists.

### 4. Storage model

The logical storage model remains:

```text
Boot partition
Root filesystem
Persistent data
```

The persistent data partition remains the final partition and must expand
idempotently to the target boot device when safe, following the safety principles
in ADR-0008.

The Pi boot partition may contain Pi-specific firmware, boot configuration,
kernel, initramfs, device tree, and overlays. This does not change the logical
separation between boot, root filesystem, and persistent data.

### 5. Role support

The Raspberry Pi 5 image must preserve ADR-0014 fixed roles. Milestone 7 starts
with both `agent` and `supervisor` as design targets from the same Pi image.

If validation proves supervisor operation unsuitable on Pi 5 for the first Pi
release, implementation must stop and update this ADR or add a superseding ADR
before shipping an agent-only Pi image.

### 6. Network provisioning

Milestone 7 does not assume x86_64 iPXE parity for Raspberry Pi 5.

Initial Pi 5 support may be direct-flash only. A Pi-specific network
provisioning path may be designed later, but it must not weaken ADR-0016 for
x86_64 UEFI provisioning or create an undocumented alternate provisioning model.

---

## Alternatives Considered

### Reuse ADR-0003 unchanged

Rejected. ADR-0003 is intentionally x86_64 UEFI-specific. Pi 5 boot behavior,
firmware assets, and validation requirements differ enough to require a new
decision record.

### Require UEFI and GRUB on Raspberry Pi 5

Proposed only if implementation evidence shows that UEFI/GRUB materially
improves reproducibility, update compatibility, or operational support. It adds
firmware and bootloader complexity that must be justified before acceptance.

### Make Raspberry Pi 5 agent-only immediately

Deferred. Agent-only support may become necessary, but ADR-0014 currently
defines fixed roles from the same release image. Milestone 7 should attempt both
roles unless hardware validation proves that unsafe or unreliable.

### Implement full Pi network boot parity first

Rejected for the initial plan. Direct-flash Pi support is the smallest complete
platform port. Network provisioning can follow after the flashable image is
validated.

---

## Consequences

### Positive

- keeps x86_64 and Pi boot decisions explicit
- preserves the appliance image model
- avoids accidental architecture drift around provisioning
- allows Pi-specific boot assets while keeping the logical storage model stable

### Negative

- creates a second board target and validation matrix
- requires hardware-specific boot testing
- delays Pi network provisioning until a separate accepted design exists
- may require new release automation for platform-specific artifacts

---

## Future Considerations

Future ADRs may define:

- Pi network provisioning or HTTP boot
- U-Boot or UEFI adoption if not selected initially
- A/B root filesystem behavior across both x86_64 and ARM64
- support for Raspberry Pi models other than Pi 5
- broader ARM SBC support

---

## References

- [Issue #153: Plan Milestone 7](https://github.com/pacificnm/folding-os/issues/153)
- [Raspberry Pi documentation: boot modes and EEPROM boot flow](https://www.raspberrypi.com/documentation/computers/raspberry-pi.html)
- [ADR-0003: x86_64 Bootloader and Image Format](0003-x86_64-bootloader-and-image-format.md)
- [ADR-0016: Network Provisioning Via Supervisor](0016-network-provisioning-via-supervisor.md)
