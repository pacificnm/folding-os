# Milestone 5 Readiness Review

**Version:** 1.0

**Status:** Approved

**Review date:** 2026-06-19

**Target milestone:** Milestone 5, Update and Recovery System

---

# Purpose

This document records the Milestone 5 update and recovery readiness review
required by issue #111.

It reconciles the shipped update discovery, apply, publication, and recovery
behavior with [ADR-0028](../adr/0028-supervisor-fleet-software-update-workflow.md),
[ADR-0029](../adr/0029-packages-channel-publication-via-rclone.md), and
[ADR-0030](../adr/0030-supervisor-recovery-backup-and-export.md). It captures
the implementation closure matrix, the supporting validation evidence, and the
remaining documentation gates for FoldOps Upgrades.

---

# Completion Status

```text
Milestone 5 update and recovery implementation: COMPLETE
Milestone 5 update and recovery validation:      COMPLETE
Milestone 5 readiness:                           SATISFIED
FoldOps Upgrades readiness:                      NEXT
```

Milestone 5 update and recovery work is complete when issues #105 through #112
are closed with matching implementation, documentation, and validation
evidence, and this readiness review is committed.

---

# Governing Specifications

GitHub Milestone 5 is titled **Update and Recovery System**. The governing
approved documents for this milestone are:

| Document | Role |
| --- | --- |
| [5-implementation-spec.md](5-implementation-spec.md) | Implementation scope and sequence |
| [5-engineering-spec.md](5-engineering-spec.md) | Concrete API, index, and file layout contract |
| [ADR-0028](../adr/0028-supervisor-fleet-software-update-workflow.md) | Discover, assign, apply workflow |
| [ADR-0029](../adr/0029-packages-channel-publication-via-rclone.md) | rclone publication and channel indexes |
| [ADR-0030](../adr/0030-supervisor-recovery-backup-and-export.md) | Supervisor recovery export and import |

---

# Issue Closure Matrix

| Issue | Title | State | Primary evidence |
| --- | --- | --- | --- |
| #105 | Implement packages channel publication via rclone (FoldOps + foldingosctl tools) | Closed | `scripts/lib/packages-channel.sh`, `scripts/publish-foldops-bundles`, `scripts/publish-foldingos-tools`, `scripts/publish-packages-release` |
| #106 | Implement supervisor update discovery API (GET /api/software/updates) | Closed | `packages/foldops/crates/foldops-supervisor/src/software/upstream.rs`, `packages/foldops/crates/foldops-supervisor/src/api/routes.rs` |
| #107 | Implement fleet software apply delegation (foldops acquire + tools acquire) | Closed | `packages/foldops/crates/foldops-agent/src/fah/websocket.rs`, `packages/foldops/crates/foldops-agent/src/node_control.rs`, `packages/foldops/crates/foldops-supervisor/src/software/apply.rs` |
| #108 | Implement supervisor recovery export and import (backup download) | Closed | `packages/foldops/crates/foldops-supervisor/src/recovery/mod.rs`, `packages/foldingosctl/src/recovery/export.rs`, `packages/foldingosctl/src/recovery/import.rs`, `packages/foldops/web/src/api.ts` |
| #109 | Add minimal supervisor admin UI for software updates and recovery | Closed | `packages/foldops/web/src/pages/admin/AdminSoftwareUpdates.tsx`, `packages/foldops/web/src/pages/admin/AdminRecovery.tsx`, `packages/foldops/web/src/pages/admin/AdminLayout.tsx` |
| #110 | Validate Milestone 5 update and recovery system on live hardware | Closed | Validation evidence recorded with issue #110; final build, checksum verification, and published artifacts in `build/output/images/`, `build/output/foldops/0.1.0-80/`, and `build/output/foldingos-tools/0.1.0-80/` |
| #111 | Finalize Milestone 5: update and recovery implementation readiness review | Closed | This document |
| #112 | Accept Milestone 5 ADRs (0028-0030) for implementation | Closed | `doc/adr/0028-supervisor-fleet-software-update-workflow.md`, `doc/adr/0029-packages-channel-publication-via-rclone.md`, `doc/adr/0030-supervisor-recovery-backup-and-export.md`, `doc/adr/README.md` |

No Milestone 5 release-blocking issue remains open or deferred through an
approved document change.

---

# Architecture And Implementation Reconciliation

Review confirms the shipped behavior matches the accepted Milestone 5
architecture:

- the supervisor discovers package-channel updates from published `index.json`
  catalogs
- `foldingosctl` tools and FoldOps bundles publish independently through
  rclone-backed scripts
- operator assignment remains distinct from package acquisition
- the supervisor proxies apply actions to agent-local `foldingosctl` endpoints
- local supervisor apply uses `foldingosctl` directly and does not restage the
  OS image
- recovery exports are authenticated, fail closed, and scoped to supervisor
  state
- the minimal admin section exposes check, assign, apply, and backup flows

No unresolved architectural conflict remains between the implementation and the
accepted Milestone 5 ADRs.

---

# Validation Evidence

The Milestone 5 implementation was validated with the following commands and
artifact checks:

- `cd packages/foldops && cargo test --workspace`
- `cd packages/foldops/web && npm run build`
- `cd packages/foldingosctl && cargo test`
- `./scripts/build`
- `cd build/output/images && sha256sum -c foldingos-x86_64-0.1.0.img.sha256`
- `cd build/output/foldops/0.1.0-80 && sha256sum -c SHA256SUMS`
- `cd build/output/foldingos-tools/0.1.0-80 && sha256sum -c SHA256SUMS`
- `./scripts/publish-foldops-bundles 0.1.0-80`
- `./scripts/publish-foldingos-tools 0.1.0-80`
- `./scripts/publish-packages-release --foldops 0.1.0-80 --tools 0.1.0-80 --dry-run`

Those checks confirm the build output, package checksums, publication scripts,
and package indexes are internally consistent.

---

# Conclusion

**Milestone 5 update and recovery readiness: PASS**

Milestone 6 work may proceed against the accepted Milestone 5 API and
publication contracts.
