# FoldingOS Milestone 3 Network Fleet Provisioning Engineering Specification

**Version:** 2.3

**Status:** Approved for Implementation

**Target Milestone:** Milestone 3, Network Fleet Provisioning

**Supersedes:** Milestone 3 combined-image installer engineering specification
v1.1 (2026-06-11)

**Revision 2.3 (2026-06-15):** Upstream object prefix `/release/` on
`releases.folding-os.com` per ADR-0017 v1.1.

**Revision 2.2 (2026-06-15):** Official upstream release origin (`releases.folding-os.com`)
per ADR-0017.

---

# Approval Record

The project owner approved the network fleet provisioning strategy on
2026-06-13:

- supersede the combined-image USB installer ([ADR-0013](../adr/0013-combined-appliance-and-installer-image.md))
- adopt supervisor-led network provisioning ([ADR-0016](../adr/0016-network-provisioning-via-supervisor.md))
- bootstrap the first supervisor by direct flash to NVMe or SATA
- provision agent nodes over UEFI PXE/iPXE with HTTP image transfer
- maintain a supervisor-local registry of approved release images
- poll upstream for new releases and stage fleet rollouts
- have agents check the supervisor on boot for desired image version
- preserve direct flash for supervisor bootstrap and emergency recovery
- preserve fixed `agent` and `supervisor` roles ([ADR-0014](../adr/0014-fixed-installation-roles.md))

---

# Purpose

This document defines the concrete implementation of supervisor-led network
fleet provisioning accepted by
[ADR-0016](../adr/0016-network-provisioning-via-supervisor.md).

Milestone 3 delivers:

- supervisor bootstrap by direct flash
- network boot and remote installation of agent nodes
- node registration with the supervisor
- supervisor image registry and upstream release polling
- agent desired-version checks and staged operating-system updates

---

# Scope

Milestone 3 adds:

- supervisor provisioning control plane
- DHCP/TFTP/HTTP boot services (hosted by or coordinated with the supervisor)
- iPXE boot chain for blank agent machines
- authenticated agent enrollment and image streaming
- persistent role assignment for `agent` and `supervisor`
- image registry, verification, and rollout state on the supervisor
- agent boot-time desired-version check and staged update workflow
- automated QEMU validation for provisioning and update paths
- physical validation for network-provisioned SATA and NVMe targets

Milestone 3 does not add:

- a separate installer operating system
- a GRUB `Install FoldingOS` boot entry
- local-console USB installer mode
- custom partitioning
- data-preserving in-place role changes
- GPU management
- Folding@home client redistribution

FoldOps package acquisition, service graphs, and supervisor ingest-token/TLS
provisioning are governed by [ADR-0014](../adr/0014-fixed-installation-roles.md),
[ADR-0018](../adr/0018-foldops-package-acquisition-and-update-model.md), and
[ADR-0019](../adr/0019-foldops-supervisor-provisioning-and-tls.md).

---

# Roles

FoldingOS supports exactly two fixed roles:

```text
supervisor
agent
```

| Role | Installation method | Services |
| --- | --- | --- |
| `supervisor` | Direct flash to internal NVMe or SATA | FoldOps agent, supervisor, web; provisioning control plane; image registry |
| `agent` | Network provisioning by supervisor | FoldOps agent; Folding@home runtime |

The first node in a fleet is always a `supervisor`. Additional nodes are
`agent` nodes provisioned over the network.

Role changes require fresh destructive reinstallation.

---

# Deployment Overview

```text
Upstream release server
        │
        │ periodic poll + verified download
        ▼
Supervisor (direct-flashed to NVMe/SATA)
        │
        ├─ DHCP / TFTP / HTTP boot services
        ├─ image registry + rollout state
        ├─ enrollment + provisioning API
        │
        ├─ network boot → stream image → agent node 1
        ├─ network boot → stream image → agent node 2
        └─ network boot → stream image → agent node N
```

---

# Supervisor Bootstrap

## Direct flash

The supervisor is installed by writing the release image to internal storage
from another machine or by booting prepared USB media and selecting the normal
appliance entry.

Required workflow:

