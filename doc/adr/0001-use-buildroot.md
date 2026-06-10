# ADR-0001: Use Buildroot as the FoldingOS Build System

**Status:** Accepted

**Version:** 1.0

**Date:** 2026-06-10

**Authors:** FoldingOS Project Contributors

---

# Context

FoldingOS is a purpose-built operating system designed exclusively for
running Folding@home compute nodes.

The project prioritizes:

* Simplicity
* Reliability
* Reproducibility
* Security
* Maintainability
* Low operational overhead

A build system is required to produce complete operating system images for
supported hardware platforms.

The selected solution must support long-term maintainability while remaining
consistent with the overall philosophy of FoldingOS.

---

# Decision

The FoldingOS project adopts **Buildroot** as its primary build system.

Buildroot will be used to generate complete operating system images,
including:

* Linux kernel
* Root filesystem
* Required packages
* Bootloader integration
* Board-specific configuration
* Release artifacts

Buildroot configuration files, overlays, patches, and supporting scripts
shall be maintained within the FoldingOS source repository.

The build process shall be fully reproducible and version controlled.

---

# Rationale

Buildroot aligns closely with the design philosophy of FoldingOS.

Advantages include:

* Small and understandable architecture
* Mature and actively maintained project
* Excellent embedded Linux support
* Reproducible image generation
* Straightforward customization
* Low maintenance overhead
* Simple integration with CI pipelines
* Minimal unnecessary abstraction

Buildroot encourages image-oriented thinking rather than package-oriented
system administration, which is consistent with the FoldingOS appliance model.

---

# Alternatives Considered

## Linux From Scratch (LFS)

Advantages:

* Complete educational value
* Maximum customization
* Full implementation visibility

Disadvantages:

* Significant maintenance burden
* No integrated build framework
* Poor scalability for ongoing releases
* High contributor onboarding cost

Decision:

Rejected as the primary build system.

LFS remains valuable for learning and experimentation but is not appropriate
for long-term project maintenance.

---

## Yocto Project

Advantages:

* Extremely powerful
* Enterprise adoption
* Sophisticated build capabilities
* Excellent hardware support

Disadvantages:

* Considerably higher complexity
* Steeper learning curve
* Larger maintenance burden
* Greater contributor overhead

Decision:

Rejected due to complexity relative to project goals.

---

## Debian-Based Distribution

Advantages:

* Familiar ecosystem
* Large package repository
* Existing tooling

Disadvantages:

* Includes unnecessary general-purpose functionality
* Larger attack surface
* Package-management complexity
* Less aligned with appliance philosophy

Decision:

Rejected.

FoldingOS is intentionally not a general-purpose Linux distribution.

---

## Custom Build System

Advantages:

* Complete control

Disadvantages:

* High engineering cost
* Reinvents existing tooling
* Long-term maintenance burden
* Increased project risk

Decision:

Rejected.

No sufficient justification exists to replace mature existing tooling.

---

# Consequences

## Positive

* Consistent image generation
* Smaller operating system footprint
* Reproducible builds
* Easier contributor onboarding
* Simpler CI/CD integration
* Lower maintenance cost
* Strong alignment with appliance philosophy

## Negative

* Buildroot ecosystem limitations
* Less flexible than Yocto for highly complex scenarios
* Requires Buildroot-specific knowledge
* Future migration would require engineering effort

These tradeoffs are acceptable given current project objectives.

---

# Implementation Guidelines

The repository should maintain Buildroot-specific assets in clearly defined
locations.

Example structure:

build/

configs/

board/

overlay/

packages/

patches/

scripts/

tools/

Documentation should remain separate under:

doc/

Local developer-specific modifications should not become part of the
authoritative build process.

---

# Success Criteria

The selected build system should enable:

* Deterministic builds
* Version-controlled configuration
* Automated CI builds
* Reproducible release artifacts
* Multi-platform image generation
* Long-term maintainability

---

# Future Review

This decision should be revisited only if:

* Buildroot becomes unmaintained
* Project requirements fundamentally change
* Significant technical limitations prevent project objectives
* A demonstrably superior alternative provides measurable long-term benefit

Migration should not occur solely because another technology becomes popular.

---

# Related Documents

* [Project charter](../../PROJECT_CHARTER.md)
* [Engineering principles](../../PRINCIPLES.md)
* [Build system](../build-system.md)
* [Architecture](../architecture.md)
* [AI context](../ai-context.md)

---

# Closing Statement

The FoldingOS project values simplicity over sophistication.

Buildroot provides a mature, understandable, and reproducible foundation that
supports the project's long-term objective of delivering a reliable,
purpose-built operating system for Folding@home.

This decision reflects a preference for disciplined engineering and
maintainable architecture rather than unnecessary complexity.
