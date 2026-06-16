# Milestone 4: Appliance Artifact Transport And Monorepo Plan

**Version:** 1.0

**Status:** Proposed

**Date:** 2026-06-14

**Target:** Milestone 4 (FoldOps integration) and Milestone 5 (update system) boundary

---

## Purpose

This document finalizes how FoldingOS appliances acquire, update, and assign:

- **FoldOps** (Rust, runtime under `/data/apps/foldops/`)
- **`foldingosctl`** (Go, platform control plane)

without requiring a full operating-system image reflash for routine fleet software
changes.

It implements the decisions in:

- [ADR-0022: FoldOps Rust Source In FoldingOS Monorepo](../adr/0022-foldops-rust-source-in-foldingos-monorepo.md)
- [ADR-0023: Runtime FoldOps And foldingosctl Updates Without OS Reimage](../adr/0023-runtime-foldops-and-foldingosctl-updates-without-os-reimage.md)

Milestone 4 delegation to `foldingosctl` ([ADR-0020](../adr/0020-foldops-delegates-node-operations-to-foldingosctl.md),
[ADR-0021](../adr/0021-machine-readable-foldingosctl-automation-interface.md)) proceeds in parallel with this transport work.

---

## Goals

| Goal | Mechanism |
| --- | --- |
| Co-develop FoldOps and FoldingOS | `packages/foldops/` Rust workspace in this repo |
| No Node.js on appliances | Rust-only FoldOps; abandon Node implementation |
| No runtime apt/dpkg | `layout-tar-zst` extract in `foldingosctl` |
| Update FoldOps without OS reimage | Supervisor-assigned manifest + `foldops acquire` |
| Update foldingosctl without OS reimage | Supervisor-assigned tools version + `tools acquire` |
| Verified, pinned acquisition | HTTPS + SHA-256 + manifest schema v2 |
| OS image stays small | Bootstrap manifest floor only; apps on `/data` |

---

## Architecture

```text
┌─────────────────────────────────────────────────────────────────┐
│  folding-os monorepo (build host)                               │
│  packages/foldingosctl   packages/foldops (Rust)                │
│  scripts/build-foldops-bundles   scripts/build (OS image)       │
└────────────┬───────────────────────────────┬────────────────────┘
             │ publish                       │ publish
             ▼                               ▼
   packages.folding-os.com/foldops/   releases.folding-os.com/
   packages.folding-os.com/foldingos-tools/
             │
             │ HTTPS acquire (no apt)
             ▼
┌─────────────────────────────────────────────────────────────────┐
│  FoldingOS appliance                                            │
│  /usr/share/foldingos/manifests/foldops.toml  (bootstrap floor) │
│  /data/config/foldops/assigned-manifest.toml  (supervisor)      │
│  /data/config/tools/assigned-version.json     (supervisor)      │
│  /data/apps/foldops/<release>/                (FoldOps tree)    │
│  /usr/bin/foldingosctl                        (tools acquire)   │
└─────────────────────────────────────────────────────────────────┘
```

---

## Five Conditions (Acceptance)

Routine FoldOps and `foldingosctl` updates without OS reflash are acceptable when all five are true:

1. **Supervisor-assigned manifest** on `/data` overrides embedded bootstrap pins for FoldOps
2. **`layout-tar-zst`** is the appliance transport for FoldOps (not runtime deb)
3. **`foldingosctl tools acquire`** updates the control-plane binary with verification
4. **Supervisor API** exposes desired FoldOps release and desired tools version per node or fleet
5. **Monorepo CI** publishes bundles and tools binaries without `./scripts/build` OS image

---

## Implementation Phases

### Phase A — Documentation and ADR acceptance (this branch)

- [x] ADR-0022 and ADR-0023 (proposed)
- [x] This plan document
- [ ] Document alignment sweep (see table below)
- [ ] Accept ADRs when stakeholders approve

### Phase B — Monorepo import

- Create `packages/foldops/` Rust workspace (import from legacy repo)
- `packages/foldops/README.md` — build and bundle instructions
- `scripts/build-foldops-bundles` — produce `layout-tar-zst` + manifest
- CI job: build and publish to staging `packages.folding-os.com`

### Phase C — foldingosctl acquisition v2

