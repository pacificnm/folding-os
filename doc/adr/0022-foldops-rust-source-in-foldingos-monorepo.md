# ADR-0022: FoldOps Rust Source In FoldingOS Monorepo

**Status:** Proposed

**Date:** 2026-06-14

**Authors:** FoldingOS project

**Amends:** [ADR-0014](0014-fixed-installation-roles.md),
[ADR-0018](0018-foldops-package-acquisition-and-update-model.md)

**Depends on:** [ADR-0023](0023-runtime-foldops-and-foldingosctl-updates-without-os-reimage.md)

---

## Context

FoldOps predates FoldingOS. The original implementation targeted Debian farm
nodes with Node.js agents and optional `apt` installation from `deb.folding-os.com`.

FoldingOS is now the primary delivery platform. Milestone 3 integrated FoldOps as
**runtime-acquired application software** under `/data/apps/foldops/`, coordinated
by `foldingosctl foldops acquire` per [ADR-0018](0018-foldops-package-acquisition-and-update-model.md).

FoldOps is being rewritten in **Rust only**. The Node.js implementation is
abandoned. Continuing to develop FoldOps in a separate repository while
FoldingOS, `foldingosctl`, acquisition manifests, systemd units, and Milestone 4
delegation contracts evolve in this repository creates avoidable friction:

- every contract change requires coordinated cross-repository pull requests
- local development depends on publishing `.deb` artifacts before testing on
  appliances
- transport and assignment model changes touch both codebases

[ADR-0018](0018-foldops-package-acquisition-and-update-model.md) states that
FoldOps remains an independent repository with its own release process. That
model fit external Debian consumers and pre-FoldingOS development. The project
direction now prioritizes the FoldingOS appliance platform.

This ADR does **not** change the rule that FoldOps binaries are **not embedded**
in the 4 GiB operating-system image at build time. Source co-location and
runtime acquisition are separate concerns.

---

## Decision

1. **FoldOps Rust source lives in this repository** under `packages/foldops/`,
   alongside `packages/foldingosctl/`.

2. **The authoritative FoldOps implementation for FoldingOS appliances** is the
   Rust workspace in `packages/foldops/`. The legacy Node.js tree in
   `pacificnm/foldops` is deprecated and not used for new FoldingOS work.

3. **FoldingOS appliances do not run Node.js** and do not acquire Node-based
   FoldOps artifacts.

4. **Buildroot does not install FoldOps into the root filesystem** at image build
   time. The OS image continues to ship only:
   - embedded bootstrap acquisition manifests
   - FoldingOS-owned systemd units
   - `foldingosctl` and other platform binaries

5. **Release artifacts** for appliances are **layout bundles** (see
   [ADR-0023](0023-runtime-foldops-and-foldingosctl-updates-without-os-reimage.md))
   built from `packages/foldops/` by repository CI or
   `scripts/build-foldops-bundles`, published to official HTTPS infrastructure,
   and installed at runtime by `foldingosctl foldops acquire`.

6. **Optional Debian packages** may still be built from `packages/foldops/` for
   non-appliance Debian hosts. That path is secondary and does not define the
   FoldingOS appliance contract.

7. **systemd units remain FoldingOS-owned** in the overlay. Bundle contents
   must not enable upstream `.deb` unit files directly on appliances.

---

## Repository Layout

```text
packages/
  foldingosctl/          # Rust — appliance control plane (see ADR-0025)
  foldops/               # Rust workspace — fleet management applications
    Cargo.toml
    crates/
      foldops-agent/
      foldops-supervisor/
      foldops-shared/
    web/                   # dashboard static assets (built on build host only)
    packaging/
      appliance-bundle/    # layout tar.zst production
      deb/                 # optional Debian output
```

---

## Alternatives Considered

### Continue separate pacificnm/foldops repository

Rejected for active development. Cross-repo coordination cost dominates Milestone
4 and runtime-update work. The historical repository may remain archived or as a
read-only mirror for external references.

### Embed FoldOps in the OS image at Buildroot build time

Rejected. Reintroduces full image rebuilds for FoldOps fixes and conflicts with
[ADR-0018](0018-foldops-package-acquisition-and-update-model.md) and
[ADR-0023](0023-runtime-foldops-and-foldingosctl-updates-without-os-reimage.md).

### Git submodule of foldops into folding-os

Rejected. Submodules add friction without stronger guarantees than vendoring the
Rust tree directly. Monorepo source is simpler for joint changes.

---

## Consequences

### Positive

- Single pull request can change FoldOps, `foldingosctl`, manifests, and docs
- Local development builds Rust platform and fleet code from one tree
- Rust-only FoldOps aligns with appliance constraints (no Node runtime)
- Clear platform ownership: FoldingOS repository is the appliance stack

### Negative

- Repository size and CI complexity increase
- External contributors expecting foldops-only repo must retarget
- License documentation must remain clear (MIT FoldOps crates within GPL project)

### Tradeoffs

- Debian `apt` consumers become optional; primary channel is layout bundles for
  appliances

---

## Required Follow-Up

- Import Rust FoldOps tree into `packages/foldops/`
- Implement `scripts/build-foldops-bundles`
- Update living documents per [4-appliance-artifact-and-monorepo-plan.md](../milestone/4-appliance-artifact-and-monorepo-plan.md) alignment table
- Archive or redirect `pacificnm/foldops` when import completes

---

## References

- [ADR-0018: FoldOps Package Acquisition And Update Model](0018-foldops-package-acquisition-and-update-model.md)
- [ADR-0020: FoldOps Delegates Node Operations To foldingosctl](0020-foldops-delegates-node-operations-to-foldingosctl.md)
- [ADR-0023: Runtime FoldOps And foldingosctl Updates Without OS Reimage](0023-runtime-foldops-and-foldingosctl-updates-without-os-reimage.md)
- [Milestone 4 appliance artifact and monorepo plan](../milestone/4-appliance-artifact-and-monorepo-plan.md)
