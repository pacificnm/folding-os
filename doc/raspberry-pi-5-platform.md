# Raspberry Pi 5 Platform Design

**Version:** 0.1

**Status:** Proposed

**Related milestone:** Milestone 7, Raspberry Pi Support

---

# Purpose

This document describes the intended Raspberry Pi 5 platform design for
FoldingOS. It summarizes the operator, build, boot, storage, runtime, and
validation expectations that implementation agents should use with the
Milestone 7 specifications.

This document does not override accepted ADRs. Where it describes proposed
behavior, implementation requires acceptance of the relevant Milestone 7 ADR.

---

# Platform Summary

| Concern | Decision or proposed direction |
| --- | --- |
| Hardware target | Raspberry Pi 5 |
| CPU architecture | `aarch64` |
| Build system | Buildroot, in-tree board target |
| Image model | raw flashable disk image |
| Initial boot media | SD card required; NVMe conditional validation |
| Network | wired Ethernet DHCP required |
| Roles | `agent` and `supervisor` from same image unless ADR narrows support |
| Persistence | same logical `/data` model as x86_64 |
| FoldOps | runtime-acquired `aarch64` layout bundles |
| Folding@home | runtime-acquired ARM64 client when verified upstream |

---

# Operator Experience

The Pi 5 deployment should remain appliance-like:

```text
Download image

↓

Verify artifact

↓

Flash SD or validated NVMe media

↓

Boot with wired Ethernet

↓

Read local commissioning output

↓

Complete role and administrator bootstrap

↓

Acquire runtime packages

↓

Fold
```

Operators should not assemble a custom kernel, firmware set, root filesystem,
or bootloader by hand.

---

# Boot Design

Raspberry Pi 5 boot support is platform-specific. The boot design must account
for Raspberry Pi firmware and EEPROM behavior rather than assuming PC UEFI.

The accepted boot chain must define:

- firmware entry point
- boot media order expected during validation
- files placed on the boot partition
- kernel image format
- device tree and overlay handling
- kernel command line ownership
- diagnostics collected when boot fails

The boot partition may contain Pi-specific assets. Runtime operator
configuration remains under `/data/config` unless an accepted ADR says
otherwise.

---

# Storage Design

The Pi image preserves the same logical storage model:

```text
Boot
Root filesystem
Persistent data
```

The persistent data partition remains the final partition and expands to the
boot media when safe. The implementation must support Pi storage device naming
without assuming a single device path.

Supported first-release media:

- SD card: required validation
- NVMe through approved M.2 HAT: conditional validation
- USB storage: optional validation

---

# Runtime Design

## FoldOps and tools

FoldOps and `foldingosctl` runtime update payloads are architecture-aware.
Pi nodes must acquire `aarch64` bundles and reject mismatched x86_64 bundles.

## Folding@home

Folding@home remains acquired after deployment from approved upstream sources.
Pi Folding support depends on verified ARM64 client and core availability.

If the Pi image boots but Folding@home ARM64 acquisition is not available, the
platform may be treated as a board-support validation artifact but not as a
complete FoldingOS compute-node release.

---

# Provisioning Design

Initial Milestone 7 support is direct flash unless a Pi-specific provisioning
ADR is accepted.

Direct flash is intentionally separate from the x86_64 supervisor-led iPXE path.
Pi network boot or HTTP boot may be added later after the base image is reliable.

---

# Validation Design

Pi validation must produce committed evidence under `validation/`.

The validation record should include:

- image version and commit
- Pi 5 board revision and RAM size
- power supply
- boot media
- firmware or EEPROM version when available
- Ethernet DHCP result
- data expansion result
- role selected
- FoldOps and tools architecture
- Folding@home acquisition result
- known limitations

Validation should distinguish:

- required pass
- conditional pass
- unsupported
- deferred

---

# Open Questions

- Which exact boot chain will be accepted?
- What firmware and bootloader assets must be pinned for reproducibility?
- What image size is appropriate for the first Pi release?
- Which NVMe HAT and SSD combination becomes the first validated NVMe target?
- Can Pi 5 reliably act as a supervisor for the expected lab fleet size?
- What is the verified Folding@home ARM64 support state?
- Should Pi network provisioning be direct-flash only for Milestone 7?

---

# Related Documents

- [ADR-0034](adr/0034-raspberry-pi-5-boot-and-image-format.md)
- [ADR-0035](adr/0035-arm64-release-artifacts-and-runtime-bundles.md)
- [Milestone 7 implementation specification](milestone/7-implementation-spec.md)
- [Milestone 7 engineering specification](milestone/7-engineering-spec.md)
- [Hardware support](hardware-support.md)
