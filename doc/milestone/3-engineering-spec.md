# FoldingOS Milestone 3 Network Fleet Provisioning Engineering Specification

**Version:** 2.0

**Status:** Approved for Implementation

**Target Milestone:** Milestone 3, Network Fleet Provisioning

**Supersedes:** Milestone 3 combined-image installer engineering specification
v1.1 (2026-06-11)

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

FoldOps package integration, service graphs, and supervisor administrator/TLS
provisioning remain governed by [ADR-0014](../adr/0014-fixed-installation-roles.md)
and require approved FoldOps implementation specifications before release.

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
3. complete FoldOps supervisor administrator and TLS provisioning
4. start the provisioning control plane
5. import or register the initial approved release image into the local registry

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
6. The supervisor writes administrator SSH public keys to the target EFI System
   Partition at `/foldingos/provision/authorized_keys`.
7. The supervisor verifies the written image and instructs the node to reboot.
8. The installed agent completes first-boot appliance initialization, registers
   with the supervisor, and begins normal operation including Folding@home
   acquisition.

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

The supervisor polls the upstream release server on a fixed interval. When a
new approved image is available:

1. download the image and sidecar metadata
2. verify checksum and signature when present
3. mark the image `ready` for rollout
4. expose the new version to operator approval before fleet-wide assignment

The upstream server may be a project-controlled HTTPS endpoint, object storage,
or GitHub Releases. The exact upstream contract is defined in the FoldOps or
FoldingOS release publication workflow.

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
7. Add FoldOps supervisor administrator and TLS provisioning
8. Add QEMU automated validation
9. Complete physical network-provisioned SATA and NVMe validation

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
- FoldOps package download at runtime
- GPU Folding@home support
- operating-system updates without supervisor coordination

---

# Related Documents

- [ADR-0016: Network Provisioning Via Supervisor](../adr/0016-network-provisioning-via-supervisor.md)
- [ADR-0014: Fixed Installation Roles](../adr/0014-fixed-installation-roles.md)
- [Deployment and provisioning](../installer.md)
- [Update system](../update-system.md)
- [FoldOps integration](../foldops-integration.md)
- [Testing strategy](../testing-strategy.md)
