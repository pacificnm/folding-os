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

- Implementation agents must read the relevant approved documentation before
  making implementation changes.
- Implementation agents must follow accepted ADRs and approved implementation
  specifications exactly.
- Do not introduce or substitute an alternate architecture pattern unless an
  accepted ADR or approved engineering specification explicitly authorizes it.
- Do not introduce Buildroot external-tree architecture unless an accepted ADR
  or approved engineering specification explicitly authorizes it.
- Do not substitute common practice, convention, or a perceived best practice
  for a documented project decision.
- When documentation and common practice conflict, approved project
  documentation wins.
- When approved documents conflict, or when a required architectural decision
  is undocumented, stop implementation and surface the issue for resolution.
- Architecture changes require an ADR or specification update before
  implementation.
- Keep FoldingOS focused on Folding@home operation.
- Prefer the simplest correct and maintainable solution.
- Minimize dependencies, runtime services, privileges, and attack surface.
- Preserve deterministic, explicit, and documented behavior.
- Design for unattended operation and expected failures.
- Keep node operation independent of FoldOps availability.
- Treat [pacificnm/foldops](https://github.com/pacificnm/foldops) as the
  authoritative FoldOps code repository and coordinate cross-repository
  contract changes explicitly.
- Preserve configuration and Folding@home work across recovery and updates.
- Do not introduce undocumented behavior.
- Update documentation with implementation changes.
- Record significant technical decisions in ADRs.

---

# Current Direction

- Build framework: Buildroot, as defined by ADR-0001.
- v0.1.0 pins Buildroot 2026.02.2 LTS and requires two independent clean builds
  with byte-identical required release artifacts.
- The v0.1.0 build-host baseline is Debian 13 on amd64.
- First implementation platform: x86_64 UEFI.
- The required v0.1.0 reference platform is QEMU with OVMF; physical x86_64
  systems are validated per release rather than universally supported.
- Milestone 1 foundation implementation and validation are complete; see
  [milestone/1-readiness-review.md](milestone/1-readiness-review.md).
- Operator build, deployment, and recovery procedures are in
  [operations.md](operations.md).
- Next planned platform: Raspberry Pi 5 using ARM64.
- Initial networking may be IPv4-only; IPv6 is a planned first-class
  capability.
- v0.1.0 networking uses Ethernet DHCP; static networking is out of scope.
- Initial remote administration uses OpenSSH with the `foldingos-admin` account
  and public keys provisioned through the EFI System Partition.
- A future installer-capable release uses one combined appliance and installer
  image with explicit GRUB boot modes; it does not introduce a separate
  installer operating system.
- FoldingOS supports fixed `agent` and `supervisor` installation roles from
  the same combined image. The supervisor includes the FoldOps agent,
  supervisor, and web services; its web interface requires initial
  administrator and TLS provisioning. Roles cannot be changed in place.
- FoldOps package artifacts are pinned and verified at Buildroot build time.
  FoldingOS does not use APT at runtime or install FoldOps packages from the
  network during installation.
- FoldingOS images do not contain Folding@home client or FahCore binaries.
  Nodes download a pinned, verified client directly from official Folding@home
  infrastructure; FoldOps may coordinate approved manifests but does not proxy
  the binaries.
- Persistent logging uses bounded `systemd-journald` storage under
  `/data/logs/journal` and must not stop Folding@home when unavailable or full.
- Structured configuration uses schema-versioned TOML files by ownership
  domain, with strict validation and atomic activation.
- The approved v0.1.0 implementation blueprint is
  [Milestone 1 engineering specification](milestone/1-engineering-spec.md).
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
