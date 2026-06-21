# Milestone 6 ADR Acceptance Review

**Version:** 1.0

**Status:** Approved

**Review date:** 2026-06-21

**Target milestone:** Milestone 6, FoldOps Upgrades

**Issue:** [#121](https://github.com/pacificnm/folding-os/issues/121)

---

# Purpose

This document records the Milestone 6 architecture acceptance review required
before FoldOps Upgrades implementation agents treat ADR-0031 through ADR-0033
as binding per [AGENTS.md](../../AGENTS.md).

It reconciles the proposed FoldOps Upgrades decisions with the accepted
Milestone 5 update and recovery contracts and confirms that no unresolved
conflict remains with ADR-0026, ADR-0027, ADR-0028, or ADR-0030.

---

# Completion Status

```text
Milestone 6 ADR review:              COMPLETE
Milestone 6 ADR acceptance:          APPROVED
Milestone 6 implementation:          AUTHORIZED
```

Milestone 6 implementation issues may proceed after this review is committed
and ADR-0031, ADR-0032, and ADR-0033 are marked Accepted.

---

# Governing Documents

| Document | Role |
| --- | --- |
| [6-implementation-spec.md](6-implementation-spec.md) | Implementation scope and sequence |
| [6-engineering-spec.md](6-engineering-spec.md) | Dashboard routes, settings contract, restore workflow |
| [ADR-0031](../adr/0031-foldops-upgrades-dashboard-and-navigation.md) | Dashboard shell and navigation inside existing FoldOps web app |
| [ADR-0032](../adr/0032-unified-settings-model-and-first-boot-configuration-wizard.md) | Canonical settings model and first-boot wizard |
| [ADR-0033](../adr/0033-supervisor-recovery-restore-workflow-in-foldops-upgrades.md) | Guided dashboard restore workflow |

Prerequisite accepted contracts:

| Document | Role |
| --- | --- |
| [5-engineering-spec.md](5-engineering-spec.md) | Milestone 5 software update and recovery export APIs |
| [ADR-0028](../adr/0028-supervisor-fleet-software-update-workflow.md) | Discover, assign, apply workflow |
| [ADR-0030](../adr/0030-supervisor-recovery-backup-and-export.md) | Supervisor recovery export and import primitives |

Dependency references reviewed for consistency (still Proposed, but
implemented and referenced by accepted Milestone 5 work):

| Document | Role |
| --- | --- |
| [ADR-0026](../adr/0026-foldops-dashboard-operator-authentication.md) | Operator session authentication |
| [ADR-0027](../adr/0027-foldops-remote-operator-api.md) | Browser → supervisor → foldingosctl delegation |

---

# ADR Review Summary

## ADR-0031: FoldOps Upgrades Dashboard And Navigation

**Decision:** Rework the existing FoldOps web application into a coherent
dashboard shell with persistent section navigation, status summaries, and
route-based admin pages.

**Milestone 5 alignment:**

- Explicitly preserves Milestone 5 HTTP contracts; the dashboard remains a thin
  client of supervisor APIs.
- Software update, recovery export, and assignment flows continue to call the
  existing `/api/software/*`, `/api/fleet/*`, and `/api/recovery/export*`
  routes defined in [5-engineering-spec.md](5-engineering-spec.md).
- Does not reintroduce browser-side `foldingosctl` invocation, consistent with
  ADR-0027.

**Conflict check:** No conflict with ADR-0028 or ADR-0030. ADR-0028 section 6
defers full dashboard polish to FoldOps Upgrades; ADR-0031 implements that
deferral without changing discover, assign, or apply semantics.

**Verdict:** Accept.

---

## ADR-0032: Unified Settings Model And First-Boot Configuration Wizard

**Decision:** Introduce a schema-versioned canonical FoldOps settings document
(TOML) with generated runtime compatibility files and a first-boot configuration
wizard in the dashboard.

**Milestone 5 alignment:**

- Does not change Milestone 5 update, publication, or recovery API semantics.
- Consolidates the scattered env-file bootstrap that Milestone 5 intentionally
  left unchanged ([5-engineering-spec.md](5-engineering-spec.md), Admin UI
  minimum section).
- Generated env files (`supervisor.env`, `agent.env`, ingest token material)
  remain the runtime contract for existing services.

**Conflict check:**

- Consistent with [ADR-0005](../adr/0005-configuration-ownership-and-precedence.md)
  layered configuration and explicit ownership under `/data/config/`.
- Extends the first-run operator flow in ADR-0026 without replacing ingest
  token generation or session authentication boundaries.
- Engineering spec canonical path `/data/config/foldops/settings.toml` lives
  under the existing FoldOps configuration domain; implementation must follow
  [ADR-0011](../adr/0011-toml-configuration-validation-and-migration.md)
  validation and migration rules.

**Verdict:** Accept.

---

## ADR-0033: Supervisor Recovery Restore Workflow In FoldOps Upgrades

**Decision:** Add a guided dashboard restore workflow with fail-closed archive
validation, explicit operator confirmation, and service restart only after
successful restore.

**Milestone 5 alignment:**

- ADR-0030 explicitly deferred one-click UI restore to FoldOps Upgrades while
  requiring `foldingosctl recovery import` and documented manual restore in
  Milestone 5.
- Restore reuses the ADR-0030 bundle layout, manifest validation, and approved
  state paths already implemented in Milestone 5.
- Dashboard restore will follow the ADR-0027 curated delegation pattern
  (browser → supervisor HTTPS → local `foldingosctl recovery import`); this
  **extends** the Milestone 5 recovery contract and does not modify export API
  semantics.

**Conflict check:** No conflict with ADR-0028. Restore and software update flows
remain separate operator workflows with independent validation gates.

**Verdict:** Accept.

---

# Milestone 5 API Boundary Reconciliation

Review confirms Milestone 6 stays within the Milestone 5 execution boundary:

| Concern | Milestone 5 contract | Milestone 6 change |
| --- | --- | --- |
| Update discovery | `GET /api/software/updates` | UI polish only; no API change |
| Assignment | `POST /api/fleet/assign` | UI polish only; no API change |
| Apply | `POST /api/fleet/software/*`, `POST /api/software/apply-local` | UI polish only; no API change |
| Recovery export | `POST/GET /api/recovery/export*` | UI polish only; no API change |
| Recovery import | `foldingosctl recovery import` (CLI) | Dashboard wrapper via supervisor delegation |
| Settings | env files under `/data/config/foldops/` | Canonical model generates same runtime files |

No unresolved conflict with Milestone 5 contracts.

---

# Implementation Readiness

After acceptance, the following Milestone 6 implementation issues may proceed:

| Issue | Title |
| --- | --- |
| #123 | Recovery restore workflow |
| #124 | Dashboard shell and navigation |
| #125 | Unified settings model and first-boot wizard |
| #127–#137 | Operator polish, telemetry, help, and alerting surfaces |

Issue #122 (documentation readiness review) remains the milestone closeout gate
after implementation and validation complete.

---

# Conclusion

**Milestone 6 ADR acceptance: PASS**

ADR-0031, ADR-0032, and ADR-0033 are accepted. FoldOps Upgrades implementation
may proceed against the Milestone 5 API and recovery contracts without
architectural revision.
