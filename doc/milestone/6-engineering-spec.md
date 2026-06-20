# FoldingOS Milestone 6 FoldOps Upgrades Engineering Specification

**Version:** 1.0

**Status:** Proposed

**Target Milestone:** Milestone 6, FoldOps Upgrades

---

# Purpose

This document defines the Milestone 6 contract for:

1. **Dashboard rework** - a coherent FoldOps Upgrades operator surface
2. **Unified settings** - canonical supervisor/FoldOps settings with generated
   runtime compatibility files
3. **Recovery restore** - guided dashboard restore of supervisor state

The current issue set also requires the dashboard to preserve operator-facing
telemetry, admin-machine details, navigation polish, help delivery, and alert
configuration surfaces. Those requirements are called out below so the
implementation stays aligned with the issue queue.

It implements:

- [ADR-0031](../adr/0031-foldops-upgrades-dashboard-and-navigation.md)
- [ADR-0032](../adr/0032-unified-settings-model-and-first-boot-configuration-wizard.md)
- [ADR-0033](../adr/0033-supervisor-recovery-restore-workflow-in-foldops-upgrades.md)

and builds on the Milestone 5 API and publication contracts.

FoldOps Upgrades remains a supervisor dashboard built on the existing web
application and API model. It does not replace the Milestone 5 contracts.

---

# Relationship To Milestone 5

| Concern | Milestone 5 delivered | Milestone 6 adds |
| --- | --- | --- |
| Software updates | discovery, assign, apply | dashboard shell and workflow polish |
| Recovery | export and download | restore upload, validation, and guided import |
| Settings | existing env files | canonical settings model and wizard |
| UI | minimal admin section | fuller dashboard navigation and operator flow |

Milestone 5 APIs remain the source of truth for update, apply, and recovery
execution.

---

# Dashboard Shell

## Route model

Milestone 6 keeps the authenticated supervisor web entry point and expands the
admin subtree into a dashboard shell with shared navigation.

Expected route families include:

- `/admin/software`
- `/admin/recovery`
- `/admin/settings`
- `/admin/diagnostics`

Existing routes may remain compatible aliases during transition.

## UX contract

The dashboard should:

- keep workflow actions close to the data they affect
- present status, warnings, and operation results in a consistent layout
- avoid pushing privileged actions into hidden modals or nested flows
- preserve accessibility and keyboard navigation

## Issue-aligned surfaces

Milestone 6 issue work expands the dashboard contract to include:

- breadcrumbs and footer/site links in the FoldOps shell
- dashboard, kiosk, and details views that show project, Folding, CPU
  temperature, PPD, and TPF state consistently
- admin services and machine status views that distinguish unknown from valid
  service states
- admin machine detail pages that store and display a full host hardware
  profile
- completed work-unit records with start and stop timestamps
- GitHub Wiki-backed help content and the FoldingOS Manual entry point
- Discord and email alert configuration in the settings model

---

# Unified Settings Model

## Canonical store

Milestone 6 introduces a schema-versioned canonical settings document for
operator-managed FoldOps configuration.

Suggested canonical location:

- `/data/config/foldops/settings.toml`

The canonical store is the source of truth for:

- initial supervisor bootstrap fields
- operator defaults
- feature flags relevant to FoldOps and the dashboard
- URLs and other supervisor-managed preferences

## Derived files

Runtime compatibility files remain supported and are generated from the
canonical settings model.

Examples:

- env files consumed by FoldOps services
- first-boot bootstrap artifacts
- derived wizard state

## Validation

Settings validation must:

- reject malformed or incompatible schema versions
- preserve existing state on validation failure
- enforce ownership boundaries for secrets and operator-visible values

---

# First-Boot Wizard

The dashboard wizard must support:

- initial administrator-facing FoldOps setup
- required settings entry for a usable supervisor install
- persistence of the canonical settings model
- clear distinction between required and optional inputs

The wizard must not require SSH for the normal setup path once the dashboard is
available.

---

# Recovery Restore

## Request flow

Restore is a supervisor dashboard flow that:

1. accepts a user-selected backup archive
2. validates the archive structure and manifest
3. previews the restore scope
4. executes the restore only after explicit operator confirmation

## Safety

- no mutation before validation passes
- service restarts occur only after successful restore
- failed restore leaves existing state unchanged

## Admin Machine Controls

The dashboard must keep the existing admin machine control surface functional
and predictable:

- actions should reflect the actual remote operator state
- status labels must match the state returned by the underlying API
- fully folded or fully active agents must not be misreported as starting

## Host Profile And Work History

The admin machine detail view must support persistent host inventory data:

- full hardware profile capture for the host
- completed work-unit records with start and stop timestamps
- display of current and historical operator data without losing the live
  status summary

## Help And Alerts

Milestone 6 also covers supporting operator workflows:

- a help system that reads published documentation from the project Wiki
- a FoldingOS Manual landing entry for the wiki-backed help content
- configuration storage for Discord and email alerts
- clear separation between configuration data and future notification delivery

---

# Implementation Targets

| Target | Notes |
| --- | --- |
| `packages/foldops/web/` | Dashboard shell, settings pages, recovery UI |
| `packages/foldops/crates/foldops-supervisor/` | Settings persistence and restore orchestration |
| `packages/foldingosctl/` | Any local helpers needed for validation or import paths |
| `doc/operations.md` | Operator procedures for first boot and restore |

---

# Validation

## Automated

- dashboard build and typecheck
- unit tests for settings validation and migrations
- unit tests for restore validation and failure handling
- unit tests for telemetry and history rendering
- validation of help-link and alert configuration routes or forms

## Integration

- exercise the first-boot wizard on a lab supervisor
- verify generated runtime files match the canonical settings model
- restore a test archive and confirm state is recovered only after validation

---

# Non-Goals

- new Milestone 5 API semantics
- new agent backup behavior
- full desktop environment features
- OS image update mechanics
- notification delivery implementation beyond configuration storage

---

# Related Documents

- [Milestone 6 implementation specification](6-implementation-spec.md)
- [ROADMAP.md](../../ROADMAP.md)
- [operations.md](../operations.md)
