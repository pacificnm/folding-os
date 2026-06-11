# FoldingOS Milestone 3 Installer Engineering Specification

**Version:** 1.0

**Status:** Approved for Implementation

**Target Milestone:** Milestone 3, Combined-Image Installer

---

# Approval Record

The project owner approved the following decisions on 2026-06-11:

- single combined operating-system image
- explicit GRUB installer selection
- installer service isolation
- local-console-only installation for Milestone 3
- USB-source installation to SATA and NVMe targets
- target eligibility requirements
- target-specific `ERASE <serial>` confirmation
- fresh destructive installation only
- exact fixed-size image copy
- stale target metadata clearing
- raw installation verification
- failed-install target invalidation
- required administrator SSH public key
- staged-source or local-console SSH key input
- SSH key limits and validation
- no automatic reboot
- volatile redacted installer logging
- automated installer validation
- physical SATA and NVMe release gates

The project owner also approved network-booted deployment as a future
capability, not part of Milestone 3.

The project owner approved the pristine source-media requirement and clarified
reinstall workflow on 2026-06-11.

---

# Purpose

This document defines the concrete implementation of the combined appliance
and installer image accepted by
[ADR-0013](../adr/0013-combined-appliance-and-installer-image.md).

This specification is approved for implementation.

---

# Scope

Milestone 3 adds:

- explicit appliance and installer GRUB boot modes
- `foldingos-installer.target`
- local-console `foldingosctl install`
- safe source and target disk handling
- target EFI administrator-key provisioning
- automated QEMU installer validation
- physical USB-source installation validation

Milestone 3 does not add a second operating system, separate installer image,
network-required installation, custom partitioning, or data-preserving
reinstallation.

---

# Boot Mode

**Decision status:** Resolved

## GRUB Entries

The combined image provides exactly two project-defined GRUB entries:

```text
Start FoldingOS
Install FoldingOS
```

`Start FoldingOS` is the default entry. GRUB automatically boots it after the
existing three-second timeout.

Required GRUB configuration:

```text
set default="0"
set timeout="3"

serial --unit=0 --speed=115200 --word=8 --parity=no --stop=1
terminal_input console serial
terminal_output console serial

menuentry "Start FoldingOS" {
    search --no-floppy --label FOLDINGOS_ROOT --set=root
    linux /boot/bzImage root=PARTUUID=464f4c44-494e-474f-5352-4f4f54000001 rootwait ro console=tty1 console=ttyS0,115200 foldingos.mode=appliance
}

menuentry "Install FoldingOS" {
    search --no-floppy --label FOLDINGOS_ROOT --set=root
    linux /boot/bzImage root=PARTUUID=464f4c44-494e-474f-5352-4f4f54000001 rootwait ro rootflags=noload console=tty1 console=ttyS0,115200 foldingos.mode=installer systemd.unit=foldingos-installer.target systemd.mask=systemd-remount-fs.service fstab=no
}
```

Installer mode advertises on the available local consoles:

```text
tty1
ttyS0 at 115200 baud
```

The first console to receive Enter at the initial installer prompt becomes the
exclusive installer console for the remainder of that boot. The installer
closes its interactive handles to every other console before displaying target
selection. This supports automated QEMU testing and physical local
serial-console installation without enabling a network or remote service.

`foldingos.mode=appliance` and `foldingos.mode=installer` are exact,
case-sensitive kernel command-line assignments. No other value is valid.

## Mode Validation

A new early oneshot service:

```text
foldingos-mode-validate.service
```

must run in both modes before mode-specific services.

Required unit behavior:

```ini
[Unit]
Description=Validate FoldingOS boot mode
DefaultDependencies=no
Before=local-fs-pre.target foldingos-installer.service
OnFailure=emergency.target
OnFailureJobMode=replace-irreversibly

[Service]
Type=oneshot
ExecStart=/usr/bin/foldingosctl mode validate
RemainAfterExit=yes
```

The unit is statically enabled through:

```text
sysinit.target.wants/foldingos-mode-validate.service
```

`foldingosctl mode validate` reads `/proc/cmdline` and succeeds only when
exactly one supported `foldingos.mode` assignment is present.

Failure behavior:

- missing `foldingos.mode` fails boot into `emergency.target`
- duplicate `foldingos.mode` assignments fail boot into `emergency.target`
- unsupported values fail boot into `emergency.target`
- `foldingos.mode=installer` without
  `systemd.unit=foldingos-installer.target` fails boot into
  `emergency.target`
- `foldingos.mode=installer` without `fstab=no` fails boot into
  `emergency.target`
- `foldingos.mode=installer` without `rootflags=noload` fails boot into
  `emergency.target`
- `foldingos.mode=installer` without
  `systemd.mask=systemd-remount-fs.service` fails boot into
  `emergency.target`
- `foldingos.mode=appliance` with
  `systemd.unit=foldingos-installer.target` fails boot into
  `emergency.target`
- `foldingos.mode=appliance` with `fstab=no` or
  `systemd.mask=systemd-remount-fs.service` or `rootflags=noload` fails boot
  into `emergency.target`

## Installer Target

Required unit:

```ini
[Unit]
Description=FoldingOS Installer
Requires=basic.target foldingos-mode-validate.service foldingos-installer.service
After=basic.target foldingos-mode-validate.service
AllowIsolate=no
```

The installer target must not pull in:

```text
multi-user.target
network.target
network-online.target
getty.target
```

`foldingos-installer.service` claims one available local console and provides
the complete interactive installer session there. General login prompts are
not available in installer mode.

The installer service will be concretely defined in the console-interaction
section of this specification.

## Source Filesystem Behavior

Installer mode passes:

