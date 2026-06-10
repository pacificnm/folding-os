# FoldingOS Documentation

This index groups the project's documentation by purpose.

## Start Here

- [Project charter](../PROJECT_CHARTER.md) - mission, scope, and project values
- [Engineering principles](../PRINCIPLES.md) - active decision-making principles
- [Vision](vision.md) - intended user experience and long-term direction
- [Architecture](architecture.md) - high-level system components and boundaries
- [Roadmap](../ROADMAP.md) - planned implementation milestones
- [AI context](ai-context.md) - condensed project context for AI assistants

## Architecture And Design

- [Boot process](boot-process.md) - logical boot sequence and failure behavior
- [Build system](build-system.md) - build goals, expected framework, and outputs
- [Hardware support](hardware-support.md) - target platforms and support policy
- [Networking](networking.md) - networking behavior and capabilities
- [Storage layout](storage-layout.md) - logical storage and persistence model
- [Security model](security.md) - runtime security architecture
- [FoldOps integration](foldops-integration.md) - fleet-management relationship

## Implementation Specifications

- [v0.1.0 scope specification](milestone/1-implementation-spec.md) - draft scope
  combining roadmap Milestones 1 and 2
- [Installer](installer.md) - draft installation and first-boot scope
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
