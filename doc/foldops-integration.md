# FoldOps Integration

Version: 0.3

Status: Living Document

---

# Purpose

FoldOps provides centralized management, monitoring, and operational visibility
for FoldingOS deployments.

This document defines the intended architectural relationship between FoldingOS
and FoldOps.

FoldOps Rust source for FoldingOS appliances lives in this repository under
`packages/foldops/` per
[ADR-0022](adr/0022-foldops-rust-source-in-foldingos-monorepo.md). The legacy
Node.js repository at [pacificnm/foldops](https://github.com/pacificnm/foldops)
is deprecated for appliance work.

Changes to the node-management protocol, enrollment workflow, configuration
contract, workload-manifest coordination, or update reporting are coordinated in
this repository alongside `foldingosctl` and acquisition manifests.

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

FoldOps layout bundles, `foldingosctl` tools binaries, and FoldingOS disk images
use separate official HTTPS origins:

| Channel | Host | Consumer |
| --- | --- | --- |
| FoldOps bundles | `packages.folding-os.com/foldops/` | `foldingosctl foldops acquire` on FoldingOS |
| foldingosctl tools | `packages.folding-os.com/foldingos-tools/` | `foldingosctl tools acquire` on FoldingOS |
| FoldOps Debian packages (optional) | `deb.folding-os.com` | `apt` on general Debian hosts only |
| FoldingOS images | `releases.folding-os.com` | supervisor `foldingosctl registry poll` |

FoldingOS appliances do not use runtime `apt`. See
[ADR-0023](adr/0023-runtime-foldops-and-foldingosctl-updates-without-os-reimage.md).

See [ADR-0017](adr/0017-official-release-publication-and-supervisor-upstream-polling.md),
[ADR-0018](adr/0018-foldops-package-acquisition-and-update-model.md),
[milestone/4-appliance-artifact-and-monorepo-plan.md](milestone/4-appliance-artifact-and-monorepo-plan.md), and
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

Milestone 4 integration strategy: FoldOps services on FoldingOS appliances
**delegate node-local operations to `foldingosctl`** rather than reimplementing
appliance inspection. See [ADR-0020](adr/0020-foldops-delegates-node-operations-to-foldingosctl.md)
and [milestone/4-engineering-spec.md](milestone/4-engineering-spec.md).

## On FoldingOS appliances

After role validation and network availability:

1. `foldingosctl foldops acquire` reads the bootstrap manifest at
   `/usr/share/foldingos/manifests/foldops.toml` and, when present, the
   supervisor-assigned manifest at `/data/config/foldops/assigned-manifest.toml`
2. downloads each required `layout-tar-zst` bundle (or legacy `.deb` during
   migration) from pinned URLs on `packages.folding-os.com`
3. verifies size and SHA-256
4. extracts and activates under `/data/apps/foldops/`
5. `foldingosctl foldops provision` imports the ingest token, renders env
   files, generates supervisor TLS when required, and writes
   `/data/state/foldops/provisioned.json`
6. role-appropriate FoldOps systemd units start (`foldingos-foldops-serve-https`,
   `foldingos-foldops-supervisor`, and `foldingos-foldops-agent`; see
   [foldingosctl.md](foldingosctl.md))

FoldingOS does **not** ship runtime APT.

`foldingosctl tools acquire` updates the control-plane binary from
`packages.folding-os.com/foldingos-tools/` without OS reimage per
[ADR-0023](adr/0023-runtime-foldops-and-foldingosctl-updates-without-os-reimage.md).

Milestone 5 delivered the **operator update loop** (check for updates, assign
fleet versions, trigger apply) and rclone publication automation. See
[ADR-0028](adr/0028-supervisor-fleet-software-update-workflow.md),
[ADR-0029](adr/0029-packages-channel-publication-via-rclone.md), and
[milestone/5-engineering-spec.md](milestone/5-engineering-spec.md). Full dashboard
rework follows in FoldOps Upgrades (Milestone 6).

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

Both paths may consume the same release artifacts; Debian hosts use `.deb`,
FoldingOS appliances use layout bundles.

## Ingest token, TLS, and operator bootstrap (supervisor)

Fleet machine authentication uses a shared ingest secret (`INGEST_TOKEN` /
`AGENT_TOKEN`). Human dashboard access uses a separate operator account per
[ADR-0026](adr/0026-foldops-dashboard-operator-authentication.md).

**Milestone 4 target:** flash a generic supervisor image, complete first-run
dashboard login (default credentials with mandatory password change), and let
supervisor setup generate the fleet ingest token and TLS material.

**Current and transitional path:** operators may still pre-stage EFI secrets at
flash time. See [ADR-0019](adr/0019-foldops-supervisor-provisioning-and-tls.md).

| Step | Supervisor | Agent |
| --- | --- | --- |
| Operator access | Dashboard login per [ADR-0026](adr/0026-foldops-dashboard-operator-authentication.md); optional SSH keys via EFI | SSH keys staged by supervisor during network install when configured |
| Fleet token | Generated during first-run setup or imported from EFI | Supervisor writes token to target EFI during network install |
| CA trust | Generates `/data/foldops/tls/ca.pem` at provision | Receives `/data/config/foldops/supervisor-ca.pem` on data partition during network install |
| Provision | `foldingosctl foldops provision` → TLS + env | `foldingosctl foldops provision` → `agent.env`; `SUPERVISOR_URL` derived from `supervisor.url` host + `:3443` |
| Remote access | `foldingosctl foldops serve-https` on `:3443` | POST ingest to `https://<supervisor-host>:3443` |

Remote operator actions from the dashboard use the supervisor API model in
[ADR-0027](adr/0027-foldops-remote-operator-api.md).

Generate token manually when pre-staging EFI: `openssl rand -hex 32`

Supervisor USB preparation (optional EFI pre-staging):

```bash
sudo ./scripts/make-bootable-usb \
  --ssh-public-key /path/to/admin-key.pub \
  --role supervisor \
  --foldops-ingest-token /path/to/foldops-ingest-token \
  /dev/sdX build/output/images/foldingos-x86_64-0.1.0.img
```

Agent HTTPS trust for self-signed supervisor TLS uses `SUPERVISOR_TLS_CA` in the
Rust FoldOps agent (`packages/foldops/`). FoldingOS stages the CA and terminates
TLS on the supervisor per [ADR-0019](adr/0019-foldops-supervisor-provisioning-and-tls.md).

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

Each FoldingOS installation possesses a persistent identity at
`/data/config/node-id`, created by `foldingosctl identity ensure`.

Milestone 4 requires FoldOps ingest to include this `node_id` so dashboard
inventory matches provisioning enrollment and update state. FoldOps must obtain
identity through `foldingosctl inspect node --format json`, not by generating a
separate identifier on FoldingOS appliances.

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
