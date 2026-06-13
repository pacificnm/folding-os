# FoldingOS Physical Hardware Validation

**Version:** 1.0

**Status:** Approved for Milestone 1 foundation validation

**Target Release:** v0.1.0

---

# Purpose

This document defines the mandatory physical hardware acceptance procedure for
the Milestone 1 bootable appliance foundation.

QEMU/OVMF validation is required but not sufficient for release. A physical
x86_64 UEFI system becomes validated for a release only after it completes this
procedure and its results are recorded in a committed validation record.

---

# Scope

Milestone 1 foundation validation covers:

- UEFI boot through GRUB
- GPT layout and required filesystem mounts
- persistent data expansion on disks larger than the release image
- Ethernet DHCP networking, DNS, and time synchronization
- SSH administrator provisioning and access
- persistent directories, journald, node identity, and configuration services
- graceful shutdown and reboot
- recovery after unexpected power interruption

Folding@home acquisition and runtime validation are release gates defined
separately and are not part of this Milestone 1 foundation record.

---

# Hardware Selection

Select one physical x86_64 UEFI system for the initial v0.1.0 foundation
validation record.

Record at minimum:

- system manufacturer and model
- firmware vendor and version
- CPU model
- installed memory
- boot/storage device manufacturer, model, serial, transport, and capacity
- Ethernet controller and interface name
- known limitations

The validated system must use:

- UEFI boot without legacy BIOS compatibility
- wired Ethernet for DHCP
- installation storage large enough to hold the 4 GiB release image and allow
  data-partition expansion testing

---

# Candidate Image

Use the exact candidate image under test:

```text
build/output/images/foldingos-x86_64-0.1.0.img
build/output/images/foldingos-x86_64-0.1.0.img.sha256
build/output/images/foldingos-x86_64-0.1.0.metadata.json
```

Record the Git revision and image SHA-256 from the candidate metadata before
starting validation.

---

# Boot Media Preparation

Physical validation must prepare boot media with
`scripts/make-bootable-usb`. Manual `dd` alone is not sufficient when the
target device is larger than the 4 GiB release image. The script writes the
image, relocates the backup GPT header for larger media, verifies the EFI boot
structure, and can stage the administrator SSH public key on the EFI System
Partition.

Required host tools:

```text
dd
gdisk (sgdisk)
lsblk
mtools (mcopy, mdir)
```

Run as root on the machine that will write the USB stick or other boot media:

```bash
sudo ./scripts/make-bootable-usb \
  --ssh-public-key /path/to/admin-key.pub \
  /dev/sdX \
  build/output/images/foldingos-x86_64-0.1.0.img
```

Replace `/dev/sdX` with the whole-disk device node for the boot media. Do not
target a partition such as `/dev/sdX1`.

The staged key is imported on first boot from:

```text
/boot/efi/foldingos/provision/authorized_keys
```

---

# Procedure

An administrator must:

1. Record the candidate Git revision and image SHA-256.
2. Record the selected physical hardware identity and firmware details.
3. Prepare boot media with `scripts/make-bootable-usb`, including a test
   administrator public key when SSH acceptance checks are required.
4. Boot the system through physical UEFI firmware and confirm GRUB loads the
   FoldingOS entry.
5. Wait for the system to reach multi-user operation.
6. Run the SSH acceptance checks:

   ```bash
   ./scripts/run-physical-acceptance <host> <ssh-private-key> [port]
   ```

7. Confirm required mounts, services, networking, DNS, time sync, SSH policy,
   node identity, and persistent storage behavior pass.
8. Run graceful shutdown, cold boot, and confirm acceptance checks still pass.
9. Run graceful reboot and confirm acceptance checks still pass.
10. While the system is running, remove power without shutdown.
11. Restore power, boot again, and confirm acceptance checks still pass.
12. Complete `validation/appliance-physical.template.json` using the recorded
    results.
13. Verify the record against the candidate image:

    ```bash
    ./scripts/verify-physical-validation-record \
      validation/appliance-physical-0.1.0.json \
      build/output/images/foldingos-x86_64-0.1.0.img
    ```

14. List the validated system in [hardware-support.md](hardware-support.md).
15. Commit the completed validation record with the documentation update.

---

# Validation Record

Committed records use:

```text
validation/appliance-physical-<version>.json
```

The record must contain:

- FoldingOS version
- Git revision
- release-image SHA-256
- validation date
- validator identity
- hardware identity, firmware, storage, and network details
- individual test results
- overall pass or fail

Use [validation/appliance-physical.template.json](../validation/appliance-physical.template.json)
as the starting point.

`physical_validation_complete` in release metadata must remain `false` until a
committed record for the exact candidate image passes
`scripts/verify-physical-validation-record` with `overall: "pass"` and every
required test marked `pass`.

---

# Required Test Results

| Test | Requirement |
| --- | --- |
| `uefi_boot` | Boots through physical UEFI and GRUB |
| `filesystem_mounts` | `/`, `/boot/efi`, and `/data` mount correctly |
| `data_expansion` | Data partition expands on disks larger than the image |
| `ethernet_dhcp` | Wired Ethernet acquires DHCP |
| `dns_resolution` | DNS resolution succeeds |
| `time_sync` | `systemd-timesyncd` synchronizes time |
| `ssh_access` | Provisioned `foldingos-admin` SSH access works |
| `foundation_services` | Required Milestone 1 FoldingOS services are active |
| `graceful_shutdown` | Clean shutdown and subsequent boot succeed |
| `reboot` | Reboot preserves required persistent state |
| `power_interruption` | Recovery succeeds after unexpected power loss |

---

# Related Documents

- [v0.1.0 engineering specification](milestone/1-engineering-spec.md)
- [Hardware support](hardware-support.md)
- [Testing strategy](testing-strategy.md)