```text
rootwait
ro
rootflags=noload
fstab=no
systemd.mask=systemd-remount-fs.service
```

`fstab=no` prevents `systemd-fstab-generator` from generating source
`boot-efi.mount` and `data.mount` units from `/etc/fstab`.

`systemd.mask=systemd-remount-fs.service` prevents the source root filesystem
from being remounted according to the writable root entry in `/etc/fstab`.

`rootflags=noload` prevents the source root ext4 journal from being replayed
during installer boot.

The source root filesystem remains mounted read-only throughout installer mode.
Installer mode must fail before starting `foldingos-installer.service` if the
source root filesystem is writable.

The installer command mounts source EFI and data partitions read-only only
while performing source eligibility checks. It unmounts them before copying
the source image.

The source EFI partition may contain a staged public-key file. No other source
partition mutation is permitted in installer mode.

## Appliance-Only Unit Isolation

Every FoldingOS appliance-only unit must include:

```ini
[Unit]
Requires=foldingos-mode-validate.service
After=foldingos-mode-validate.service
ConditionKernelCommandLine=!foldingos.mode=installer
```

Required appliance-only units:

```text
foldingos-data-expand.service
foldingos-persistent-dirs.service
var-log-journal.mount
foldingos-journal-flush.service
foldingos-identity.service
foldingos-config-validate.service
foldingos-ssh-provision.service
sshd.service
foldingos-fah-acquire.service
foldingos-fah-acquire.timer
foldingos-fah-prepare.service
folding-at-home.service
```

The condition is required even when a unit is not currently pulled into
`foldingos-installer.target`. This prevents accidental activation through
dependencies, aliases, future enablement changes, or manual starts.

Installer-mode drop-ins must also add the same mode-validation requirement,
ordering, and condition to:

```text
systemd-networkd.service
systemd-networkd.socket
systemd-networkd-varlink.socket
systemd-networkd-wait-online.service
systemd-resolved.service
systemd-resolved-monitor.socket
systemd-resolved-varlink.socket
systemd-timesyncd.service
systemd-time-wait-sync.service
```

Milestone 3 installer mode is offline. No network-management, name-resolution,
time-synchronization, SSH, Folding@home, or FoldOps service may start.

## Installer-Mode Enabled Units

Installer mode permits only the base operating-system units required to reach
`basic.target`, discover local storage, log to the volatile journal, and run
the installer.

Project-defined units enabled in installer mode are exactly:

```text
foldingos-mode-validate.service
foldingos-installer.target
foldingos-installer.service
```

The installer must use the volatile system journal. It must not mount or flush
the persistent journal from the source media.

## Boot-Mode Acceptance Tests

Automated tests must verify:

- the default GRUB entry boots appliance mode
- appliance mode retains the existing v0.1.0 unit graph and behavior
- selecting the installer GRUB entry reaches `foldingos-installer.target`
- installer mode keeps the source root filesystem read-only
- installer mode does not mount source EFI or data through `/etc/fstab`
- installer mode does not expand the source data partition
- installer mode does not create node identity or SSH host keys
- installer mode does not start network, SSH, Folding@home, or FoldOps units
- installer mode does not provide a general login prompt
- the first local console to receive Enter becomes the sole interactive
  installer console
- missing, duplicate, unsupported, or contradictory mode parameters fail
  closed into `emergency.target`
- installer mode without `fstab=no` or the root-remount mask fails closed
- rebooting the same unmodified media in appliance mode still succeeds

---

# Disk Safety

**Decision status:** Resolved

## General Safety Model

The installer operates on exactly two whole block devices:

```text
source device: the device from which the running installer booted
target device: one explicitly selected eligible SATA or NVMe disk
```

The installer must fail closed when it cannot prove the identity or eligibility
of either device.

Milestone 3 supports:

```text
USB source -> SATA target
USB source -> NVMe target
```

USB-attached target disks, eMMC, SD cards, device-mapper devices, software RAID,
multipath devices, loop devices, and virtual block devices other than the
documented QEMU test topology are not supported installation targets in
Milestone 3.

## Structured Block-Device Inventory

`foldingosctl install` must obtain block-device inventory using:

```text
lsblk --json --bytes --paths --output NAME,KNAME,PATH,TYPE,PKNAME,TRAN,SIZE,LOG-SEC,PHY-SEC,RO,RM,MODEL,SERIAL,WWN,MAJ:MIN,PARTN,PARTUUID,PARTLABEL,FSTYPE,LABEL,UUID,MOUNTPOINTS
```

The JSON output must be decoded using Go's structured JSON parser. Human-readable
`lsblk` output must not be parsed.

Before every inventory operation, the installer runs:

```text
udevadm settle
```

Each whole-disk identity snapshot records:

```text
canonical sysfs device path
kernel name
device path
major:minor
transport
size in bytes
logical sector size
physical sector size
read-only flag
removable flag
model
serial
WWN when available
```

The canonical sysfs device path, `major:minor`, size, transport, model, serial,
and WWN must remain unchanged between selection and the first destructive
write.

Milestone 3 requires every eligible target to report a non-empty serial number.
A target without a serial number is rejected. The serial must contain only
printable ASCII characters, must not contain leading or trailing whitespace,
and must not contain control characters.

## Source-Device Identification

The source device is resolved using this exact sequence:

1. Run:

   ```text
   findmnt --json --mountpoint / --output SOURCE,FSTYPE,OPTIONS
   ```

2. Require exactly one root mount.
3. Require root filesystem type `ext4`.
4. Require root mount options to include `ro` and exclude `rw`.
5. Resolve the root source path using `filepath.EvalSymlinks`.
6. Use structured `lsblk` inventory to require that the resolved root source:
   - is partition 2
   - has partition UUID
     `464f4c44-494e-474f-5352-4f4f54000001`
   - has partition label `FOLDINGOS_ROOT`
   - uses ext4 with label `FOLDINGOS_ROOT`
   - is directly beneath exactly one whole-disk parent
