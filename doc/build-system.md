# FoldingOS Build System

## Purpose

This document defines the design philosophy and architecture of the FoldingOS build system.

The build system is responsible for producing deterministic, reproducible, bootable operating system images for all supported hardware platforms.

It should require minimal manual intervention and be suitable for both local development and automated CI/CD pipelines.

---

# Goals

The FoldingOS build system shall:

- Produce reproducible builds
- Be fully automated
- Be version controlled
- Minimize external dependencies
- Support multiple target architectures
- Produce release-ready images
- Be understandable by contributors

---

# Design Principles

## Reproducibility

Every released image should be reproducible from source using the documented build process.

No undocumented manual steps should exist.

FoldingOS v0.1.0 pins Buildroot 2026.02.2 LTS. Release reproducibility requires
two independent clean builds with byte-identical required artifacts, as defined
by [ADR-0012](adr/0012-reproducible-build-environment-and-verification.md).

The v0.1.0 build-host baseline is Debian 13 on amd64. Exact kernel and installed
build-package versions are captured in release metadata.

---

## Automation

Building FoldingOS should require as few manual actions as possible.

A typical workflow should resemble:

```text
git clone

↓

configure

↓

build

↓

output image
```

Eventually:

```bash
./build.sh
```

should produce release images with no additional interaction.

---

## Version Control

All build configuration should be stored in Git.

Examples include:

- Buildroot configuration
- package definitions
- overlays
- patches
- scripts
- generated configuration templates

No important configuration should exist only on developer workstations.

---

# Build Framework

FoldingOS uses Buildroot as its primary build system.

Reasons include:

- simplicity
- reproducibility
- small footprint
- embedded focus
- active maintenance

The selection rationale is defined by
[ADR-0001](adr/0001-use-buildroot.md).

---

# Build Outputs

The build system should be capable of generating:

- x86_64 bootable images
- release artifacts
- checksum files
- version metadata

Planned capabilities include:

- Raspberry Pi images
- signed manifests

Signing capability must be available before the first stable release. Future
release formats may evolve as project requirements change.

---

# Build Configuration

Build configuration should remain declarative whenever possible.

Platform-specific configuration should be isolated from common configuration.

The repository should remain easy to understand and navigate.

---

# Repository Organization

Example:

build/
configs/
packages/
overlay/
scripts/
tools/

Documentation remains separate under:

doc/

---

# Continuous Integration

Future CI systems should automatically:

- validate builds
- perform static analysis
- execute automated tests
- verify release-candidate reproducibility using independent clean builds
- generate release artifacts

CI should become the authoritative verification mechanism for releases.

---

# Release Philosophy

Published stable release artifacts should be:

- deterministic
- versioned
- documented
- reproducible
- cryptographically verifiable

Users should have confidence that published images correspond directly to the published source code.

---

# Future Considerations

Potential future capabilities include:

- multi-platform builds
- automated release publication
- SBOM generation
- supply-chain verification
- reproducible build verification

These capabilities should be added only when they provide measurable value to project reliability and security.

---

# Summary

The FoldingOS build system is intended to be simple, deterministic, and transparent.

It should enable any contributor to reproduce official releases from source while minimizing complexity and long-term maintenance burden.
