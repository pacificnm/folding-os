# ADR-0019: FoldOps Supervisor Provisioning, Token Bootstrap, And TLS

**Status:** Accepted

**Version:** 1.1

**Date:** 2026-06-15

**Revised:** 2026-06-15 (agent URL derivation, CA staging, HTTPS terminator)

**Authors:** FoldingOS Project Contributors

**Depends on:**

- [ADR-0007](0007-first-boot-administrator-and-ssh-provisioning.md) (EFI provisioning channel)
- [ADR-0014](0014-fixed-installation-roles.md) (supervisor role requirements)
- [ADR-0018](0018-foldops-package-acquisition-and-update-model.md) (FoldOps package acquisition)

**Amended by:** [ADR-0020](0020-foldops-delegates-node-operations-to-foldingosctl.md),
[ADR-0023](0023-runtime-foldops-and-foldingosctl-updates-without-os-reimage.md)
(supervisor assignment of desired FoldOps and tools versions extends provisioning APIs)

---

# Context

Supervisor-role installations must run FoldOps management services, but the
FoldOps web dashboard and ingest API must not become remotely reachable until
initial secrets and TLS identity are configured ([ADR-0014](0014-fixed-installation-roles.md)).

FoldOps authenticates agent ingest with a shared bearer secret:

- supervisor: `INGEST_TOKEN`
- every agent: `AGENT_TOKEN` (must match `INGEST_TOKEN`)

On general Debian hosts, operators generate this token with
`openssl rand -hex 32` and configure `/etc/foldops/supervisor.env` and
`/etc/foldops/agent.env` manually. On FoldingOS appliances,
`foldingosctl foldops provision` automates the equivalent paths under
`/data/config/foldops/`. The authoritative FoldOps implementation for appliances
is the Rust workspace in `packages/foldops/` per
[ADR-0022](0022-foldops-rust-source-in-foldingos-monorepo.md).

FoldingOS must automate that bootstrap while:

- keeping SSH administration separate ([ADR-0007](0007-first-boot-administrator-and-ssh-provisioning.md))
- propagating the same token to network-provisioned agents
- serving the supervisor dashboard over **HTTPS** with a **self-signed** certificate for Milestone 3
- failing closed until provisioning succeeds

The Rust `foldops-supervisor` binary serves plain HTTP internally today.
FoldingOS terminates TLS in front of the supervisor process.

---

# Decision

## Administrator bootstrap = INGEST_TOKEN

FoldingOS does not introduce a separate FoldOps web login for Milestone 3.
Initial FoldOps “administrator” provisioning means:

1. establish a fleet-wide ingest secret (`INGEST_TOKEN` / `AGENT_TOKEN`)
2. configure the supervisor and agents to use it
3. enable remote HTTPS access only after secrets and TLS material exist

This matches the current FoldOps configuration model.

## EFI provisioning channel

The ingest secret is staged on the EFI System Partition, parallel to SSH keys in
[ADR-0007](0007-first-boot-administrator-and-ssh-provisioning.md):

```text
/foldingos/provision/foldops-ingest-token
```

Runtime path on a booted system:

```text
/boot/efi/foldingos/provision/foldops-ingest-token
```

File format:

- exactly one line
- hex-encoded secret from `openssl rand -hex 32` (64 hex characters)
- no comments or trailing whitespace beyond a single optional newline
- mode `0644` on EFI; imported persistent copy is `0600`

Optional recovery rotation uses the same EFI path as SSH key rotation.

### Supervisor direct flash

Before first boot of the first supervisor, the operator writes
`foldops-ingest-token` to the EFI System Partition (via
`scripts/make-bootable-usb`, direct ESP edit, or removable-media workflow).

On first boot, after FoldOps packages are acquired
([ADR-0018](0018-foldops-package-acquisition-and-update-model.md)),
`foldingosctl foldops provision` will:

1. import and validate the EFI token file
2. persist it under `/data/config/foldops/ingest-token` (`0600`)
3. remove the EFI staging file after successful import
4. generate self-signed TLS material if not already present
5. render supervisor and local-agent environment files
6. mark provisioning complete

### Network-provisioned agents

Blank agent machines cannot receive EFI secrets before PXE. During supervisor
network install, when staging agent EFI provisioning files, the supervisor
copies the **already imported** fleet ingest token to the target EFI partition at:

```text
/foldingos/provision/foldops-ingest-token
```

This mirrors staging of `authorized_keys` and `installation-role` today.

On agent first boot, after FoldOps acquisition, `foldingosctl foldops provision`
imports the token, renders `agent.env` with `AGENT_TOKEN`, sets
`SUPERVISOR_URL` from the agent's existing FoldingOS supervisor base URL,
and removes the EFI staging file.

The supervisor does not transmit the token over the network during PXE beyond
writing it into the target disk image offline on the selected internal disk.

During network install, when staging agent persistent configuration, the
supervisor also writes the public TLS trust anchor to the target data
partition at:

```text
/data/config/foldops/supervisor-ca.pem
```

This file is copied from the supervisor's `/data/foldops/tls/ca.pem` after
supervisor FoldOps provisioning succeeds. It is not secret and is not staged
on EFI.

## Self-signed TLS (HTTPS)

Milestone 3 uses a FoldingOS-generated **self-signed** certificate for the
supervisor HTTPS front end.

Persistent paths:

```text
/data/foldops/tls/cert.pem
/data/foldops/tls/key.pem
/data/foldops/tls/ca.pem          # same as cert for self-signed; agents trust this
```

Provisioning generates a 365-day (minimum) RSA or ECDSA certificate whose
subject includes the appliance hostname and `127.0.0.1` SAN entries.

Public listen model:

| Component | Bind | Protocol |
| --- | --- | --- |
| `foldingosctl foldops serve-https` | `0.0.0.0:3443` | HTTPS (TLS terminator) |
| `foldops-supervisor` | `127.0.0.1:3000` | HTTP (loopback only) |

`foldingosctl foldops serve-https` is the sole Milestone 3 HTTPS front end. It
terminates TLS and reverse-proxies to the loopback supervisor. It runs under a
FoldingOS-managed systemd unit, not as an external package such as stunnel.

Port `3443` avoids conflicting with the FoldingOS provisioning API on `:8743`.

### Agent `SUPERVISOR_URL` derivation

Agents already persist the FoldingOS supervisor API base URL at
`/data/config/provision/supervisor.url` (typically `http://<host>:8743`).

`foldingosctl foldops provision` on agents derives FoldOps HTTPS URL from that
file:

1. parse the host from `supervisor.url` (ignore scheme and port)
2. set `SUPERVISOR_URL=https://<host>:3443`

Example:

```text
/data/config/provision/supervisor.url  →  http://192.168.88.238:8743
/data/config/foldops/agent.env         →  SUPERVISOR_URL=https://192.168.88.238:3443
```

On the supervisor node, the co-located agent may use `https://127.0.0.1:3443`
or the same host-derived URL.

Agents use:

```env
SUPERVISOR_URL=https://<supervisor-hostname>:3443
SUPERVISOR_TLS_CA=/data/config/foldops/supervisor-ca.pem
```

On network-provisioned agents, `supervisor-ca.pem` is written during network
install. On supervisor direct-flash recovery paths, `foldops provision` copies
from `/data/foldops/tls/ca.pem`. The HTTPS terminator is owned by FoldingOS; the
Rust FoldOps agent in `packages/foldops/` consumes `SUPERVISOR_TLS_CA` from the
rendered `agent.env`. The supervisor process remains HTTP on loopback.

## Persistent configuration layout

FoldingOS maps Debian `/etc/foldops/*.env` paths to persistent data:

| Debian path | FoldingOS path |
| --- | --- |
| `/etc/foldops/supervisor.env` | `/data/config/foldops/supervisor.env` |
| `/etc/foldops/agent.env` | `/data/config/foldops/agent.env` |
| `/var/lib/foldops/foldops.db` | `/data/foldops/foldops.db` |
| `/usr/share/foldops/web` | `/data/apps/foldops/current/foldops-web/usr/share/foldops/web` after acquisition |

Systemd units use `EnvironmentFile=` pointing at the `/data/config/foldops/`
paths. Symlinks from `/etc/foldops/` are not required.

Supervisor env minimum after provision:

```env
HOST=127.0.0.1
PORT=3000
INGEST_TOKEN=<imported-secret>
DB_PATH=/data/foldops/foldops.db
WEB_ROOT=/data/apps/foldops/current/foldops-web/usr/share/foldops/web
```

Agent env minimum after provision (paths adjusted for FoldingOS FAH layout):

