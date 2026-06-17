# ADR-0027: FoldOps Remote Operator API

**Status:** Proposed

**Date:** 2026-06-14

**Authors:** FoldingOS project

**Depends on:** [ADR-0020](0020-foldops-delegates-node-operations-to-foldingosctl.md),
[ADR-0021](0021-machine-readable-foldingosctl-automation-interface.md),
[ADR-0024](0024-foldops-supervisor-fleet-mutation-authorization.md),
[ADR-0026](0026-foldops-dashboard-operator-authentication.md)

**Related:** [ADR-0025](0025-implement-foldingosctl-in-rust.md)

---

## Context

Milestone 4 requires operators to manage FoldingOS fleets from the FoldOps web
dashboard without SSH to supervisor or agent nodes for routine work.

[ADR-0020](0020-foldops-delegates-node-operations-to-foldingosctl.md) requires
FoldOps to delegate node-local behavior to `foldingosctl` rather than
reimplement appliance logic.

The browser cannot invoke `foldingosctl` directly. Remote management therefore
requires HTTP APIs. The design question is **how** those APIs relate to
`foldingosctl`, supervisor nodes, and agent nodes — not whether HTTP is used.

Supervisor-only fleet operations already delegate locally
(`POST /api/fleet/assign`, `GET /api/fleet/enrollments`, and related routes).
Agent-local operations (configuration activation, diagnostics, control actions)
require a path from the dashboard to the correct node.

---

## Decision

The FoldOps web UI will use a **single remote entry point** on the supervisor,
with **two execution paths** behind it.

### 1. Operator entry point

- The browser talks only to **supervisor HTTPS** (`foldingosctl foldops
  serve-https`, typically port `3443`).
- The static dashboard and `/api/*` routes are served or proxied through that
  HTTPS front door.
- Operator authentication uses dashboard sessions per
  [ADR-0026](0026-foldops-dashboard-operator-authentication.md).

Agents are not browser targets.

### 2. Supervisor-local delegation

When the requested operation is defined for the `supervisor` installation role,
`foldops-supervisor` invokes local `foldingosctl --format json` subprocesses.

Examples:

| HTTP route | `foldingosctl` backing |
| --- | --- |
| `GET /api/fleet/enrollments` | `provision list-enrollments` |
| `POST /api/fleet/assign` | `provision assign` |
| `GET /api/fleet/allow-boot` | `provision list-allow-boot` |
| `POST /api/fleet/allow-boot` | `provision allow-boot` |
| `GET /api/fleet/registry` | `registry list`, `registry show` |

`foldops-supervisor` must not parse or rewrite enrollment, registry, or allowlist
files directly.

### 3. Agent-proxied delegation

When the operation must run on an agent, the flow is:

```text
Browser → foldops-supervisor API → foldops-agent HTTP → foldingosctl
```

- `foldops-supervisor` proxies authenticated requests to the target agent's
  existing HTTP server (`AGENT_HTTP_PORT`, default `9100`).
- `foldops-agent` invokes approved local `foldingosctl` commands and returns
  structured JSON results.
- Machine-to-machine calls on the agent HTTP server continue to use
  `AGENT_TOKEN` / `INGEST_TOKEN` between supervisor and agent.

Examples already aligned with this model or planned in Milestone 4:

| Operator action | Supervisor route | Agent backing |
| --- | --- | --- |
| View machine inventory / health | `GET /api/machines` | ingest + `inspect` on agent |
| Remote logs | `GET /api/machines/{name}/logs` | agent HTTP log endpoints |
| Service control | `POST /api/machines/{name}/control` | agent control executor |
| Folding@home config push | future curated route | `config activate foldinghome` |

### 4. Curated workflows only

Remote APIs expose **named operator workflows**, not a generic remote shell and
not arbitrary `foldingosctl` passthrough.

- Each route maps to one or a bounded set of validated `foldingosctl` commands.
- Supervisor fleet mutators for the `foldops` service user remain limited by
  [ADR-0024](0024-foldops-supervisor-fleet-mutation-authorization.md).
- Agent mutation endpoints require their own explicit allowlists in the
  engineering specification.
- Commands that are boot-only, initramfs-only, or unsafe for remote use remain
  systemd/operator-SSH-only.

### 5. Read vs write

| Data source | Pattern |
| --- | --- |
| Periodic inventory, health, FAH summaries | agent `inspect` → ingest → supervisor DB → dashboard |
| Supervisor fleet state | supervisor local `foldingosctl` reads |
| Live logs / control / config push | supervisor proxy → agent HTTP → `foldingosctl` |

The dashboard should prefer ingest-backed reads when freshness requirements allow,
and proxy to agents when live data or mutation is required.

### 6. Implementation technology

- Delegation remains **subprocess invocation** of `/usr/bin/foldingosctl` on the
  node where the command must execute.
- [ADR-0025](0025-implement-foldingosctl-in-rust.md) changes the language of
  the binary, not this boundary.
- FoldOps must not embed `foldingosctl` as an in-process library in this
  milestone phase.

---

## Alternatives Considered

### Browser connects directly to each agent API

Rejected. Exposes many endpoints, complicates TLS and operator auth, and bypasses
the supervisor as fleet gatekeeper.

### SSH from supervisor to agents

Rejected for Milestone 4 per [ADR-0020](0020-foldops-delegates-node-operations-to-foldingosctl.md).

### Generic `POST /api/foldingosctl` command proxy

Rejected. Unbounded command surface, weak auditability, and difficult authorization.

### Second long-running foldingosctl HTTP daemon on every node

Rejected. Splits authority beside the existing CLI and systemd-owned services.

### In-process Rust library instead of subprocess

Rejected for Milestone 4. Does not replace the need for supervisor/agent HTTP
routing for the web UI and removes a single CLI operator entry point.

---

## Consequences

### Positive

- Clear architecture for "manage everything from the UI"
- Supervisor remains the fleet control plane
- `foldingosctl` stays the sole implementation of appliance behavior
- Agent and supervisor paths reuse existing HTTP patterns

### Negative

- New curated routes and allowlists require ongoing maintenance
- Supervisor→agent proxy adds failure modes when agents are offline
- Dashboard auth must be implemented before remote mutations are production-safe

### Tradeoffs

- Subprocess overhead is acceptable relative to operator action latency
- Curated APIs are more work than a CLI tunnel but far safer

---

## Future Considerations

- Additional agent-local mutation routes as operator workflows mature
- Optional event streaming for long-running operations

---

## References

- [FoldOps integration](../foldops-integration.md)
- [foldingosctl command reference](../foldingosctl.md)
- [Milestone 4 engineering specification](../milestone/4-engineering-spec.md)
- [ADR-0020](0020-foldops-delegates-node-operations-to-foldingosctl.md)
- [ADR-0021](0021-machine-readable-foldingosctl-automation-interface.md)
- [ADR-0024](0024-foldops-supervisor-fleet-mutation-authorization.md)
- [ADR-0026](0026-foldops-dashboard-operator-authentication.md)
