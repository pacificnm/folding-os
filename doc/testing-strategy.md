# FoldingOS Testing Strategy

Version: 0.1

Status: Living Document

---

# Purpose

This document defines the testing philosophy and validation strategy for
FoldingOS.

Testing is considered a fundamental engineering discipline rather than an
optional development activity.

The objective is to ensure that every release of FoldingOS is reliable,
predictable, reproducible, and suitable for unattended operation.

---

# Philosophy

Testing exists to build confidence.

Every change should increase confidence in the system rather than reduce it.

Whenever practical:

- automate the test
- document the test
- repeat the test
- include the test in CI

Manual testing should be minimized over time.

---

# Engineering Principles

Testing should be:

- deterministic
- repeatable
- automated
- understandable
- maintainable

Tests should not depend on developer-specific environments.

---

# Testing Pyramid

                 End-to-End Tests

              Integration Testing

                Component Testing

                   Unit Testing

Each layer provides increasing confidence while decreasing execution speed.

The majority of tests should exist at the lowest practical level.

---

# Unit Testing

Individual functions and components should be tested independently whenever
practical.

Examples:

- configuration parsing

- version parsing

- metrics calculations

- retry logic

- state transitions

Unit tests should execute quickly and deterministically.

---

# Component Testing

Component testing validates larger functional units.

Examples:

- FoldOps Agent

- update subsystem

- configuration manager

- health monitor

- logging subsystem

Dependencies should be mocked where appropriate.

---

# Integration Testing

Integration tests verify interaction between components.

Examples:

- startup sequence

- configuration loading

- service communication

- networking initialization

- update workflow

- registration workflow

Integration testing should verify expected system behavior rather than
implementation details.

---

# End-to-End Testing

End-to-end testing validates complete system behavior.

Typical scenarios:

Power On

↓

Boot

↓

Initialize

↓

Connect

↓

Start Folding

↓

Report Health

↓

Continue Operation

These tests should simulate real deployment scenarios whenever practical.

---

# Boot Validation

Every release should validate:

- successful boot

- service startup

- network initialization

- Folding startup

- health reporting

- graceful shutdown

Boot regressions should block release.

---

# Storage Expansion Validation

Every release using automatic data-partition expansion should validate:

- the raw image has the exact documented size

- booting on a device equal to the image size performs no resize

- booting on a larger device expands the final data partition and ext4
  filesystem to the maximum usable aligned capacity

- files written before expansion remain intact afterward

- repeated expansion attempts make no further changes

- an unexpected partition layout fails safely without shrinking, formatting,
  or recreating persistent data

Storage expansion regressions should block release.

---

# Folding@home Acquisition Validation

Folding@home workload acquisition testing should verify:

- release images contain no Folding@home client or FahCore binaries

- the embedded approved manifest pins an exact version, origin, size, and
  SHA-256 digest

- nodes download only from the approved official upstream origin

- hash, size, architecture, and manifest-schema mismatches fail closed

- unverified artifacts are never installed or executed

- verified versions are installed into versioned persistent application storage

- activation preserves configuration, work units, and checkpoints

- failed acquisition or activation preserves the last known-good client

- FoldOps unavailability does not stop an already installed client

---

# Update Validation

Future update testing should verify:

- successful update

- rollback capability

- configuration preservation

- checkpoint preservation

- recovery after interruption

Updates should never unnecessarily destroy scientific work.

---

# Recovery Testing

The operating system should be validated against common failure scenarios.

Examples:

- unexpected reboot

- power failure

- network outage

- DNS failure

- FoldOps unavailable

- Folding@home unavailable

- storage full

- corrupted configuration

Expected recovery behavior should be documented.

---

# Hardware Testing

Supported hardware should undergo validation for:

- boot

- networking

- storage

- stability

- Folding operation

- recovery behavior

Hardware compatibility should be documented separately.

---

# Security Testing

Security validation should include:

- exposed services

- authentication

- encrypted communication

- update verification

- configuration validation

- dependency review

Security testing should become part of normal release validation.

Administrator provisioning tests should verify:

- release images contain no administrator credentials
- SSH is inaccessible before a valid administrator key is provisioned
- valid public keys are imported from the EFI provisioning path
- malformed provisioning files do not replace existing valid keys
- successful import applies restrictive ownership and permissions
- successful import replaces the complete authorized-key set
- provisioning files are removed after successful import
- direct root SSH login remains disabled
- SSH password authentication remains disabled
- key recovery works without FoldOps

---

# Performance Testing

Performance testing should focus on:

- boot time

- memory usage

- CPU overhead

- storage usage

- network overhead

Optimization should only occur after measurement.

Correctness remains the highest priority.

---

# Continuous Integration

Future CI pipelines should automatically:

- build FoldingOS

- execute automated tests

- perform static analysis

- validate release artifacts

- publish reports

A failed validation should prevent release publication.

---

# Regression Testing

Every resolved defect should produce a corresponding regression test whenever
practical.

Defects should not be fixed repeatedly.

The test suite should grow stronger over time.

---

# Logging and Diagnostics

Tests should produce meaningful diagnostics.

Failures should clearly identify:

- expected behavior

- observed behavior

- relevant logs

- reproduction information

Debugging should not require guesswork.

---

# Manual Testing

Manual testing remains valuable for:

- exploratory testing

- new hardware

- usability validation

- release verification

Manual testing should supplement automation rather than replace it.

---

# Release Criteria

A production release should not occur unless:

- all required tests pass

- no known critical regressions exist

- release artifacts are validated

- documentation is current

- release notes are complete

Quality takes precedence over schedule.

---

# Long-Term Vision

The long-term objective is a fully automated validation pipeline capable of
building, testing, and verifying FoldingOS releases with minimal human
intervention.

Contributors should have confidence that passing tests represent a reliable,
deployable operating system.

---

# Summary

Testing is not an afterthought.

Testing is part of the architecture.

Every feature should be designed with verification in mind.

Every release should increase confidence in the reliability and stability of
FoldingOS.
