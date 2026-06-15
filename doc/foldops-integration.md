# FoldOps Integration

Version: 0.3

Status: Living Document

---

# Purpose

FoldOps provides centralized management, monitoring, and operational visibility
for FoldingOS deployments.

This document defines the intended architectural relationship between FoldingOS
and FoldOps.

FoldOps is developed in a separate repository:

```text
https://github.com/pacificnm/foldops
```

Changes to the node-management protocol, enrollment workflow, configuration
contract, workload-manifest coordination, or update reporting must be
coordinated with that repository.

---

# Design Philosophy

FoldingOS should remain lightweight.

Management complexity belongs in FoldOps rather than on individual nodes.

Nodes should remain simple appliances.

FoldingOS uses the fixed installation roles defined by
[ADR-0014](adr/0014-fixed-installation-roles.md), acquires FoldOps packages per
[ADR-0018](adr/0018-foldops-package-acquisition-and-update-model.md), and
provisions fleets through the supervisor model defined by
[ADR-0016](adr/0016-network-provisioning-via-supervisor.md).

---

# Fleet Bootstrap

The first node in a deployment is a `supervisor` installed by direct flash.

The supervisor:

- runs FoldOps management services
- hosts or coordinates network boot services for agent provisioning
- maintains a registry of approved FoldingOS release images
- polls `releases.folding-os.com` for new FoldingOS image releases
- assigns desired image versions to enrolled agents

FoldOps `.deb` packages and FoldingOS disk images use separate official HTTPS
origins on Cloudflare:

| Channel | Host | Consumer |
| --- | --- | --- |
| FoldOps packages | `deb.folding-os.com` | `apt` on Debian; `foldingosctl foldops acquire` on FoldingOS |
| FoldingOS images | `releases.folding-os.com` | supervisor `foldingosctl registry poll` |

See [ADR-0017](adr/0017-official-release-publication-and-supervisor-upstream-polling.md),
[ADR-0018](adr/0018-foldops-package-acquisition-and-update-model.md), and
[FoldOps installation](https://www.folding-os.com/foldops).

Additional nodes are `agent` roles provisioned over the network by the
supervisor. They do not require USB media or local-console installation.

See [Deployment and provisioning](installer.md).

---

# Installation Roles

FoldingOS supports exactly two fixed roles:

```text
agent
supervisor
```

The agent role runs `foldops-agent` and does not enable the FoldOps supervisor
or web interface.

The supervisor role runs:

```text
foldops-agent
foldops-supervisor
foldops-web
```

The web interface is enabled by default for the supervisor role, but it must
not become remotely available until initial administrator and TLS provisioning
succeeds.

Roles are assigned during provisioning and cannot be changed in place.
Changing roles requires fresh destructive reinstallation.

The supervisor role is assigned during direct-flash bootstrap. Agent roles are
assigned by the supervisor during network provisioning.

Whether the supervisor role also runs Folding@home remains unresolved.

---

# Package Integration

FoldOps packages are **not** embedded in the FoldingOS release image. The image
contains a pinned acquisition manifest and the official archive keyring; see
[ADR-0018](adr/0018-foldops-package-acquisition-and-update-model.md).

## On FoldingOS appliances

After role validation and network availability:

1. `foldingosctl foldops acquire` reads
   `/usr/share/foldingos/manifests/foldops.toml`
2. downloads each required `.deb` from pinned URLs on `deb.folding-os.com`
3. verifies size and SHA-256
4. extracts and activates under `/data/apps/foldops/`
5. enables role-appropriate FoldOps systemd units

FoldingOS does **not** ship runtime APT.

## On general Debian hosts

Operators install the same packages with apt:

```bash
curl -fsSL https://deb.folding-os.com/foldops-archive-keyring.gpg \
  | sudo gpg --dearmor -o /usr/share/keyrings/foldops.gpg

echo 'deb [signed-by=/usr/share/keyrings/foldops.gpg] https://deb.folding-os.com stable main' \
  | sudo tee /etc/apt/sources.list.d/foldops.list

sudo apt update
sudo apt install foldops-agent              # every FAH node
sudo apt install foldops-supervisor         # supervisor node
```

Both paths consume identical official `.deb` artifacts.

## Ingest token and TLS (supervisor bootstrap)

Fleet-wide FoldOps authentication uses a shared ingest secret (`INGEST_TOKEN` /
`AGENT_TOKEN`), not a separate web login. See
[ADR-0019](adr/0019-foldops-supervisor-provisioning-and-tls.md).

| Step | Supervisor | Agent |
| --- | --- | --- |
| EFI staging | Operator writes `/foldingos/provision/foldops-ingest-token` at flash time | Supervisor writes token to target EFI during network install |
| CA trust | Generates `/data/foldops/tls/ca.pem` at provision | Receives `/data/config/foldops/supervisor-ca.pem` on data partition during network install |
| Provision | `foldingosctl foldops provision` → TLS + env | `foldingosctl foldops provision` → `agent.env`; `SUPERVISOR_URL` derived from `supervisor.url` host + `:3443` |
| Remote access | `foldingosctl foldops serve-https` on `:3443` | POST ingest to `https://<supervisor-host>:3443` |

Generate token: `openssl rand -hex 32`

---

# Objectives

FoldOps should enable:

- centralized monitoring
- fleet management
- node inventory
- health reporting
- diagnostics
- update coordination
- remote configuration

without increasing unnecessary complexity on individual nodes.

---

# Node Identity

Each FoldingOS installation should possess a unique identity.

Future implementation details remain subject to ADR approval.

Potential identity sources include:

- generated UUID

- TPM identity

- hardware identity

- cryptographic key pairs

---

# Health Reporting

Potential metrics include:

- uptime

- CPU usage

- memory usage

- storage usage

- temperature

- Folding status

- work unit information

- estimated PPD

- software version

- update status

---

# Inventory

FoldOps should maintain inventory information including:

- hostname

- architecture

- operating system version

- hardware information

- CPU details

- memory

- storage

- network interfaces

---

# Remote Configuration

Future capabilities may include:

- configuration updates

- node naming

- grouping

- labels

- maintenance mode

Configuration changes should remain explicit and auditable.

---

# Updates

FoldOps may coordinate:

- update discovery

- update scheduling

- staged rollout

- rollback

- deployment status

- selection and rollout of approved Folding@home workload manifests

Actual update behavior is defined in the
[update system specification](update-system.md).

**FoldOps package updates** and **FoldingOS image updates** use separate
official channels ([ADR-0017](adr/0017-official-release-publication-and-supervisor-upstream-polling.md)).
Milestone 3 changes FoldOps version pins by embedding an updated manifest in a
new FoldingOS release.

For Folding@home client updates, FoldOps distributes only approved version
policy and manifest metadata. Nodes download pinned artifacts directly from
official Folding@home infrastructure and verify them locally. FoldOps does not
host or proxy Folding@home binaries under the non-redistribution model defined
by [ADR-0009](adr/0009-fah-acquisition-and-update-model.md).

---

# Security

Communication should be:

- authenticated

- encrypted

- verifiable

Nodes should never trust unauthenticated management requests.

---

# Failure Philosophy

Failure of FoldOps should not prevent:

- node boot

- Folding startup

- continued Folding operation

Nodes should continue contributing scientific computation independently.

---

# Long-Term Vision

A single FoldOps deployment should eventually manage:

- one node

- ten nodes

- hundreds of nodes

- thousands of nodes

with a consistent operational model.

---

# Summary

FoldOps exists to simplify management.

FoldingOS exists to perform computation.

The separation of responsibilities should remain clear throughout the lifetime
of the project.
