# ADR-0028: Supervisor Fleet Software Update Workflow

**Status:** Proposed

**Date:** 2026-06-18

**Authors:** FoldingOS project

**Depends on:** [ADR-0020](0020-foldops-delegates-node-operations-to-foldingosctl.md),
[ADR-0023](0023-runtime-foldops-and-foldingosctl-updates-without-os-reimage.md),
[ADR-0024](0024-foldops-supervisor-fleet-mutation-authorization.md),
[ADR-0027](0027-foldops-remote-operator-api.md)

**Related:** [ADR-0017](0017-official-release-publication-and-supervisor-upstream-polling.md),
[Milestone 5 engineering specification](../milestone/5-engineering-spec.md)

---

## Context

Milestone 4 delivered the **acquisition primitives** for runtime updates without
OS reimage:

- supervisor-assigned FoldOps manifest release on
  `/data/config/foldops/assigned-manifest.toml`
- supervisor-assigned tools version on
  `/data/config/tools/assigned-version.json`
- `foldingosctl foldops acquire` and `foldingosctl tools acquire`
- fleet assignment through `POST /api/fleet/assign` → `provision assign`

Operators still lack an approved **end-to-end workflow** in the supervisor UI:

1. check whether newer FoldOps bundles or `foldingosctl` tools releases exist
2. set desired versions for the supervisor and enrolled agents
3. trigger nodes to apply assigned versions without SSH and without rebaking OS
   images

FoldOps UI rework is planned after this milestone. Milestone 5 must define the
**supervisor API and node execution contract** so a minimal admin UI can ship
first and a richer dashboard can follow in FoldOps Upgrades.

Operating-system image updates remain on the separate channel defined in
[update-system.md](../update-system.md) and [ADR-0017](0017-official-release-publication-and-supervisor-upstream-polling.md).
This ADR covers **FoldOps application bundles** and **`foldingosctl` tools**
only.

---

## Decision

FoldingOS will implement a **supervisor-led fleet software update workflow**
for FoldOps and `foldingosctl` with three operator steps: **discover**, **assign**,
**apply**.

### 1. Discover (check for updates)

The supervisor exposes a read-only HTTP API that compares:

| Source | Local state | Upstream catalog |
| --- | --- | --- |
| FoldOps bundles | active and assigned manifest release from `inspect foldops` on supervisor and agents | published index at `packages.folding-os.com/foldops/index.json` |
| foldingosctl tools | active and assigned tools version from `inspect tools` | published index at `packages.folding-os.com/foldingos-tools/index.json` |

The response summarizes, per machine and for the supervisor itself:

- current active version
- assigned desired version (if any)
- latest published version on the packages channel
- whether an update is available (`latest > active` or `assigned != active`)

The supervisor fetches upstream indexes over HTTPS. Failure to reach the packages
channel returns a structured error; it must not block FoldOps services or
Folding@home.

### 2. Assign (set desired versions)

Desired FoldOps and tools versions are assigned through the **existing**
`POST /api/fleet/assign` contract:

- `foldops_manifest` → `provision assign --foldops-manifest …`
- `tools_version` → `provision assign --tools-version …`

Assignment may target:

- one enrolled node (`--node`)
- the full fleet (`--all`)
- the supervisor node when it is in assignment scope (existing self-assignment
  behavior)

Assignment writes enrollment records and supervisor-local assignment files only.
It does **not** download or activate artifacts.

### 3. Apply (trigger acquisition)

After assignment, operators trigger apply through supervisor HTTP routes that
proxy to agent-local curated endpoints (same pattern as remote config push per
[ADR-0027](0027-foldops-remote-operator-api.md)):

| Operator action | Supervisor route | Node execution |
| --- | --- | --- |
| Apply FoldOps on agents | `POST /api/fleet/software/apply-foldops` | agent `POST /software/foldops-acquire` → `foldingosctl foldops acquire --format json` |
| Apply tools on agents | `POST /api/fleet/software/apply-tools` | agent `POST /software/tools-acquire` → `foldingosctl tools acquire --format json` |
| Apply on supervisor | `POST /api/software/apply-local` | local `foldingosctl` acquire commands on supervisor role |

Apply routes accept optional node scope (`hostname` list or fleet-wide). Offline
nodes are skipped with explicit errors; online nodes return structured JSON from
`foldingosctl`.

Successful FoldOps apply restarts only the approved FoldOps systemd units.
Successful tools apply restarts only units that embed `foldingosctl` per
[ADR-0023](0023-runtime-foldops-and-foldingosctl-updates-without-os-reimage.md).

### 4. Delegation and authorization

- Node-local acquisition remains **`foldingosctl` only** per
  [ADR-0020](0020-foldops-delegates-node-operations-to-foldingosctl.md).
- Agent apply endpoints require the same machine auth as other agent HTTP
  mutations (`AGENT_TOKEN` / ingest token).
- Supervisor fleet apply routes require operator session auth when dashboard
  auth is enabled ([ADR-0026](0026-foldops-dashboard-operator-authentication.md));
  until then, the existing ingest-token-protected API model applies.
- `foldops` user automation policy must authorize any new mutating commands
  exposed to the agent HTTP server.

### 5. Failure behavior

- Acquisition or apply failure must be **fail-closed** (no partial activation).
- `folding-at-home.service` must remain running through FoldOps/tools apply
  failures.
- FoldOps unavailability must not block node boot or FAH operation.
- Apply operations are idempotent; repeating apply with the same assignment is
  safe.

### 6. UI placement (Milestone 5 minimum)

Milestone 5 ships a **supervisor admin section** (may be minimal HTML/JSON
forms before full FoldOps dashboard rework) that implements:

- “Check for updates”
- assign FoldOps manifest release and/or tools version to selected nodes
- “Apply assigned updates” for supervisor and agents

Full dashboard polish and first-boot settings consolidation move to FoldOps
Upgrades.

---

## Alternatives Considered

### Wait for full FoldOps dashboard rework

Rejected. Operators need runtime updates before the large UI refactor. The API
contract must land first.

### Push artifacts from supervisor to agents directly

Rejected. Agents already download from pinned HTTPS URLs per ADR-0023. The
supervisor assigns and triggers; it does not mirror bundle bytes to agents.

### Rely on periodic acquire timers only (no operator trigger)

Rejected as the sole model. Timers may remain as safety net, but operators need
explicit apply after assignment.

### Replace `/usr/bin/foldingosctl` with a `/data/apps` symlink tree now

Deferred. ADR-0023 chose atomic `/usr/bin/foldingosctl` replace for Milestone
4/5 simplicity.

---

## Consequences

### Positive

- Routine FoldOps and `foldingosctl` fixes roll out from the supervisor UI
- Reuses assignment and acquire primitives from Milestones 3–4
- Clear boundary before FoldOps Upgrades UI work

### Negative

- New upstream index files and publication automation required
- Agent HTTP surface grows; each route needs policy and tests
- Skewed versions during rollout require careful operator messaging

---

## References

- [Milestone 5 engineering specification](../milestone/5-engineering-spec.md)
- [Milestone 5 implementation specification](../milestone/5-implementation-spec.md)
- [ADR-0023](0023-runtime-foldops-and-foldingosctl-updates-without-os-reimage.md)
- [foldops-integration.md](../foldops-integration.md)
