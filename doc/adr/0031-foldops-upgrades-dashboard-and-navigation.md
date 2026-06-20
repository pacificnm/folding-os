# ADR-0031: FoldOps Upgrades Dashboard And Navigation

**Status:** Proposed

**Date:** 2026-06-20

**Authors:** FoldingOS project

**Depends on:** [ADR-0026](0026-foldops-dashboard-operator-authentication.md),
[ADR-0027](0027-foldops-remote-operator-api.md),
[ADR-0028](0028-supervisor-fleet-software-update-workflow.md),
[ADR-0030](0030-supervisor-recovery-backup-and-export.md)

**Related:** [Milestone 6 implementation specification](../milestone/6-implementation-spec.md),
[Milestone 6 engineering specification](../milestone/6-engineering-spec.md)

---

## Context

Milestone 5 shipped a minimal supervisor admin section for software updates and
recovery. FoldOps Upgrades is the next step: a durable dashboard and operator
navigation model that can host the broader fleet-management workflow without
changing the Milestone 5 HTTP contracts.

The current admin surface is functional but intentionally minimal. Milestone 6
needs a coherent dashboard shell, shared navigation, and workflow grouping for:

- software updates
- recovery
- fleet settings
- operator-facing status and diagnostics

The dashboard must remain a thin client for supervisor HTTP APIs and must not
reintroduce direct browser-side access to privileged node operations.

---

## Decision

FoldingOS will implement FoldOps Upgrades as a **dashboard rework inside the
existing FoldOps web application**, not as a separate control plane.

### 1. Dashboard shell

The supervisor web app will keep a single authenticated admin entry point, but
Milestone 6 will introduce a stronger dashboard shell:

- persistent section navigation
- status summaries for software, recovery, and settings
- shared action and notification handling
- route-based admin pages that remain compatible with the Milestone 5 APIs

### 2. Workflow grouping

The dashboard will organize operational tasks into these groups:

- software updates and apply flows
- recovery export and restore flows
- first-boot and runtime settings
- diagnostics and operator status

The dashboard should favor compact, operational layout over marketing-style or
consumer-style presentation.

### 3. API boundary

FoldOps Upgrades remains a client of the Milestone 5 supervisor APIs. The UI
must continue to call supervisor endpoints rather than shelling out to
`foldingosctl` from the browser.

### 4. Navigation compatibility

Existing `/admin/*` routes may remain as entry points, but Milestone 6 may add
new routes and route-level layout composition to make the dashboard coherent.

### 5. Future growth

The dashboard shell should leave room for later work on richer fleet views and
operator tooling without forcing a second full redesign.

---

## Alternatives Considered

### Keep the minimal Milestone 5 admin section permanently

Rejected. It is sufficient for completion of Milestone 5, but not for the
longer-term operator workflow the roadmap already names FoldOps Upgrades.

### Build a separate standalone settings console

Rejected. Splitting the operator surface would duplicate authentication, route
structure, and workflow state.

---

## Consequences

### Positive

- gives Milestone 6 a stable UI boundary
- preserves Milestone 5 APIs
- reduces duplicated layout and status handling across admin pages

### Negative

- dashboard refactor touches many existing web pages
- shared layout and state handling need care to avoid regressions