7. Resolve that parent as the source whole-disk device.

The source root may not be layered through device mapper, RAID, loop, network
block storage, or any other intermediate block-device stack.

The installer must not infer the source from transport type, removable status,
enumeration order, `/dev/sdX` naming, or the first disk containing FoldingOS
labels.

## Source-Media Eligibility

**Approval status:** Approved

Before displaying target disks, the installer verifies that the source is
eligible installation media.

The source whole disk must:

- report `TRAN=usb` on physical installer hardware; the documented QEMU
  topology may use `virtio`
- have physical capacity greater than or equal to `4294967296` bytes
- use 512-byte logical sectors
- contain a GPT
- contain disk GUID `464F4C44-494E-474F-5344-49534B000001`
- contain exactly the three release partitions
- retain the exact release partition start and end sectors
- retain the required partition names, type GUIDs, and unique GUIDs
- contain the required filesystem labels and UUIDs

Required fixed partition geometry:

| Partition | Start sector | End sector |
| --- | ---: | ---: |
| `FOLDINGOS_EFI` | 2048 | 1050623 |
| `FOLDINGOS_ROOT` | 1050624 | 5244927 |
| `FOLDINGOS_DATA` | 5244928 | 8386559 |

Source-media eligibility checks mount the source EFI and data partitions at
private temporary mount points under:

```text
/run/foldingos-installer/source/
```

The source EFI mount uses:

```text
ro,nosuid,nodev,noexec
```

The source data ext4 mount uses:

```text
ro,noload,nosuid,nodev,noexec
```

`noload` prevents ext4 journal replay from mutating source media during
eligibility checks.

The source data filesystem must contain exactly:

```text
lost+found/
```

at its root. Any additional source data entry proves that appliance
initialization or another mutation occurred and makes the source ineligible.

The already-mounted source root must contain a regular
`/etc/machine-id` file of zero bytes. A non-empty or missing source machine ID
makes the source ineligible.

The source EFI filesystem may differ from the release image only at:

```text
/foldingos/provision/authorized_keys
```

The source EFI check compares its complete path, file-type, size, and SHA-256
manifest against the deterministic release EFI manifest at:

```text
/foldingos/release/installer-source-efi.json
```

The manifest excludes itself and the optional provisioning file. Directory
entries are compared by path and type; regular files are compared by path,
type, size, and SHA-256 digest. Unexpected files, symlinks, directories, or
modified release EFI files make the source ineligible.

The optional source provisioning file must pass the ADR-0007 public-key
validation rules before target selection begins.

All source eligibility mounts must be unmounted before target selection is
displayed. Failure to unmount makes installation unavailable.

## Reinstallation Behavior

Reinstallation is a fresh destructive installation performed from eligible
source media.

An existing installed FoldingOS SATA or NVMe disk may be selected as the
target. Its existing FoldingOS partition identities, configuration, node
identity, SSH host keys, logs, Folding@home work, and checkpoints do not make
it ineligible as a target and are destroyed after target-specific
confirmation.

The installed appliance cannot act as its own installation source. Appliance
boot expands the data partition and creates persistent node-specific state, so
copying it would clone mutable appliance state rather than install the fixed
release image.

To reinstall:

1. Boot an eligible pristine FoldingOS installer USB.
2. Select the existing internal FoldingOS disk.
3. Confirm `ERASE <serial>`.
4. Complete a fresh installation and SSH-key provisioning.

If no eligible installer USB remains available, the administrator reflashes a
USB drive from the release image before reinstalling. Internal target-disk
removal is not required.

## Eligible Target Definition

A Milestone 3 target candidate must satisfy all of the following:

- it is a whole block device with `TYPE=disk`
- it is not the source whole-disk device
- it is not a source partition or descendant
- `TRAN` is exactly `sata` or `nvme`
- `RM` is false
- `RO` is false
- size is at least `4294967296` bytes
- logical sector size is exactly 512 bytes
- serial number is non-empty
- it has no mounted descendants
- no descendant appears in `/proc/swaps`
- it has no holders under `/sys/class/block/<kernel-name>/holders`
- it has no active exclusive holder that prevents an exclusive open

Existing partition tables, filesystems, and data do not disqualify an otherwise
eligible target. They are shown to the administrator and destroyed only after
target-specific confirmation.

If no eligible target exists, the installer reports why each discovered whole
disk was rejected and performs no writes.

## Target Presentation And Selection

Eligible targets are sorted by:

1. transport
2. model
3. serial
4. device path

The installer assigns temporary numeric menu indexes for selection, but a menu
index is never treated as target identity.

Each candidate display includes:

```text
device path
transport
capacity
model
serial
WWN when available
existing child partitions and filesystem labels
```

The administrator selects exactly one displayed candidate.

## Destructive Confirmation

After selection, the installer displays:

```text
WARNING: Installation will permanently erase the selected disk.

Target: <device-path>
Model:  <model>
Serial: <serial>
Size:   <size>

Type exactly:
ERASE <serial>
```

The required confirmation is the exact, case-sensitive string:

```text
ERASE <serial>
```

Leading or trailing whitespace, a generic yes/no response, a menu index, a
device path alone, or a mismatched serial does not confirm installation.

An incorrect confirmation returns to target selection without writing.
End-of-file, interruption, or cancellation exits without writing.

## Final Revalidation And Exclusive Access

After valid destructive confirmation and immediately before the first write,
the installer:

