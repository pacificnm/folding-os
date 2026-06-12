# FoldingOS Installer

Version: 0.3
Status: Approved Architecture

## Purpose

This document defines the intended FoldingOS installation experience.

FoldingOS is an appliance operating system. Installation must remain simple,
reproducible, explicit, and safe while supporting systems whose intended boot
disk is installed internally.

The governing architecture decision is
[ADR-0013](adr/0013-combined-appliance-and-installer-image.md). Fixed
installation roles are defined by
[ADR-0014](adr/0014-fixed-installation-roles.md).

## Installation Philosophy

FoldingOS uses one combined appliance and installer image.

The project does not maintain a separate installer operating system. The same
release image, kernel, root filesystem, packages, and tools boot in one of two
modes:

```text
FoldingOS appliance mode
FoldingOS installer mode
```

The image remains directly flashable for deployments where the target storage
can be prepared from another machine.

For systems with internal target disks, an administrator can flash the image
to USB media, boot installer mode, and install FoldingOS onto the internal
disk.

## Supported Installation Methods

### Direct Flash

The complete bootable image may be written directly to the intended appliance
storage.

After flashing, administrator public keys are placed on the EFI System
Partition as defined by
[ADR-0007](adr/0007-first-boot-administrator-and-ssh-provisioning.md).

The equivalent direct-flash mechanism for selecting a fixed installation role
before first boot remains to be defined. Role-specific services must not be
implemented until that mechanism is approved.

### Combined-Image Installer

The same image may be booted from USB media in installer mode:

```text
Flash FoldingOS image to USB
↓
Boot USB and select installer mode
↓
Select an internal target disk
↓
Select the fixed installation role
↓
Provide administrator public keys
↓
Confirm the destructive installation
↓
Install and verify FoldingOS
↓
Remove USB and boot the installed appliance
```

Installer mode copies the fixed release image from the source boot device to
the selected target. The installed appliance then performs its normal
first-boot initialization and data-partition expansion.

Source media that has previously booted in appliance mode and contains
appliance-generated persistent node state is not eligible installation media.

## Boot Modes

GRUB provides explicit boot entries for:

```text
Start FoldingOS
Install FoldingOS
```

Appliance mode is the default and passes:

```text
foldingos.mode=appliance
```

Installer mode requires local selection and passes:

```text
foldingos.mode=installer
```

Installer mode activates `foldingos-installer.target` instead of the normal
appliance target.

## Installer Scope

The first combined-image installer will:

- run through the local console
- identify and exclude its source boot device
- verify that the source retains the expected release layout and contains no
  appliance-generated persistent node state
- discover eligible target disks
- display target path, capacity, and stable identifying information
- require explicit target selection
- require explicit selection of the `agent` or `supervisor` role
- require target-specific destructive confirmation
- reject targets smaller than the release image
- write the fixed release image to the selected target
- provision administrator public keys on the target EFI partition
- provision the selected fixed role onto the target
- verify and flush the completed installation
- require reboot or poweroff after completion

The first implementation performs fresh destructive installation only.

## Installation Roles

The installer offers exactly:

```text
agent
supervisor
```

The agent role enables the FoldOps agent without the FoldOps supervisor or web
interface.

The supervisor role enables the FoldOps agent, supervisor, and web interface.
It also requires successful initial administrator and TLS provisioning before
the web interface becomes remotely available.

The selected role cannot be changed in place. Changing roles requires fresh
destructive reinstallation.

Role selection activates approved payloads already present in the image. It is
not arbitrary package selection and does not require network access.

The exact role-provisioning transaction and supervisor administrator and TLS
workflow require an approved installer engineering-specification amendment
before implementation.

## Installer-Mode Isolation

Installer mode must preserve a pristine source image and must not behave as an
appliance.

It must not:

- expand the source data partition
- create persistent node identity
- generate SSH host keys
- start OpenSSH
- acquire or start Folding@home
- create deployment-specific persistent state
- write to any disk before destructive confirmation

