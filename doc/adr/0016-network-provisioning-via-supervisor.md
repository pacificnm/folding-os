# ADR-0016: Network Provisioning Via Supervisor

**Status:** Accepted

**Version:** 1.0

**Date:** 2026-06-13

**Authors:** FoldingOS Project Contributors

**Supersedes:** [ADR-0013](0013-combined-appliance-and-installer-image.md)

**Amends:** [ADR-0014](0014-fixed-installation-roles.md) (role assignment mechanism)

---

# Context

FoldingOS v0.1.0 validated a headless appliance workflow: flash or boot,
acquire DHCP, provision SSH from EFI, acquire Folding@home from upstream HTTPS,
and remain operational without local administration.

[ADR-0013](0013-combined-appliance-and-installer-image.md) addressed internal-disk
deployment through a combined appliance and USB installer image with local-console
installer mode. That approach requires USB media, local or serial-console
interaction, and per-machine manual installation steps.

The project now targets small fleets of homogeneous compute nodes. The first
installed node should act as a **supervisor** that provisions additional
**agent** nodes over the network, tracks desired image versions, and coordinates
operating-system updates. This matches the headless commissioning model already
validated on physical hardware.

Network boot was previously deferred beyond Milestone 3. It is now the primary
fleet expansion mechanism.

---

# Decision

FoldingOS will use **supervisor-led network provisioning** instead of a
combined-image USB installer.

## Bootstrap supervisor

The first node in a fleet is installed by **direct flash** of the release image
to internal NVMe or SATA storage using the existing
`scripts/make-bootable-usb` workflow or an equivalent whole-disk write from
another machine.

That node is provisioned with the fixed `supervisor` role defined by
[ADR-0014](0014-fixed-installation-roles.md). The supervisor runs FoldOps
management services and the FoldingOS provisioning control plane.

## Agent provisioning

Additional nodes boot from the network using UEFI PXE and iPXE. The boot chain
uses:

- **DHCP** for network configuration and PXE options
- **TFTP** only for the initial iPXE loader and bootstrap script
- **HTTP or HTTPS** for the iPXE script, kernel, and full release disk image

Blank machines do not require USB media or a local keyboard. The supervisor:

1. recognizes the requesting node (MAC address, optional enrollment token)
2. assigns the fixed `agent` role
3. stages administrator SSH public keys on the target EFI System Partition
4. streams the verified release image to the selected internal target disk
5. verifies the installation
6. directs the node to reboot into appliance mode

## Image registry and updates

The supervisor maintains a local registry of approved FoldingOS release images.

The supervisor periodically polls an upstream release server for new signed or
checksum-verified images. When a newer approved image is available, the
supervisor downloads it, verifies it, and marks it ready for rollout.

Agent nodes check the supervisor on boot for their **desired image version**.
When the supervisor assigns a newer version, the agent downloads the image
through the supervisor (or a documented redirect to the upstream origin),
verifies it, stages the update, and applies it on reboot using the update
workflow defined in [update-system.md](../update-system.md).

## Direct flash remains supported

Direct flash remains the supported bootstrap path for:

- the first supervisor node
- emergency recovery
- development and validation

Direct flash does not replace network provisioning for routine fleet expansion.

## Single release artifact

FoldingOS continues to ship one reproducible raw GPT disk image per release.
The image does not contain a separate installer operating system and does not
provide a GRUB `Install FoldingOS` entry.

Appliance images for `agent` and `supervisor` roles use the same root
filesystem. Role differences are limited to enabled services and persistent role
state provisioned at install time.

---

# Alternatives Considered

## Combined-image USB installer (ADR-0013)

Advantages:

- no network infrastructure required
- simple mental model

Disadvantages:

- poor fit for headless fleet expansion
- repeated manual USB workflow per node
- duplicates provisioning logic that belongs in the supervisor

Decision:

Superseded by this ADR.

## TFTP-only transfer of the full disk image

Advantages:

- minimal services

Disadvantages:

- slow and unreliable for multi-gigabyte images
- poor resume and verification behavior

Decision:

Rejected. TFTP is limited to the PXE/iPXE bootstrap chain. Image transfer uses
HTTP or HTTPS.

## Separate installer operating system

Advantages:

- isolated installation environment

Disadvantages:

- second kernel, root filesystem, and validation matrix

Decision:

Rejected, consistent with ADR-0013 rationale.

---

# Consequences

## Positive

- fleet expansion matches the validated headless appliance model
- one supervisor coordinates provisioning, enrollment, and image rollout
- operators avoid per-node USB installation for internal disks
- direct flash remains available for bootstrap and recovery
- one release image and one build matrix are preserved

## Negative

- requires DHCP, TFTP, and HTTP infrastructure (usually provided by or
  coordinated with the supervisor)
- network provisioning must be implemented and validated before Milestone 3
  completion
- update and rollback behavior must be specified and tested
- initial supervisor bootstrap still requires one direct-flash step

---

# Security Requirements

- only approved, checksum-verified, and when available cryptographically signed
  release images may be installed or staged
- provisioning requests must be authenticated (enrollment token or supervisor
  operator approval)
- the supervisor must not install arbitrary images outside its verified registry
- agent nodes must fail closed when an assigned image fails verification
- SSH public-key provisioning follows [ADR-0007](0007-first-boot-administrator-and-ssh-provisioning.md)
- Folding@home acquisition remains governed by
  [ADR-0009](0009-fah-acquisition-and-update-model.md); OS provisioning does
  not redistribute FAH client binaries

---

# Standalone Operation

Network provisioning requires a reachable supervisor during initial agent
installation.

After installation, agent nodes must continue to satisfy [ADR-0009](0009-fah-acquisition-and-update-model.md):
Folding@home operation must not depend on FoldOps or supervisor availability.
Loss of the supervisor must not stop an already installed and verified FAH
client.

---

# Related Documents

- [ADR-0013: Combined Appliance And Installer Image](0013-combined-appliance-and-installer-image.md) (superseded)
- [ADR-0014: Fixed Installation Roles](0014-fixed-installation-roles.md)
- [ADR-0007: First-Boot Administrator and SSH-Key Provisioning](0007-first-boot-administrator-and-ssh-provisioning.md)
- [ADR-0009: Folding@home Acquisition and Update Model](0009-fah-acquisition-and-update-model.md)
- [Deployment and provisioning](../installer.md)
- [Milestone 3 engineering specification](../milestone/3-engineering-spec.md)
- [Update system](../update-system.md)
- [FoldOps integration](../foldops-integration.md)

---

# Closing Statement

FoldingOS remains one reproducible appliance image. Deployment becomes
supervisor-centric: flash the first node, boot the rest from the network, and
let the supervisor keep the fleet on approved image versions.
