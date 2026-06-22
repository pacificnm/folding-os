# foldingosctl Component Reference

**Status:** Living document for agents and developers

**Scope:** `packages/foldingosctl/src/` — the on-appliance control-plane Rust binary
(excludes `VENDOR/`).

This document maps implementation modules to command groups, privilege boundaries,
persistent paths, and FoldOps delegation. It complements — but does not replace —
the operator command reference in [foldingosctl.md](foldingosctl.md).

For FoldOps application internals (supervisor, agent, dashboard), see
[foldops-components.md](foldops-components.md).

---

# Architecture Overview

`foldingosctl` is installed at `/usr/bin/foldingosctl` (setuid root on appliances).
It drops to the invoking user and re-elevates only for policy-approved privileged
commands per [ADR-0024](adr/0024-foldops-supervisor-fleet-mutation-authorization.md).

```text
Operator / systemd / FoldOps subprocess
    │
    ▼
main.rs → cli.rs (parse --format json, privilege guard)
    │
    ├── automation.rs        JSON success/failure envelopes
    ├── automation_policy.rs foldops user command allowlists
    ├── setuid_privilege.rs  re-elevate for approved mutators
    │
    └── command group modules (boot, config, fah, foldops, …)
            │
            ├── read/write /data/* state
            ├── spawn systemd units / long-running services
            └── subprocess helpers (process.rs)
```

**JSON automation:** Most fleet-facing commands support `--format json` and return
`{ schema_version, ok, command, data | error }` per
[ADR-0021](adr/0021-machine-readable-foldingosctl-automation-interface.md).

**MCP indexing:** This file is under `doc/` and indexed by `tools/index_memory.py`:

```bash
.venv/bin/python tools/index_memory.py
```

---

# Dispatch Layer

| Module | Role |
| --- | --- |
| `main.rs` | Initialize setuid drop; call `cli::dispatch` |
| `cli.rs` | Route top-level groups; JSON vs human output; `USAGE` string |
| `automation.rs` | `OutputFormat`, `strip_format_flag`, `publish_success` / `publish_failure` |
| `automation_policy.rs` | Merge bundled + `/data` automation TOML; authorize `foldops` user |
| `setuid_privilege.rs` | Per-command privilege guard before dispatch |
| `paths.rs` | `AppliancePaths` — canonical `/data` and `/etc` path constants |
| `role.rs` | Read `installation-role`; `require_supervisor_role` / `require_agent_role` |
| `process.rs` | `command_output`, `command_stdout`, deferred systemd restart helpers |
| `fs_atomic.rs` | Atomic file replace utilities |
| `software_install_log.rs` | Append structured install/automation outcomes |
| `assignments.rs` | Sync software assignments to enrolled agents |
| `enrollment.rs` | Enrollment record helpers shared with provision |

## Automation policy files

| Path | Role |
| --- | --- |
| `/usr/share/foldingos/foldops-supervisor-automation.toml` | Bundled supervisor policy |
| `/usr/share/foldingos/foldops-agent-automation.toml` | Bundled agent policy |
| `/data/config/foldingos/automation-policy.toml` | System-share overrides (merged) |

When the real UID is `foldops`, mutation commands must appear in the merged policy
for the active installation role.

## Privilege summary

| Invoker | Typical access |
| --- | --- |
| `root` / systemd | Full command surface |
| `foldingos-admin` | Operator commands; re-elevate for approved mutators |
| `foldops` | Read-only inspect + policy-listed mutations only |

FoldOps HTTP services subprocess `foldingosctl`; they never call `sudo` directly.

---

# Command Group Index

Legend: **JSON** = supports `--format json` via `cli.rs` automation envelope.
**Role** = `supervisor`, `agent`, or `both` when installation role is required.

## `boot`

| Subcommand | Module | Role | JSON | Side effects |
| --- | --- | --- | --- | --- |
| `status` | `boot_cmd.rs` | both | — | Read commissioning state; console/TUI output |
| `refresh` | `boot_cmd.rs` | both | — | Refresh boot status display |

## `config`

| Subcommand | Module | Role | JSON | Side effects |
| --- | --- | --- | --- | --- |
| `validate` | `config/parse.rs` | both | yes | Validate TOML domains |
| `effective` | `config/effective.rs` | both | yes / human | Render effective config |
| `activate` | `config/apply.rs` | both | yes | Apply candidate; may restart services |

**FoldOps:** `foldops-agent` calls `config activate` after remote FAH config push.

## `identity`

