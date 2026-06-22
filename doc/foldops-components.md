# FoldOps Component Reference

**Status:** Living document for agents and developers

**Scope:** `packages/foldops/` — Rust workspace (`foldops-types`, `foldops-agent`,
`foldops-supervisor`) and React dashboard (`packages/foldops/web`).

This document maps implementation components to responsibilities, APIs, and UI
surfaces. It complements — but does not replace — approved specifications in
`/doc`, especially [foldops-integration.md](foldops-integration.md) and
[ADR-0027](adr/0027-foldops-remote-operator-api.md).

Operator-facing help and wiki content is tracked separately in issue #131.

---

# Architecture Overview

FoldOps on a FoldingOS appliance consists of two long-running Rust services plus
a static React dashboard. On FoldingOS, both services delegate node-local
behavior to `foldingosctl` subprocesses rather than reimplementing appliance
logic.

```text
Browser (HTTPS :3443 via foldops serve-https)
    │
    ▼
foldops-supervisor  ──SQLite──►  fleet DB (/data/foldops/foldops.db)
    │   /api/*
    ├── local foldingosctl  (fleet, registry, recovery, services)
    └── HTTP proxy ──► foldops-agent :9100  (per-node control, logs, config, software)
                           │
                           └── foldingosctl inspect / config / foldops / tools
```

| Process | Default bind | Role |
| --- | --- | --- |
| `foldops-supervisor` | `127.0.0.1:3000` (loopback) | Fleet DB, `/api/*`, static `WEB_ROOT` |
| `foldops serve-https` | `0.0.0.0:3443` | TLS front door; proxies to supervisor |
| `foldops-agent` | ingest loop + `0.0.0.0:9100` | Periodic ingest; node HTTP API |

**MCP indexing:** This file lives under `doc/` and is picked up by
`tools/index_memory.py`. Re-index after edits:

```bash
.venv/bin/python tools/index_memory.py
```

---

# Workspace Layout

```text
packages/foldops/
  Cargo.toml                 # workspace root
  crates/
    foldops-types/             # shared ingest + control contracts
    foldops-agent/             # node agent binary
    foldops-supervisor/        # supervisor binary
  web/                         # React dashboard (Vite)
  tests/contract/            # golden JSON fixtures for foldops-types
  packaging/appliance-bundle/
```

Build and publish commands: [packages/foldops/README.md](../packages/foldops/README.md).

---

# Crate: foldops-types

**Path:** `packages/foldops/crates/foldops-types/`

Shared API contracts between agent ingest and supervisor storage. Field names
align with the historical TypeScript schema in `packages/shared/src/schema.ts`.

| Module | Role |
| --- | --- |
| `ingest.rs` | `IngestPayload`, `Fah`, `System`, `Network`, nested telemetry types |
| `control.rs` | `ControlAction`, `CONTROL_ACTIONS`, `is_control_action()` |
| `validate.rs` | `validate_ingest_payload()`, JSON parse helpers |

**Inputs:** JSON from agent ingest or tests.

**Outputs:** Validated structs consumed by supervisor `db.rs` and web `types.ts`.

**Tests:** `cargo test -p foldops-types`; contract fixtures in
`packages/foldops/tests/contract/`.

**Debug:** Ingest validation failures surface as `400` from `POST /api/ingest` with
error detail in the response body.

---

# Crate: foldops-agent

**Path:** `packages/foldops/crates/foldops-agent/`

**Binary entry:** `src/main.rs` — starts HTTP server task, then loops
`ingest.collect_and_post()` on `INTERVAL_MS` (default 60s).

## Module map

| Module | Role | Key env / config |
| --- | --- | --- |
| `config.rs` | Env loading, feature flags | `AGENT_TOKEN`, `SUPERVISOR_URL`, `SUPERVISOR_TLS_CA`, `CONTROLS_ENABLED`, `CONFIG_ENABLED`, `FAH_*` |
| `ingest.rs` | POST snapshots to supervisor | Delegates collection to `foldingos.rs` on FoldingOS |
| `collector.rs` | Legacy/non-FoldingOS ingest path | Direct FAH log/db/ws + sysinfo metrics |
| `foldingos.rs` | `foldingosctl inspect` delegation | `inspect node`, config activate, software acquire |
| `http/server.rs` | Agent HTTP API (`:9100`) | Bearer `AGENT_TOKEN` middleware |
| `node_control.rs` | systemd + FAH WebSocket controls | `CONTROLS_ENABLED`, `CONTROLS_ALLOW_REBOOT` |
| `fah/*` | FAH telemetry sources | log tail, client.db, WebSocket :7396 |
| `update.rs` | Agent self-update via script | `UPDATE_ENABLED`, `UPDATE_SCRIPT` |
| `log_tail.rs` | Tail log files for ingest/logs API | |
| `temperatures.rs` | CPU/chassis temp for ingest | |