1. Runs `udevadm settle`.
2. Repeats source-device identification and source-media eligibility checks.
3. Repeats structured block-device inventory.
4. Requires the selected target's identity snapshot to match exactly.
5. Rechecks every target eligibility rule.
6. Rechecks that source and target are distinct whole devices.
7. Opens the target whole-disk block device read-write with exclusive access.
8. Retains that exclusive target handle through raw image copy, raw
   verification, and device flush.

If any identity or eligibility property changes, installation stops before
writing and requires a new target selection and confirmation.

The implementation must perform raw copy and raw verification through the
retained exclusive target handle. It must not reopen the target by a
potentially changed `/dev` path during those operations.

After raw verification and flush succeed, the installer closes the exclusive
whole-disk handle so the kernel can refresh the installed partition table. It
then runs `udevadm settle`, repeats the complete target identity snapshot
comparison, and verifies the installed GPT identities before accessing the
target EFI partition. Any mismatch stops installation before provisioning.

## Disk-Safety Failure Behavior

Before destructive confirmation, every failure or cancellation performs no
target writes.

After destructive confirmation, failure to obtain exclusive target access
performs no target writes.

Any unexpected source-media change, target-device change, device removal,
mount, holder, swap activation, or inventory ambiguity stops installation.

The installer never falls back to another target and never automatically
retries a destructive operation against a newly enumerated device.

## Disk-Safety Acceptance Tests

Automated tests must verify:

- source identification does not depend on `/dev/sdX` ordering
- source identification rejects layered or ambiguous roots
- the source disk and its partitions never appear as target candidates
- expanded source media is rejected
- source media with a non-empty machine ID is rejected
- source media with persistent data entries is rejected
- source media with unexpected EFI changes is rejected
- valid staged public keys do not make source media ineligible
- targets without serial numbers are rejected
- removable, read-only, undersized, mounted, swap-backed, held, and
  unsupported-transport targets are rejected
- existing data on an eligible target is reported but does not bypass
  confirmation
- incorrect confirmation produces no writes
- cancellation and end-of-file produce no writes
- target identity changes after selection produce no writes
- source identity changes after selection produce no writes
- exclusive-open failure produces no writes
- only the explicitly selected target can reach the destructive-write phase

---

# Installation Mechanism

**Decision status:** Resolved

## Installation Transaction

The destructive installation transaction begins only after disk-safety final
revalidation and successful exclusive opening of the selected target.

The transaction performs these ordered phases:

```text
clear stale target backup GPT
copy fixed release-image byte range
flush target
verify copied byte range and cleared target tail
refresh and verify installed partition table
provision target EFI administrator keys
verify target EFI provisioning
flush and unmount target filesystems
report success
```

Installation success means every phase completed successfully. A successfully
copied raw image without completed SSH-key provisioning is not a successful
installation.

## Fixed Copy Range

The installer copies exactly:

```text
4294967296 bytes
```

starting at byte offset zero of the source whole-disk device to byte offset
zero of the target whole-disk device.

The installer must not derive the copy length from source physical-device
capacity, target capacity, the final source partition, or filesystem sizes.

The source and target must both use 512-byte logical sectors as required by the
disk-safety section.

## Copy Implementation

Raw installation is implemented inside `foldingosctl install`. It must not
invoke `dd`, a shell pipeline, or another general-purpose copy command.

The implementation:

1. Opens the resolved source whole-disk device read-only.
2. Uses the retained exclusive read-write target handle from final
   revalidation.
3. Uses explicit-offset reads and writes.
4. Copies sequentially using a fixed 4 MiB buffer.
5. Handles short reads and short writes correctly.
6. Treats end-of-file before `4294967296` source bytes as failure.
7. Calculates a SHA-256 digest of source bytes while copying.
8. Reports progress no more frequently than once per second.

All reads and writes must remain within the defined fixed copy range except for
the stale-backup-GPT clearing operation below.

## Stale Target Backup-GPT Clearing

Before copying, the installer writes zeros to the final 1 MiB of the physical
target device.

This removes a backup GPT or other conflicting end-of-disk metadata left by
the target's previous contents. The operation is required even when the target
is exactly the release-image size; the subsequent fixed-range copy restores
the correct release-image bytes.

The installer records the exact zeroed offset and verifies that the final
1 MiB remains zero after copy when the target is larger than the fixed release
image.

The installer does not otherwise wipe bytes beyond the fixed copy range.
Installation is not a secure-erasure operation, and the destructive
confirmation must state that previous target data may remain recoverable with
forensic tools.

## Raw Flush And Verification

After raw copy completes, the installer:

1. Calls `fsync` on the target whole-disk handle.
2. Issues the Linux `BLKFLSBUF` ioctl on the target whole-disk handle to flush
   and invalidate block-device buffers.
3. Reads exactly `4294967296` bytes back from the target through the retained
   target handle.
4. Calculates the target SHA-256 digest.
5. Requires the target digest to equal the source digest calculated during
   copy.
6. For targets larger than the fixed image, reads and verifies that the final
   physical-target 1 MiB remains all zero.
7. Calls `fsync` again before closing the exclusive target handle.

Digest mismatch, read failure, flush failure, or non-zero stale-tail data fails
installation.

The digest comparison verifies the source bytes actually used for this
installation. It does not compare against a release-wide whole-image digest
because a valid staged EFI provisioning file intentionally changes the source
image.

## Installed Partition-Table Refresh And Verification

After successful raw verification, the installer closes the exclusive target
handle and runs:

```text
blockdev --rereadpt <target>
udevadm settle
```

It then repeats the complete target identity snapshot comparison and verifies:

- the expected GPT disk GUID
- exactly three expected partitions
- exact fixed release partition geometry
- required partition names, type GUIDs, and unique GUIDs
- required filesystem labels and UUIDs
- target partition devices are descendants of the selected target

