# FoldingOS Milestone 5 Update And Recovery System Implementation Specification

**Version:** 1.0

**Status:** Proposed

**Target Milestone:** Milestone 5, Update and Recovery System

---

# Purpose

This document defines implementation scope and sequence for Milestone 5.

Concrete API contracts, index schemas, and file layouts are in
[5-engineering-spec.md](5-engineering-spec.md).

Approved ADRs:

- [ADR-0028](../adr/0028-supervisor-fleet-software-update-workflow.md)
- [ADR-0029](../adr/0029-packages-channel-publication-via-rclone.md)
- [ADR-0030](../adr/0030-supervisor-recovery-backup-and-export.md)

---

# Milestone Goal

```text
Operator builds FoldOps + tools release on build host

↓

rclone publishes to packages.folding-os.com

↓

Supervisor admin UI: check for updates

↓

Operator assigns desired FoldOps manifest + tools version to fleet

↓

Operator triggers apply (supervisor + agents)

↓

Nodes acquire without OS reimage; FAH keeps running

↓

Operator exports supervisor backup before risky changes
```

This milestone completes the **operator loop** for runtime fleet software before
the FoldOps Upgrades dashboard rework.

---

# Scope

## In scope

- packages channel `index.json` publication and supervisor update discovery
- rclone publication scripts for FoldOps and tools
- supervisor HTTP routes for update check, fleet apply, local apply
- agent HTTP routes for `foldops acquire` and `tools acquire` delegation
- JSON automation output for acquire commands when gaps remain
- supervisor recovery export/import via `foldingosctl` and HTTP download
- minimal admin UI section (update check, assign, apply, backup download)
- live-lab validation on supervisor + agents

## Out of scope

- FoldOps Upgrades (full dashboard, first-boot admin wizard, config UI polish)
- OS image A/B partitions and signed update bundles
- agent backup
- CI-only publication without rclone (may follow later)

---

# Prerequisites

Milestone 4 implementation merged:

- Rust `foldingosctl` with `foldops acquire`, `tools acquire`, `provision assign`
- layout-tar-zst bundles and assigned manifest precedence
- FoldOps supervisor fleet API and agent HTTP proxy pattern
- `scripts/build-foldops-bundles`, `scripts/publish-foldops-bundles`

---

# Implementation Sequence

## Phase 1 — Publication pipeline

1. Add `scripts/publish-foldingos-tools`
2. Add `scripts/publish-packages-release` with `--build` and `--dry-run`
3. Keep tools publication independent of OS image builds; use
   `build-foldingosctl-release --sync-overlay` to pin the next image bootstrap
4. Define and publish initial `index.json` for both channels
5. Document operator commands in [operations.md](../operations.md)

**Exit criteria:** test release visible at `packages.folding-os.com` from build
host using existing `~/.config/rclone/rclone.conf`.

## Phase 2 — Update discovery

1. Supervisor module to fetch and cache upstream indexes
2. `GET /api/software/updates` combining indexes, enrollments, and ingest state
3. Unit tests for version comparison and compatibility filtering

**Exit criteria:** API returns correct `*_update_available` flags against a
published test release.

## Phase 3 — Apply delegation

1. Agent HTTP: `POST /software/foldops-acquire`, `POST /software/tools-acquire`
2. Extend agent automation policy for acquire commands
3. Supervisor proxy routes: `apply-foldops`, `apply-tools`, `apply-local`
4. Ensure acquire JSON envelopes and service restart behavior
5. Fail-closed tests; confirm FAH unaffected

**Exit criteria:** assign + apply on lab agent moves active FoldOps manifest and
tools version without reimage.

## Phase 4 — Recovery

1. `foldingosctl recovery export` / `import` on supervisor role
2. `POST /api/recovery/export`, `GET /api/recovery/export/latest`
3. tmpfiles for `/data/foldops/backups/`
4. Document restore procedure in [operations.md](../operations.md)

**Exit criteria:** export downloaded from supervisor; import restores DB and
config on a test instance.

## Phase 5 — Admin UI minimum

1. Admin section pages calling Milestone 5 APIs
2. Update check table with assign + apply actions
3. Recovery download button

**Exit criteria:** operator completes update and backup flows without SSH.

## Phase 6 — Validation

1. Live lab runbook executed on supervisor + ≥1 agent
2. Optional: extend `scripts/test-api-json` with software update routes

---

# Component Ownership

| Component | Owner path |
| --- | --- |
| Publication scripts | `scripts/` |
| Index schema | `doc/milestone/5-engineering-spec.md` |
| `foldingosctl` acquire JSON, recovery | `packages/foldingosctl/` |
| Supervisor update + recovery API | `packages/foldops/crates/foldops-supervisor/` |
| Agent apply HTTP | `packages/foldops/crates/foldops-agent/` |
| Admin UI pages | `packages/foldops/web/` (minimal `/admin` subtree) |
| Overlay tmpfiles | `overlay/usr/lib/tmpfiles.d/foldingos.conf` |

---

# Dependencies On FoldOps Upgrades

The following intentionally defer to the next milestone:

- unified settings model (replace scattered env files)
- polished fleet UX and error surfacing
- first-boot supervisor setup wizard
- dashboard auth hardening beyond ADR-0026 baseline

Milestone 5 APIs must remain stable so FoldOps Upgrades can consume them without
breaking changes.

---

# Acceptance Criteria

- Operator publishes FoldOps and tools releases via rclone scripts from build host
- Supervisor admin UI checks for updates against packages channel indexes
- Operator assigns and applies FoldOps + tools updates to supervisor and agents
  without OS reimage
- `folding-at-home.service` stays active through apply failures
- Operator exports and downloads supervisor backup archive
- All Milestone 5 ADRs and this specification committed before implementation
  agents treat the milestone as approved

---

# Related Documents

- [Milestone 5 engineering specification](5-engineering-spec.md)
- [ROADMAP.md](../../ROADMAP.md)
- [foldops-integration.md](../foldops-integration.md)
