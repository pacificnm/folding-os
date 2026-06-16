# FoldingOS Milestone 4 FoldOps Integration Engineering Specification

**Version:** 1.0

**Status:** Proposed

**Target Milestone:** Milestone 4, FoldOps Integration

---

# Purpose

This document defines the concrete Milestone 4 contract for integrating FoldOps
with FoldingOS appliances through `foldingosctl`.

It implements [ADR-0020](../adr/0020-foldops-delegates-node-operations-to-foldingosctl.md)
and [ADR-0021](../adr/0021-machine-readable-foldingosctl-automation-interface.md).

---

# Relationship To Milestone 3

| Concern | Milestone 3 owner | Milestone 4 owner |
| --- | --- | --- |
| Network install and agent OS enrollment | `foldingosctl provision enroll` → supervisor `/v1/agents/register` | unchanged |
| Desired image version and staged updates | `foldingosctl provision check-version`, `apply-update`, supervisor assign API | FoldOps dashboard invokes same commands locally on supervisor |
| FoldOps package install | `foldingosctl foldops acquire` | unchanged |
| FoldOps token/TLS bootstrap | `foldingosctl foldops provision` | unchanged |
| Metrics and fleet dashboard | FoldOps agent direct OS inspection (legacy) | FoldOps agent delegates to `foldingosctl inspect` |
| Machine inventory in FoldOps DB | hostname only | hostname + `node_id` + `installation_role` + FoldingOS versions |

Milestone 3 provisioning enrollment and Milestone 4 FoldOps ingest registration
serve different purposes and both remain required on agents.

---

# foldingosctl Automation Contract

## Global flags

All automation-enabled commands accept:

```text
--format json
```

JSON success document:

```json
{
  "schema_version": 1,
  "ok": true,
  "command": "inspect node",
  "data": {}
}
```

JSON failure document:

```json
{
  "schema_version": 1,
  "ok": false,
  "command": "inspect node",
  "error": {
    "code": "role_required",
    "message": "command requires supervisor role"
  }
}
```

Exit code remains non-zero on failure.

## New command group: `inspect`

Read-only commands invokable by the `foldops` user:

### `inspect node`

Returns:

- `node_id` from `/data/config/node-id`
- `hostname`
- `installation_role`
- `foldingos_version` from `/usr/lib/os-release`
- `kernel_version`
- primary wired IPv4 address when available
- `mac_addresses`

### `inspect system`

Returns:

- uptime seconds
- load averages
- memory total/used/free/percent
- root filesystem size/used/free/percent
- primary network interface counters
- CPU and chassis temperature when exposed by sysfs (nullable)

### `inspect fah`

Returns:

- verified active client version under `/data/apps/fah/current`
- `folding-at-home.service` active state
- runtime summary fields needed by FoldOps ingest (project, progress, PPD,
  recent errors) using the same sources as `foldingosctl fah prepare` /
  existing FAH data paths, not ad hoc log scraping in FoldOps

### `inspect commissioning`

Returns the readiness checks currently computed for the commissioning display
(identity, config validation, FAH path, FoldOps provisioned state, role-specific
services).

### `inspect update`

Returns:

- `current_image_version`
- `desired_image_version` from supervisor when reachable
- staged update metadata from `/data/state/provision/staged-update.json` when
  present
- last reported update status when present

## Existing commands with JSON output

| Command | Role | Milestone 4 JSON |
| --- | --- | --- |
| `provision list-enrollments` | supervisor | required |
| `provision assign --version … --all\|--node …` | supervisor | result summary |
| `registry list` | supervisor | required |
| `registry show <version>` | supervisor | required |
| `config effective <domain>` | any | required for `foldinghome`, `system`, `network` |
| `config validate --all` | any | required |
| `fah validate-manifest` | any | optional diagnostics |
| `foldops validate-manifest` | any | optional diagnostics |

Commands **not** exposed to FoldOps automation in Milestone 4:

- `provision install`, `apply-update` (initramfs / boot path only)
- `provision boot`, `provision serve` (long-running services)
- `storage expand-data` (boot-once)
- `foldops acquire`, `foldops provision`, `foldops serve-https` (systemd owned)

---

# FoldOps Agent Integration

## Collection loop

On FoldingOS agents, `foldops-agent` replaces direct OS inspection with:

```text
foldingosctl inspect node --format json
foldingosctl inspect system --format json
foldingosctl inspect fah --format json
foldingosctl inspect update --format json
```

The agent assembles the ingest payload from these documents plus FoldOps-local
metadata (collection timestamp, agent version).

## Ingest correlation fields

Each ingest payload must include at minimum:

| Field | Source |
| --- | --- |
| `hostname` | `inspect node` |
| `node_id` | `inspect node` |
| `installation_role` | `inspect node` |
| `foldingos_version` | `inspect node` |
| `system` | `inspect system` |
| `fah` | `inspect fah` |
| `maintenance.reboot_required` | derived from `inspect update` / `/run/reboot-required` via foldingosctl |
| `maintenance.updates_available` | Milestone 4 reports FoldingOS/FoldOps state only; no APT on appliance |

Legacy fields such as `aptUpdatesAvailable` must be `0` on FoldingOS appliances
or omitted with schema documentation in the foldops repository.

## Privilege model

- `foldops-agent` runs as the `foldops` user on FoldingOS
- `foldingosctl inspect …` must succeed for `foldops` without root
- implementation may use file ACLs, dedicated helper subcommands, or minimal
  capability boundaries; it must not run the agent as root on FoldingOS