## SSH-Key Provisioning

The installer provisions public keys through the target EFI System Partition:

```text
/foldingos/provision/authorized_keys
```

The first implementation accepts:

- a public-key file from the source EFI System Partition
- public keys entered through the local console

Keys use the validation rules defined by ADR-0007. The installer never requests
or stores private keys.

On first appliance boot, FoldingOS imports the keys into persistent
configuration and starts OpenSSH.

## Storage Behavior

The release image remains exactly the deterministic fixed size defined by
[ADR-0008](adr/0008-raw-image-size-and-data-expansion.md).

Installer mode copies only that fixed release-image byte range. The installed
target expands its final persistent data partition during its first appliance
boot.

Devices smaller than the release image are unsupported.

## Safety Requirements

The installer must:

- fail closed if the source boot device cannot be identified
- never offer the source boot device as a target
- never select a target automatically
- revalidate source and target identities immediately before writing
- require confirmation that identifies the selected target
- clearly warn that the selected target will be overwritten
- write only to the selected target
- stop on verification failure
- clearly report whether installation completed

An interrupted installation may leave the selected target unbootable.
Repeating installation is the recovery path.

## Reinstallation

Reinstallation uses the same fresh destructive installation workflow:

```text
Boot pristine FoldingOS installer USB
↓
Select the existing internal FoldingOS disk
↓
Confirm erasure
↓
Install and provision FoldingOS again
```

An existing FoldingOS installation remains an eligible target when it
otherwise satisfies the target safety rules. Reinstallation does not require
removing the internal disk.

The installer USB must remain eligible source media. If it has previously
booted in appliance mode or accumulated persistent appliance state, it must be
reflashed from the release image before it can be used to reinstall a node.

The first installer does not preserve existing target configuration, node
identity, logs, Folding@home work, or checkpoints during reinstallation.

## Non-Goals

The first installer does not provide:

- a separate installer operating system
- GUI installation
- arbitrary package selection beyond the two approved fixed roles
- custom partitioning
- network-required installation
- unattended destructive installation
- preservation of existing target data
- installation to multiple targets at once
- network-booted installation

## Implementation Plan

Implementation follows the approved installer engineering specification, which
defines the concrete commands, units, dependencies, and failure behavior
required by ADR-0013.

The approved specification is:

[Milestone 3 installer engineering specification](milestone/3-engineering-spec.md)

The implementation sequence is:

1. Define the installer engineering specification.
2. Add appliance and installer GRUB entries and boot parameters.
3. Define and add `foldingos-installer.target`.
4. Implement safe source-media eligibility and source-device identification.
5. Implement target discovery, selection, identity revalidation, and
   target-specific destructive confirmation.
6. Implement `foldingosctl install`.
7. Implement target EFI administrator-key provisioning.
8. Add QEMU tests proving installer mode cannot overwrite its source device or
   any unselected disk.
9. Add installed-target boot, expansion, and SSH acceptance tests.
10. Complete physical USB-source installation validation for approved SATA and
    NVMe targets.

Installer support must not be claimed until the automated and physical
validation gates pass.

## Validation

Automated QEMU validation must prove:

- installer mode cannot overwrite its source boot device
- source media containing appliance-generated persistent state is rejected
- no target changes before explicit confirmation
- only the selected target changes
- undersized targets are rejected
- invalid SSH keys are rejected
- installed targets boot and expand successfully
- provisioned SSH access becomes available

Physical validation must cover approved USB-source and SATA/NVMe target
combinations before installer support is claimed for them.

## Summary

FoldingOS remains one reproducible appliance image:

```text
Flash directly, or boot the image and install it locally.
```

The combined installer solves internal-disk deployment without creating a
second operating system.

Future releases may provide network-booted deployment using the same installer
logic and safety requirements without introducing a separate installer
operating system.
