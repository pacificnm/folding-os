# FoldingOS Documentation

This index groups the project's documentation by purpose.

## Start Here

- [Project charter](../PROJECT_CHARTER.md) - mission, scope, and project values
- [Engineering principles](../PRINCIPLES.md) - active decision-making principles
- [Vision](vision.md) - intended user experience and long-term direction
- [Architecture](architecture.md) - high-level system components and boundaries
- [Operations](operations.md) - build, deploy, administer, diagnose, and recover
- [Roadmap](../ROADMAP.md) - planned implementation milestones
- [AI context](ai-context.md) - condensed project context for AI assistants

## Architecture And Design

- [Boot process](boot-process.md) - logical boot sequence and failure behavior
- [Build system](build-system.md) - build goals, expected framework, and outputs
- [Hardware support](hardware-support.md) - target platforms and support policy
- [Physical validation](physical-validation.md) - Milestone 1 physical acceptance procedure and boot-media preparation
- [Networking](networking.md) - networking behavior and capabilities
- [Storage layout](storage-layout.md) - logical storage and persistence model
- [Security model](security.md) - runtime security architecture
- [FoldOps integration](foldops-integration.md) - fleet-management relationship
  with the separate [pacificnm/foldops](https://github.com/pacificnm/foldops)
  repository

## Implementation Specifications

- [v0.1.0 scope specification](milestone/1-implementation-spec.md) - approved scope
  combining roadmap Milestones 1 and 2
- [v0.1.0 engineering specification](milestone/1-engineering-spec.md) - concrete
  implementation blueprint for the first release
- [Milestone 1 readiness review](milestone/1-readiness-review.md) - foundation
  completion status and validation evidence
- [Installer](installer.md) - approved combined-image installation architecture
- [Installer engineering specification](milestone/3-engineering-spec.md) -
  approved concrete implementation contract for the combined-image installer
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