## Agent HTTP routes

All routes require `Authorization: Bearer <AGENT_TOKEN>`.

| Route | Handler | Purpose |
| --- | --- | --- |
| `GET /logs/fah` | `logs_fah` | FAH client log tail |
| `GET /logs/work` | `logs_work` | Work-unit log tail |
| `GET /control/status` | `control_status` | systemd + FAH folding state |
| `POST /control` | `control_action` | Remote service control |
| `POST /config/foldinghome` | `foldinghome_config` | Write candidate + `config activate` |
| `GET /inspect/foldops` | `inspect_foldops` | `foldingosctl inspect foldops` |
| `GET /inspect/tools` | `inspect_tools` | `foldingosctl inspect tools` |
| `POST /software/foldops-acquire` | `software_foldops_acquire` | `foldingosctl foldops acquire` |
| `POST /software/tools-acquire` | `software_tools_acquire` | `foldingosctl tools acquire` |
| `POST /update` | `update_agent` | Run update script |

**Related UI:** `MachineControlsPanel`, `MachineLogsPanel`, `AdminFoldingHome`
(config push), software update flows via supervisor proxy.

**Debug starting points:**

- `journalctl -u foldingos-foldops-agent.service`
- Confirm `CONTROLS_ENABLED` / `CONFIG_ENABLED` in agent env
- `curl -H "Authorization: Bearer …" http://127.0.0.1:9100/control/status`

---

# Crate: foldops-supervisor

**Path:** `packages/foldops/crates/foldops-supervisor/`

**Binary entry:** `src/main.rs` — mounts `/api` router, serves `WEB_ROOT`, spawns
alert evaluation loop.

## Module map

| Module | Role |
| --- | --- |
| `api/routes.rs` | All `/api/*` HTTP handlers |
| `config.rs` | Env + settings-backed alert config |
| `db.rs` | SQLite machines, snapshots, alert state |
| `foldingos.rs` | Local `foldingosctl --format json` subprocess delegation |
| `agent/control.rs` | Proxy control to agent HTTP |
| `agent/logs.rs` | Proxy logs to agent HTTP |
| `agent/config.rs` | Folding@home config push orchestration |
| `agent/inspect.rs` | Proxy agent inspect for software views |
| `agent/software.rs` | Fleet software apply to agents |
| `agent/update.rs` | Agent update delegation |
| `software/*` | Upstream index, local apply, fleet apply |
| `recovery/mod.rs` | Backup export via `foldingosctl recovery export` |
| `services/mod.rs` | Managed systemd services via `inspect services` |
| `settings/*` | Canonical alert settings TOML (`/data/config/foldops/settings.toml`) |
| `alerts/*` | Evaluation engine, Discord webhook, email config storage |
| `deploy/*` | Agent deploy job tracking (optional `DEPLOY_ENABLED`) |
| `fah_projects.rs` | Proxy FAH project API for dashboard |
| `supervisor_logs.rs` | Tail foldops / foldingosctl logs |
| `install_log.rs` | Software install log aggregation |

## Supervisor config flags

| Env / feature | Effect |
| --- | --- |
| `INGEST_TOKEN` | Required; auth for ingest + browser API |
| `CONTROL_ENABLED` | Gates `/api/machines/{name}/control*` (installation role file) |
| `CONFIG_ENABLED` | Gates Folding@home config push |
| `DEPLOY_ENABLED` | Agent deploy routes |
| `DB_PATH` | SQLite path (default `./data/foldops.db`) |
| `WEB_ROOT` | Static dashboard files |
| `AGENT_HTTP_PORT` | Target port for agent proxy (default `9100`) |
| Settings store | Alert Discord/email config via `GET/PUT /api/settings/alerts` |

