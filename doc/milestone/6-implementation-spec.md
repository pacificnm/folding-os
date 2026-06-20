# FoldingOS Milestone 6 FoldOps Upgrades Implementation Specification

**Version:** 1.0

**Status:** Proposed

**Target Milestone:** Milestone 6, FoldOps Upgrades

---

# Purpose

This document defines the implementation scope for Milestone 6.

Milestone 5 completed the update, publication, and recovery foundation. This
milestone turns that foundation into a durable operator experience: a fuller
FoldOps dashboard, a unified settings model, and a guided restore workflow.

The current Milestone 6 issue set expands that core into operator-facing
polish and data capture work:

- dashboard shell, breadcrumbs, footer, and navigation cleanup
- restored project, Folding, CPU temperature, PPD, and TPF display surfaces
- admin machine controls and host hardware profile capture
- work-unit history with start and stop timestamps
- help/manual delivery backed by the GitHub Wiki
- Discord and email alert configuration

Concrete UI routes, settings contracts, and validation requirements are in
[6-engineering-spec.md](6-engineering-spec.md).

Approved ADRs:

- [ADR-0031](../adr/0031-foldops-upgrades-dashboard-and-navigation.md)
- [ADR-0032](../adr/0032-unified-settings-model-and-first-boot-configuration-wizard.md)
- [ADR-0033](../adr/0033-supervisor-recovery-restore-workflow-in-foldops-upgrades.md)

---

# Milestone Goal

```text
FoldOps supervisor boots

↓

Dashboard presents a coherent operator shell

↓

Operator configures first-boot and runtime settings from one model

↓

Operator reviews software, recovery, and diagnostic workflows in one place

↓

Operator restores supervisor state through a validated guided flow

↓

Fleet APIs from Milestone 5 remain the execution boundary
```

This milestone completes the **FoldOps Upgrades** UI and settings layer while
preserving the Milestone 5 API contracts.

---

# Scope

## In scope

- dashboard shell and route layout for the supervisor admin surface
- unified FoldOps settings model with generated compatibility files
- first-boot configuration wizard for supervisor initialization
- recovery restore workflow in the admin UI
- navigation and workflow polish for software updates, recovery, diagnostics,
  breadcrumbs, and footer/site links
- host inventory, telemetry, and work-history presentation in the admin views
- admin machine control behavior and host hardware profile persistence
- help system and operator manual delivery through project documentation
- configuration entry points for Discord and email alerting
- validation on supervisor and at least one agent in lab or QEMU where relevant

## Out of scope

- new agent or supervisor mutation APIs
- changing Milestone 5 update, publication, or recovery API semantics
- OS image A/B partitioning
- general-purpose desktop UI features
- agent backup

---

# Prerequisites

Milestone 5 implementation merged:

- software update discovery, apply, and publication scripts
- supervisor recovery export and download
- minimal admin update and backup UI
- accepted ADRs 0028 through 0030

Milestone 6 also depends on the existing dashboard auth and remote operator API
contracts from Milestone 4 and the Milestone 5 API surface.

---

# Implementation Sequence

## Phase 1 - Dashboard foundation

1. Replace the minimal admin page grouping with a consistent dashboard shell
2. Add navigation and summary components for the Milestone 5 workflows
3. Preserve the existing `/admin/*` entry points

**Exit criteria:** operators can move between software, recovery, and settings
pages without losing context.

## Phase 2 - Unified settings model

1. Introduce a canonical FoldOps settings document
2. Generate compatibility env files from the canonical model
3. Add validation and migration paths for existing bootstrap values

**Exit criteria:** first-boot and runtime settings can be edited in one place and
rendered back into the existing runtime contract.

## Phase 3 - First-boot wizard

1. Add a guided first-boot path in the dashboard
2. Capture the minimum operator inputs needed for a usable supervisor
3. Persist the canonical settings model atomically

**Exit criteria:** a newly provisioned supervisor can be initialized through the
dashboard without hand-editing multiple env files.

## Phase 4 - Recovery restore

1. Add restore upload, validation, and confirmation flows
2. Restore only approved state paths after validation passes
3. Surface success or failure clearly and keep the service restart boundary
   explicit

**Exit criteria:** a backup can be restored through the dashboard with
validation gates and no partial mutation on failure.

## Phase 5 - Validation and documentation

1. Run the dashboard build and unit tests
2. Validate settings migration and restore failure behavior
3. Validate telemetry, history, and alerting surfaces
4. Update operations and cross-reference docs

## Issue alignment

Milestone 6 is tracked through the following issue groups:

- `#120` Milestone 6 planning umbrella
- `#121` ADR acceptance and implementation readiness
- `#122` documentation readiness review
- `#123` recovery restore workflow
- `#124` dashboard shell and navigation
- `#125` unified settings model and first-boot wizard
- `#126` live-hardware validation
- `#127` admin machine controls
- `#128` host hardware profile capture
- `#129` breadcrumbs
- `#130` completed work-unit history
- `#131` GitHub Wiki help system and manual
- `#132` footer and site navigation
- `#133` admin services status rendering
- `#134` dashboard project and Folding details
- `#135` kiosk header navigation cleanup
- `#136` CPU temperature, project, PPD, and TPF telemetry restoration
- `#137` Discord and email alert configuration

---

# Component Ownership

| Component | Owner path |
| --- | --- |
| Dashboard shell and route layout | `packages/foldops/web/` |
| Settings model and render/migration logic | `packages/foldops/crates/foldops-supervisor/` and `packages/foldops/web/` |
| Recovery restore flow | `packages/foldops/crates/foldops-supervisor/`, `packages/foldops/web/`, `packages/foldingosctl/` |
| Documentation updates | `doc/`, `ROADMAP.md` |

---

# Acceptance Criteria

- Dashboard has a stable FoldOps Upgrades shell with coherent navigation
- Supervisor settings are managed through a unified model with generated runtime
  compatibility files
- First-boot configuration wizard initializes a new supervisor cleanly
- Recovery restore is available in the dashboard and validates archives before
  mutation
- Dashboard, kiosk, and detail views retain the operator telemetry and project
  context expected by the current issue set
- Admin machine details include host hardware profile data and work-unit
  history
- Help, footer, breadcrumb, and alerting entry points are documented and
  implemented to the milestone scope
- Milestone 5 APIs remain the execution boundary for update and recovery flows
- The milestone is documented before implementation work is treated as approved

---

# Related Documents

- [Milestone 6 engineering specification](6-engineering-spec.md)
- [ROADMAP.md](../../ROADMAP.md)
- [foldops-integration.md](../foldops-integration.md)
