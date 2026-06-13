# FoldingOS Operations

Version: 1.0

Status: Approved for Milestone 1 foundation

---

# Purpose

This document describes how operators and developers build, deploy, administer,
diagnose, and recover a FoldingOS v0.1.0 foundation appliance.

FoldingOS is a headless appliance. Normal administration uses SSH. A local
display may remain blank on physical hardware that lacks framebuffer or GPU
drivers in the v0.1.0 kernel.

---

# Build

## Build host

Required host:

```text
Debian 13 amd64
```

Verify the host before building:

```bash
./scripts/check-host-tools
```

## Source and image build

```bash
./scripts/fetch-sources
./scripts/build
```

Release outputs are written to:

```text
build/output/images/foldingos-x86_64-0.1.0.img
build/output/images/foldingos-x86_64-0.1.0.img.sha256
build/output/images/foldingos-x86_64-0.1.0.metadata.json
```

Development builds record `release_eligible: false` in metadata until all
mandatory release gates in
[milestone/1-engineering-spec.md](milestone/1-engineering-spec.md) are
satisfied.

## Automated validation

Foundation acceptance on the QEMU/OVMF reference platform:

```bash
./scripts/test-qemu
```

Additional static verification helpers:

```bash
./scripts/verify-systemd-graph build/output/images/rootfs.tar
./scripts/verify-config build/output/images/rootfs.tar
./scripts/verify-persistent-logging build/output/images/rootfs.tar
```

Reproducibility verification requires two independent clean builds. See
[ADR-0012](adr/0012-reproducible-build-environment-and-verification.md) and
`scripts/build-a`, `scripts/build-b`, and `scripts/verify-reproducible`.

---

# Direct Flash Deployment

v0.1.0 ships a complete 4 GiB raw GPT disk image. The same image may be
written to internal storage or to USB media for boot testing.

## Prepare boot media

Use `scripts/make-bootable-usb` for USB sticks and other removable media
larger than the release image:

```bash
sudo ./scripts/make-bootable-usb \
  --ssh-public-key /path/to/admin-key.pub \
  /dev/sdX \
  build/output/images/foldingos-x86_64-0.1.0.img
```

Replace `/dev/sdX` with the whole-disk device node. Do not target a partition
such as `/dev/sdX1`.

Manual `dd` without backup GPT relocation is not an approved preparation method
for physical validation. See [physical-validation.md](physical-validation.md).

## Boot the appliance

On the target system:

1. Use UEFI boot without legacy BIOS compatibility.
2. Disable Secure Boot when firmware requires it for unsigned media.
3. Select the UEFI USB or internal-disk boot entry.
4. Confirm GRUB loads the FoldingOS entry.

The appliance reaches normal operation when it acquires DHCP on wired Ethernet
and accepts SSH from a provisioned administrator key.

---

# SSH Administrator Provisioning

Administrator access is defined by
[ADR-0007](adr/0007-first-boot-administrator-and-ssh-provisioning.md).

## Provision before first boot

Stage one or more valid OpenSSH public keys on the EFI System Partition before
the first boot:

```text
/foldingos/provision/authorized_keys
```

`scripts/make-bootable-usb --ssh-public-key` writes this path on prepared USB
media. The runtime path is:

```text
/boot/efi/foldingos/provision/authorized_keys
```

## Connect after boot

SSH allows only the `foldingos-admin` account:

```bash
ssh -i /path/to/private-key foldingos-admin@<node-address>
```

Root SSH login and password authentication are disabled.

## Replace or recover keys

Place a new complete authorized-key file on the EFI System Partition and reboot.
On successful import, the provisioning file is removed and the persistent key
set under `/data/config/ssh/authorized_keys` is replaced.

Malformed or private-key material does not replace an existing valid key set.

---

# Normal Operation

After boot, the foundation appliance should provide:

- `/`, `/boot/efi`, and `/data` mounted
- DHCP on wired Ethernet through `systemd-networkd`
- DNS through `systemd-resolved`
- time synchronization through `systemd-timesyncd`
- persistent node identity under `/data/config/node-id`
- validated TOML configuration under `/data/config/`
- bounded persistent journal storage under `/data/logs/journal`
- SSH access for `foldingos-admin`

Foundation acceptance over SSH:

```bash
./scripts/run-physical-acceptance <host> <ssh-private-key> [port]
```

Useful remote inspection commands:

```bash
findmnt / /boot/efi /data
networkctl status
systemctl status foldingos-identity.service foldingos-config-validate.service
journalctl -b --no-pager
foldingosctl config effective system
```

Folding@home acquisition and runtime are Milestone 2 scope and are not part of
the Milestone 1 foundation image.

---

# Diagnostics

## Service and boot state

```bash
systemctl --failed
journalctl -b -p err..alert --no-pager
networkctl list
timedatectl status
```

## Configuration and identity

```bash
foldingosctl config validate --all
foldingosctl identity ensure
cat /data/config/node-id
```

## Storage and expansion

```bash
lsblk
findmnt /data
foldingosctl storage expand-data
```

## SSH policy

```bash
sudo sshd -T | grep -E 'permitrootlogin|passwordauthentication|allowusers'
```

## Build and release artifacts

```bash
sha256sum -c build/output/images/foldingos-x86_64-0.1.0.img.sha256
./scripts/verify-physical-validation-record \
  validation/appliance-physical-0.1.0.json \
  build/output/images/foldingos-x86_64-0.1.0.img
```

---

# Recovery

## No SSH access

1. Mount the EFI System Partition from another machine or from prepared boot
   media.
2. Place a valid `authorized_keys` file at
   `/foldingos/provision/authorized_keys`.
3. Boot the node and wait for first-boot provisioning to import the key.

## Network unavailable

1. Confirm wired Ethernet is connected.
2. Inspect `networkctl status` and `journalctl -u systemd-networkd`.
3. Verify the target NIC is supported by the v0.1.0 kernel baseline in
   [milestone/1-engineering-spec.md](milestone/1-engineering-spec.md).

Wireless networking is not part of the foundation image.

## Unexpected power loss

Boot again and rerun:

```bash
./scripts/run-physical-acceptance <host> <ssh-private-key>
```

The foundation appliance is designed to recover persistent configuration,
journal records, and node identity across reboot and unclean shutdown when
storage remains intact.

## Full appliance replacement

v0.1.0 updates use full-image reflashing. Reflash the release image, provision
a new administrator key on EFI, and boot. Preservation of an existing data
partition across reflashes is not guaranteed in v0.1.0.

---

# Known Foundation Limitations

- No local GUI or installer UI in Milestone 1
- No package manager on the target image
- No FoldOps services in v0.1.0
- No Folding@home client embedded in the foundation image
- Physical display may remain blank even when the system is healthy
- Only documented validated hardware carries a support claim

Validated physical systems are listed in [hardware-support.md](hardware-support.md).

---

# Related Documents

- [Build system](build-system.md)
- [Boot process](boot-process.md)
- [Physical validation](physical-validation.md)
- [Security model](security.md)
- [Testing strategy](testing-strategy.md)