The installer must access target partitions using their verified relationship
to the selected whole-disk target. It must not select partitions using globally
ambiguous filesystem labels or partition UUID symlinks because source and
target intentionally contain identical release identities during
installation.

The copied backup GPT remains at the fixed release-image boundary. Installer
mode does not move it to the physical target end. Normal first appliance boot
performs the approved ADR-0008 expansion process.

## Successful Completion

After target EFI provisioning succeeds, the installer:

1. Unmounts every target filesystem.
2. Runs `udevadm settle`.
3. Opens the selected target whole disk read-only and repeats its identity
   snapshot comparison.
4. Calls `syncfs` or the equivalent filesystem flush for every target
   filesystem before unmount.
5. Calls `fsync` on the target whole-disk device.
6. Reports installation success.

The success message identifies the installed target by path, model, serial,
and size and instructs the administrator to power off, remove the source USB,
and boot the installed target.

The installer must not automatically reboot. This prevents firmware from
booting the still-attached source USB back into installer or appliance mode.

## Failure And Target Invalidation

Before the first destructive write, failure leaves the target unchanged.

After the first destructive write, any failure means the target installation
is incomplete. The installer must:

1. Stop all further normal installation work.
2. Best-effort unmount target filesystems.
3. Revalidate the selected target identity and confirm no target descendant is
   mounted before any invalidation write.
4. When identity remains certain and no target descendant is mounted,
   best-effort write zeros to the first 1 MiB of the selected target and flush
   it.
5. When identity cannot be proven or a target descendant remains mounted,
   perform no invalidation write and report that the target could not be safely
   invalidated.
6. Report that the selected target is incomplete and must not be booted.
7. Require a new complete installation attempt.

Zeroing the first 1 MiB invalidates the installed primary GPT and EFI
filesystem header so an incomplete target is unlikely to boot accidentally.
Failure of best-effort invalidation must be reported explicitly.

The installer never resumes a partial copy, never reports a partially
installed target as usable, and never switches to another target
automatically.

## Installation-Mechanism Acceptance Tests

Automated tests must verify:

- exactly `4294967296` source bytes are copied
- source media larger than the image does not increase the copy length
- target bytes beyond the fixed range are unchanged except for the final
  cleared 1 MiB
- stale target backup GPT metadata is removed
- short reads, short writes, source read failures, and target write failures
  fail installation
- target flush failure fails installation
- raw target digest mismatch fails installation
- non-zero stale-tail verification fails installation
- installed GPT and filesystem identities match the release layout
- target partitions are resolved only as descendants of the selected target
- the installed backup GPT remains at the release-image boundary before first
  appliance boot
- no success is reported before SSH-key provisioning and all flushes complete
- failure after destructive writing triggers best-effort target invalidation
- an incomplete invalidated target does not boot in QEMU
- successful installation does not automatically reboot

---

# SSH-Key Provisioning

**Decision status:** Resolved

## Required Administrator Access

Combined-image installation requires at least one valid administrator public
key. The installer must not complete an installation without a validated key
set.

The key set provisioned by the installer is the complete desired
`foldingos-admin` authorized-key set. Keys are not merged with unknown target
contents.

The installer never requests, accepts, copies, or stores a private key.

## Key Input Sources

The installer supports exactly two public-key input sources:

```text
validated staged source EFI key file
interactive local-console public-key entry
```

The staged source path is:

```text
/foldingos/provision/authorized_keys
```

on the source EFI partition.

If a valid staged source file exists, the installer presents its key
fingerprints and offers it as the default complete key set. The administrator
may explicitly replace the staged set with a complete console-entered set.

If no staged source file exists, the administrator must enter at least one key
through the local console before target selection.

Staged and console-entered sets are never implicitly merged.

## Console Key Entry

Interactive entry accepts one complete OpenSSH public-key line at a time.

Behavior:

- each non-empty line is validated immediately
- a blank line completes entry after at least one valid key
- a blank line before the first valid key does not complete entry
- `cancel` entered as the exact first line cancels installation
- `cancel` after one or more accepted keys is treated as invalid key input,
  not as a command
- invalid input reports the reason without echoing the complete submitted line
- the administrator may discard the entire candidate set and restart entry

Limits:

```text
maximum keys: 32
maximum public-key line length: 16384 bytes
maximum canonical candidate file size: 262144 bytes
```

Input exceeding a limit is rejected.

Console echo remains enabled because public keys are not secrets. Logs must not
contain complete key lines or comments.

## Key Validation And Canonicalization

The installer uses the same shared validation implementation as:

```text
foldingosctl provision ssh
```

Supported key types and validation rules remain exactly those defined by
ADR-0007 and the approved v0.1.0 engineering specification:

```text
ssh-ed25519
ecdsa-sha2-nistp256
ssh-rsa with at least 3072 bits
```

The validator:

- rejects private-key material
- rejects authorized-key options
- rejects malformed and unsupported keys
- rejects duplicate public-key blobs
- ignores blank lines and comment-only lines in a staged file
- validates every accepted key using `ssh-keygen`
- canonicalizes each accepted key to one line
- writes exactly one trailing newline after each accepted key

Comments attached to valid key lines are preserved in the provisioned file but
are not displayed or logged.

Before destructive confirmation, the installer displays only:

```text
key type
key bit length
SHA-256 fingerprint
```

The administrator must confirm the displayed candidate key set as part of the
installation summary before proceeding to the disk-erasure confirmation.

## In-Memory Candidate Handling

The validated canonical candidate key set is retained in process memory while
installation runs.

If temporary storage is required, it may exist only under:

```text
/run/foldingos-installer/
```

which is volatile. Temporary key files use `root:root` ownership and mode
`0600` and are removed immediately after use.