- Manifest schema v2 parsing (`artifact_format`, `install_prefix`)
- `layout-tar-zst` extract path in `foldops acquire`
- Assigned manifest precedence over embedded bootstrap
- `foldingosctl tools acquire` command and state under `/data/state/tools/`
- Supervisor assignment writers (`foldingosctl provision` / registry extensions)
- Unit tests and QEMU acceptance for acquire without reimage

### Phase D — FoldOps Rust integration (Milestone 4)

- Agent ingest via `foldingosctl inspect` ([ADR-0020](../adr/0020-foldops-delegates-node-operations-to-foldingosctl.md))
- Supervisor local fleet commands
- Dashboard workflows for version assignment
- Physical validation records

### Phase E — Deprecate appliance deb path

- New releases publish `layout-tar-zst` only
- Remove deb extract from default appliance code path after fleet migration

---

## Build Host Commands (Target)

```bash
# OS image (unchanged — does not embed FoldOps)
./scripts/build

# FoldOps layout bundles + manifest (new)
./scripts/build-foldops-bundles

# foldingosctl static binary for tools channel (new or extended)
./scripts/build-foldingosctl-release
```

FoldOps bundle build uses **Rust `cargo`** and repository shell scripts only. There is no Node.js or `npm` in the FoldingOS build path.

---

## Publication Layout (Target)

```text
packages.folding-os.com/
  foldops/
    2026.06.14/
      manifest.toml
      foldops-agent-x86_64.tar.zst
      foldops-supervisor-x86_64.tar.zst
  foldingos-tools/
    2026.06.14/
      foldingosctl-x86_64
      SHA256SUMS
```

---

## Document Alignment Matrix

Use this table for the cross-reference sweep. **Status** tracks whether each
document reflects ADR-0022/0023.

| Document | What must align | Status |
| --- | --- | --- |
| [ADR-0018](../adr/0018-foldops-package-acquisition-and-update-model.md) | Amended-by header; deb = secondary; assigned manifest + layout bundles per 0023 | Header updated |
| [ADR-0014](../adr/0014-fixed-installation-roles.md) | FoldOps still runtime on `/data`; no role change | Review |
| [ADR-0017](../adr/0017-official-release-publication-and-supervisor-upstream-polling.md) | Add `packages.folding-os.com` channels | Review |
| [ADR-0019](../adr/0019-foldops-supervisor-provisioning-and-tls.md) | Assignment API for FoldOps/tools versions | Review |
| [ADR-0020](../adr/0020-foldops-delegates-node-operations-to-foldingosctl.md) | Rust monorepo; no Node/apt on appliances | Updated |
| [ADR-0021](../adr/0021-machine-readable-foldingosctl-automation-interface.md) | `inspect` includes assigned vs active tools/FoldOps versions | Review |
| [foldops-integration.md](../foldops-integration.md) | Monorepo source; layout bundles; no separate-repo authority | Updated |
| [ai-context.md](../ai-context.md) | Source monorepo; runtime separate | Updated |
| [3-engineering-spec.md](3-engineering-spec.md) | Note bootstrap floor vs assigned pins (historical M3) | Review |
| [4-implementation-spec.md](4-implementation-spec.md) | Scope includes transport + monorepo | Updated |
| [4-engineering-spec.md](4-engineering-spec.md) | Phases A–E work breakdown | Updated |
| [ROADMAP.md](../../ROADMAP.md) | M4 bullets for monorepo and runtime updates | Updated |
| [testing-strategy.md](../testing-strategy.md) | Acquire v2 and tools acquire tests | Review |
| [operations.md](../operations.md) | Operator commands for assignment and acquire | Review |
| [installer/operations.md](../installer/operations.md) | Publication URLs | Review |
| [packages/foldops/README.md](../../packages/foldops/README.md) | Placeholder until import | Created |

---

## Non-Goals (This Plan)

- Embedding FoldOps in Buildroot rootfs
- Runtime `apt` on appliances
- Node.js FoldOps on appliances
- Replacing OS image channel for platform updates
- Full Milestone 5 A/B rootfs design (tools acquire is a stepping stone)

---

## Related Documents

- [Milestone 4 implementation specification](4-implementation-spec.md)
- [Milestone 4 engineering specification](4-engineering-spec.md)
- [FoldOps integration](../foldops-integration.md)
- [ADR README](../adr/README.md)
