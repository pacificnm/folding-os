# FoldingOS Milestone 5 Update And Recovery System Engineering Specification

**Version:** 1.0

**Status:** Proposed

**Target Milestone:** Milestone 5, Update and Recovery System

---

# Purpose

This document defines the Milestone 5 contract for:

1. **Runtime fleet software updates** — FoldOps bundles and `foldingosctl` tools
   without OS reimage
2. **Publication automation** — build and rclone upload to `packages.folding-os.com`
3. **Supervisor recovery** — backup export and operator restore of fleet state

It implements:

- [ADR-0028](../adr/0028-supervisor-fleet-software-update-workflow.md)
- [ADR-0029](../adr/0029-packages-channel-publication-via-rclone.md)
- [ADR-0030](../adr/0030-supervisor-recovery-backup-and-export.md)

and builds on acquisition and assignment from
[ADR-0023](../adr/0023-runtime-foldops-and-foldingosctl-updates-without-os-reimage.md).

FoldOps dashboard rework is **out of scope** for this milestone except a minimal
supervisor **admin section** for update and recovery actions.

---

# Relationship To Milestone 4

| Concern | Milestone 4 delivered | Milestone 5 adds |
| --- | --- | --- |
| Assigned FoldOps manifest | `provision assign --foldops-manifest` | discover + apply workflow in UI/API |
| Assigned tools version | `provision assign --tools-version` | discover + apply workflow in UI/API |
| Node acquisition | `foldops acquire`, `tools acquire` CLI | operator-triggered apply via HTTP |
| Bundle build | `build-foldops-bundles` | publish + index automation |
| Tools build | `build-foldingosctl-release` | publish script + index |
| Supervisor fleet API | `/api/fleet/assign` | `/api/software/*`, `/api/fleet/software/*` |
| Recovery | direct flash / SSH | export/import supervisor state |

OS **disk image** updates remain unchanged from Milestone 3
([update-system.md](../update-system.md)).

---

# Packages Channel Catalog

## Index schema (schema_version = 1)

Each channel publishes `index.json` at its root.

### FoldOps index

Path: `https://packages.folding-os.com/foldops/index.json`

```json
{
  "schema_version": 1,
  "channel": "foldops",
  "releases": [
    {
      "manifest_release": "0.1.0-1",
      "published_at": "2026-06-18T12:00:00Z",
      "manifest_url": "https://packages.folding-os.com/foldops/0.1.0-1/manifest.toml",
      "minimum_foldingos_version": "0.1.0"
    }
  ]
}
```

Releases sort descending by `published_at`. **Latest** is the first entry whose
`minimum_foldingos_version` is satisfied by the querying node.

### Tools index

Path: `https://packages.folding-os.com/foldingos-tools/index.json`

```json
{
  "schema_version": 1,
  "channel": "foldingos-tools",
  "releases": [
    {
      "tools_version": "0.1.0",
      "published_at": "2026-06-18T12:00:00Z",
      "binary_url": "https://packages.folding-os.com/foldingos-tools/0.1.0/foldingosctl-x86_64",
      "sha256_url": "https://packages.folding-os.com/foldingos-tools/0.1.0/SHA256SUMS",
      "minimum_foldingos_version": "0.1.0"
    }
  ]
}
```

---

# Supervisor Software Update API

All routes live under `/api` on the supervisor HTTPS entry point per
[ADR-0027](../adr/0027-foldops-remote-operator-api.md).

## `GET /api/software/updates`

Checks upstream indexes and compares against local fleet state.

Query parameters:

| Param | Default | Description |
| --- | --- | --- |
| `refresh` | `false` | when `true`, bypass cached upstream index (max once per 60s) |

Response:

```json
{
  "checked_at": "2026-06-18T12:00:00Z",
  "upstream": {
    "foldops": {
      "latest_manifest_release": "0.1.0-2",
      "published_at": "2026-06-18T11:00:00Z"
    },
    "tools": {
      "latest_tools_version": "0.1.1",
      "published_at": "2026-06-18T10:30:00Z"
    }
  },
  "supervisor": {
    "hostname": "folding-supervisor",
    "active_foldops_manifest_release": "0.1.0-1",
    "assigned_foldops_manifest_release": "0.1.0-1",
    "active_tools_version": "0.1.0",
    "assigned_tools_version": "0.1.0",
    "foldops_update_available": true,
    "tools_update_available": true
  },
  "agents": [
    {
      "hostname": "folding-agent-01",
      "node_id": "550e8400-e29b-41d4-a716-446655440000",
      "online": true,
      "active_foldops_manifest_release": "0.1.0-1",
      "assigned_foldops_manifest_release": "0.1.0-2",
      "active_tools_version": "0.1.0",
      "assigned_tools_version": "0.1.0",
      "foldops_apply_pending": true,
      "tools_apply_pending": false
    }
  ]
}
```

Field sources:

- **active** values from latest ingest payload or live `inspect foldops` /
  `inspect tools` when online
- **assigned** values from enrollment records / assignment files
- **latest** values from upstream indexes

## Assignment (existing)

`POST /api/fleet/assign` remains the assignment entry point. Milestone 5 UI must
expose `foldops_manifest` and `tools_version` fields alongside `version` (OS
image).

## `POST /api/fleet/software/apply-foldops`

Request:

```json
{
  "hostnames": ["folding-agent-01"],
  "all": false
}
```

When `all` is true, apply to all **online** enrolled agents. Supervisor self is
included only when explicitly listed or when a dedicated local apply is used.

Response:

```json
{
  "results": [
    {
      "hostname": "folding-agent-01",
      "ok": true,
      "active_manifest_release": "0.1.0-2",
      "message": "foldops acquire completed"
    }
  ]
}
```