| Subcommand | Module | Role | JSON | Side effects |
| --- | --- | --- | --- | --- |
| `ensure` | `identity.rs` | both | — | Ensure node-id and identity files |

## `storage`

| Subcommand | Module | Role | JSON | Side effects |
| --- | --- | --- | --- | --- |
| `expand-data` | `storage.rs` | both | — | Grow `/data` ext4 partition |

## `inspect` (always JSON when `--format json`)

| Subcommand | Module | Role | Primary JSON consumers |
| --- | --- | --- | --- |
| `node` | `identity` + paths | both | Agent ingest → `nodeId`, `installationRole`, hostname |
| `system` | `inspect/system.rs` | both | Agent ingest → CPU, memory, disk, network |
| `fah` | `inspect/fah.rs` | both | Agent ingest → FAH telemetry, client state |
| `commissioning` | `inspect/commissioning.rs` | both | Diagnostics; boot readiness |
| `update` | `inspect/update.rs` | both | Agent ingest → `maintenance.rebootRequired` |
| `foldops` | `inspect/foldops.rs` | both | Supervisor software UI; agent `/inspect/foldops` |
| `tools` | `inspect/tools.rs` | both | Supervisor software UI; agent `/inspect/tools` |
| `services` | `inspect/services.rs` → `services/mod.rs` | both | Supervisor `GET /api/services` |

`inspect::run` requires inspectable role (`agent` or `supervisor`, or `foldops` user).

### Inspect → FoldOps ingest mapping

On FoldingOS agents, `foldops-agent` `foldingos.rs` runs:

```text
foldingosctl inspect node --format json    → IngestPayload identity fields
foldingosctl inspect system --format json  → IngestPayload.system
foldingosctl inspect fah --format json     → IngestPayload.fah
foldingosctl inspect update --format json  → IngestPayload.maintenance
```

`commissioning`, `foldops`, and `tools` inspect failures are logged but do not
block ingest when partial data is available.

## `provision`

| Subcommand | Module | Role | JSON | Notes |
| --- | --- | --- | --- | --- |
| `list-enrollments` | `provision/assign.rs` | supervisor | yes | Fleet enrollment store |
| `assign` | `provision/assign.rs` | supervisor | yes | Desired image version |
| `assign-local` | `provision/assign.rs` | supervisor | yes | Local supervisor assignments |
| `list-allow-boot` | `provision/boot.rs` | supervisor | yes | PXE allowlist |
| `allow-boot` | `provision/boot.rs` | supervisor | yes | Add MAC (+ optional disk) |
| `deny-boot` | `provision/boot.rs` | supervisor | yes | Remove MAC |
| `sync-software-assignments` | `assignments.rs` | supervisor | yes | Push FoldOps/tools versions |
| `ssh` | `provision/ssh.rs` | both | — | SSH host keys |
| `role` | `provision/role_cmd.rs` | both | — | Persist installation role |
| `serve` | `provision/serve.rs` | supervisor | — | Long-running provisioning API |
| `boot` | `provision/network_boot.rs` | supervisor | — | PXE/TFTP/HTTP boot assistance |
| `install` | `provision/install.rs` | — | — | Network install initramfs hook |
| `enroll` | `provision/enroll.rs` | agent | — | Register with supervisor |
| `check-version` | `provision/update.rs` | agent | — | Stage OS update |
| `apply-update` | `provision/update.rs` | agent | — | Apply staged image + reboot |
| `report-update-status` | `provision/update.rs` | agent | — | Report apply outcome |

**Key paths:** `/data/provision/enrollments/`, `/data/config/provision/boot-allowlist`,
`/data/registry/`, staged update under `/data/state/`.

**FoldOps supervisor APIs:** `/api/fleet/enrollments`, `/api/fleet/allow-boot`,
`/api/fleet/assign` delegate to provision JSON commands.

## `registry`

| Subcommand | Module | Role | JSON | Notes |
| --- | --- | --- | --- | --- |
| `list` | `registry_image.rs` | supervisor | yes | OS image registry index |
| `show` | `registry_image.rs` | supervisor | yes | Single version metadata |
| `import-bootstrap` | `registry_import.rs` | supervisor | — | Bootstrap registry from image |
| `poll` | `registry_poll.rs` | supervisor | — | Upstream release poll |
| `list-foldops-manifests` | `registry_foldops_tools.rs` | supervisor | — | FoldOps manifest registry |
| `list-tools-versions` | `registry_foldops_tools.rs` | supervisor | — | Tools version registry |
| `import-foldops-manifest` | `registry_foldops_tools.rs` | supervisor | — | Import packages channel manifest |
| `import-tools-release` | `registry_foldops_tools.rs` | supervisor | — | Import tools release |