Fleet delegation to `foldingosctl` is enabled when
`/data/config/installation-role` exists and role is `supervisor`
(`uses_supervisor_fleet_delegation()`).

## Supervisor HTTP routes

Routes are registered in `api/routes.rs` under `/api` (browser calls `/api/...`
through HTTPS front door).

| Route | Method | Backend | UI consumers |
| --- | --- | --- | --- |
| `/ingest` | POST | `db` ingest | agent only |
| `/machines` | GET | `db` | Dashboard, AdminFoldingHome |
| `/machines/{name}` | GET | `db` | MachineDetail, AdminFoldingMachineDetail |
| `/machines/{name}/logs` | GET | agent proxy | MachineLogsPanel, AdminLogs |
| `/machines/{name}/control/status` | GET | agent proxy | MachineControlsPanel |
| `/machines/{name}/control` | POST | agent proxy | MachineControlsPanel |
| `/machines/{name}/config/foldinghome` | POST | agent config | AdminFoldingHome |
| `/snapshots/{name}` | GET | `db` | History charts |
| `/projects/{id}` | GET | FAH API cache | ProjectInfoPanel |
| `/fleet/enrollments` | GET | `foldingosctl provision list-enrollments` | (API) |
| `/fleet/allow-boot` | GET/POST/DELETE | `foldingosctl provision *-boot` | AdminMachines |
| `/fleet/registry` | GET | `foldingosctl registry list` | AdminSoftwareUpdates |
| `/fleet/registry/{version}` | GET | `foldingosctl registry show` | AdminSoftwareUpdates |
| `/fleet/assign` | POST | `foldingosctl provision assign` | AdminSoftwareUpdates |
| `/fleet/software/apply-foldops` | POST | fleet + agent acquire | AdminSoftwareUpdates |
| `/fleet/software/apply-tools` | POST | fleet + agent acquire | AdminSoftwareUpdates |
| `/software/updates` | GET | upstream index + DB | AdminSoftwareUpdates |
| `/software/install-log` | GET | `install_log` | AdminSoftwareUpdates |
| `/software/apply-local` | POST | local acquire | AdminSoftwareUpdates |
| `/recovery/export` | POST | `foldingosctl recovery export` | AdminRecovery |
| `/recovery/export/latest` | GET | backup file download | AdminRecovery |
| `/services` | GET | `foldingosctl inspect services` | AdminServices |
| `/services/restart` | POST | `foldingosctl services restart` | AdminServices |
| `/services/restart-all` | POST | `foldingosctl services restart-all` | AdminServices |
| `/settings/alerts` | GET/PUT | settings TOML | AdminAlertSettings |
| `/alerts`, `/alerts/status`, `/alerts/history`, `/alerts/test` | various | alerts engine | AlertBanner, AlertHistory |
| `/supervisor/logs` | GET | log tail | AdminLogs |
| `/deploy/*` | various | deploy jobs | (optional) |

**Auth:** Browser and agents use `Authorization: Bearer <INGEST_TOKEN>` unless
noted otherwise in deployment docs.

---

# React Dashboard

**Path:** `packages/foldops/web/`

Built with Vite + React 19 + React Router 7. Static assets are copied into the
FoldOps layout bundle and served from `WEB_ROOT` on the supervisor.

## Route map (`App.tsx`)

| Path | Page | Shell |
| --- | --- | --- |
| `/` | `KioskHome` | Kiosk header |
| `/dashboard` | `Dashboard` | `PageLayout` + breadcrumbs |
| `/admin` | `AdminIndex` | `AdminLayout` nav |
| `/admin/machines` | `AdminMachines` | Network install allowlist |
| `/admin/folding` | `AdminFoldingHome` | Fleet FAH config + node table |
| `/admin/folding/:machineId` | `AdminFoldingMachineDetail` | Tabs: overview, hardware, work, logs, controls |
| `/admin/software` | `AdminSoftwareUpdates` | Milestone 5 update UI |
| `/admin/services` | `AdminServices` | systemd service control |
| `/admin/logs` | `AdminLogs` | Supervisor + machine logs |
| `/admin/recovery` | `AdminRecovery` | Backup export/download |
| `/admin/settings/alerts` | `AdminAlertSettings` | Discord + email settings |
| `/alerts` | `AlertHistory` | Alert history + test |
| `/machine/:hostname` | `MachineDetail` | Operator machine view (non-admin) |

