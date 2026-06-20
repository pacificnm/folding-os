# ADR-0030: Supervisor Recovery Backup And Export

**Status:** Accepted

**Date:** 2026-06-18

**Authors:** FoldingOS project

**Depends on:** [ADR-0019](0019-foldops-supervisor-provisioning-and-tls.md),
[ADR-0026](0026-foldops-dashboard-operator-authentication.md),
[ADR-0027](0027-foldops-remote-operator-api.md)

**Related:** [Milestone 5 engineering specification](../milestone/5-engineering-spec.md),
[update-system.md](../update-system.md)

---

## Context

Supervisor nodes hold fleet-critical state:

- FoldOps SQLite database (`/data/foldops/foldops.db`)
- FoldOps configuration (`/data/config/foldops/`)
- provisioning enrollment records (`/data/provision/enrollments/`)
- TLS material and ingest tokens
- assigned FoldOps manifest and tools version files

Operating-system image updates and FoldOps software updates must not be the
only recovery path when an operator needs to preserve or migrate supervisor
state. Milestone 5 requires a **simple, approved backup and download** workflow
accessible from the supervisor admin UI before the larger FoldOps rework.

This ADR does **not** define full disaster-recovery automation, off-site backup
scheduling to object storage, or agent-node backup. It covers **supervisor
export and restore primitives** sufficient for operator-driven recovery.

---

## Decision

FoldingOS will provide **supervisor recovery export** through authenticated
supervisor HTTP APIs and matching `foldingosctl` helpers where appropriate.

### 1. Export scope

A recovery export bundle includes:

| Content | Source path | Notes |
| --- | --- | --- |
| FoldOps database | `/data/foldops/foldops.db` | SQLite; quiesce or hot-copy with SQLite backup API |
| FoldOps env and tokens | `/data/config/foldops/` | includes `supervisor.env`, `agent.env`, `ingest-token` |
| Enrollment index and records | `/data/provision/enrollments/` | fleet desired versions |
| Boot allowlists | `/data/config/provision/boot-allowlist`, `boot-install-disk-allowlist` | when present |
| Assigned software pins | `/data/config/foldops/assigned-manifest.toml`, `/data/config/tools/assigned-version.json` | when present |
| TLS CA export | `/data/config/foldops/supervisor-ca.pem` | public CA only in default export |

**Excluded by default** (operator may opt in later):

- private TLS keys under `/data/foldops/tls/` (require explicit `--include-secrets`)
- full `/data/config` tree (too broad for Milestone 5)
- OS registry images under `/data/registry/images/`

### 2. Export format

- Single **`tar.zst`** archive with a top-level `manifest.json` describing
  included files, export timestamp, supervisor hostname, and FoldingOS version
- Filename pattern: `foldingos-supervisor-backup-<hostname>-<timestamp>.tar.zst`
- Maximum size guard with clear error when registry images are mistakenly included

### 3. Operator interfaces

| Interface | Purpose |
| --- | --- |
| `GET /api/recovery/export` | stream or redirect download of latest on-demand export |
| `POST /api/recovery/export` | create new export bundle on supervisor disk, return download URL |
| `foldingosctl recovery export` | local CLI equivalent for SSH operators |

Restore remains **operator-guided** in Milestone 5:

- `foldingosctl recovery import <archive>` validates manifest and restores files
  to approved paths with fail-closed checks
- services restart only after successful import validation

Full one-click UI restore may follow in FoldOps Upgrades; Milestone 5 requires
export download and documented manual restore steps.

### 4. Authentication and audit

- Export routes require **operator authentication** when ADR-0026 dashboard
  auth is enabled; otherwise ingest-token-level protection applies interim.
- Every export creates an audit log entry (supervisor journal) with operator
  identity, timestamp, and bundle checksum.
- Exported archives contain secrets; operators must store them accordingly.

### 5. Local retention

Optional on-supervisor retention under `/data/foldops/backups/`:

- keep last **N** exports (default 3)
- tmpfiles creates directory `0750 root:foldops`
- older exports pruned on new export

### 6. Failure behavior

- Export failure must not stop FoldOps supervisor, agent, or Folding@home on
  agents.
- Partial exports are rejected; no incomplete bundle is offered for download.
- Import failure must not overwrite existing files until validation passes.

---

## Alternatives Considered

### Full supervisor disk imaging

Out of scope. OS image channel handles platform recovery.

### Automatic off-site backup to R2

Deferred. Milestone 5 delivers on-demand export; scheduled upload may follow.

### Agent backup symmetry

Deferred. Agent persistent state is narrower; supervisor is the fleet authority.

---

## Consequences

### Positive

- Operators can migrate or recover supervisor fleet state without re-enrolling
  every node
- Complements update workflow in ADR-0028

### Negative

- Exported bundles are sensitive; mishandling exposes ingest tokens
- Hot SQLite backup requires careful implementation
- Restore validation must track schema evolution

---

## References

- [Milestone 5 engineering specification](../milestone/5-engineering-spec.md)
- [operations.md](../operations.md)
- [ADR-0019](0019-foldops-supervisor-provisioning-and-tls.md)
