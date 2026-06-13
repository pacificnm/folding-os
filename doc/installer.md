# FoldingOS Deployment and Provisioning

**Version:** 1.0

**Status:** Approved Architecture

---

# Purpose

This document defines how FoldingOS nodes are deployed and how fleets expand
after the first supervisor is installed.

FoldingOS is an appliance operating system. Deployment must remain simple,
reproducible, explicit, and safe while supporting headless fleet operation.

The governing architecture decisions are:

- [ADR-0016](adr/0016-network-provisioning-via-supervisor.md)
- [ADR-0014](adr/0014-fixed-installation-roles.md)

The combined-image USB installer described by superseded
[ADR-0013](adr/0013-combined-appliance-and-installer-image.md) is no longer the
project direction.

---

# Deployment Philosophy

FoldingOS ships one reproducible release image per version. The image always
boots in appliance mode. Fleet expansion is supervisor-centric:

```text
Direct-flash the first supervisor
↓
Supervisor caches approved release images
↓
Blank machines network boot
↓
Supervisor installs agent image and registers the node
↓
Agents check supervisor for updates on boot
```

Direct flash remains supported for supervisor bootstrap and emergency recovery.

---

# Installation Roles

FoldingOS supports exactly two fixed roles:

```text
supervisor
agent
```

| Role | Purpose |
| --- | --- |
| `supervisor` | First node; fleet management, image registry, network provisioning, FoldOps supervisor and web |
| `agent` | Compute node; FoldOps agent and Folding@home runtime |

Roles are fixed for the life of an installation. Changing roles requires fresh
destructive reinstallation.

---

# Supervisor Bootstrap (First Node)

The first node is always the supervisor. Install it by direct flash to internal
NVMe or SATA.

## Prepare boot media

```bash
sudo ./scripts/make-bootable-usb \
  --ssh-public-key /path/to/admin-key.pub \
  /dev/sdX \
  build/output/images/foldingos-x86_64-0.1.0.img
```

Boot the target system from the prepared media or write the image directly to
internal storage from another machine. Use the whole-disk device node, not a
partition path such as `/dev/sdX1`.

See [physical-validation.md](physical-validation.md) and
[operations.md](operations.md) for the validated direct-flash workflow.

## First supervisor boot

After boot the supervisor must:

1. acquire DHCP, DNS, and time synchronization
2. import the staged SSH administrator key
3. persist `role=supervisor`
4. complete FoldOps supervisor administrator and TLS provisioning
5. start the provisioning control plane
6. import the current approved release image into the local registry

The supervisor then polls the upstream release server for newer approved images.

---

# Agent Provisioning (Network Boot)

Additional nodes do not use USB media.

## Prerequisites

- supervisor is operational
- DHCP, TFTP, and HTTP boot services are available (hosted by or coordinated
  with the supervisor)
- blank agent machine has wired Ethernet and internal SATA or NVMe storage
- UEFI network boot is enabled for the agent machine

## Workflow

```text
Enable network boot on blank machine
↓
Machine PXE/iPXE boots
↓
Supervisor recognizes MAC / enrollment token
↓
Supervisor assigns role=agent
↓
Supervisor streams verified image to internal disk over HTTP(S)
↓
Supervisor stages SSH public keys on target EFI partition
↓
Machine reboots into installed appliance
↓
Agent registers with supervisor and begins normal operation
```

TFTP is used only for the PXE/iPXE bootstrap chain. The full release image is
transferred over HTTP or HTTPS.

---

# Updates

The supervisor maintains a registry of approved FoldingOS release images and
polls upstream for new versions.

On boot, each agent asks the supervisor for its desired image version. When a
newer approved version is assigned:

1. the agent downloads and verifies the image
2. stages the update
3. applies it on reboot

See [update-system.md](update-system.md) for trust model and failure behavior.

Folding@home client acquisition remains independent of operating-system updates
and is governed by [ADR-0009](adr/0009-fah-acquisition-and-update-model.md).

---

# SSH-Key Provisioning

Administrator public keys are staged on the EFI System Partition during
installation:

```text
/foldingos/provision/authorized_keys
```

Direct-flash supervisor bootstrap and network-provisioned agents both use this
path. First appliance boot imports keys into persistent configuration per
[ADR-0007](adr/0007-first-boot-administrator-and-ssh-provisioning.md).

---

# Direct Flash (Recovery)

Direct flash remains supported when:

- bootstrapping the first supervisor
- recovering a node without working network provisioning
- performing development and validation

The workflow matches the supervisor bootstrap procedure. Role assignment must be
provisioned as part of the direct-flash transaction defined in the
[Milestone 3 engineering specification](milestone/3-engineering-spec.md).

---

# Safety Requirements

Provisioning must:

- authenticate enrollment or operator-approved install requests
- install only verified images from the supervisor registry
- reject targets smaller than the release image
- never modify the wrong disk
- fail closed on verification errors
- clearly report success or failure

Interrupted network installation may leave the target unbootable. Re-run
network provisioning after correcting the fault.

---

# Non-Goals

Milestone 3 deployment does not provide:

- a separate installer operating system
- USB installer mode with local-console target selection
- custom partitioning
- in-place role changes
- runtime FoldOps package installation from the network
- Folding@home client redistribution inside the OS image

---

# Implementation And Validation

The approved implementation specification is:

[Milestone 3 engineering specification](milestone/3-engineering-spec.md)

Validation must include:

- QEMU network provisioning and update tests
- physical supervisor bootstrap by direct flash
- physical agent provisioning to internal SATA or NVMe
- post-update Folding@home runtime behavior on agents

---

# Related Documents

- [ADR-0016: Network Provisioning Via Supervisor](adr/0016-network-provisioning-via-supervisor.md)
- [Operations](operations.md)
- [Physical validation](physical-validation.md)
- [FoldOps integration](foldops-integration.md)
- [Update system](update-system.md)

---

# Summary

```text
Flash the supervisor once.
Boot the rest from the network.
Let the supervisor keep the fleet current.
```
