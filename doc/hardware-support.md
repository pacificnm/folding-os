# FoldingOS Hardware Support

Version: 0.1

Status: Living Document

---

# Purpose

This document defines the hardware support philosophy and target platforms
for FoldingOS.

The objective is not maximum compatibility.

The objective is maximum reliability.

---

# Philosophy

Supporting fewer platforms extremely well is preferred over supporting many
platforms poorly.

Every supported platform increases:

- engineering effort

- testing effort

- maintenance burden

- documentation requirements

Hardware support should grow deliberately.

---

# Planned Target Platforms

Platform sequence:

- first implementation target: x86_64 UEFI systems

- next planned target: Raspberry Pi 5

The roadmap defines when support work begins and when a platform becomes an
officially supported release target.

---

# x86_64

Primary use cases:

- desktop hardware

- mini PCs

- enterprise surplus hardware

- small form factor systems

- homelab deployments

Preference should be given to widely supported hardware.

For v0.1.0, the mandatory x86_64 reference platform is:

- QEMU virtual machine
- OVMF UEFI firmware
- virtual Ethernet supported by the release image
- virtual disk larger than or equal to the release image

The QEMU/OVMF reference platform must pass the complete automated release test
suite.

A physical x86_64 UEFI system becomes validated for a release only after it
passes the documented hardware acceptance test and is listed in that release's
compatibility documentation.

Unlisted x86_64 UEFI systems may work but are not considered supported or
validated.

---

# Raspberry Pi

Primary goals:

- simple deployment

- educational use

- low-power operation

- home Folding nodes

The first planned Raspberry Pi target is:

- Raspberry Pi 5

Milestone 7 planning for Raspberry Pi 5 is proposed in
[raspberry-pi-5-platform.md](raspberry-pi-5-platform.md),
[milestone/7-implementation-spec.md](milestone/7-implementation-spec.md), and
[milestone/7-engineering-spec.md](milestone/7-engineering-spec.md). The proposed
minimum validation target is Raspberry Pi 5 booting from SD card with wired
Ethernet DHCP and persistent data expansion. NVMe boot through an approved Pi 5
M.2 HAT is a validation target, but support must be recorded as validated,
deferred, or unsupported in the Milestone 7 readiness review.

Future support may expand based on project priorities.

---

# General Principles

Supported hardware should:

- boot reliably

- network reliably

- operate continuously

- recover cleanly

- support Folding workloads

Reliability takes precedence over hardware diversity.

---

# Storage

Expected storage technologies include:

- SATA SSD

- NVMe SSD

- USB storage

- SD card (where appropriate)

Future recommendations should prioritize reliability over cost.

## Boot Media Preparation

Release images are 4 GiB raw GPT disk images. USB sticks and other removable
media are often much larger. Writing the image with `dd` alone leaves the backup
GPT header at the image end and can make UEFI firmware refuse to boot the
device.

Administrators must prepare boot media with
[scripts/make-bootable-usb](../scripts/make-bootable-usb). The script writes the
release image, relocates the backup GPT header when required, verifies the EFI
bootloader layout, and can stage administrator SSH public keys before first
boot.

During commissioning, a temporarily attached monitor shows boot progress and a
final ready message with the DHCP IPv4 address. See
[ADR-0015](adr/0015-local-commissioning-display.md).

Physical validation, installer USB workflows, and direct-flash recovery all use
this preparation step when the target media is larger than the release image.

---

# Networking

Preferred:

- wired Ethernet

Wireless support may be provided where appropriate but should not become a
primary architectural dependency.

Reliable networking is essential for Folding@home operation.

---

# Memory

Minimum requirements will evolve over time.

Design should strive to minimize:

- memory usage

- storage usage

- unnecessary runtime overhead

Resource efficiency remains an important objective.

---

# CPU Architecture

Planned architecture sequence:

- first implementation architecture: x86_64

- next planned architecture: ARM64

ARM64 release artifacts and runtime bundles are proposed in
[ADR-0035](adr/0035-arm64-release-artifacts-and-runtime-bundles.md). Platform
support must distinguish generic architecture artifacts from Raspberry Pi
5-specific boot images.

Future architecture support should require documented engineering
justification.

---

# Unsupported Philosophy

The project should avoid attempting universal compatibility.

Unsupported hardware is acceptable.

Engineering resources should remain focused on supported platforms.

---

# Validation

Supported hardware must pass the validation requirements defined in the
[testing strategy](testing-strategy.md).

Hardware status uses these categories:

- **Reference:** required platform for automated release validation
- **Validated:** physical hardware that passed the documented acceptance test
  for a specific release
- **Unvalidated:** hardware that may work but carries no support claim

---

# Compatibility Matrix

Future releases should publish a compatibility matrix documenting:

- officially supported hardware

- validated hardware

- community-tested hardware

- known limitations

Transparency is preferred over unsupported claims.

## v0.1.0 Validated Physical Systems

A system is listed here only after it completes
[physical-validation.md](physical-validation.md) and a passing record is
committed under `validation/`.

| Manufacturer | Model | Firmware | Storage transport | Validation record | Notes |
| --- | --- | --- | --- | --- | --- |
| Dell | OptiPlex Micro | Dell UEFI | USB | [appliance-physical-0.1.0.json](../validation/appliance-physical-0.1.0.json) | Milestone 1 foundation; validated over wired Ethernet; local commissioning display approved by ADR-0015 |
| Dell | OptiPlex Micro | Dell UEFI | NVMe (supervisor direct flash), NVMe and SATA (network agents) | [network-provision-physical-0.1.0.json](../validation/network-provision-physical-0.1.0.json) | Milestone 3 network fleet provisioning; dual-disk SATA agent used `allow-boot --disk /dev/sda` |

---

# Long-Term Vision

The long-term objective is to provide excellent support for a carefully
selected set of hardware platforms rather than broad but inconsistent
compatibility.

Users should have confidence that officially supported systems will operate
reliably for extended unattended deployments.

---

# Summary

Hardware support should reflect the overall philosophy of FoldingOS:

- simplicity

- predictability

- reliability

- maintainability

Engineering quality is more valuable than hardware quantity.
