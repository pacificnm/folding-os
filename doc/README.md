# FoldingOS Documentation

This index groups the project's documentation by purpose.

## Start Here

- [Project charter](../PROJECT_CHARTER.md) - mission, scope, and project values
- [Engineering principles](../PRINCIPLES.md) - active decision-making principles
- [Vision](vision.md) - intended user experience and long-term direction
- [Architecture](architecture.md) - high-level system components and boundaries
- [Operations](operations.md) - build, deploy, administer, diagnose, and recover
- [foldingosctl command reference](foldingosctl.md) - on-appliance CLI for provisioning, config, storage, and Folding@home
- [Roadmap](../ROADMAP.md) - planned implementation milestones
- [AI context](ai-context.md) - condensed project context for AI assistants
- [Agent subsystem guide](agent-subsystems.md) - implementation-agent map from
  subsystems to governing docs, owner paths, and verification commands

## Architecture And Design

- [Boot process](boot-process.md) - logical boot sequence and failure behavior
- [Build system](build-system.md) - build goals, expected framework, and outputs
- [Hardware support](hardware-support.md) - target platforms and support policy
- [Physical validation](physical-validation.md) - Milestone 1 physical acceptance procedure and boot-media preparation
- [Deployment and provisioning](installer.md) - supervisor bootstrap and network fleet provisioning
- [ADR-0016: Network provisioning via supervisor](adr/0016-network-provisioning-via-supervisor.md)
- [ADR-0017: Official release publication and supervisor upstream polling](adr/0017-official-release-publication-and-supervisor-upstream-polling.md)
- [ADR-0018: FoldOps package acquisition and update model](adr/0018-foldops-package-acquisition-and-update-model.md)
- [ADR-0019: FoldOps supervisor provisioning and TLS](adr/0019-foldops-supervisor-provisioning-and-tls.md)
- [ADR-0015: Local commissioning display](adr/0015-local-commissioning-display.md) - boot messages and ready status on `tty1`
- [Networking](networking.md) - networking behavior and capabilities
- [Storage layout](storage-layout.md) - logical storage and persistence model
- [Security model](security.md) - runtime security architecture
- [FoldOps integration](foldops-integration.md) - fleet-management relationship;
  Rust source in `packages/foldops/` per
  [ADR-0022](adr/0022-foldops-rust-source-in-foldingos-monorepo.md);
  `foldingosctl` migrates to Rust per
  [ADR-0025](adr/0025-implement-foldingosctl-in-rust.md)

## Implementation Specifications

- [v0.1.0 scope specification](milestone/1-implementation-spec.md) - approved scope
  combining roadmap Milestones 1 and 2
- [v0.1.0 engineering specification](milestone/1-engineering-spec.md) - concrete
  implementation blueprint for the first release
- [Milestone 1 readiness review](milestone/1-readiness-review.md) - foundation
  completion status and validation evidence
- [Milestone 2 readiness review](milestone/2-readiness-review.md) - Folding@home
  runtime completion status and validation evidence
- [Milestone 3 readiness review](milestone/3-readiness-review.md) - network fleet
  provisioning completion status and validation evidence
- [Milestone 4 implementation specification](milestone/4-implementation-spec.md) -
  proposed scope for FoldOps integration through foldingosctl delegation
- [Milestone 4 engineering specification](milestone/4-engineering-spec.md) -
  proposed concrete contract for inspect commands, JSON output, and FoldOps wiring
- [Milestone 4 appliance artifact and monorepo plan](milestone/4-appliance-artifact-and-monorepo-plan.md) -
  proposed layout-bundle transport, monorepo source, and runtime updates without OS reimage
- [Milestone 5 implementation specification](milestone/5-implementation-spec.md) -
  proposed scope for fleet software updates and supervisor recovery
- [Milestone 5 engineering specification](milestone/5-engineering-spec.md) -
  proposed update discovery, apply APIs, publication indexes, and recovery export
- [Deployment and provisioning](installer.md) - supervisor bootstrap and network
  fleet provisioning
- [Milestone 3 engineering specification](milestone/3-engineering-spec.md) -
  approved concrete implementation contract for network fleet provisioning
- [Update system](update-system.md) - draft update requirements and trust model

## Engineering Process

- [Coding standards](coding-standards.md)
- [Testing strategy](testing-strategy.md)
- [Release strategy](release-strategy.md)
- [Architecture Decision Records](adr/README.md)
- [Contributing guide](../CONTRIBUTING.md)
- [Changelog](../CHANGELOG.md)

## Project Policies

- [Security policy](../SECURITY.md) - vulnerability reporting and project policy
- [Code of conduct](../CODE_OF_CONDUCT.md)
- [License](../LICENSE)

## Document Roles

The project charter defines scope and mission. The active engineering principles
guide decisions. Accepted ADRs govern technical choices. Implementation
specifications define concrete mechanisms and must conform to accepted ADRs.
Architecture and subsystem documents summarize intended system behavior, while
the roadmap defines implementation sequence. The AI context is a summary for
assistants and does not override these sources.

When documents disagree, stop and resolve the disagreement explicitly. Update
all affected documents, and record significant technical decisions in an ADR.
