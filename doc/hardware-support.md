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

---

# Raspberry Pi

Primary goals:

- simple deployment

- educational use

- low-power operation

- home Folding nodes

The first planned Raspberry Pi target is:

- Raspberry Pi 5

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

---

# Compatibility Matrix

Future releases should publish a compatibility matrix documenting:

- officially supported hardware

- validated hardware

- community-tested hardware

- known limitations

Transparency is preferred over unsupported claims.

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
