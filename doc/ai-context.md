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
- FoldOps Rust source lives in `packages/foldops/` in this repository per
  [ADR-0022](adr/0022-foldops-rust-source-in-foldingos-monorepo.md). Runtime
  acquisition remains separate from the OS image per
  [ADR-0018](adr/0018-foldops-package-acquisition-and-update-model.md) and
  [ADR-0023](adr/0023-runtime-foldops-and-foldingosctl-updates-without-os-reimage.md).
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
- Milestone 2 Folding@home runtime implementation and validation are complete;
  see [milestone/2-readiness-review.md](milestone/2-readiness-review.md).
- Milestone 3 network fleet provisioning implementation and validation are
  complete; see [milestone/3-readiness-review.md](milestone/3-readiness-review.md),
  [ADR-0016](adr/0016-network-provisioning-via-supervisor.md), and
  [milestone/3-engineering-spec.md](milestone/3-engineering-spec.md).
- Milestone 4 FoldOps integration is the active implementation target; FoldOps
  delegates node-local operations to `foldingosctl` per
  [ADR-0020](adr/0020-foldops-delegates-node-operations-to-foldingosctl.md),
  [ADR-0021](adr/0021-machine-readable-foldingosctl-automation-interface.md),
  [ADR-0022](adr/0022-foldops-rust-source-in-foldingos-monorepo.md),
  [ADR-0023](adr/0023-runtime-foldops-and-foldingosctl-updates-without-os-reimage.md), and
  [ADR-0024](adr/0024-foldops-supervisor-fleet-mutation-authorization.md), and
  [ADR-0025](adr/0025-implement-foldingosctl-in-rust.md),
  [ADR-0026](adr/0026-foldops-dashboard-operator-authentication.md),
  [ADR-0027](adr/0027-foldops-remote-operator-api.md), and
  [milestone/4-engineering-spec.md](milestone/4-engineering-spec.md).
- Operator build, deployment, recovery, and Folding@home runtime procedures are
  in [operations.md](operations.md).
- Next planned platform: Raspberry Pi 5 using ARM64.
- Initial networking may be IPv4-only; IPv6 is a planned first-class
  capability.
- v0.1.0 networking uses Ethernet DHCP; static networking is out of scope.
- Initial remote administration uses OpenSSH with the `foldingos-admin` account
  and public keys provisioned through the EFI System Partition.
- Local commissioning display shows boot messages and a final ready message with
  the DHCP IPv4 address on a temporarily attached monitor. See
  [ADR-0015](adr/0015-local-commissioning-display.md).
- A future installer-capable release used one combined appliance and installer
  image; that approach was superseded by supervisor-led network provisioning.
  See [ADR-0016](adr/0016-network-provisioning-via-supervisor.md).
- FoldingOS supports fixed `agent` and `supervisor` installation roles from
  the same release image. The supervisor is direct-flashed first; agents are
  provisioned over the network. Roles cannot be changed in place.
- FoldOps packages are acquired at runtime from pinned manifests per
  [ADR-0018](adr/0018-foldops-package-acquisition-and-update-model.md).
  Supervisor-assigned manifests and `layout-tar-zst` transport extend this per
  [ADR-0023](adr/0023-runtime-foldops-and-foldingosctl-updates-without-os-reimage.md).
  Ingest-token bootstrap and HTTPS use EFI staging per
  [ADR-0019](adr/0019-foldops-supervisor-provisioning-and-tls.md).
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