## Failure behavior

If any required inspect command fails:

- the agent logs the structured error
- the agent still attempts ingest when partial data is policy-allowed, marking
  missing sections null
- the agent must not fall back to direct OS inspection on FoldingOS

---

# FoldOps Supervisor Integration

On the `supervisor` role, `foldops-supervisor` uses local delegation for fleet
operations:

| Operator action | foldingosctl backing |
| --- | --- |
| View enrolled agents | `provision list-enrollments --format json` |
| Assign fleet desired version | `provision assign --format json …` |
| View registry | `registry list --format json`, `registry show --format json` |
| Trigger upstream poll | invoke `registry poll` through an approved one-shot helper or document manual/timer ownership |
| View supervisor commissioning state | `inspect commissioning --format json` |

FoldOps must not reimplement parsing of `/data/provision/enrollments/` or
`/data/registry/` directly on FoldingOS.

---

# Remote Configuration

Milestone 4 includes one approved configuration workflow:

**Folding@home username / team policy push**

1. Operator edits policy in FoldOps dashboard
2. FoldOps supervisor validates candidate TOML against foldinghome schema
3. Candidate is delivered to the agent through an approved FoldOps agent local
   HTTP action endpoint (existing `AGENT_HTTP_PORT` pattern)
4. Agent invokes:

   ```text
   foldingosctl config activate foldinghome <candidate-path> --format json
   ```

5. Agent reports success or structured validation failure to FoldOps

General arbitrary shell or unrestricted TOML domains remain out of scope.

---

# Cross-Repository Contract

Changes that require coordination:

| Contract | folding-os owner | foldops owner |
| --- | --- | --- |
| `inspect` JSON schemas | `foldingosctl` implementation + tests | ingest mapping + dashboard |
| ingest payload `node_id` | produced by `inspect node` | DB schema + UI |
| supervisor fleet actions | JSON for provision/registry | dashboard API handlers |
| config push workflow | `config activate` JSON | agent HTTP handler |

The foldops repository must detect FoldingOS appliances (for example
`installation_role` file present) and enable delegation mode. Non-FoldingOS
Debian nodes may retain legacy collection until deprecated separately.

---

# Commands And Services

Milestone 4 adds or extends:

| Component | Responsibility |
| --- | --- |
| `foldingosctl inspect …` | Read-only structured node state |
| `foldingosctl --format json` | Automation output on approved commands |
| `foldops-agent` (Rust) | Subprocess delegation on FoldingOS |
| `foldops-supervisor` (Rust) | Local fleet command delegation on supervisor |
| `foldingos-foldops-agent.service` | unchanged unit; starts delegated agent |
| `foldingos-foldops-supervisor.service` | unchanged unit |

No new long-running FoldingOS systemd services are required for Milestone 4
unless implementation discovers a polling gap; prefer extending existing timers.

---

# Implementation Sequence

1. **Automation foundation**
   - JSON envelope helpers in `foldingosctl`
   - `inspect node`, `inspect system`
   - unit tests and fixture-based golden JSON
   - `foldops` user authorization for inspect

2. **FAH and update inspection**
   - `inspect fah`, `inspect update`, `inspect commissioning`
   - map fields to FoldOps ingest schema in foldops repo

3. **FoldOps agent delegation**
   - Rust agent executes inspect commands on FoldingOS
   - ingest includes `node_id`
   - remove direct OS inspection on FoldingOS code paths

4. **Supervisor fleet visibility**
   - JSON output for `provision list-enrollments`, `registry list/show`
   - foldops dashboard fleet page backed by local commands

5. **Operator assignment workflow**
   - dashboard desired-version assignment via `provision assign`
   - confirmation and error surfacing from JSON responses

6. **Remote foldinghome config workflow**
   - agent HTTP endpoint + `config activate` delegation
   - audit log entry in FoldOps and foldingos journal

7. **Validation**
   - extend QEMU harness or add `scripts/test-foldops-integration-qemu`
   - physical validation record under `validation/`
   - readiness review

Each step requires automated tests before dependent steps proceed.

---

# Validation

## QEMU/OVMF

A Milestone 4 acceptance suite must validate at minimum:

- `foldingosctl inspect … --format json` succeeds on agent and supervisor images
- `foldops-agent` ingest uses delegation on FoldingOS (mock or live supervisor)
- ingest payload contains stable `node_id`
- supervisor dashboard/API reads enrollments via `provision list-enrollments --format json`
- desired-version assignment through delegated `provision assign` updates enrollment state
- foldinghome config push succeeds and rejects invalid candidates
- FoldOps or folding@home failure does not stop `folding-at-home.service`

## Physical

Physical validation must cover:

- supervisor dashboard shows enrolled agents with FoldingOS `node-id`
- agent ingest continues after FoldOps agent restart
- operator assigns desired version from FoldOps and agent stages update
- folding@home runtime remains active throughout

Validation records must be committed before Milestone 4 is marked complete.

---

# Non-Goals

See [4-implementation-spec.md](4-implementation-spec.md).

---

# Related Documents

- [Milestone 4 implementation specification](4-implementation-spec.md)
- [Milestone 3 engineering specification](3-engineering-spec.md)
- [FoldOps integration](../foldops-integration.md)
- [foldingosctl command reference](../foldingosctl.md)
- [Testing strategy](../testing-strategy.md)
- [ADR-0020](../adr/0020-foldops-delegates-node-operations-to-foldingosctl.md)
- [ADR-0021](../adr/0021-machine-readable-foldingosctl-automation-interface.md)