```bash
sudo ./scripts/make-bootable-usb \
  --ssh-public-key /path/to/admin-key.pub \
  --role supervisor \
  /dev/sdX \
  build/output/images/foldingos-x86_64-0.1.0.img
```

`--role supervisor` is a required Milestone 3 extension to
`scripts/make-bootable-usb`. Until implemented, operators may provision the
supervisor role through the approved interim mechanism defined in the
implementation sequence below.

After first boot the supervisor must:

1. complete normal appliance initialization
2. persist `role=supervisor`
3. acquire and activate required FoldOps packages (`foldingosctl foldops acquire`)
4. complete FoldOps ingest-token and TLS provisioning (`foldingosctl foldops provision`)
5. start the provisioning control plane
6. import or register the initial approved release image into the local registry

## Persistent role storage

The installation role must be persisted before role-specific services start.
Approved persistent location:

```text
/data/config/installation-role
```

The file must contain exactly `agent` or `supervisor`. Invalid or missing role
state must fail closed for role-specific FoldOps services.

---

# Network Boot Architecture

## Boot chain

Blank agent machines use UEFI network boot:

```text
UEFI firmware
  → DHCP (options 66/67 or DHCP user class)
  → TFTP: iPXE loader
  → HTTP(S): iPXE script
  → HTTP(S): kernel / initrd if required, or direct disk-image staging flow
  → supervisor-directed install
  → reboot to internal disk
```

TFTP is used only for the PXE/iPXE bootstrap chain. The full release image is
never transferred over TFTP.

## Provisioning flow

1. Agent machine powers on with network boot enabled and no local boot disk.
2. iPXE obtains an address and fetches the supervisor-provided script.
3. The supervisor recognizes the node by MAC address and optional enrollment
   token.
4. The supervisor assigns `role=agent`, reserves the node, and returns install
   parameters.
5. The supervisor streams the verified release image to the selected internal
   target disk over HTTP or HTTPS.
6. The supervisor resets inherited persistent state on the target data
   partition, then writes agent-only provisioning files under `/data/config/`.
7. The supervisor writes administrator SSH public keys and the fleet ingest
   token to the target EFI System Partition at
   `/foldingos/provision/authorized_keys` and
   `/foldingos/provision/foldops-ingest-token`, and clears any inherited GRUB
   one-shot boot state on the target EFI partition.
8. The supervisor verifies the written image and instructs the node to reboot.
9. The installed agent completes first-boot appliance initialization, registers
   with the supervisor, and begins normal operation including Folding@home
   acquisition.

### Inherited state reset during network install

`registry import-bootstrap` copies the running supervisor disk into the local
registry ([foldingosctl.md](../foldingosctl.md)). That image may contain
supervisor-local persistent state in partition 3 and stale GRUB environment
state on the EFI partition. Network install must not deliver that inherited
runtime state to a newly provisioned agent ([ADR-0014](../adr/0014-fixed-installation-roles.md)).

After the release image is written to the target disk and before agent-only
staging files are created, network install must:

1. remove inherited runtime trees under `/data/config/`, `/data/registry/`,
   `/data/provision/`, and `/data/state/` on the target data partition
2. write fresh agent-only files under `/data/config/` and
   `/data/config/provision/` as defined by `foldingosctl provision install`
3. not inherit a persistent node identity; node identity is created on first
   agent boot by existing identity services

After the release image is written and before the target reboots, network
install must clear `next_entry` from EFI `grubenv` when present so the agent's
first boot uses the normal GRUB entry unless a later supervisor-assigned update
schedules the update boot path.

These steps are implemented by `foldingosctl provision install` during network
install. They are not performed on running agents after installation.

## Target requirements

Network provisioning supports internal storage only in Milestone 3:

- SATA
- NVMe

USB-attached targets remain out of scope for network provisioning. Direct flash
remains available for removable media.

Every eligible target must expose a non-empty serial number before installation
proceeds.

---

# Image Registry And Upstream Polling

The supervisor maintains a local registry of approved FoldingOS release images.

Each registry entry must record at minimum:

- FoldingOS version
- Git revision
- image SHA-256
- artifact size
- retrieval URL
- signature or checksum verification metadata
- import timestamp
- rollout state (`staged`, `ready`, `retired`)

