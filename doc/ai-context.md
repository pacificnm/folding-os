# FoldingOS AI Context

> This document is a concise navigation and operating guide for AI assistants.
> It summarizes, but does not override, the project's governing documents,
> accepted ADRs, architecture, or implementation specifications.

Version: 0.2

Status: Living Document

---

# Project Summary

FoldingOS is a purpose-built appliance operating system for Folding@home compute
nodes. It prioritizes reliable scientific contribution, simple operation,
security, maintainability, and reproducible engineering.

It is not a general-purpose Linux distribution.

---

# Read Before Working

Use these sources according to their roles:

1. [Project charter](../PROJECT_CHARTER.md) defines mission and scope.
2. [Engineering principles](../PRINCIPLES.md) guide decisions.
3. Accepted [ADRs](adr/README.md) govern technical choices.
4. [Architecture](architecture.md) and subsystem documents describe intended
   behavior.
5. Implementation specifications define concrete mechanisms.
6. [Roadmap](../ROADMAP.md) defines implementation sequence.
7. [Coding standards](coding-standards.md) and
   [testing strategy](testing-strategy.md) guide implementation work.

When documents disagree, stop and resolve the disagreement. Do not invent
precedence or silently choose one statement.

---

# Engineering Rules

- Keep FoldingOS focused on Folding@home operation.
- Prefer the simplest correct and maintainable solution.
- Minimize dependencies, runtime services, privileges, and attack surface.
- Preserve deterministic, explicit, and documented behavior.
- Design for unattended operation and expected failures.
- Keep node operation independent of FoldOps availability.
- Preserve configuration and Folding@home work across recovery and updates.
- Do not introduce undocumented behavior.
- Update documentation with implementation changes.
- Record significant technical decisions in ADRs.

---

# Current Direction

- Build framework: Buildroot, as defined by ADR-0001.
- First implementation platform: x86_64 UEFI.
- Next planned platform: Raspberry Pi 5 using ARM64.
- Initial networking may be IPv4-only; IPv6 is a planned first-class
  capability.
- Initial remote administration uses OpenSSH with the `foldingos-admin` account
  and public keys provisioned through the EFI System Partition.
- FoldingOS images do not contain Folding@home client or FahCore binaries.
  Nodes download a pinned, verified client directly from official Folding@home
  infrastructure; FoldOps may coordinate approved manifests but does not proxy
  the binaries.
- Production updates must be authenticated, integrity-verified, and
  recoverable once update capability is enabled.
- Stable release artifacts must be reproducible and cryptographically
  verifiable.

Treat undecided implementation details as undecided. Do not convert expectations
or future plans into accepted decisions.

---

# Working Expectations

Before changing the system:

1. Read the relevant architecture and subsystem documents.
2. Check for an accepted ADR governing the decision.
3. Confirm whether the roadmap schedules the capability.
4. Keep changes focused and testable.
5. Update affected documentation and specifications.

The objective is reliable scientific contribution, not feature accumulation.