**FoldOps:** `/api/fleet/registry` → `registry list` / `registry show`.

## `fah`

| Subcommand | Module | Role | Privilege | Side effects |
| --- | --- | --- | --- | --- |
| `validate-manifest` | `fah/manifest.rs` | both | — | Verify embedded manifest |
| `acquire` | `fah/acquire.rs` | both | re-elevate | Download, verify, stage client |
| `verify-install` | `fah/verify_install.rs` | both | — | Check staged version |
| `activate` | `fah/activate.rs` | both | re-elevate | Atomic `current` symlink |
| `prepare` | `fah/prepare.rs` | both | — | Render `config.xml`, reconcile CPUs |
| `run` | `fah/run_cmd.rs` | both | — | Start FAH client foreground (debug) |

**Paths:** `/data/apps/fah/`, `/data/fah/`, `/data/config/fah/`.

## `foldops`

| Subcommand | Module | Role | JSON | Side effects |
| --- | --- | --- | --- | --- |
| `validate-manifest` | `foldops/manifest.rs` | both | — | Verify embedded FoldOps manifest |
| `acquire` | `foldops/acquire.rs` | both | yes | Download layout bundle, verify, stage |
| `provision` | `foldops/provision.rs` | supervisor | — | Write env files, TLS, fleet perms |
| `serve-https` | `foldops/serve_https.rs` | supervisor | — | Long-running TLS :3443 front door |

**Paths:** `/data/apps/foldops/`, `/data/config/foldops/`, `/data/foldops/tls/`.

**FoldOps:** Supervisor `software/apply-local` and agent `POST /software/foldops-acquire`
invoke `foldops acquire --format json`.

## `tools`

| Subcommand | Module | Role | JSON | Side effects |
| --- | --- | --- | --- | --- |
| `acquire` | `tools/mod.rs`, `download.rs`, `replace.rs` | both | yes | Self-update `foldingosctl` binary |

**FoldOps:** Agent `POST /software/tools-acquire`; supervisor fleet tools apply.

## `recovery`

| Subcommand | Module | Role | JSON | Side effects |
| --- | --- | --- | --- | --- |
| `export` | `recovery/export.rs` | supervisor | yes | Tar backup to `/data/foldops/backups/` |
| `import` | `recovery/import.rs` | supervisor | yes | Validate + restore archive (`--dry-run` supported) |

**FoldOps:** `/api/recovery/export` → `recovery export --format json`.

## `services`

| Subcommand | Module | Role | JSON | Side effects |
| --- | --- | --- | --- | --- |
| `restart` | `services/mod.rs` | both | yes | Restart one managed unit |
| `restart-all` | `services/mod.rs` | both | yes | Restart runtime fleet (deferred HTTPS) |

`inspect services` lists managed units with systemd status (see `inspect` group).

**FoldOps:** `/api/services*` delegates to `inspect services` and `services restart*`.

---

# FoldOps Delegation Map

Commands invoked by FoldOps services (subprocess, `--format json` unless noted):

| FoldOps caller | foldingosctl command |
| --- | --- |
| Supervisor fleet APIs | `provision list-enrollments`, `list-allow-boot`, `allow-boot`, `deny-boot`, `assign`, `assign-local` |
| Supervisor registry APIs | `registry list`, `registry show` |
| Supervisor software | `inspect foldops`, `inspect tools`, `registry import-*`, `foldops acquire`, `tools acquire` |
| Supervisor recovery | `recovery export` |
| Supervisor services | `inspect services`, `services restart`, `services restart-all` |
| Agent ingest loop | `inspect node`, `inspect system`, `inspect fah`, `inspect update` |
| Agent HTTP config | `config activate` (foldinghome domain) |
| Agent HTTP software | `foldops acquire`, `tools acquire` |
| Agent HTTP inspect | `inspect foldops`, `inspect tools` |

FoldOps must not parse enrollment, registry, or allowlist files directly on FoldingOS.

---

# Key Persistent Paths

`AppliancePaths` in `paths.rs` centralizes locations. High-signal paths:

