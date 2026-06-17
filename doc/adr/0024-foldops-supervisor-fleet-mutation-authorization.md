# ADR-0024: FoldOps Supervisor Fleet Mutation Authorization

**Status:** Proposed

**Date:** 2026-06-14

**Authors:** FoldingOS project

**Depends on:** [ADR-0020](0020-foldops-delegates-node-operations-to-foldingosctl.md),
[ADR-0021](0021-machine-readable-foldingosctl-automation-interface.md),
[ADR-0014](0014-fixed-installation-roles.md),
[ADR-0016](0016-network-provisioning-via-supervisor.md)

**Related:** [ADR-0019](0019-foldops-supervisor-provisioning-and-tls.md),
[ADR-0023](0023-runtime-foldops-and-foldingosctl-updates-without-os-reimage.md)

---

## Context

Milestone 4 requires the FoldOps web dashboard to perform fleet operations on
the supervisor node without operator SSH or direct CLI use. Per
[ADR-0020](0020-foldops-delegates-node-operations-to-foldingosctl.md), those
workflows are exposed through the `foldops-supervisor` HTTP API, which delegates
to local `foldingosctl --format json` subprocesses.

`foldops-supervisor` runs as the unprivileged `foldops` service user per
[ADR-0019](0019-foldops-supervisor-provisioning-and-tls.md). Read-only
delegation (`inspect`, `provision list-enrollments`, `registry list`, and
similar) already works for that identity.

[ADR-0021](0021-machine-readable-foldingosctl-automation-interface.md) deferred
mutating-command authorization to an explicit automation policy. Without that
policy and matching image permissions, dashboard actions such as desired-version
assignment and network-boot allowlisting fail with filesystem permission errors
even when the HTTP API shape is correct.

Operator workflows through `foldingos-admin` and long-running systemd services
(`provision serve`, `provision boot`, `registry poll`, and similar) remain
unchanged. This ADR authorizes only the narrow supervisor fleet mutations
required for FoldOps dashboard operation.

---

## Decision

FoldingOS will authorize **supervisor-role fleet mutations** for the `foldops`
service identity under a fixed automation policy.

### 1. Operator entry point

On the `supervisor` installation role, approved fleet mutations are initiated
only through `foldops-supervisor` HTTP routes. The dashboard and other remote
clients must not require operators to invoke `foldingosctl` directly.

| FoldOps HTTP route | `foldingosctl` backing |
| --- | --- |
| `POST /api/fleet/assign` | `provision assign --format json …` |
| `POST /api/fleet/allow-boot` | `provision allow-boot --format json …` |

Read routes (`GET /api/fleet/enrollments`, `GET /api/fleet/allow-boot`,
`GET /api/fleet/registry`, and related) remain read-only delegation as defined
in [ADR-0020](0020-foldops-delegates-node-operations-to-foldingosctl.md).

### 2. Automation policy file

The project ships a read-only policy at:

```text
/usr/share/foldingos/foldops-supervisor-automation.toml
```

The file is part of the OS image, owned by `root`, and not writable by the
`foldops` user. `foldingosctl` must reject mutating automation invocations by
the `foldops` user unless all of the following are true:

- effective installation role is `supervisor`
- invoking UID is the `foldops` user
- command group and subcommand appear in the policy file

Milestone 4 baseline policy:

| Command group | Subcommand | Purpose |
| --- | --- | --- |
| `provision` | `assign` | Set desired image, FoldOps manifest, and tools versions for enrolled agents |
| `provision` | `allow-boot` | Authorize PXE/iPXE clients and optional install-disk pins |

No other mutating `foldingosctl` commands are exposed to the `foldops` user in
Milestone 4. Agent-role nodes do not install this automation policy for
mutation; `foldingosctl` role checks remain mandatory even when paths are
group-writable.

### 3. Filesystem permissions on the supervisor

Mutating delegated commands write only to the following supervisor state paths:

| Path | Written by |
| --- | --- |
| `/data/provision/enrollments/` | `provision assign` |
| `/data/config/provision/boot-allowlist` | `provision allow-boot` |
| `/data/config/provision/boot-install-disk-allowlist` | `provision allow-boot --disk …` |
| `/data/config/foldops/assigned-manifest.toml` | `provision assign` when the supervisor node is in assignment scope |
| `/data/config/tools/assigned-version.json` | `provision assign` when the supervisor node is in assignment scope |

