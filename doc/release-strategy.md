# FoldingOS Release Strategy

Version: 0.1

Status: Living Document

---

# Purpose

This document defines the release philosophy and versioning strategy for
FoldingOS.

The primary objective is to produce releases that are predictable,
reproducible, stable, and suitable for long-term unattended operation.

Reliability always takes precedence over release frequency.

---

# Release Philosophy

FoldingOS is an appliance operating system.

Releases should emphasize:

- stability
- predictability
- reproducibility
- security
- maintainability

Feature velocity is not a primary objective.

---

# Engineering Principles

Every release should be:

- fully documented
- reproducible
- version controlled
- tested
- traceable

Every released image should correspond directly to a tagged source revision.

---

# Versioning

The project will initially follow Semantic Versioning.

Examples:

0.1.0

0.2.0

0.3.0

1.0.0

1.1.0

2.0.0

Major versions indicate significant architectural or compatibility changes.

Minor versions introduce compatible functionality.

Patch releases correct defects without changing intended behavior.

---

# Pre-1.0 Philosophy

Versions prior to 1.0 should be considered development releases.

Breaking architectural improvements are acceptable when justified.

Documentation should clearly identify unstable functionality.

---

# Release Types

## Development

Internal engineering work.

No stability guarantees.

---

## Preview

Feature complete enough for wider evaluation.

May contain known issues.

---

## Release Candidate

Expected production behavior.

Only critical defects should block release.

---

## Stable

Recommended for production deployment.

Fully documented.

Fully tested.

---

# Release Requirements

Before release:

- documentation updated

- ADRs updated

- automated testing passes

- manual validation completed

- release notes prepared

- version information updated

- artifacts verified

---

# Release Artifacts

Any release may include:

- bootable images

- checksums

- signatures

- release notes

- changelog

- source archive

- build metadata

Stable releases must include checksums and cryptographic signatures. Signing is
planned alongside the update system and is not required for early development
artifacts.

---

# Source Control

Every release should correspond to:

- Git tag

- source revision

- documented build process

No release should be produced from unknown source state.

---

# Reproducibility

Given:

- source code

- documented build process

- required dependencies

another engineer should be able to reproduce release artifacts.

Reproducibility remains a strategic objective.

---

# Security

Stable releases must support:

- cryptographic signatures

- integrity verification

- provenance validation

Release authenticity should be verifiable.

---

# Quality Philosophy

Release criteria prioritize:

1. Correctness

2. Stability

3. Reliability

4. Security

5. Maintainability

6. Performance

7. New features

---

# Long-Term Vision

The long-term objective is a fully automated release pipeline capable of:

- building

- testing

- validating

- signing

- publishing

official FoldingOS releases with minimal manual intervention.

---

# Summary

Every FoldingOS release should increase confidence in the project.

Users should trust that a released image has been built, tested,
documented, and verified according to consistent engineering standards.