The installer must not write the candidate set to source root, source data, or
source EFI storage.

## Target EFI Resolution And Mount

After raw copy verification and installed GPT verification, the installer
resolves target partition 1 only through its verified descendant relationship
to the selected target whole disk.

The installer must not use globally ambiguous label or PARTUUID symlinks to
choose the target EFI partition.

The target EFI partition must match:

```text
partition number: 1
partition label: FOLDINGOS_EFI
partition UUID: 464f4c44-494e-474f-5345-464900000001
filesystem type: vfat
filesystem label: FOLDING_EFI
filesystem UUID: 464F-5345
```

The installer mounts it at:

```text
/run/foldingos-installer/target-efi
```

with:

```text
rw,nosuid,nodev,noexec,umask=0077
```

No other target filesystem is mounted for SSH provisioning.

## Target Provisioning Write

The target provisioning destination is:

```text
/run/foldingos-installer/target-efi/foldingos/provision/authorized_keys
```

The installer:

1. Revalidates the target identity immediately before mounting target EFI.
2. Creates `/foldingos/provision` when absent.
3. Writes the canonical candidate set to a temporary file in the destination
   directory.
4. Flushes and closes the temporary file.
5. Atomically renames it over `authorized_keys`.
6. Flushes the destination directory and target EFI filesystem.
7. Reads the destination back and requires byte-for-byte equality with the
   canonical candidate set.
8. Runs the shared authorized-key validator against the read-back file.
9. Unmounts target EFI.

The provisioning file remains on target EFI after installation. Normal first
appliance boot imports and removes it according to ADR-0007.

## Provisioning Failure Behavior

Any key-input, target-EFI mount, write, flush, read-back, validation, or
unmount failure means installation is incomplete.

Failure after destructive writing invokes the target-invalidation behavior
defined by the installation-mechanism section.

The installer never reports success merely because a copied staged
provisioning file happened to be present in the raw target image. It must
complete the explicit target provisioning and verification sequence.

## SSH-Provisioning Acceptance Tests

Automated tests must verify:

- installation cannot complete without at least one valid key
- a valid staged source key set is accepted
- console-entered keys are accepted
- staged and console-entered sets are not implicitly merged
- malformed, unsupported, option-prefixed, duplicate, oversized, and private
  key inputs are rejected
- RSA keys smaller than 3072 bits are rejected
- no complete key lines or comments appear in logs
- target EFI is resolved only as a child of the selected target
- no target filesystem other than EFI is mounted for provisioning
- target provisioning uses the complete selected key set
- read-back bytes and key validation must both succeed
- provisioning failure invalidates the incomplete target
- successful first appliance boot imports the key set, removes the EFI
  provisioning file, and starts OpenSSH

---

# Installer Command And Console

**Decision status:** Resolved

## Command Interfaces

Milestone 3 adds:

```text
foldingosctl mode validate
foldingosctl install
```

`foldingosctl mode validate` implements the boot-mode validation contract
defined earlier in this specification.

`foldingosctl install` implements the complete interactive installer
transaction. It accepts no command-line arguments in Milestone 3.

`foldingosctl install` must refuse to run unless all of the following are true:

- effective UID is zero
- `foldingos.mode=installer` is present exactly once
- `systemd.unit=foldingos-installer.target` is present exactly once
- `fstab=no` is present
- `rootflags=noload` is present
- `systemd.mask=systemd-remount-fs.service` is present
- source root is mounted read-only
- at least one supported local installer console is available

It must refuse to run in appliance, rescue, emergency, SSH, redirected-input,
or non-interactive contexts.

There is no non-interactive, unattended, answer-file, or remote installer
interface in Milestone 3.

## Installer Service

Required unit:

```ini
[Unit]
Description=Install FoldingOS
Requires=foldingos-mode-validate.service
After=foldingos-mode-validate.service systemd-udevd.service
ConditionKernelCommandLine=foldingos.mode=installer
Conflicts=shutdown.target
Before=shutdown.target

[Service]
Type=oneshot
ExecStart=/usr/bin/foldingosctl install
RemainAfterExit=yes
StandardInput=null
StandardOutput=journal
StandardError=journal
Environment=TERM=linux
PrivateTmp=yes
PrivateMounts=yes
ProtectHome=yes
ProtectSystem=strict
ReadWritePaths=/run/foldingos-installer
```

The service intentionally runs as root because it must inspect, open, write,
flush, and mount block devices. It receives no network dependencies and opens
no listening socket.

`foldingosctl install` opens available `/dev/tty1` and `/dev/ttyS0` devices
read-write and displays an initial `Press Enter to begin` prompt on each. The
first console to provide Enter is retained exclusively for all further prompts
and responses; interactive handles to other consoles are closed. Structured
status events and redacted diagnostics are written separately to stdout and
stderr for the volatile journal.

The installer service is started only by `foldingos-installer.target`. It is
not enabled under `multi-user.target` or any appliance target.

## Volatile Working Directory

Installer runtime files exist only under:

```text
/run/foldingos-installer/
```

At startup, the installer creates that directory as:

```text
root:root 0700
```

It contains only volatile mounts, temporary validated key files when required,
and non-sensitive installer state. The installer removes temporary files and
unmounts private mounts during cleanup.

Nothing under `/run/foldingos-installer` is copied to the target.

## Console Workflow

The local-console workflow is exactly:

1. Display installer identity, FoldingOS version, and fresh-install warning.
2. State that installation is offline and no remote access is available.
3. Validate boot mode and source-media eligibility.
4. Select and validate the complete administrator public-key set.
5. Discover and display eligible and rejected target disks.
6. Select exactly one eligible target.
7. Display a final summary containing:
   - source device identity
   - target device identity
   - target erasure warning
   - administrator key fingerprints
   - statement that installation is not secure erasure
