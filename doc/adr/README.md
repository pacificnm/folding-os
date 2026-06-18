# Architecture Decision Records (ADR)

Version: 1.0

Status: Active

---

# Purpose

Architecture Decision Records (ADRs) document significant technical and
architectural decisions made throughout the FoldingOS project.

Their purpose is to preserve engineering knowledge and explain not only
*what* decisions were made, but *why* they were made.

An ADR captures the reasoning behind a decision at a specific point in time.

---

# Philosophy

Software evolves.

Requirements evolve.

Understanding evolves.

Without documentation, architectural knowledge becomes tribal knowledge and
eventually disappears.

ADRs provide a permanent engineering history for the project.

Future contributors should be able to understand the reasoning behind major
design decisions years after they were originally made.

---

# Accepted Decisions

- [ADR-0001: Use Buildroot as the FoldingOS Build System](0001-use-buildroot.md)
- [ADR-0002: Use systemd for Init and Service Supervision](0002-init-and-service-supervision.md)
- [ADR-0003: x86_64 Bootloader and Image Format](0003-x86_64-bootloader-and-image-format.md)
- [ADR-0004: Partition and Persistence Layout](0004-partition-and-persistence-layout.md)
- [ADR-0005: Configuration Ownership and Precedence](0005-configuration-ownership-and-precedence.md)
- [ADR-0006: Folding@home Packaging and Privilege Model](0006-fah-packaging-and-privilege-model.md)
- [ADR-0007: First-Boot Administrator and SSH-Key Provisioning](0007-first-boot-administrator-and-ssh-provisioning.md)
- [ADR-0008: Raw Image Size and Data-Partition Expansion](0008-raw-image-size-and-data-expansion.md)
- [ADR-0009: Folding@home Acquisition and Update Model](0009-fah-acquisition-and-update-model.md)
- [ADR-0010: Persistent Logging And Retention](0010-persistent-logging-and-retention.md)
- [ADR-0011: TOML Configuration Validation And Migration](0011-toml-configuration-validation-and-migration.md)
- [ADR-0012: Reproducible Build Environment And Verification](0012-reproducible-build-environment-and-verification.md)
- [ADR-0013: Combined Appliance And Installer Image](0013-combined-appliance-and-installer-image.md) (superseded by ADR-0016)
- [ADR-0014: Fixed Installation Roles](0014-fixed-installation-roles.md)
- [ADR-0015: Local Commissioning Display](0015-local-commissioning-display.md)
- [ADR-0016: Network Provisioning Via Supervisor](0016-network-provisioning-via-supervisor.md)
- [ADR-0017: Official Release Publication And Supervisor Upstream Polling](0017-official-release-publication-and-supervisor-upstream-polling.md)
- [ADR-0018: FoldOps Package Acquisition And Update Model](0018-foldops-package-acquisition-and-update-model.md)
- [ADR-0019: FoldOps Supervisor Provisioning And TLS](0019-foldops-supervisor-provisioning-and-tls.md)
- [ADR-0020: FoldOps Delegates Node Operations To foldingosctl](0020-foldops-delegates-node-operations-to-foldingosctl.md) (proposed)
- [ADR-0021: Machine-Readable foldingosctl Automation Interface](0021-machine-readable-foldingosctl-automation-interface.md) (proposed)
- [ADR-0022: FoldOps Rust Source In FoldingOS Monorepo](0022-foldops-rust-source-in-foldingos-monorepo.md) (proposed)
- [ADR-0023: Runtime FoldOps And foldingosctl Updates Without OS Reimage](0023-runtime-foldops-and-foldingosctl-updates-without-os-reimage.md) (proposed)
- [ADR-0024: FoldOps Supervisor Fleet Mutation Authorization](0024-foldops-supervisor-fleet-mutation-authorization.md) (proposed)
- [ADR-0025: Implement foldingosctl In Rust](0025-implement-foldingosctl-in-rust.md) (proposed)
- [ADR-0026: FoldOps Dashboard Operator Authentication](0026-foldops-dashboard-operator-authentication.md) (proposed)
- [ADR-0027: FoldOps Remote Operator API](0027-foldops-remote-operator-api.md) (proposed)

---

# When To Create An ADR

An ADR should be created whenever a decision significantly affects:

- architecture

- security

- build system

- storage

- networking

- update strategy

- deployment

- APIs

- major dependencies

- long-term maintainability

Minor implementation details generally do not require ADRs.

---

# ADR Lifecycle

An ADR may have one of the following statuses:

## Proposed

The decision is under discussion.

No implementation commitment has been made.

---

## Accepted

The decision has been approved and represents the current project direction.

Implementation may already exist or be planned.

---

## Superseded

A newer ADR replaces this decision.

Historical information remains valuable and should not be deleted.

---

## Deprecated

The decision is no longer recommended but may remain in historical use.

---

## Rejected

The proposal was evaluated but intentionally not adopted.

Rejected ADRs help prevent repeated discussion of previously evaluated ideas.

---

# Naming Convention

Files should follow the format:

0001-short-title.md

Examples:

0001-buildroot.md

0002-init-system.md

0003-headless-operation.md

0004-update-strategy.md

Numbers should never be reused.

Deleted ADR numbers should remain retired.

---

# ADR Format

Every ADR should contain:

```text
# ADR-0001

Title

Status

Date

Authors

---

## Context

Describe the problem.

---

## Decision

Describe the selected approach.

---

## Alternatives Considered

Describe competing solutions.

---

## Consequences

Positive outcomes.

Negative outcomes.

Tradeoffs.

---

## Future Considerations

Potential future evolution.

---

## References

Optional supporting material.
```

Consistency across ADRs is strongly encouraged.

---

# Engineering Expectations

An ADR should explain:

- why the decision exists

- alternatives considered

- expected benefits

- expected drawbacks

- implementation implications

The objective is understanding rather than justification.

---

# Modifying ADRs

Accepted ADRs should generally not be rewritten.

If project direction changes:

Create a new ADR.

Mark the previous ADR as:

Superseded

or

Deprecated

Historical engineering decisions should remain visible.

---

# Relationship To Documentation

High-level documentation describes the intended architecture.

ADRs document the reasoning behind major engineering decisions.

Source code implements those decisions.

Together they provide a complete understanding of the project.

---

# Relationship To Implementation

Implementation should follow accepted ADRs.

If implementation and ADRs disagree:

- update implementation

or

- create a new ADR

Undocumented architectural drift should be avoided.

---

# Decision Quality

Good ADRs are:

- concise

- understandable

- evidence-based

- technically honest

- transparent about tradeoffs

No technology is perfect.

Good engineering acknowledges tradeoffs explicitly.

---

# Example ADR Topics

Examples include:

- Buildroot selection

- Init system selection

- Boot loader selection

- Immutable filesystem

- Update architecture

- A/B partitioning

- FoldOps communication

- Security architecture

- Logging strategy

- Identity management

- Package management philosophy

- Dependency policy

---

# Closing Principle

The purpose of an ADR is not to prove that a decision was perfect.

The purpose is to ensure that future contributors understand why the decision
was made.

Well-documented engineering decisions are an investment in the long-term
maintainability of FoldingOS.
