# ADR-0020: FoldOps Delegates Node Operations To foldingosctl

**Status:** Proposed

**Date:** 2026-06-14

**Authors:** FoldingOS project

**Amends:** [ADR-0018](0018-foldops-package-acquisition-and-update-model.md),
[ADR-0019](0019-foldops-supervisor-provisioning-and-tls.md)

**Related:** [ADR-0022](0022-foldops-rust-source-in-foldingos-monorepo.md),
[ADR-0027](0027-foldops-remote-operator-api.md),
[ADR-0023](0023-runtime-foldops-and-foldingosctl-updates-without-os-reimage.md)

---

## Context

Milestone 3 integrated FoldOps packages into FoldingOS appliances and
established supervisor-led fleet provisioning through `foldingosctl provision`
and related services.

FoldOps collects node metrics, Folding@home state, and maintenance signals
through its agent implementation. On FoldingOS appliances the authoritative
implementation is the Rust workspace in `packages/foldops/` per
[ADR-0022](0022-foldops-rust-source-in-foldingos-monorepo.md). The legacy
Node.js tree is not used on appliances. FoldingOS already centralizes
appliance behavior in `foldingosctl` for:

- node identity and configuration
- Folding@home acquisition and runtime preparation
- supervisor registry and fleet assignment
- provisioning enrollment and staged updates
- FoldOps package acquisition and local provisioning

Duplicating OS-specific logic inside FoldOps creates drift risk. FoldOps Rust
source lives in `packages/foldops/` in this repository per
[ADR-0022](0022-foldops-rust-source-in-foldingos-monorepo.md), while runtime
binaries remain acquired on `/data` separately from the OS image. When FoldingOS
changes storage layout, service names, paths, or validation rules, FoldOps should
not need parallel reimplementation outside `foldingosctl`.

Milestone 4 requires full FoldOps integration: inventory, health reporting,
remote configuration, and fleet operations. The project needs a stable boundary
between fleet management (FoldOps) and node-local appliance control
(`foldingosctl`).

---

## Decision

FoldOps **delegates node-local FoldingOS operations to `foldingosctl`**.

1. **`foldingosctl` is the sole supported local control interface** for
   FoldingOS appliance behavior. FoldOps must not reimplement equivalent
   inspection or mutation logic against raw paths, systemd units, or package
   managers on FoldingOS nodes.

2. **FoldOps agent and supervisor invoke `foldingosctl` on the local node**
   when collecting inventory, health, Folding@home status, update state, or
   performing approved operator actions that map to existing command groups.

3. **Milestone 3 provisioning enrollment remains separate from FoldOps ingest
   registration.** `foldingosctl provision enroll` registers the node with the
   FoldingOS supervisor for image updates. FoldOps ingest continues to use the
   FoldOps supervisor API, but ingest payloads must correlate to the FoldingOS
   `node-id` and installation role reported by `foldingosctl`.

4. **Supervisor-side fleet actions run on the supervisor node only.** Commands
   such as `foldingosctl provision assign`, `registry list`, and
   `provision list-enrollments` are invoked locally by `foldops-supervisor` on
   the `supervisor` role. Milestone 4 does not introduce SSH-based remote shell
   access from FoldOps to agents.

   The FoldOps dashboard mutates fleet state through `foldops-supervisor` HTTP
   routes that delegate to approved `foldingosctl` commands. Operators must
   not need direct CLI access for assignment or network-boot allowlisting when
   using the web UI. Mutating delegation authorization and supervisor image
   permissions are defined in
   [ADR-0024](0024-foldops-supervisor-fleet-mutation-authorization.md).

5. **Agent-side actions that require mutation** (for example configuration
   activation) must go through explicit, auditable `foldingosctl` commands
   authorized for automation. FoldOps must not bypass validation performed by
   `foldingosctl config activate` or role checks.

6. **FoldOps failure must not block node boot or Folding@home operation.** This
   preserves the failure philosophy in [foldops-integration.md](../foldops-integration.md).

---

## Alternatives Considered

### FoldOps agent continues direct OS inspection

Rejected. This duplicates appliance knowledge in two repositories, conflicts
with the fixed-role appliance model, and already diverges on FoldingOS (for
example the `foldops` unprivileged service user versus legacy Debian deployments
that run the agent as root).

### New FoldingOS D-Bus or gRPC daemon beside foldingosctl

Rejected for Milestone 4. A second local API surface would split authority and
complicate the boot graph. `foldingosctl` already exists, is invoked by systemd,
and is documented. Machine-readable output can extend the same binary per
[ADR-0021](0021-machine-readable-foldingosctl-automation-interface.md).

### FoldOps replaces foldingosctl provision APIs

Rejected. Supervisor-led network provisioning is accepted architecture
([ADR-0016](0016-network-provisioning-via-supervisor.md)). FoldOps may expose
operator workflows in its dashboard, but the underlying fleet operations remain
`foldingosctl` and the supervisor provisioning HTTP API.

### Supervisor SSHes to agents to run foldingosctl

Rejected. FoldingOS agents are headless appliances without an approved remote
command channel in Milestone 4. Agent-local collection uses the FoldOps agent
process on each node; supervisor-local collection uses the supervisor process
on the fleet node.

---

## Consequences

### Positive

- Single source of truth for appliance behavior on FoldingOS nodes
- FoldOps releases can track FoldingOS command contracts instead of relearning
  filesystem layout and service names
- Operator documentation in [foldingosctl.md](../foldingosctl.md) applies to both
  human CLI use and FoldOps automation
- Easier testing: foldingosctl unit tests cover behavior consumed by FoldOps

### Negative

- FoldOps Rust/Node code must parse structured `foldingosctl` output and handle
  command failures explicitly
- Some legacy FoldOps Debian deployments may still use direct inspection until
  parity helpers exist outside FoldingOS
- Cross-repository coordination is required when adding new automated capabilities

### Tradeoffs

- Subprocess invocation is simpler than a long-lived IPC service but adds parse
  boundaries and startup overhead acceptable at 60-second ingest intervals
- Not every existing `foldingosctl` command is suitable for automation on day
  one; Milestone 4 defines a phased command surface in
  [4-engineering-spec.md](../milestone/4-engineering-spec.md)

---

## Future Considerations

- Agent-local HTTP endpoints implemented by `foldops-agent` may wrap selected
  `foldingosctl` mutations for supervisor-initiated actions
- A stable versioned JSON schema for command output may later support non-Go
  consumers beyond FoldOps
- Remote command delivery beyond local delegation requires a separate ADR

---

## References

- [FoldOps integration](../foldops-integration.md)
- [Milestone 4 implementation specification](../milestone/4-implementation-spec.md)
- [Milestone 4 engineering specification](../milestone/4-engineering-spec.md)
- [ADR-0021: Machine-Readable foldingosctl Automation Interface](0021-machine-readable-foldingosctl-automation-interface.md)
- [ADR-0024: FoldOps Supervisor Fleet Mutation Authorization](0024-foldops-supervisor-fleet-mutation-authorization.md)
- [ADR-0022: FoldOps Rust Source In FoldingOS Monorepo](0022-foldops-rust-source-in-foldingos-monorepo.md)
- [ADR-0023: Runtime FoldOps And foldingosctl Updates Without OS Reimage](0023-runtime-foldops-and-foldingosctl-updates-without-os-reimage.md)