8. Require the exact `ERASE <serial>` destructive confirmation.
9. Revalidate source and target.
10. Perform installation with progress display.
11. Verify raw installation and SSH provisioning.
12. Display success or failure.

No step opens a shell or general login prompt.

## Console Interaction Rules

All prompts:

- identify the expected input format
- accept input only from the configured local console
- treat end-of-file as cancellation before destructive writing
- treat interruption as cancellation before destructive writing
- never interpret arbitrary input as a shell command
- never display or log complete administrator key lines

Before the first destructive write, the administrator may enter:

```text
cancel
```

at a menu prompt to cancel installation. Cancellation performs no target
writes and proceeds to the final poweroff prompt.

After destructive writing begins, cancellation is unavailable. Interruptions
invoke incomplete-target failure handling.

## Progress Display

During raw copy and verification, the installer displays:

```text
phase
bytes completed
total bytes
integer completion percentage
elapsed time
```

Progress updates occur no more frequently than once per second.

The installer must not estimate remaining time or claim completion before
flush and verification finish.

## Completion And Poweroff

After success or terminal failure, the installer does not provide a shell.

It displays the final result and prompts:

```text
Type exactly:
POWER OFF
```

On exact, case-sensitive `POWER OFF`, the installer requests:

```text
systemctl poweroff
```

Other input repeats the prompt. Physical power-button handling remains
available through systemd.

The installer never automatically reboots.

## Exit Codes

`foldingosctl install` uses:

| Exit code | Meaning |
| ---: | --- |
| `0` | Installation completed and verified |
| `2` | Invalid command usage |
| `10` | Invalid installer boot mode or execution environment |
| `20` | Source media is not eligible |
| `21` | No eligible target disk exists |
| `22` | Administrator cancelled before destructive writing |
| `30` | Source identity or eligibility changed |
| `31` | Target identity or eligibility changed |
| `40` | Raw copy, flush, or verification failed |
| `50` | SSH-key input or target provisioning failed |
| `60` | Cleanup, unmount, or final flush failed |

Errors after the first destructive write also invoke target invalidation when
the selected target identity remains certain.

`foldingosctl mode validate` exits `0` only for a valid supported mode and exits
`1` for invalid or contradictory mode state.

## Logging

The installer logs to the volatile system journal and local console.

Required logged events:

- validated boot mode
- source identity and eligibility result
- candidate target identities and rejection reasons
- selected target identity
- destructive confirmation success
- start and completion of each installation phase
- digest verification result
- target provisioning result
- target invalidation attempts and outcomes
- final result and exit code

Logs must not contain:

- complete public-key lines
- public-key comments
- private-key material
- arbitrary raw console input
- data read from existing target filesystems

Installer logs are not copied to the installed target. The administrator may
photograph or transcribe console diagnostics when installation fails.

## Required Buildroot Capabilities

Milestone 3 uses the existing FoldingOS kernel, root filesystem, and package
set.

Required target tools:

```text
blockdev
findmnt
lsblk
mount
sgdisk
ssh-keygen
systemctl
udevadm
umount
```

Required Go functionality uses the standard library and Linux syscalls for:

```text
structured JSON parsing
block-device file operations
exclusive target opening
SHA-256
fsync and syncfs
BLKFLSBUF
console input
```

No shell, scripting-language runtime, network client, separate installer
package set, or new general-purpose copy utility is required.

The build process generates a deterministic source EFI manifest before EFI
filesystem image assembly at:

```text
/foldingos/release/installer-source-efi.json
```

The manifest is structured JSON containing expected EFI paths, entry types,
regular-file sizes, and SHA-256 digests. It excludes itself and the optional
`/foldingos/provision/authorized_keys` path.

## Installer-Command Acceptance Tests

Automated tests must verify:

- `foldingosctl install` refuses every non-installer execution context
- installer service binds interaction to only the first claimed local console
  and no login shell is available
- redirected and non-interactive input are rejected
- cancellation before destructive writing performs no writes
- interruption after destructive writing invokes incomplete-target handling
- progress never claims completion before verification and flush
- success and failure both end at the explicit poweroff prompt
- no automatic reboot occurs
- exit codes match documented failure classes
- required events appear in volatile logs
- prohibited key and target-data content does not appear in logs
- no installer runtime state is copied to the installed target

---

# QEMU And Physical Validation

**Decision status:** Resolved

## Automated Test Entry Point

Milestone 3 adds:

```text
scripts/test-installer-qemu
```

The existing `scripts/test-qemu` remains responsible for v0.1.0 appliance,
storage-expansion, networking, and SSH acceptance behavior.

`scripts/test-installer-qemu` validates the combined-image installer and the
installed appliance. Both scripts are required release gates for an
installer-capable release.

The installer test harness always creates disposable sparse copies. It never
modifies the release image directly.

## QEMU Reference Topology

Required installer-test environment:

```text
qemu-system-x86_64
OVMF UEFI firmware
q35 machine
2 vCPUs
2 GiB RAM
USB mass-storage source device
one selected SATA or NVMe target
optional additional unselected target
local serial console
QMP control socket
```

The source is a disposable exact copy of the release image attached through a
QEMU USB mass-storage controller with a fixed serial:

```text
FOLDINGOS-INSTALL-SOURCE
```

The target cases are:

```text
SATA target serial: FOLDINGOS-TARGET-SATA
NVMe target serial: FOLDINGOS-TARGET-NVME
```

Each normal successful-install target is a sparse 6 GiB file. Tests requiring
an exact-size or undersized target create separate disposable files.