| Path | Owner module(s) |
| --- | --- |
| `/data/config/installation-role` | `role.rs`, all role gates |
| `/data/config/foldops/` | `foldops/provision.rs`, settings |
| `/data/apps/foldops/current` | `foldops/acquire.rs`, `activate.rs` |
| `/data/apps/fah/current` | `fah/activate.rs` |
| `/data/provision/enrollments/` | `provision/assign.rs` |
| `/data/config/provision/boot-allowlist` | `provision/boot.rs` |
| `/data/registry/` | `registry_image.rs`, `registry_poll.rs` |
| `/data/foldops/foldops.db` | Used by FoldOps supervisor (not foldingosctl) |
| `/data/foldops/backups/` | `recovery/export.rs` |
| `/data/state/foldops/` | provision markers, acquire retry state |

Full path table: see [foldingosctl.md § Key Paths](foldingosctl.md#key-paths).

---

# Debug Playbook

| Symptom | First checks |
| --- | --- |
| FoldOps fleet API 502 | Run underlying `foldingosctl … --format json` on supervisor; check role file |
| `automation denied` | Compare command to `foldops-*-automation.toml`; confirm invoking user |
| Agent ingest sparse | `foldingosctl inspect fah --format json`; journal for partial inspect failures |
| FAH acquire stuck | `/data/state/fah/acquire-state.toml`; network/time sync |
| Provision serve won't start | TLS + ingest token provisioned; `foldops provision` completed |
| Services show `unknown` | `systemctl is-active` semantics — use `command_stdout` path in `services/mod.rs` |
| Recovery export empty | Supervisor role; disk space under `/data/foldops/backups/` |

**Verification:**

```bash
cd packages/foldingosctl && cargo test
./scripts/test-api-json --foldingosctl packages/foldingosctl/target/debug/foldingosctl
./scripts/build-foldingosctl-release --version <ver> --sync-overlay
```

---

# File Inventory (`src/`, exclude `VENDOR/`)

## Dispatch / infrastructure

`main.rs`, `cli.rs`, `automation.rs`, `automation_policy.rs`, `setuid_privilege.rs`,
`paths.rs`, `role.rs`, `process.rs`, `fs_atomic.rs`, `assignments.rs`,
`software_install_log.rs`, `enrollment.rs`, `config_host.rs`

## Command groups

- **boot:** `boot_cmd.rs`
- **storage:** `storage.rs`
- **identity:** `identity.rs`
- **config:** `config_cmd.rs`, `config/mod.rs`, `config/parse.rs`, `config/apply.rs`, `config/effective.rs`
- **fah:** `fah/mod.rs`, `acquire.rs`, `acquire_state.rs`, `activate.rs`, `extract.rs`, `manifest.rs`, `prepare.rs`, `passkey.rs`, `run_cmd.rs`, `verify_install.rs`, `util.rs`
- **foldops:** `foldops/mod.rs`, `acquire.rs`, `acquire_state.rs`, `activate.rs`, `extract.rs`, `manifest.rs`, `provision.rs`, `serve_https.rs`, `tls.rs`, `verify.rs`, `util.rs`, `supervisor_permissions.rs`, `foldops_manifest.rs`
- **inspect:** `inspect/mod.rs`, `commissioning.rs`, `fah.rs`, `foldops.rs`, `system.rs`, `tools.rs`, `update.rs`, `services.rs`
- **provision:** `provision/mod.rs`, `assign.rs`, `authorize.rs`, `boot.rs`, `enroll.rs`, `enrollment_api.rs`, `grub_env.rs`, `http_server.rs`, `install.rs`, `network_boot.rs`, `release_image.rs`, `role_cmd.rs`, `serve.rs`, `ssh.rs`, `staged_lock.rs`, `targets.rs`, `update.rs`, `util.rs`
- **recovery:** `recovery/mod.rs`, `bundle.rs`, `export.rs`, `import.rs`
- **registry:** `registry_cmd.rs`, `registry_import.rs`, `registry_poll.rs`, `registry_image.rs`, `registry_foldops_tools.rs`
- **services:** `services/mod.rs`
- **tools:** `tools/mod.rs`, `download.rs`, `replace.rs`, `acquire_state.rs`

---

# Related Documents

- [foldingosctl.md](foldingosctl.md) — operator command reference
- [foldops-components.md](foldops-components.md) — FoldOps Rust/React map
- [foldops-integration.md](foldops-integration.md) — integration architecture
- [agent-subsystems.md](agent-subsystems.md) — subsystem navigation
- [ADR-0020](adr/0020-foldops-delegates-node-operations-to-foldingosctl.md)
- [ADR-0021](adr/0021-machine-readable-foldingosctl-automation-interface.md)
- [ADR-0024](adr/0024-foldops-supervisor-fleet-mutation-authorization.md)
- [tools/MCP-SETUP.md](../tools/MCP-SETUP.md)
