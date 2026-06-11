# ADR-0013: Combined Appliance And Installer Image

**Status:** Accepted

**Version:** 1.0

**Date:** 2026-06-11

**Authors:** FoldingOS Project Contributors

---

# Context

FoldingOS is distributed as a complete raw disk image. The v0.1.0 deployment
workflow requires an administrator to write that image and provision SSH keys
from another operating system before booting the target node.

That workflow is suitable for removable media and virtual machines, but it is
awkward for systems whose intended boot disk is installed internally. Requiring
administrators to remove internal disks or attach them to another machine
creates unnecessary deployment friction.

A separate installer operating system would solve the deployment problem, but
would introduce another kernel, root filesystem, package set, security surface,
build artifact, and validation matrix. FoldingOS should not maintain two
operating systems when the appliance image already contains the capabilities
needed to install itself.

---

# Decision

FoldingOS will use one combined appliance and installer image.

The same release image, kernel, root filesystem, packages, and system utilities
will support two explicit boot modes:

```text
FoldingOS appliance mode
FoldingOS installer mode
```

GRUB will select the boot mode by passing a documented kernel command-line
parameter:

```text
foldingos.mode=appliance
foldingos.mode=installer
```

Appliance mode remains the default.

Installer mode will activate:

```text
foldingos-installer.target
```

instead of the normal appliance boot target. Installer mode is a local-console,
explicitly initiated, destructive installation environment. It is not a
second operating system and does not have an independently maintained root
filesystem or package set.

---

# Installer Behavior

Installer mode will:

1. Identify the physical boot device containing the running FoldingOS image.
2. Preserve the source image's fixed release layout by preventing automatic
   data-partition expansion.
3. Confirm that the source media retains the expected release-image geometry
   and contains no appliance-generated persistent node state.
4. Avoid normal appliance initialization, including node identity creation,
   SSH host-key generation, SSH service startup, Folding@home acquisition, and
   persistent runtime-state creation.
5. Discover eligible target disks while excluding the source boot device and
   its partitions.
6. Require explicit local-console selection of one target disk.
7. Display the target's stable identifying information, capacity, and device
   path before making changes.
8. Require destructive confirmation that names the selected target.
9. Refuse targets smaller than the release image.
10. Copy exactly the fixed release-image byte range from the source boot device
   to the selected target.
11. Provision administrator public keys through the target EFI System
    Partition using the path defined by ADR-0007.
12. Flush writes and verify the installed image before reporting success.
13. Require reboot or poweroff after successful installation.

The installed target will perform normal first-boot behavior, including
data-partition expansion, node identity creation, SSH key import, SSH host-key
generation, and appliance service startup.

---

# Installation Command

The installer workflow will be implemented through:

```text
foldingosctl install
```

The command is available only in installer mode and requires a local console.
It must refuse to run in appliance mode.

The first implementation supports fresh destructive installation only. It does
not preserve data already present on the selected target.

---

# Source And Target Safety

The installer must fail closed when it cannot unambiguously identify the source
boot device or selected target.

The installer must never:

- select a target automatically
- write to the source boot device
- accept a source partition as a target
- write to any unselected disk
- continue after target identity changes
- install to a target smaller than the fixed release image
- silently preserve or overwrite existing target data
- install deployment-specific state created while running installer mode
- install from source media containing appliance-generated persistent node
  state

Before writing, the installer must confirm that the selected target is not the
source device and must revalidate that relationship immediately before the
first destructive operation.

Destructive confirmation must require the administrator to enter a value that
identifies the selected target. A generic yes/no confirmation is insufficient.

---

# SSH-Key Provisioning

Installer mode will provision public keys using the existing ADR-0007 channel:

```text
/foldingos/provision/authorized_keys
```

on the installed target's EFI System Partition.

The first implementation will accept:

- a public-key file present on the source EFI System Partition
- public keys entered through the local installer console

Keys must pass the same validation rules used by normal FoldingOS SSH
provisioning. Private keys must never be requested, copied, or stored.

The installed appliance remains responsible for importing validated keys into
persistent configuration and starting OpenSSH.

---

# Release And Reproducibility Model

The combined image remains the primary release artifact defined by ADR-0003.
No separate installer image is produced.

Installer capability must not make the release image deployment-specific or
reduce reproducibility. Required release artifacts remain subject to ADR-0012.

The installer copies the fixed release-image byte range from eligible source
media. Administrator public keys may be staged on the source EFI System
Partition, but installer mode must not otherwise mutate the copied byte range
before installation. Source media containing appliance-generated persistent
node state is not eligible installation media.

---

# Failure Behavior

Installer failures must:

- stop further writes
- report the failed operation clearly
- never claim installation success without verification
- leave all unselected disks untouched
- require the administrator to restart installation after resolving the cause

An interrupted target write may leave the selected target unbootable. The
installer must clearly state this before destructive confirmation. Recovery is
performed by repeating installation to the explicitly selected target.

---

# Validation Requirements

Automated QEMU tests must verify:

- appliance mode remains the default
- installer mode does not expand or mutate the source image
- installer mode does not start appliance-only services
- source media containing appliance-generated persistent state is rejected
- the source boot device cannot be selected or overwritten
- no disk is written before explicit destructive confirmation
- only the selected target changes
- undersized targets are rejected
- invalid SSH keys are rejected
- interrupted and failed writes do not report success
- the installed target boots in appliance mode
- the installed target expands its data partition
- provisioned SSH access becomes available

Physical validation must cover approved combinations of:

- USB source media
- SATA targets
- NVMe targets
- USB-attached targets where explicitly supported

Physical validation must confirm source-device exclusion and target identity
presentation before a release may claim installer support for that hardware.

---

# Alternatives Considered

## Separate Installer Operating System

Rejected because it duplicates the operating system, package set, build
pipeline, security maintenance, and validation burden.

## Embed A Second Complete Target Image

Rejected because it unnecessarily increases image size and duplicates data
already present on the boot media.

## Require Another Machine To Flash Internal Disks

Rejected as the only supported workflow because it creates avoidable physical
deployment friction.

## Network Installer

Deferred because installation must not depend on network or FoldOps
availability.

---

# Consequences

## Positive

- one operating system and one release image
- no duplicated installer distribution
- internal disks can be installed without removal
- installation works without network access
- existing first-boot and SSH provisioning contracts remain valid
- reproducible raw-image deployment remains authoritative

## Negative

- the appliance image contains destructive installation capability
- boot-mode isolation becomes security-critical
- source and target disk identification require extensive testing
- installer support increases the physical validation matrix
- the initial installer performs fresh destructive installs only

---

# Future Considerations

Future ADRs may define:

- preservation or migration of an existing data partition
- FoldOps-assisted enrollment during installation
- signed provisioning bundles
- automated fleet installation
- recovery and reinstall workflows

These capabilities must not weaken explicit target selection or source-device
protection.

---

# Related Documents

- [ADR-0003: x86_64 Bootloader and Image Format](0003-x86_64-bootloader-and-image-format.md)
- [ADR-0007: First-Boot Administrator and SSH-Key Provisioning](0007-first-boot-administrator-and-ssh-provisioning.md)
- [ADR-0008: Raw Image Size and Data-Partition Expansion](0008-raw-image-size-and-data-expansion.md)
- [ADR-0012: Reproducible Build Environment And Verification](0012-reproducible-build-environment-and-verification.md)
- [Installer](../installer.md)

---

# Closing Statement

FoldingOS will install itself using the same tested appliance image it runs,
without introducing a second operating system or requiring internal disks to be
removed for initial deployment.