The harness supplies a generated Ed25519 public key through the source EFI
provisioning path before boot. That staged file is part of the test source
baseline.

The harness captures:

```text
serial console transcript
QMP event log
source image before and after digests and partition tables
every target before and after digest and partition table
installed-target appliance serial log
failure diagnostics
```

## Installer Console Automation

GRUB serial input and output are enabled by the committed release
configuration.

The harness:

1. Boots the exact combined image through OVMF.
2. Selects `Install FoldingOS` through the serial GRUB menu.
3. Sends Enter on `ttyS0` to claim the serial installer console.
4. Responds to the documented interactive prompts.
5. Uses the exact target serial for destructive confirmation.
6. Sends `POWER OFF` only after validating the reported terminal result.

The harness must fail when prompts occur out of order, undocumented prompts
appear, expected prompts are missing, or success is reported early.

## Required Successful QEMU Scenarios

The suite must complete these successful installations:

1. USB source to SATA target.
2. USB source to NVMe target.
3. USB source with SATA target plus an unselected NVMe disk.
4. USB source with NVMe target plus an unselected SATA disk.
5. Exact 4 GiB eligible target.
6. Reused larger target containing an existing GPT and filesystems.

For every successful installation, the suite verifies:

- the source image is byte-identical before and after installer boot
- the unselected target is byte-identical before and after installation
- the selected target contains the expected fixed installed layout
- the selected target physical-end stale GPT area was cleared
- target EFI contains the validated staged administrator key set
- installer mode powered off without rebooting

The harness then boots only the installed selected target through QEMU/OVMF in
appliance mode and verifies:

- appliance mode is the default
- data expansion succeeds when the target is larger than 4 GiB
- exact-size installation performs no expansion
- source-media node identity and host keys were not copied
- a new node identity and SSH host key are generated
- EFI administrator keys are imported and removed
- OpenSSH starts
- `foldingos-admin` SSH access succeeds
- root and password SSH access remain rejected

## Required Safety And Failure QEMU Scenarios

The suite must verify:

- allowing the GRUB timeout boots appliance mode, not installer mode
- cancelling before confirmation writes nothing
- incorrect destructive confirmation writes nothing
- source disk cannot be selected
- unselected disk cannot be written
- undersized target is rejected
- target without serial is rejected
- read-only and unsupported-transport targets are rejected
- source with expanded partition geometry is rejected
- source with non-empty machine ID is rejected
- source with persistent data state is rejected
- source with unexpected EFI mutation is rejected
- target hot removal or identity change after selection writes nothing when it
  occurs before destructive writing
- target exclusive-open failure writes nothing
- injected target write failure reports failure and invalidates the incomplete
  target when identity remains certain
- injected verification corruption reports failure and invalidates the
  incomplete target
- injected target-EFI provisioning failure reports failure and invalidates the
  incomplete target
- an invalidated incomplete target does not boot
- source removal or identity ambiguity stops installation without selecting a
  replacement source

QMP hotplug and unplug operations provide identity-change scenarios.
QEMU `blkdebug` fault injection provides deterministic write, flush, and
verification failures.

Short-read, short-write, digest, parsing, confirmation, and exit-code behavior
that cannot be reliably produced through QEMU must have focused Go unit tests.

## Automated Source And Unselected-Disk Protection

Before every scenario, the harness records SHA-256 digests for:

```text
complete source physical image
complete unselected target images
selected target image before installation
```

After installer shutdown, complete source and unselected-target digests must
match their pre-boot values exactly.

Any source or unselected-target byte difference fails the suite and blocks
installer release.

## Physical Validation Scope

Installer support requires physical validation using:

```text
USB flash-drive source -> SATA target
USB flash-drive source -> NVMe target
```

Each claimed hardware system must already satisfy the applicable FoldingOS
physical hardware acceptance requirements.

USB-attached targets are not claimed or validated in Milestone 3.

## Physical Validation Procedure

For each SATA and NVMe target case, an administrator must:

1. Flash the release-candidate combined image to a USB drive.
2. Stage a test administrator public key on source EFI.
3. Record source USB manufacturer, model, serial, and capacity.
4. Record target manufacturer, model, serial, firmware, transport, and
   capacity.
5. Attach at least one additional unselected disk when the hardware permits.
6. Boot installer mode through local display and keyboard.
7. Confirm the source USB is never offered as a target.
8. Confirm every displayed target identity matches physical hardware.
9. Confirm the unselected disk remains untouched.
10. Complete installation to the selected target.
11. Power off and remove the source USB.
12. Boot the installed target.
13. Verify data expansion, node identity creation, SSH host-key generation,
    key import, and SSH access.
14. Reboot and verify persistent operation.
15. Repeat installation using the local serial console when the validated
    hardware exposes one.

The SATA and NVMe cases may use different validated physical systems.

## Physical Validation Record

Results are committed or supplied to the release process as:

```text
build/validation/installer-physical.json
```

The structured record contains:

```text
FoldingOS version
Git revision
release-image SHA-256
validation date
validator identity
source USB identity
target identity and firmware
additional unselected-disk identity when present
console type
individual test results
overall pass or fail
```

The release process must verify that the record references the exact release
candidate image and Git revision.

Installer physical validation is complete only when both required target
transport cases pass. QEMU validation alone is insufficient.

## Release Gates

An installer-capable release is blocked unless:

1. All `foldingosctl` installer unit tests pass.
2. Existing appliance QEMU acceptance tests pass.
3. Combined-image installer QEMU tests pass.
4. SATA physical installation validation passes.
5. NVMe physical installation validation passes.
6. Two independent clean builds produce byte-identical required release
   artifacts.
7. Installer documentation matches implemented behavior.

Until every gate passes, installer capability must be described as development
only and must not be claimed as supported.