Admin navigation labels: `adminBreadcrumbs.ts` → `ADMIN_SECTIONS`.

## Pages

| Page | Role | Primary APIs |
| --- | --- | --- |
| `KioskHome` | Compact farm kiosk | `fetchMachines` |
| `Dashboard` | Full farm dashboard | `fetchMachines`, `fetchAlerts` |
| `MachineDetail` | Single-node operator view | `fetchMachine`, `fetchSnapshots`, `fetchFahProject` |
| `AlertHistory` | Alert history + test | `fetchAlertHistory`, `sendAlertTest` |
| `AdminIndex` | Admin landing | — |
| `AdminMachines` | PXE allowlist CRUD | `fetchAllowBootDevices`, add/remove |
| `AdminFoldingHome` | Bulk FAH config push | `fetchMachines`, `pushFoldinghomeConfig` |
| `AdminFoldingMachineDetail` | Deep node view | `fetchMachine`, `fetchSnapshots`, controls/logs tabs |
| `AdminSoftwareUpdates` | Fleet software updates | software + fleet APIs |
| `AdminServices` | Service restart | `fetchServices`, restart endpoints |
| `AdminLogs` | Log viewer | `fetchSupervisorLogs`, `fetchMachineLogs` |
| `AdminRecovery` | Backup workflow | recovery export APIs |
| `AdminAlertSettings` | Alert configuration | `fetchAlertSettings`, `saveAlertSettings` |

## Shared components

| Component | Role | Used by |
| --- | --- | --- |
| `PageLayout` | Header, breadcrumbs, footer shell | Dashboard, admin, detail pages |
| `Breadcrumbs` | Breadcrumb trail | via `PageLayout` |
| `SiteFooter` | Site + admin links | `PageLayout` |
| `AlertBanner` | Active alerts strip | `Dashboard` |
| `MachineCard` | Dashboard machine tile | `Dashboard` |
| `CompactAgentTile` | Kiosk compact tile | `KioskHome` |
| `MachineControlsPanel` | Remote control UI | `AdminFoldingMachineDetail` |
| `MachineLogsPanel` | Live log viewer | `AdminFoldingMachineDetail` |
| `ProjectInfoPanel` | FAH project metadata | detail pages |
| `HistoryChart` | Snapshot time series | detail pages |
| `LogViewer` | Scrollable log lines | `AdminLogs`, `MachineLogsPanel` |
| `FahStatsLinks` | External stats URLs | detail pages |
| `Tabs` | Tabbed sections | `AdminFoldingMachineDetail` |

## Client modules (`web/src/`)

| Module | Role |
| --- | --- |
| `api.ts` | All `fetch('/api/...')` wrappers |
| `types.ts` | TypeScript mirrors of API shapes |
| `fahActivity.ts` | Derive folding activity label from ingest |
| `fahConfig.ts` | Display configured donor/team/token/cpus |
| `fahPasskey.ts` | Passkey normalization for config push |
| `fahProject.ts` | FAH project API normalization |
| `fahStats.ts` | Stats URL helpers |
| `fahTelemetry.ts` | Unified CPU temp / PPD / TPF from machine |
| `machineControlUi.ts` | Control button disable + optimistic status |
| `history.ts` | Snapshot → chart points |
| `adminBreadcrumbs.ts` | Admin nav + breadcrumb builders |
| `siteLinks.ts` | Footer external links |
| `utils/format.ts` | PPD, temp, uptime formatting |
| `utils/alerts.ts` | Alert display helpers |

**API client:** All dashboard traffic goes to relative `/api/*` on the supervisor
HTTPS origin. See `api.ts` for the complete function list.

## UI conventions (admin)

- **Loading:** `loading` state + `admin-muted` placeholder text
- **Busy:** `busy` disables forms/buttons during mutations
- **Success/error:** `message admin-status` / `message error` paragraphs
- **Tables:** `deploy-table admin-table` with `mono` for hostnames
- **Controls:** `machine-controls-btn` family; danger variant for destructive actions
- **Polling:** 15–60s `setInterval` refresh on several admin lists

---

# Cross-Cutting Maps