Proxies to agent `POST /software/foldops-acquire`.

## `POST /api/fleet/software/apply-tools`

Same shape as apply-foldops; proxies to agent `POST /software/tools-acquire`.

## `POST /api/software/apply-local`

Runs on the **supervisor role only** via `foldops-supervisor` subprocess:

1. `foldingosctl foldops acquire --format json` when FoldOps assignment differs
   from active or when `force` is true
2. `foldingosctl tools acquire --format json` under the same rules

Privileged steps execute inside setuid `/usr/bin/foldingosctl` per
[ADR-0024](../adr/0024-foldops-supervisor-fleet-mutation-authorization.md).
FoldOps does not invoke `sudo` or perform privileged OS operations directly.

Request:

```json
{
  "foldops": true,
  "tools": true,
  "force": false
}
```

---

# Agent Software Apply HTTP

Agent-local curated endpoints on `AGENT_HTTP_PORT` (default 9100):

## `POST /software/foldops-acquire`

- Auth: `Authorization: Bearer <AGENT_TOKEN>`
- Executes: `foldingosctl foldops acquire --format json`
- Restarts: `foldingos-foldops-agent.service` on success
- Does not restart `folding-at-home.service`

## `POST /software/tools-acquire`

- Executes: `foldingosctl tools acquire --format json`
- Restarts: FoldOps agent and other units per tools acquire policy
- Fail-closed on verification errors

Both endpoints honor appliance feature flags (enabled when installation role
exists unless explicitly disabled).

---

# foldingosctl Extensions

## Automation JSON

`foldops acquire` and `tools acquire` must return structured JSON with
`--format json` (if not already complete):

```json
{
  "schema_version": 1,
  "ok": true,
  "command": "foldops acquire",
  "data": {
    "manifest_release": "0.1.0-2",
    "activated": true,
    "packages": ["foldops-agent"]
  }
}
```

## `recovery export` / `recovery import`

Supervisor-role commands:

- `foldingosctl recovery export [--output /path/to/archive.tar.zst]`
- `foldingosctl recovery import <archive> [--dry-run]`

Implement bundle layout defined in ADR-0030.

Agent automation policy must authorize recovery commands only on supervisor role.

---

# Publication Scripts

| Script | Action |
| --- | --- |
| `scripts/build-foldops-bundles` | build bundles (existing) |
| `scripts/build-foldingosctl-release` | build tools binary; `--sync-overlay` pins next image bootstrap tools assignment (existing) |
| `scripts/publish-foldops-bundles <release>` | rclone upload FoldOps release (existing) |
| `scripts/publish-foldingos-tools <version>` | rclone upload tools release (**new**) |
| `scripts/publish-packages-release` | build (optional) + publish both + refresh indexes (**new**) |

Environment defaults documented in [ADR-0029](../adr/0029-packages-channel-publication-via-rclone.md).

Operator workflow:

```bash
./scripts/publish-packages-release --foldops 0.1.0-2 --tools 0.1.1 --build
```

Tools-only workflow:

```bash
./scripts/build-foldingosctl-release --version 0.1.1 --sync-overlay
./scripts/publish-foldingos-tools 0.1.1
```

Publishing tools does not require `./scripts/build`; `--sync-overlay` writes the
overlay `tools.json` pin so the next OS image build embeds the current tools
assignment.

Requires rclone remote configured at `~/.config/rclone/rclone.conf`.

---

# Recovery API

## `POST /api/recovery/export`

Creates export bundle on supervisor disk.

Request:

```json
{
  "include_secrets": false
}
```

Response:

```json
{
  "ok": true,
  "path": "/data/foldops/backups/foldingos-supervisor-backup-folding-supervisor-20260618T120000.tar.zst",
  "sha256": "<hex>",
  "size_bytes": 1234567,
  "download_url": "/api/recovery/export/latest"
}
```

## `GET /api/recovery/export/latest`

Streams the most recent export archive to authenticated operators.

---

# Admin UI (Minimum)

Milestone 5 adds a supervisor **admin section** (static pages served with the
existing dashboard or a thin `/admin` route) with:

| Screen | Actions |
| --- | --- |
| Software updates | Check for updates, show fleet table, assign versions, apply |
| Recovery | Create backup, download latest backup |

Styling and navigation integration with the full FoldOps dashboard defer to
FoldOps Upgrades. Functionality must call the HTTP APIs above, not invoke
`foldingosctl` from the browser.

First-boot settings (tokens, feature flags, upstream URLs) consolidate in
FoldOps Upgrades; Milestone 5 may read existing env files unchanged.

---

# Validation

## Live lab (preferred)

- publish a test FoldOps release and tools version to staging or production
  packages channel via rclone scripts
- supervisor “check for updates” detects new versions
- assign + apply updates supervisor and one agent without OS reimage
- FAH remains active through failed apply attempt
- export backup, download, restore on a test supervisor

## Automated

- unit tests for index parsing and update comparison logic
- HTTP handler tests with mock `foldingosctl` for apply and export paths
- script dry-run tests for publication helpers

---

# Non-Goals

- Full FoldOps dashboard rework (FoldOps Upgrades)
- OS image A/B rootfs slots (future milestone)
- Agent node backup
- Automatic scheduled off-site backup upload
- Runtime `apt` on appliances

---

# Related Documents

- [Milestone 5 implementation specification](5-implementation-spec.md)
- [ADR-0023](../adr/0023-runtime-foldops-and-foldingosctl-updates-without-os-reimage.md)
- [update-system.md](../update-system.md)
- [operations.md](../operations.md)
