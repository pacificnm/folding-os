# ADR-0026: FoldOps Dashboard Operator Authentication

**Status:** Proposed

**Date:** 2026-06-14

**Authors:** FoldingOS project

**Depends on:** [ADR-0007](0007-first-boot-administrator-and-ssh-provisioning.md),
[ADR-0014](0014-fixed-installation-roles.md),
[ADR-0019](0019-foldops-supervisor-provisioning-and-tls.md)

**Related:** [ADR-0027](0027-foldops-remote-operator-api.md),
[ADR-0025](0025-implement-foldingosctl-in-rust.md)

---

## Context

Milestone 4 targets an operator workflow where a user downloads a release image,
flashes a supervisor, boots the appliance, and **manages the fleet from the FoldOps
web dashboard** without routine SSH access.

[ADR-0019](0019-foldops-supervisor-provisioning-and-tls.md) established HTTPS and
a fleet-wide `INGEST_TOKEN` for agent ingest, and rejected a separate FoldOps web
login because Milestone 3 did not implement dashboard authentication.

That model is insufficient for the Milestone 4 distribution goal:

- the dashboard and most supervisor `/api/*` routes are not meaningfully
  protected for human operators today
- `INGEST_TOKEN` is a **machine secret** shared across all agents; it is a poor
  substitute for per-operator web sessions
- requiring operators to stage `foldops-ingest-token` on EFI before first boot
  blocks a simple flash-and-manage experience

[ADR-0007](0007-first-boot-administrator-and-ssh-provisioning.md) remains the
SSH policy: public-key authentication only for `foldingos-admin`, no OS password
login, and **no build-time or release-image embedded administrator keys**.

SSH and web login serve different roles. SSH is break-glass, recovery, and
development. The web UI is the primary management path.

---

## Decision

FoldingOS will add **human operator authentication** for the FoldOps supervisor
dashboard and API.

### 1. Separate human and machine credentials

| Credential | Purpose | Consumers |
| --- | --- | --- |
| **Dashboard operator account** | Human login to the web UI and authenticated supervisor API sessions | Browser, operators |
| **`INGEST_TOKEN` / `AGENT_TOKEN`** | Fleet machine authentication | `foldops-agent` ingest, supervisor→agent HTTP |

Operator passwords or sessions must **not** replace `INGEST_TOKEN` for agent
ingest.

### 2. First-boot supervisor experience

For supervisor-role appliances, the target operator flow is:

```text
Flash generic release image
↓
Boot supervisor (DHCP, commissioning display)
↓
Open HTTPS dashboard
↓
Sign in with shipped default operator credentials
↓
Forced password change before any other UI action
↓
Complete first-run supervisor setup (FoldOps provision, fleet token generation)
↓
Manage fleet from the UI
```

The release image may ship a **known default operator username and password** for
the **FoldOps dashboard only**. This is distinct from OS/SSH password login,
which remains disabled per [ADR-0007](0007-first-boot-administrator-and-ssh-provisioning.md).

On first successful login, the operator **must** set a new password before
accessing other dashboard features.

### 3. Session model

- Operator authentication uses **server-side sessions** or equivalent
  short-lived, HTTP-only session tokens issued by `foldops-supervisor` after
  password verification.
- All supervisor `/api/*` routes require a valid operator session **except**
  explicitly documented public first-run endpoints (login, password change,
  health/status as needed).
- Passwords are stored hashed under `/data/foldops/` or `/data/config/foldops/`
  with restrictive ownership. Plaintext passwords are never logged.

### 4. Fleet ingest token bootstrap

When first-run setup completes on a supervisor:

1. generate a cryptographically strong `INGEST_TOKEN`
2. persist it to `/data/config/foldops/ingest-token` (`0600`)
3. render `supervisor.env` and `agent.env` as today
4. complete `foldingosctl foldops provision` / TLS bootstrap as required by
   [ADR-0019](0019-foldops-supervisor-provisioning-and-tls.md)

Network-provisioned agents continue to receive the fleet ingest token from the
supervisor during install per existing Milestone 3 behavior.

### 5. EFI staging compatibility

During transition, supervisor direct flash **may** still support EFI staging of:

- `/foldingos/provision/foldops-ingest-token`
- `/foldingos/provision/authorized_keys`

via `scripts/make-bootable-usb`. When first-run dashboard setup has already
generated secrets, EFI staging must not weaken or replace completed persistent
configuration without operator intent.

Long-term distribution target: **supervisor flash without mandatory EFI secrets**,
with dashboard first-run establishing fleet credentials.

### 6. SSH remains optional and key-based

SSH administration for `foldingos-admin` is unchanged:

- **no** default SSH password
- **no** release-image embedded administrator keys
- operators who want SSH supply **their own** public keys through EFI staging or
  an optional first-run UI step
- SSH is for recovery, diagnostics, and development — not routine fleet operation

First-run setup may offer optional SSH public-key import; it is not required for
dashboard-only operators.

---

## Alternatives Considered

### Reuse INGEST_TOKEN as the web UI secret

Rejected. Shared machine tokens cannot support per-operator sessions, rotation, or
reasonable browser UX, and they expose the fleet ingest boundary to humans.

### TLS-only dashboard protection

Rejected. Encryption without authentication leaves management APIs open to anyone
who can reach the supervisor HTTPS port.

### SSH as the only remote administration path

Rejected for Milestone 4 product goals. SSH remains available but is not the
primary operator interface.

### Default SSH password for foldingos-admin

Rejected. Conflicts with [ADR-0007](0007-first-boot-administrator-and-ssh-provisioning.md)
and expands attack surface on a full shell account.

---

## Consequences

### Positive

- Flash-and-manage supervisor workflow becomes achievable
- Human and machine authentication boundaries are explicit
- Dashboard mutations can be protected before wide deployment
- SSH break-glass path remains for operators who need it

### Negative

- Default dashboard credentials require forced rotation and clear documentation
- First-run setup becomes a critical security path requiring careful testing
- Implementation spans `foldops-supervisor`, web UI, and provisioning bootstrap

### Tradeoffs

- A known default dashboard password is a deliberate product exception with
  mandatory change, not a general OS password policy change
- EFI pre-staging remains supported for advanced operators and lab automation

---

## Future Considerations

- Optional multi-operator accounts or external identity integration
- Audit logging of operator actions separate from agent ingest

---

## References

- [FoldOps integration](../foldops-integration.md)
- [Security model](../security.md)
- [Deployment and provisioning](../installer.md)
- [ADR-0007](0007-first-boot-administrator-and-ssh-provisioning.md)
- [ADR-0019](0019-foldops-supervisor-provisioning-and-tls.md)
- [ADR-0027](0027-foldops-remote-operator-api.md)
- [Milestone 4 engineering specification](../milestone/4-engineering-spec.md)