The image and supervisor bootstrap path must ensure the `foldops` user can
create and update only these paths on the `supervisor` role:

1. **Enrollment store:** `/data/provision/enrollments/` is owned
   `root:foldops` with mode `2775` so new enrollment records inherit the
   `foldops` group.

2. **Boot allowlists:** `/data/config/provision/boot-allowlist` and
   `/data/config/provision/boot-install-disk-allowlist` are owned
   `root:foldops` with mode `0664`. The parent directory
   `/data/config/provision/` remains `root:root` `0755` so unrelated provision
   secrets are not group-writable.

3. **Supervisor self-assignment files:** when installation role is
   `supervisor`, bootstrap must ensure `/data/config/foldops/` and
   `/data/config/tools/` permit the `foldops` group to create and update only
   `assigned-manifest.toml` and `assigned-version.json`. Other files in those
   directories remain `root`-owned and not group-writable.

Role-specific permission application may occur during supervisor direct-flash
bootstrap (`foldingosctl provision install` / `foldingosctl foldops provision`)
rather than through identical tmpfiles rules on agent-role nodes.

### 4. Authorization boundary

- **FoldOps HTTP API** is the remote operator interface for fleet mutation on
  the supervisor.
- **`foldingosctl`** remains the sole local implementation of validation,
  registry checks, and persistence.
- **`foldops-supervisor`** must not parse or rewrite enrollment, registry, or
  allowlist files directly.
- **`foldingos-admin`** retains full operator CLI access, including commands not
  delegated to FoldOps.

### 5. Audit and failure behavior

- `foldops-supervisor` logs delegation failures with the structured
  `foldingosctl` error document when `--format json` is used.
- Permission or policy denial must surface as an HTTP API error; the service
  must not escalate to root.
- FoldOps unavailability must not block node boot, provisioning services, or
  Folding@home operation.

---

## Alternatives Considered

### Run mutating delegation as root

Rejected. Running `foldops-supervisor` as root or invoking unrestricted
`sudo foldingosctl` from the service expands blast radius beyond fleet mutation
and conflicts with the unprivileged FoldOps service model.

### Setuid `foldingosctl` helper

Rejected for Milestone 4. Group permissions on explicit state paths plus a
fixed automation policy file are simpler to audit and match the existing
`foldops` service identity.

### Reimplement mutation in `foldops-supervisor`

Rejected. Duplicates validation and persistence logic already centralized in
`foldingosctl`, violating [ADR-0020](0020-foldops-delegates-node-operations-to-foldingosctl.md).

### Require operators to use `provision serve` HTTP instead of FoldOps

Rejected. Milestone 4 dashboard workflows are part of the FoldOps supervisor
API. Underlying persistence remains shared; only the operator entry point
differs.

---

## Consequences

### Positive

- Dashboard fleet assignment and network-boot allowlisting work without SSH
- Mutations remain validated by `foldingosctl` and bounded by a shipped policy
  file
- Read-only and mutating automation share the same `foldops` service identity
  with explicit separation

### Negative

- Supervisor images require role-aware permission bootstrap beyond generic tmpfiles
- Every new delegated mutation needs ADR or policy-file amendment plus tests
- Permission mistakes fail at runtime until image/bootstrap changes land

### Tradeoffs

- Group-writable state directories are broader than per-file ACLs but are
  easier to maintain in Buildroot overlay and QEMU acceptance harnesses
- Agent-role nodes share the same release image; role-gated permission setup
  avoids granting agent nodes unnecessary write access

---

## Future Considerations

- Add `foldingosctl` unit tests that invoke approved mutators as the `foldops`
  user on a supervisor fixture
- Extend the policy file if additional dashboard mutations are approved
- Consider structured audit logging of delegated mutations to the supervisor
  journal

---

## References

- [Milestone 4 engineering specification](../milestone/4-engineering-spec.md)
- [foldingosctl command reference](../foldingosctl.md)
- [FoldOps integration](../foldops-integration.md)
- [ADR-0020](0020-foldops-delegates-node-operations-to-foldingosctl.md)
- [ADR-0021](0021-machine-readable-foldingosctl-automation-interface.md)
