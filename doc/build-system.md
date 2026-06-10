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

The initial implementation is expected to utilize Buildroot.

Reasons include:

- simplicity
- reproducibility
- small footprint
- embedded focus
- active maintenance

The formal selection rationale will be documented through an Architecture Decision Record.

---

# Build Outputs

The build system should be capable of generating:

- x86_64 bootable images
- Raspberry Pi images
- release artifacts
- checksum files
- signed manifests
- version metadata

Future release formats may evolve as project requirements change.

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

docs/

---

# Continuous Integration

Future CI systems should automatically:

- validate builds
- perform static analysis
- execute automated tests
- verify reproducibility where practical
- generate release artifacts

CI should become the authoritative verification mechanism for releases.

---

# Release Philosophy

Release artifacts should be:

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
- release signing
- automated release publication
- SBOM generation
- supply-chain verification
- reproducible build verification

These capabilities should be added only when they provide measurable value to project reliability and security.

---

# Summary

The FoldingOS build system is intended to be simple, deterministic, and transparent.

It should enable any contributor to reproduce official releases from source while minimizing complexity and long-term maintenance burden.