```env
SUPERVISOR_URL=https://<supervisor-host>:3443
SUPERVISOR_TLS_CA=/data/config/foldops/supervisor-ca.pem
AGENT_TOKEN=<same-as-ingest-token>
FAH_LOG_PATH=/data/fah/log.txt
FAH_DB_PATH=/data/fah/client.db
FAH_WORK_DIR=/data/fah/work
```

## Fail-closed service graph

Until `/data/state/foldops/provisioned.json` exists and validates:

- `foldops-supervisor` must not bind off loopback
- HTTPS front end must not listen on `0.0.0.0`
- `foldops-agent` on supervisor and agent roles must not start if FoldOps env is incomplete

After successful `foldingosctl foldops provision`:

- HTTPS listener starts on `0.0.0.0:3443`
- loopback supervisor starts
- agents may POST ingest to the HTTPS URL

FoldOps failure must not block boot or Folding@home ([ADR-0014](0014-fixed-installation-roles.md)).

## Commands

| Command | Role | Purpose |
| --- | --- | --- |
| `foldingosctl foldops acquire` | agent, supervisor | Download and activate FoldOps packages |
| `foldingosctl foldops provision` | agent, supervisor | Import EFI token, TLS, env, provisioned marker |
| `foldingosctl foldops serve-https` | supervisor | TLS terminator on `:3443` → loopback `:3000` |
| `foldingosctl foldops validate-manifest` | any | Verify embedded acquisition manifest |

Supervisor bootstrap order:

```text
role → foldops acquire → foldops provision → provisioning control plane → registry import
```

---

# Alternatives Considered

## Separate FoldOps web username/password

Rejected for Milestone 3 because FoldOps does not implement dashboard login;
`INGEST_TOKEN` is the current security boundary.

## Operator runs foldops provision over SSH only (no EFI)

Rejected because network-provisioned agents need the token written offline to
their target disk; EFI staging matches the established SSH key pattern and
supports recovery without FoldOps availability.

## TLS inside foldops-supervisor binary

Deferred. FoldingOS terminates TLS externally in Milestone 3 via
`foldingosctl foldops serve-https` so appliance delivery is not blocked on
in-process TLS in `packages/foldops/`.

## Public HTTP until TLS exists

Rejected. Remote management must use HTTPS before exposure beyond loopback.

## External TLS terminator (stunnel)

Rejected for Milestone 3. `foldingosctl foldops serve-https` keeps TLS inside
the existing appliance control plane.

## Separate FoldOps URL configuration

Rejected. Agent `SUPERVISOR_URL` is derived from `/data/config/provision/supervisor.url`
so operators do not maintain a second hostname source.

---

# Consequences

## Positive

- One fleet secret, one provisioning story, aligned with FoldOps upstream
- Agents receive token during network install without a separate enrollment API
- EFI recovery path matches ADR-0007 operator workflow
- HTTPS satisfies #62 without inventing FoldOps login

## Negative

- Self-signed TLS requires distributing `supervisor-ca.pem` to agents
- Two-process supervisor stack (HTTPS front + HTTP backend) adds operational surface
- FoldOps agent may require a small change to honor `SUPERVISOR_TLS_CA` if not already present

## Required follow-up

FoldingOS (Issue #62):

- `foldops provision`, `foldops serve-https`, network-install ingest-token and
  CA staging, and `make-bootable-usb --foldops-ingest-token` are implemented in
  `foldingosctl` and the Milestone 3 systemd graph.

- FoldOps agent in `packages/foldops/` honors `SUPERVISOR_TLS_CA` from rendered
  `agent.env` per [ADR-0022](0022-foldops-rust-source-in-foldingos-monorepo.md)

---

# Related Documents

- [Issue #62](https://github.com/pacificnm/folding-os/issues/62)
- [FoldOps integration](../foldops-integration.md)
- [ADR-0022: FoldOps Rust Source In FoldingOS Monorepo](0022-foldops-rust-source-in-foldingos-monorepo.md)
- [ADR-0023: Runtime FoldOps And foldingosctl Updates Without OS Reimage](0023-runtime-foldops-and-foldingosctl-updates-without-os-reimage.md)
- [FoldOps installation](https://www.folding-os.com/foldops)
- [Milestone 3 engineering specification](../milestone/3-engineering-spec.md)
- [foldingosctl command reference](../foldingosctl.md)
