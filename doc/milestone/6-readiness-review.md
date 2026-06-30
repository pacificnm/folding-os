# Milestone 6 Readiness Review

**Version:** 1.0

**Status:** Proposed

**Review date:** 2026-06-21

**Target milestone:** Milestone 6, FoldOps Upgrades

**Issues:** [#122](https://github.com/pacificnm/folding-os/issues/122),
[#126](https://github.com/pacificnm/folding-os/issues/126)

---

# Purpose

This document records the Milestone 6 FoldOps Upgrades readiness and
live-hardware validation status required by issues #122 and #126.

It reconciles shipped dashboard, settings, recovery, and operator-polish work
with [ADR-0031](../adr/0031-foldops-upgrades-dashboard-and-navigation.md),
[ADR-0032](../adr/0032-unified-settings-model-and-first-boot-configuration-wizard.md),
and [ADR-0033](../adr/0033-supervisor-recovery-restore-workflow-in-foldops-upgrades.md).
It captures the issue closure matrix, validation evidence collected to date,
and the remaining release-blocking work.

This review does **not** close Milestone 6. Several implementation and
validation items remain open.

---

# Completion Status

```text
Milestone 6 ADR acceptance:                 COMPLETE  (issue #121, doc/milestone/6-adr-acceptance-review.md)
Milestone 6 operator-polish implementation:  PARTIAL   (issues #129-#137 largely closed)
Milestone 6 core implementation:           INCOMPLETE (issues #123-#125 open)
Milestone 6 live-hardware validation:        PARTIAL   (issue #126 open)
Milestone 6 readiness:                       NOT SATISFIED
```

Milestone 6 may close only when all release-blocking implementation issues are
closed with evidence, live-hardware validation for dashboard, settings, and
restore is complete, and this document is updated to **Approved** with an
overall **PASS** verdict.

---

# Governing Specifications

| Document | Role |
| --- | --- |
| [6-implementation-spec.md](6-implementation-spec.md) | Implementation scope and sequence |
| [6-engineering-spec.md](6-engineering-spec.md) | Dashboard routes, settings contract, restore workflow |
| [6-adr-acceptance-review.md](6-adr-acceptance-review.md) | ADR-0031 through ADR-0033 acceptance |
| [ADR-0031](../adr/0031-foldops-upgrades-dashboard-and-navigation.md) | Dashboard shell and navigation |
| [ADR-0032](../adr/0032-unified-settings-model-and-first-boot-configuration-wizard.md) | Canonical settings model and first-boot wizard |
| [ADR-0033](../adr/0033-supervisor-recovery-restore-workflow-in-foldops-upgrades.md) | Guided dashboard restore workflow |

---

# Issue Closure Matrix

| Issue | Title | State | Primary evidence |
| --- | --- | --- | --- |
| #120 | Plan Milestone 6: FoldOps Upgrades | Closed | Milestone 6 specs and roadmap alignment |
| #121 | Accept Milestone 6 ADRs (0031-0033) for implementation | Closed | [6-adr-acceptance-review.md](6-adr-acceptance-review.md) |
| #122 | Finalize Milestone 6 documentation and readiness review | Closed | This document (initial partial review) |
| #123 | Implement supervisor recovery restore workflow in FoldOps Upgrades | Open | `foldingosctl recovery import` exists; supervisor exposes export only (`/api/recovery/export*`) |
| #124 | Implement FoldOps Upgrades dashboard shell and navigation | Open | Partial: `AdminLayout`, admin nav, breadcrumbs (#129), footer (#132) |
| #125 | Implement unified settings model and first-boot configuration wizard | Open | Alert settings shipped (#137); canonical settings wizard not present |
| #126 | Validate Milestone 6 FoldOps Upgrades on live hardware | Open | Partial validation recorded below |
| #127 | Fix admin machine controls that do not behave as expected | Open | — |
| #128 | Collect and persist full host hardware profile for admin machine details | Open | Packages path shipped: `inspect hardware`, ingest, DB, admin Hardware tab; DMI platform fields blocked on running agents until OS image with `CONFIG_DMI*` — see [hardware-profile-rollout.md](../hardware-profile-rollout.md) |
| #129 | Implement breadcrumb navigation in FoldOps Upgrades | Closed | PR #145, `Breadcrumbs.tsx`, `adminBreadcrumbs.ts` |
| #130 | Persist completed work-unit history with start and stop times | Open | — |
| #131 | Create GitHub Wiki-backed help system and FoldingOS Manual | Open | Footer links to wiki; no in-app help surface |
| #132 | Improve FoldOps footer with site links and navigation | Closed | PR #144, `SiteFooter.tsx`, `siteLinks.ts` |
| #133 | Fix unknown status rendering in Admin Services | Closed | PR #143 |
| #134 | Restore project and Folding details in dashboard view | Closed | Issue closed; dashboard/project surfaces present in web UI |
| #135 | Clean up kiosk view header navigation | Closed | PR #142 |
| #136 | Restore CPU temp, project, PPD, and TPF telemetry in dashboard, kiosk, and details views | Closed | PR #141 |
| #137 | Add configuration system for Discord and email alerts | Closed | PR #140, `AdminAlertSettings.tsx` |

Release-blocking Milestone 6 issues remain open: #123, #124, #125, #126, #127,
#128, #130, and #131.

---

# Architecture And Implementation Reconciliation

Review confirms the shipped operator-polish work stays within the accepted
Milestone 6 boundaries:

- dashboard navigation, breadcrumbs, and footer remain thin web UI over existing
  supervisor HTTP APIs
- Milestone 5 software update and recovery export routes remain the execution
  boundary for fleet mutation
- alert configuration is exposed through the admin UI without changing the
  underlying supervisor alert engine contract

Unresolved gaps relative to the accepted Milestone 6 architecture:

- **Restore workflow (#123):** export and download are implemented; guided
  dashboard restore upload, validation preview, and confirmed import are not
- **Unified settings (#125):** Discord and email alert settings exist, but the
  canonical `/data/config/foldops/settings.toml` model and first-boot wizard are
  not shipped
- **Dashboard shell (#124):** admin section navigation exists, but the full
  FoldOps Upgrades shell described in the engineering spec is not complete

No architectural conflict was found between the shipped polish work and the
accepted ADRs. The remaining work is incomplete implementation, not an ADR
change.

---

# Live-Hardware Validation Evidence (Issue #126)

Validation host: lab supervisor **`folding-test`** (physical x86_64 appliance).

## Validated on live hardware

| Area | Result | Notes |
| --- | --- | --- |
| Farm dashboard and kiosk views | Pass | Operator telemetry restored (#136); kiosk header cleanup (#135) |
| Admin navigation shell | Pass | Admin section tabs, breadcrumbs (#129), footer (#132) |
| Admin services status | Pass | Unknown/inactive unit rendering fixed (#133) |
| Alert configuration | Pass | Discord and email settings reachable from admin UI (#137) |
| Milestone 5 software updates | Pass | Discover, assign, and apply flows exercised without regression |
| Milestone 5 recovery export | Pass | Backup export and browser download from `/admin/recovery` |

## Not yet validated on live hardware

| Area | Blocker | Required before #126 close |
| --- | --- | --- |
| First-boot / settings wizard | #125 not implemented | End-to-end settings entry without SSH |
| Recovery restore | #123 not implemented | Upload test archive, validate, confirm, restore, verify state |
| Admin machine controls | #127 open | Control actions match remote operator state |
| Host hardware profile | #128 partial | CPU/storage/network via packages channel validated; full DMI platform inventory requires agent OS image rollout (`roll-agent-os-update-lab`) |
| Work-unit history | #130 open | Completed WU records with start/stop timestamps |
| Help / manual | #131 open | Wiki-backed help entry beyond footer link |

## Automated checks run during this review

From the repository root on the validation workstation:

```bash
cd packages/foldops && cargo test --workspace
cd packages/foldops/web && npm run typecheck
cd packages/foldops/web && npm run build
cd packages/foldingosctl && cargo test
```

These commands confirm the current web UI and Rust workspace build cleanly.
They do not replace live-hardware restore or first-boot wizard validation.

---

# Milestone 5 Regression Checks

The following Milestone 5 operator paths were re-exercised on `folding-test`
during Milestone 6 validation and showed no regression:

- `/admin/software` update discovery and fleet assignment
- `/admin/software` local and fleet apply actions
- `/admin/recovery` backup export and download

Milestone 5 APIs and `foldingosctl` delegation behavior remain the execution
boundary. Milestone 6 UI polish did not introduce browser-side privileged
mutation paths.

---

# Conclusion

**Milestone 6 FoldOps Upgrades readiness: PARTIAL — not ready for milestone closeout**

**Issue #126 live-hardware validation: PARTIAL — remain open**

Operator-facing polish for navigation, telemetry, services status, alerts, and
footer/site links is validated on live hardware. Core Milestone 6 deliverables
for unified settings, dashboard restore, hardware profile, work-unit history,
help delivery, and full dashboard shell completion remain open.

Next release-blocking work:

1. Implement dashboard recovery restore (#123)
2. Implement unified settings model and first-boot wizard (#125)
3. Complete dashboard shell scope (#124)
4. Re-run live-hardware validation and update this document to **Approved**
   with an overall **PASS** verdict
5. Close issue #126 with a link to the approved readiness review

---

# Related Documents

- [Milestone 6 implementation specification](6-implementation-spec.md)
- [Milestone 6 engineering specification](6-engineering-spec.md)
- [Milestone 5 readiness review](5-readiness-review.md)
- [Physical validation](../physical-validation.md)
- [Operations](../operations.md)