## Supervisor ↔ agent proxy

| Supervisor route | Agent route |
| --- | --- |
| `GET …/logs?source=fah` | `GET /logs/fah` |
| `GET …/logs?source=work` | `GET /logs/work` |
| `GET …/control/status` | `GET /control/status` |
| `POST …/control` | `POST /control` |
| `POST …/config/foldinghome` | `POST /config/foldinghome` |
| fleet software apply | `POST /software/foldops-acquire` or `/software/tools-acquire` |

Supervisor returns `502`/`503` when agent is offline or proxy fails.

## Milestone 5 boundaries

Software update and recovery **execution** stays in `foldingosctl` via supervisor
delegation. The dashboard only orchestrates; it does not mutate OS state directly.

## Contract tests

`packages/foldops/tests/contract/` — golden JSON for ingest validation:

```bash
cargo test -p foldops-types --test contract
```

---

# Debug Playbook

| Symptom | First checks |
| --- | --- |
| All nodes offline | Agent `SUPERVISOR_URL`, TLS CA, supervisor running, ingest token |
| Control buttons disabled | `CONTROL_ENABLED` + `CONTROLS_ENABLED`, node online, `control/status` |
| Services show wrong status | `foldingosctl inspect services`; see issue #133 fix in `services/mod.rs` |
| Software update stuck | `AdminSoftwareUpdates` install log; `journalctl` on supervisor/agent |
| Recovery export fails | Fleet delegation enabled; `foldingosctl recovery export` manually |
| Empty dashboard | `WEB_ROOT` path; rebuild `npm run build` in `web/` |
| Ingest missing FAH metrics | Agent logs; `client.db` permissions; FAH WebSocket :7396 |

**Verification commands:**

```bash
cd packages/foldops && cargo test --workspace
cd packages/foldops/web && npm run typecheck && npm run build
```

---

# File Inventory

## Rust — foldops-agent (`src/`)

`collector.rs`, `config.rs`, `fah/` (client_db, control, log, mod, state, status,
websocket, work_log), `foldingos.rs`, `http/` (mod, server), `ingest.rs`,
`log_tail.rs`, `main.rs`, `node_control.rs`, `temperatures.rs`, `update.rs`

## Rust — foldops-supervisor (`src/`)

`agent/` (config, control, inspect, logs, mod, software, update), `alerts/`
(db, engine, evaluate, mod, notify, stuck, types), `api/` (mod, routes),
`config.rs`, `db.rs`, `deploy/` (db, job, mod), `fah_projects.rs`,
`foldingos.rs`, `install_log.rs`, `main.rs`, `recovery/mod.rs`, `services/mod.rs`,
`settings/` (mod, types), `software/` (apply, assign_local, mod, upstream),
`supervisor_logs.rs`

## React — pages

`Dashboard.tsx`, `KioskHome.tsx`, `MachineDetail.tsx`, `AlertHistory.tsx`,
`admin/AdminAlertSettings.tsx`, `AdminFoldingHome.tsx`,
`AdminFoldingMachineDetail.tsx`, `AdminIndex.tsx`, `AdminLayout.tsx`,
`AdminLogs.tsx`, `AdminMachines.tsx`, `AdminRecovery.tsx`, `AdminServices.tsx`,
`AdminSoftwareUpdates.tsx`

## React — components

`AlertBanner.tsx`, `Breadcrumbs.tsx`, `CompactAgentTile.tsx`, `FahStatsLinks.tsx`,
`HistoryChart.tsx`, `LogViewer.tsx`, `MachineCard.tsx`, `MachineControlsPanel.tsx`,
`MachineLogsPanel.tsx`, `PageLayout.tsx`, `ProjectInfoPanel.tsx`, `SiteFooter.tsx`,
`Tabs.tsx`, `FahClientStatus.ts`

---

# Related Documents

- [packages/foldops/README.md](../packages/foldops/README.md) — build and runtime
- [foldops-integration.md](foldops-integration.md) — integration architecture
- [foldingosctl.md](foldingosctl.md) — CLI delegation targets (command reference)
- [agent-subsystems.md](agent-subsystems.md) — subsystem navigation
- [operations.md](operations.md) — operator procedures
- [tools/MCP-SETUP.md](../tools/MCP-SETUP.md) — project memory indexing