The supervisor polls the official upstream release origin on a fixed interval.
See [ADR-0017](../adr/0017-official-release-publication-and-supervisor-upstream-polling.md).

Default manifest URL on supervisor appliances:

```text
https://releases.folding-os.com/release/releases.json
```

Configured at `/data/config/provision/upstream-releases.url`. When a new approved
image is available:

1. download the image and sidecar metadata from `releases.folding-os.com`
2. verify SHA-256 digest and size
3. mark the image `ready` for rollout
4. expose the new version to operator approval before fleet-wide assignment

Published disk images use:

```text
https://releases.folding-os.com/release/images/foldingos-x86_64-<version>.img
```

FoldOps Debian packages remain on `deb.folding-os.com` ([FoldOps install](https://www.folding-os.com/foldops)); operating-system images use the releases host above. FoldingOS nodes acquire FoldOps packages at runtime per [ADR-0018](../adr/0018-foldops-package-acquisition-and-update-model.md).

---

# FoldOps Package Acquisition

FoldingOS release images embed a pinned FoldOps acquisition manifest and the
official archive keyring. They do **not** embed FoldOps application binaries.

## Manifest and trust

| Path | Purpose |
| --- | --- |
| `/usr/share/foldingos/manifests/foldops.toml` | pinned package URLs, sizes, and SHA-256 digests |
| `/usr/share/keyrings/foldops.gpg` | official apt archive keyring (same as Debian install) |

Builds must verify the manifest before image generation, analogous to
`scripts/verify-fah-manifest`.

## Role-specific packages

| Role | Packages |
| --- | --- |
| `agent` | `foldops-agent` |
| `supervisor` | `foldops-agent`, `foldops-supervisor`, `foldops-web` |

## Acquisition command

`foldingosctl foldops acquire`:

1. reads the embedded manifest
2. downloads each required `.deb` from pinned HTTPS URLs on `deb.folding-os.com`
3. verifies size and SHA-256
4. extracts the Debian data archive only (no maintainer scripts)
5. installs under `/data/apps/foldops/<manifest_release>/`
6. atomically activates `/data/apps/foldops/current`
7. writes a verified marker

Staging and retry state live under `/data/state/foldops/`.

## Boot ordering

FoldOps acquisition runs after:

- installation role is validated
- networking and time synchronization are available

FoldOps systemd units must not start until acquisition succeeds. On supervisor
role, FoldOps web must not listen remotely until administrator and TLS
provisioning also succeed.

Scheduled acquisition uses `foldingos-foldops-acquire.timer`, analogous to
Folding@home acquisition.

## Parallel Debian install path

General Debian hosts install the same artifacts with `apt` against
`deb.folding-os.com`. See [FoldOps integration](../foldops-integration.md).

---

# FoldOps Ingest-Token And TLS Provisioning

Defined by [ADR-0019](../adr/0019-foldops-supervisor-provisioning-and-tls.md).

## EFI staging

| File | Purpose |
| --- | --- |
| `/foldingos/provision/foldops-ingest-token` | Fleet-wide `INGEST_TOKEN` / `AGENT_TOKEN` (64 hex chars) |

Supervisor direct flash: operator writes this file before first boot (via
`make-bootable-usb --foldops-ingest-token` or manual ESP edit).

Network-provisioned agents: supervisor copies the imported token from
`/data/config/foldops/ingest-token` to the target EFI partition during network
install, parallel to SSH keys.

## Provision command

`foldingosctl foldops provision`:

1. reads and validates the EFI token file (supervisor and agent)
2. persists `/data/config/foldops/ingest-token` (`0600`) on supervisor
3. on supervisor: generates self-signed TLS under `/data/foldops/tls/`
4. renders `/data/config/foldops/supervisor.env` and local `agent.env`
5. on agent: renders `agent.env` with `AGENT_TOKEN` and
   `SUPERVISOR_URL=https://<host>:3443`, where `<host>` is parsed from
   `/data/config/provision/supervisor.url`
6. writes `/data/state/foldops/provisioned.json`
7. removes the EFI staging file after successful import

Network install also writes `/data/config/foldops/supervisor-ca.pem` on the
target data partition (public CA copied from the supervisor).

## HTTPS front end

| Listener | Address | Purpose |
| --- | --- | --- |
| `foldingosctl foldops serve-https` | `0.0.0.0:3443` | TLS terminator → loopback `:3000` |
| `foldops-supervisor` | `127.0.0.1:3000` | Loopback HTTP only |

Remote HTTPS must not listen until provision succeeds.

## Boot ordering (supervisor)

```text
foldingos-installation-role.service
  → foldingos-foldops-acquire.service
  → foldingos-foldops-provision.service
  → foldingos-foldops-provision.service
  → foldingos-provision.service / registry import
  → foldingos-foldops-serve-https.service + foldops-supervisor (loopback)
```

Agents run `foldops acquire` → `foldops provision` before `foldops-agent`.

---

# Agent Registration And Desired Version

After installation, each agent registers with the supervisor and reports:

- persistent node identity
- MAC addresses
- hardware inventory
- current image version
- FoldingOS and FoldOps versions
- Folding@home runtime status when available

On every boot, the agent requests its desired image version from the supervisor.
The supervisor returns one of:

- `current` — no change required
- `<version>` — a newer approved image assigned by rollout policy

When a newer version is assigned:

1. the agent downloads the image from the supervisor registry or approved redirect
2. verifies checksum and signature
3. stages the update
4. applies the update on reboot

Agents must not install images outside the supervisor-approved registry.

Loss of supervisor connectivity must not stop an already installed agent from
running Folding@home, per [ADR-0009](../adr/0009-fah-acquisition-and-update-model.md).

---

# SSH-Key Provisioning

Network-provisioned agents receive administrator public keys through the target
EFI System Partition:

```text
/foldingos/provision/authorized_keys
```

The supervisor stages keys during installation. First appliance boot imports
them into persistent configuration per
[ADR-0007](../adr/0007-first-boot-administrator-and-ssh-provisioning.md).

The supervisor bootstrap uses the same EFI staging path during direct flash.

---

# Update Workflow

Milestone 3 uses full-image replacement for agent updates.

Required behavior:

- preserve `/data/config`, `/data/fah`, and other approved persistent domains
  where the update specification allows
- verify the staged image before switching boot priority
- fail closed and retain the previous bootable image on verification failure
- report update status to the supervisor

Staged update metadata at `/data/state/provision/staged-update.json` must
record an `apply_state` lifecycle:

| `apply_state` | Meaning |
| --- | --- |
| `staged` | Verified update image and metadata are present; ready to schedule the one-shot update boot |
| `boot_scheduled` | GRUB `next_entry` is set for the update boot path; waiting for update initramfs boot |
| `applying` | Update initramfs offline apply is running |
| `failed` | Offline apply failed; the agent remains on the current bootable image |

Required transitions:

1. `check-version` creates staged files with `apply_state=staged`.
2. Normal-boot `apply-update` runs while `apply_state` is `staged` or retries while
   `boot_scheduled`, sets `boot_scheduled`, stages update boot assets, sets GRUB
   `next_entry` to the update menu entry index `1` (`foldingos-update`), and reboots once.
3. Update initramfs `apply-update --offline` sets `applying`, copies the staged
   image EFI and root partitions onto the boot disk while preserving partition
   3, records outcome in `/data/state/provision/pending-update-report.json`,
   clears staged files on success, and reboots. The update initramfs has no
   network stack; `check-version` on the first normal boot with network
   delivers the pending report to the supervisor and removes the pending file.
4. On offline apply failure, the agent sets `apply_state=failed`, reports
   `failed` to the supervisor, and reboots into the normal boot path without
   scheduling another update boot automatically.
5. While `apply_state` is `boot_scheduled`, `applying`, or `failed`,
   `check-version` must not overwrite staged update files. Retry requires a new
   supervisor assignment or operator recovery action.

`foldingos-agent-apply-update.service` must invoke normal-boot `apply-update`
while `apply_state` is `staged` or `boot_scheduled`.

A/B partition slots and automatic rollback remain future enhancements documented
in [update-system.md](../update-system.md). Milestone 3 must not leave agents
without a bootable fallback image after a failed update attempt.

---

# FoldingOS Commands And Services

Milestone 3 adds or extends:

| Component | Responsibility |
| --- | --- |
| `foldingosctl provision serve` | Supervisor provisioning API and boot services |
| `foldingosctl provision enroll` | Agent registration after first boot |
| `foldingosctl provision check-version` | Agent desired-version query on boot |
| `foldingosctl provision apply-update` | Agent staged image activation |
| `foldingos-provision.service` | Supervisor control plane |
| `foldingos-provision-boot.service` | DHCP/TFTP/HTTP boot assistance |
| `foldingos-agent-register.service` | Agent registration oneshot |
| `foldingos-agent-version-check.service` | Agent desired-version check on boot |
| `foldingos-agent-apply-update.service` | Agent staged update scheduling on boot while `apply_state` is `staged` or `boot_scheduled` |

GRUB EFI must include the `loadenv` module so `grub.cfg` can read and clear
`next_entry` from `grubenv` for one-shot update boots
(`BR2_TARGET_GRUB2_BUILTIN_MODULES_EFI` in the Buildroot defconfig).

Exact unit names, dependencies, and failure behavior must match the
implementation merged with this specification.

---

# Safety Requirements

Provisioning and updates must:

- authenticate enrollment or operator-approved installation requests
- never write to a target before identity revalidation
- reject targets smaller than the release image
- reject unknown or unverified images
- stop on verification failure
- log failures through `systemd-journald`
- avoid exposing unprovisioned supervisor management interfaces

Interrupted network installation may leave the target unbootable. Repeating
network provisioning is the recovery path.

---

# Implementation Sequence

Implementation should proceed in this order:

1. Persist and validate installation role on direct-flash bootstrap
2. Implement supervisor image registry and upstream polling
3. Implement agent registration and desired-version API
4. Implement supervisor HTTP image streaming to a selected target
5. Add iPXE/TFTP boot assistance and enrollment
6. Implement agent staged update and reboot apply
7. Implement FoldOps package acquisition from `deb.folding-os.com`
8. Add FoldOps ingest-token and TLS provisioning ([#62](https://github.com/pacificnm/folding-os/issues/62))
9. Add QEMU automated validation
10. Complete physical network-provisioned SATA and NVMe validation

Each step must add its required automated tests before dependent steps proceed.

---

# Validation

## QEMU/OVMF

`scripts/test-provision-qemu` must validate at minimum:

- supervisor registry imports a verified release image
- a blank QEMU guest can network boot through the supervisor path
- only the selected target disk is modified
- undersized targets are rejected
- invalid or unenrolled requests are rejected
- installed agents boot, expand `/data`, and register successfully
- desired-version assignment stages and applies an update
- failed image verification does not replace the active boot image

## Physical

Physical validation must cover:

- supervisor direct-flash bootstrap to NVMe or SATA
- network provisioning of at least one agent to internal SATA or NVMe
- agent registration and SSH access after provisioning
- desired-version update across reboot
- Folding@home runtime still starts on updated agents

Validation records must be committed before Milestone 3 is marked complete.

---

# Non-Goals

Milestone 3 does not implement:

- USB-source network provisioning for internal disks
- local-console installer mode
- arbitrary package selection beyond fixed roles
- runtime APT or general-purpose package management
- GPU Folding@home support
- operating-system updates without supervisor coordination

---

# Related Documents

- [ADR-0018: FoldOps Package Acquisition And Update Model](../adr/0018-foldops-package-acquisition-and-update-model.md)
- [ADR-0016: Network Provisioning Via Supervisor](../adr/0016-network-provisioning-via-supervisor.md)
- [ADR-0014: Fixed Installation Roles](../adr/0014-fixed-installation-roles.md)
- [ADR-0019: FoldOps Supervisor Provisioning And TLS](../adr/0019-foldops-supervisor-provisioning-and-tls.md)
- [Deployment and provisioning](../installer.md)
- [Update system](../update-system.md)
- [FoldOps integration](../foldops-integration.md)
- [Testing strategy](../testing-strategy.md)
