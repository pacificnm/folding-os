# ADR-0033: Supervisor Recovery Restore Workflow In FoldOps Upgrades

**Status:** Proposed

**Date:** 2026-06-20

**Authors:** FoldingOS project

**Depends on:** [ADR-0030](0030-supervisor-recovery-backup-and-export.md),
[ADR-0026](0026-foldops-dashboard-operator-authentication.md),
[ADR-0031](0031-foldops-upgrades-dashboard-and-navigation.md)

**Related:** [Milestone 6 implementation specification](../milestone/6-implementation-spec.md),
[Milestone 6 engineering specification](../milestone/6-engineering-spec.md)

---

## Context

Milestone 5 delivered recovery export and download. Operators still need a
guided restore workflow in the dashboard so they can return a supervisor to a
known-good state without dropping into SSH for the routine path.

Restore is more sensitive than export. It must remain fail closed, validate the
archive before mutation, and make the restart boundary explicit.

---

## Decision

FoldingOS will add a **supervisor recovery restore workflow** to FoldOps
Upgrades.

### 1. Guided restore

The dashboard will provide a restore flow that accepts a previously exported
backup archive, validates it, and then restores only the approved supervisor
state paths.

### 2. Fail-closed behavior

The restore path must not modify any data until validation passes. Failed
validation must leave the current supervisor state untouched.

### 3. Authentication

Restore follows the same authenticated operator model as the rest of the
dashboard.

### 4. Service restart boundary

Service restarts happen only after a validated restore completes successfully.

---

## Alternatives Considered

### Leave restore as SSH-only forever

Rejected. That would leave the dashboard incomplete and make the recovery
workflow unnecessarily manual.

### Auto-restore on upload without validation

Rejected. Too risky for fleet-critical state.

---

## Consequences

### Positive

- gives operators a complete dashboard recovery loop
- keeps restore guardrails aligned with the export contract

### Negative

- restore UI and server-side flow must handle archive validation carefully
- the workflow touches sensitive supervisor state

