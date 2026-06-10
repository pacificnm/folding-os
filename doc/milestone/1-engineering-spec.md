# FoldingOS v0.1.0 Engineering Specification

**Version:** 1.0

**Status:** Approved for Implementation

**Target Release:** v0.1.0

---

# Purpose

This document defines the concrete implementation of the approved
[v0.1.0 scope](1-implementation-spec.md).

It translates accepted architecture decisions into repository structure,
Buildroot configuration, image layout, runtime services, configuration
schemas, security policy, and release verification requirements.

Implementation must conform to this specification and the accepted
[architecture decision records](../adr/README.md).

---

# Implementation Boundaries

v0.1.0 implements:

- one x86_64 UEFI Buildroot image
- GRUB 2 boot through OVMF and validated physical UEFI firmware
- systemd-managed boot and services
- Ethernet IPv4 DHCP networking
- OpenSSH public-key administration
- bounded persistent journald storage
- schema-versioned TOML configuration
- automatic persistent-data expansion
- verified post-deployment Folding@home 8.5 acquisition
- CPU Folding@home workloads

v0.1.0 contains no:

- FoldOps agent, placeholder, enrollment, configuration, or runtime state
- static networking
- IPv6 requirement
- package manager
- OTA update implementation
- interactive installer
- GPU Folding@home support or proprietary GPU drivers

---

# Repository Layout

The implementation will use:

```text
build/
  buildroot/
    buildroot-2026.02.2.tar.xz.sha256
    buildroot-2026.02.2.tar.xz.sign
  host/
    debian-13-packages.txt

configs/
  foldingos_x86_64_defconfig
  busybox.config
  linux-x86_64.config

board/
  foldingos/
    x86_64/
      genimage.cfg
      grub.cfg
      post-build.sh
      post-image.sh

overlay/
  etc/
  usr/

packages/
  foldingosctl/
    Config.in
    foldingosctl.mk
    src/

scripts/
  build
  check-host-tools
  clean
  fetch-sources
  test-qemu
  verify-reproducible

tools/
  qemu/
  release/
```

The `board/` directory must be added when implementation begins. Generated
Buildroot output, downloaded source archives, temporary images, private keys,
and release-working directories must not be committed.

---

# Build Host

Required release-build host:

```text
Debian 13 amd64
```

The initial dedicated builder reports:

```text
Linux 6.12.90+deb13.1-amd64
Debian kernel package 6.12.90-2
x86_64 GNU/Linux
```

Required Debian build and test packages must be captured with exact installed
versions in:

```text
build/host/debian-13-packages.txt
```

The initial package set must include at least:

```text
bash
bc
binutils
build-essential
bzip2
ca-certificates
cpio
diffutils
dosfstools
e2fsprogs
file
findutils
gawk
git
gzip
make
mtools
ovmf
patch
perl
python3
qemu-system-x86
rsync
sed
tar
unzip
wget
xz-utils
```

Builds run as an unprivileged user. Release verification must not use `sudo`
inside Buildroot.

---

# Buildroot Baseline

Pinned Buildroot release:

```text
2026.02.2
```

The upstream tarball, PGP signature, and SHA-256 digest are verified by
`scripts/fetch-sources` before extraction.

Buildroot is extracted into a disposable working directory. Project
customizations remain outside the upstream tree.

Buildroot configuration paths are resolved relative to the extracted Buildroot
source directory:

```text
build/work/buildroot-2026.02.2/
```

With the approved repository layout, the committed defconfig uses these paths:

```text
BR2_ROOTFS_OVERLAY="../../../overlay"
BR2_ROOTFS_POST_BUILD_SCRIPT="../../../board/foldingos/x86_64/post-build.sh"
BR2_ROOTFS_POST_IMAGE_SCRIPT="../../../board/foldingos/x86_64/post-image.sh"
BR2_LINUX_KERNEL_CUSTOM_CONFIG_FILE="../../../configs/linux-x86_64.config"
```

The build wrapper must verify that these paths resolve before starting the
build. FoldingOS does not use a Buildroot external tree.

Required Buildroot configuration direction:

```text
Architecture: x86_64
Toolchain: Buildroot internal toolchain
C library: glibc
Init system: systemd
Device management: systemd-udevd
Root filesystem: ext4
Reproducible build support: enabled
Compiler cache: disabled for verification builds
```

The initial defconfig must include the pinned-release equivalents of at least:

```text
BR2_x86_64=y
BR2_TOOLCHAIN_BUILDROOT_GLIBC=y
BR2_INIT_SYSTEMD=y
BR2_ROOTFS_DEVICE_CREATION_DYNAMIC_EUDEV=y
BR2_REPRODUCIBLE=y
BR2_LINUX_KERNEL=y
BR2_LINUX_KERNEL_CUSTOM_VERSION=y
BR2_LINUX_KERNEL_CUSTOM_VERSION_VALUE="6.12.90"
BR2_LINUX_KERNEL_USE_CUSTOM_CONFIG=y
BR2_LINUX_KERNEL_NEEDS_HOST_LIBELF=y
BR2_PACKAGE_CA_CERTIFICATES=y
BR2_PACKAGE_LIBCURL=y
BR2_PACKAGE_LIBCURL_CURL=y
BR2_PACKAGE_E2FSPROGS=y
BR2_PACKAGE_E2FSPROGS_RESIZE2FS=y
BR2_PACKAGE_GPTFDISK=y
BR2_PACKAGE_HOST_DOSFSTOOLS=y
BR2_PACKAGE_HOST_GENIMAGE=y
BR2_PACKAGE_HOST_MTOOLS=y
BR2_PACKAGE_OPENSSH=y
BR2_PACKAGE_SUDO=y
BR2_PACKAGE_UTIL_LINUX=y
BR2_TARGET_GRUB2=y
BR2_TARGET_GRUB2_X86_64_EFI=y
BR2_TARGET_ROOTFS_EXT2=y
BR2_TARGET_ROOTFS_EXT2_4=y
```

The generated defconfig from Buildroot 2026.02.2 is authoritative when an
option name differs from this minimum capability list.

The initial target kernel is Linux `6.12.90`. Its archive and hash must be
pinned in the Buildroot configuration before the first build.

The committed defconfig is authoritative. Interactive `menuconfig` changes
must be saved back to the defconfig before review.

---

# Kernel Capability Baseline

The kernel configuration must build boot-critical drivers into the kernel
rather than depending on loadable modules before the root filesystem is
available.

Required built-in capabilities include:

- x86_64 and SMP
- EFI boot and EFI variables
- GPT partition support
- ext4 and vfat filesystems, including the EFI FAT filesystem's codepage 437
  and ISO 8859-1 I/O charset
- device mapper support required by systemd tooling
- virtio PCI, block, and network for the QEMU reference platform
- NVMe
- SATA AHCI
- USB mass storage
- common wired Ethernet support selected for validated physical hardware
- namespaces, seccomp, cgroups, and capabilities required by systemd hardening
- random-number generation suitable for node identity and SSH host keys

Drivers not required by the QEMU reference platform or validated physical
hardware should remain disabled or modular.

Kernel modules, when enabled, are installed only from the pinned kernel build.

---

# Required Target Packages

The image must include only packages required by the approved scope.

Required capability set:

```text
ca-certificates
curl
e2fsprogs with resize2fs and fsck.ext4
gptfdisk with sgdisk
GRUB 2 x86_64 EFI
OpenSSH server and ssh-keygen
sudo
systemd
systemd-journald
systemd-networkd
systemd-resolved
systemd-timesyncd
util-linux tools required for lsblk, findmnt, and block-device inspection
```

The BusyBox configuration may provide basic shell and file utilities. Applets
that duplicate a required full implementation must be disabled where ambiguity
would affect behavior.

The target image must not contain:

- a C/C++ compiler
- development headers
- Git
- general-purpose package-management tooling
- Folding@home client or FahCore binaries
- FoldOps components

---

# Project Utility

FoldingOS will implement one non-resident project utility:

```text
/usr/bin/foldingosctl
```

`foldingosctl` will be written in Go and built as a Buildroot package from
repository-local source. Third-party Go modules must be pinned and vendored.

Release builds use Go path- and metadata-trimming flags equivalent to:

```text
-trimpath
-buildvcs=false
-ldflags=-buildid=
```

The initial implementation may use vendored modules for:

- strict TOML parsing
- XZ decompression of the approved Debian FAH artifact payload

`foldingosctl` implements the minimal `ar` container and tar payload handling
required to extract the approved FAH artifact. It is not a general-purpose
Debian package manager and does not execute package maintainer scripts.

The utility exposes narrowly scoped commands:

```text
foldingosctl config validate [--all | <domain>]
foldingosctl config effective <domain>
foldingosctl config activate <domain> <candidate>
foldingosctl identity ensure
foldingosctl provision ssh
foldingosctl storage expand-data
foldingosctl fah acquire
foldingosctl fah verify-install <version>
foldingosctl fah activate <version>
foldingosctl fah run
```

Commands invoked during boot must be non-interactive, idempotent, and log
through standard output and standard error for journald capture.

Privileged commands are invoked only by root-owned systemd units. The
`foldingos-admin` user invokes administrative commands through sudo.

---

# Release Image

Primary artifact:

```text
foldingos-x86_64-<version>.img
```

Exact image size:

```text
4 GiB
```

Image creation uses Buildroot host tools and `genimage`.

The image uses GPT with these fixed labels:

| Number | GPT Name | Filesystem Label | Filesystem | Initial Size |
| --- | --- | --- | --- | --- |
| 1 | `FOLDINGOS_EFI` | `FOLDING_EFI` | vfat | 512 MiB |
| 2 | `FOLDINGOS_ROOT` | `FOLDINGOS_ROOT` | ext4 | 2 GiB |
| 3 | `FOLDINGOS_DATA` | `FOLDINGOS_DATA` | ext4 | remaining image capacity |

Partition alignment is 1 MiB. The data partition is the final partition.

Disk GUID, partition GUIDs, filesystem UUIDs, FAT volume ID, filesystem
creation options, and timestamps must be fixed in committed image-generation
configuration so two builds produce byte-identical images.

The root filesystem is writable in v0.1.0. Routine persistent state must still
reside under `/data`.

---

# EFI And GRUB Layout

The EFI System Partition contains:

```text
/EFI/BOOT/BOOTX64.EFI
/boot/grub/grub.cfg
/foldingos/provision/
```

GRUB locates the kernel on the root filesystem using its filesystem label.
Because v0.1.0 does not require an initramfs, GRUB passes the root
partition's fixed GPT partition UUID to the kernel rather than a filesystem
label or device name.

Required kernel command-line properties:

```text
root=PARTUUID=464f4c44-494e-474f-5352-4f4f54000001
rootwait
ro
```

The root filesystem may be remounted writable by systemd during v0.1.0 boot.

GRUB boots the default entry automatically and provides no project-defined
recovery entry. Physical access to the boot device remains trusted according to
the v0.1.0 security model.

---

# Mounts And Persistent Directories

Required mounts:

```text
/boot/efi  LABEL=FOLDING_EFI    vfat
/          LABEL=FOLDINGOS_ROOT ext4
/data      LABEL=FOLDINGOS_DATA ext4
```

Required persistent directories and initial ownership:

| Path | Owner | Mode | Purpose |
| --- | --- | --- | --- |
| `/data/apps/fah` | `root:root` | `0755` | versioned FAH client installs |
| `/data/config` | `root:root` | `0755` | persistent configuration |
| `/data/config/last-good` | `root:root` | `0700` | last-known-good TOML |
| `/data/config/overrides` | `root:root` | `0755` | administrator overrides |
| `/data/config/secrets` | `root:root` | `0700` | secret values |
| `/data/config/ssh` | `root:root` | `0700` | administrator authorized keys |
| `/data/config/ssh/host-keys` | `root:root` | `0700` | persistent SSH host identity |
| `/data/fah` | `fah:fah` | `0750` | FAH work, checkpoints, and runtime state |
| `/data/logs/journal` | `root:systemd-journal` | `2755` | persistent journal |
| `/data/state` | `root:root` | `0755` | FoldingOS operational state |

Directory creation and permission repair are performed by committed
`systemd-tmpfiles` rules. Permission repair must not delete existing data.

---

# Data-Partition Expansion

Expansion is performed by:

```text
foldingosctl storage expand-data
```

Required tools:

```text
findmnt
lsblk
sgdisk
partx
resize2fs
```

Algorithm:

1. Resolve the mounted root filesystem source.
2. Resolve its parent boot disk without assuming `/dev/sda`.
3. Confirm GPT and required partition labels.
4. Confirm partition 3 is `FOLDINGOS_DATA` and is the final partition.
5. Record partition 3 start sector and unique GUID.
6. Move the backup GPT header to the physical device end when required.
7. Recreate only partition 3 with the identical start sector, type, name, and
   unique GUID, extending its end to the maximum aligned usable sector.
8. Use `partx` to update the kernel's view of partition 3 without requiring
   partitions 1 or 2 to be unmounted.
9. Run `resize2fs` on the unmounted data filesystem.
10. Confirm the filesystem is mountable before allowing persistent writers.

The command exits successfully without changes when the partition and
filesystem already occupy available capacity.

The unit must never call `mkfs`, shrink a filesystem, alter partitions 1 or 2,
or change the data partition start sector or identity.

---

# Networking

v0.1.0 uses:

```text
systemd-networkd
systemd-resolved
systemd-networkd-wait-online
```

Default network file:

```ini
[Match]
Type=ether

[Network]
DHCP=ipv4
IPv6AcceptRA=no
LinkLocalAddressing=no

[DHCPv4]
UseDNS=yes
UseRoutes=yes
UseHostname=no
```

`/etc/resolv.conf` is a symlink to the systemd-resolved stub resolver.

Static networking and required IPv6 operation are rejected by the v0.1.0
configuration schema.

`network-online.target` succeeds when at least one Ethernet interface has a
usable IPv4 address and default route. Its timeout is 120 seconds. Failure does
not block SSH provisioning or local boot completion, but FAH acquisition waits
and retries.

---

# Time Synchronization

v0.1.0 uses `systemd-timesyncd`.

Default configuration:

```ini
[Time]
NTP=time.cloudflare.com time.google.com
FallbackNTP=0.pool.ntp.org 1.pool.ntp.org
```

FAH acquisition requires:

```text
network-online.target
systemd-time-wait-sync.service
```

The initial time-sync wait timeout is 180 seconds. Failure prevents new HTTPS
artifact acquisition but does not stop an already installed verified FAH
client from starting.

---

# Users And Groups

Required accounts use fixed numeric identifiers committed in Buildroot user
configuration:

| Account | UID | Group | GID | Shell |
| --- | ---: | --- | ---: | --- |
| `foldingos-admin` | 1000 | `foldingos-admin` | 1000 | `/bin/sh` |
| `fah` | 200 | `fah` | 200 | `/usr/sbin/nologin` |

`foldingos-admin` has no usable password hash.

`fah` has no interactive login, password, or administrative privileges.

---

# SSH Configuration And Provisioning

OpenSSH listens on TCP port 22 only when at least one valid persistent
administrator key exists.

Required `sshd_config` policy:

```text
PermitRootLogin no
PasswordAuthentication no
KbdInteractiveAuthentication no
PubkeyAuthentication yes
AuthorizedKeysFile /data/config/ssh/authorized_keys
HostKey /data/config/ssh/host-keys/ssh_host_ed25519_key
AllowUsers foldingos-admin
X11Forwarding no
AllowTcpForwarding no
PermitTunnel no
```

The provisioning command:

```text
foldingosctl provision ssh
```

reads:

```text
/boot/efi/foldingos/provision/authorized_keys
```

Supported v0.1.0 key types:

```text
ssh-ed25519
ecdsa-sha2-nistp256
ssh-rsa with at least 3072 bits
```

Authorized-key options and private-key material are rejected. Blank lines and
comments are ignored.

The complete candidate key set is validated with `ssh-keygen`, written to a
temporary file under `/data/config/ssh`, flushed, and atomically renamed over
the active file. The provisioning file is removed only after successful
activation.

On first boot, the provisioning service generates one Ed25519 SSH host key
under `/data/config/ssh/host-keys` when no valid host key exists. The private
key uses `root:root 0600`; its public key uses `root:root 0644`. Existing valid
host keys are never regenerated automatically.

The sudo policy is:

```text
foldingos-admin ALL=(ALL:ALL) NOPASSWD: ALL
```

---

# Structured Configuration

Image defaults:

```text
/etc/foldingos/defaults/system.toml
/etc/foldingos/defaults/network.toml
/etc/foldingos/defaults/foldinghome.toml
```

Persistent configuration:

```text
/data/config/system.toml
/data/config/network.toml
/data/config/foldinghome.toml
```

Administrator overrides:

```text
/data/config/overrides/system.toml
/data/config/overrides/network.toml
/data/config/overrides/foldinghome.toml
```

Last-known-good files:

```text
/data/config/last-good/system.toml
/data/config/last-good/network.toml
/data/config/last-good/foldinghome.toml
```

Runtime temporary overrides are not implemented in v0.1.0.

Validated effective configuration is written under:

```text
/run/foldingos/effective/
```

Generated runtime configuration for system services is written only under
`/run` and regenerated at boot.

The FAH renderer writes validated effective client configuration under:

```text
/run/foldingos/fah/
```

Rendered FAH configuration is owned by `root:fah` with mode `0640`. Secret
values are read only while rendering and are not written to diagnostic
effective-configuration output.

---

# TOML Schemas

All files reject unknown keys and require:

```toml
schema_version = 1
```

## system.toml

```toml
schema_version = 1

[identity]
hostname = ""
```

Rules:

- `hostname` may be empty to request generated identity-based naming
- non-empty hostnames must satisfy the RFC 1123 host-label form
- generated hostname format is `folding-<first-6-node-id-hex>`

## network.toml

```toml
schema_version = 1

[ethernet]
dhcp = true
required_for_online = true
```

Rules:

- `dhcp` must be `true` in v0.1.0
- no static address, gateway, DNS, VLAN, bond, or Wi-Fi keys are accepted

## foldinghome.toml

```toml
schema_version = 1

[identity]
username = "Anonymous"
team = 0
passkey_secret = ""

[resources]
cpus = 0
gpus = false
```

Rules:

- `username` is a non-empty UTF-8 string of at most 128 bytes
- `team` is an integer from `0` through `2147483647`
- `passkey_secret` is empty or a filename under `/data/config/secrets`
- `passkey_secret` must be a basename containing only letters, digits, `.`,
  `_`, and `-`; path traversal is rejected
- `cpus = 0` means automatic selection; otherwise it must be a positive integer
- `gpus` must be `false` in v0.1.0
- exact translation to the FAH 8.5 client is validated during FAH artifact
  integration

---

# Secrets

v0.1.0 defines one optional secret:

```text
/data/config/secrets/fah-passkey
```

Required ownership and mode:

```text
root:fah 0640
```

The file contains only the passkey followed by an optional final newline.

Secret contents must never appear in generated effective-configuration output,
logs, diagnostic bundles, or release artifacts.

---

# Configuration Activation

`foldingosctl config activate <domain> <candidate>` performs:

1. Acquire `/run/lock/foldingos-config-<domain>.lock`.
2. Confirm the candidate is a regular file on `/data`.
3. Parse and validate the candidate schema.
4. Validate secret references and security invariants.
5. Build and validate effective configuration using precedence.
6. Write the previous valid active file to
   `/data/config/last-good/<domain>.toml.tmp`.
7. Flush and atomically rename the last-known-good file.
8. Flush and atomically rename the candidate over the active file.
9. Reload or restart the affected service.
10. Confirm service health for up to 30 seconds.

If service health validation fails, the command restores last-known-good
configuration and restarts the service once. Failure is logged and returned to
the caller.

Configuration files use `root:root 0644`. Files containing secret references
remain readable because references are not secret values.

---

# Node Identity

`foldingosctl identity ensure` creates:

```text
/data/config/node-id
```

The node ID is a lowercase UUIDv4 stored as `root:root 0644`.

If `system.toml` does not contain a hostname, the command derives and applies:

```text
folding-<first-6-node-id-hex>
```

Node identity is created once and never regenerated automatically when a valid
identity exists.

---

# Folding@home Manifest

The embedded approved manifest is TOML:

```text
/usr/share/foldingos/manifests/fah.toml
```

Schema:

```toml
schema_version = 1
client_version = "8.5.x"
architecture = "x86_64"
artifact_url = "REQUIRED_BEFORE_RELEASE"
artifact_size = 0
sha256 = "REQUIRED_BEFORE_RELEASE"
artifact_format = "deb"
minimum_foldingos_version = "0.1.0"
terms_url = "REQUIRED_BEFORE_RELEASE"
executable_path = "REQUIRED_BEFORE_RELEASE"
arguments = ["REQUIRED_BEFORE_RELEASE"]
```

These fields are release inputs, not implementation choices. Before release,
they must be populated from the exact tested official FAH 8.5 artifact.
`artifact_size` must be non-zero. `executable_path` must be an absolute path
under `/data/apps/fah/current`, and every argument must be explicitly listed.

The embedded manifest is trusted through the FoldingOS image. v0.1.0 does not
accept external manifests.

---

# Folding@home Acquisition And Activation

`foldingosctl fah acquire`:

1. Exits successfully without network access when `current` already references
   a verified compatible installation.
2. Validates the embedded manifest schema and architecture.
3. Rejects non-HTTPS and non-approved Folding@home origins.
4. Downloads to `/data/apps/fah/.downloads/<version>.partial`.
5. Enforces the expected maximum size during download.
6. Verifies exact size and SHA-256 digest.
7. Extracts the approved artifact into
   `/data/apps/fah/<version>.staging`.
8. Validates required executable files and runtime library compatibility.
9. Changes installed files to `root:root` ownership and removes unexpected
   writable permissions.
10. Atomically renames staging to `/data/apps/fah/<version>`.
11. Runs `foldingosctl fah verify-install <version>`.
12. Atomically replaces `/data/apps/fah/current` with a relative symlink to the
    verified version.
13. Starts or restarts `folding-at-home.service`.

The partial download and staging directory are removed after failure.
Previously verified versions are retained.

Runtime compatibility validation records the executable interpreter and shared
library requirements reported by ELF inspection. Every required library must
be supplied by the pinned FoldingOS image before the artifact can be approved
for release.

The initial retry schedule is:

```text
1 minute
5 minutes
15 minutes
1 hour
6 hours
```

After reaching six hours, retries continue every six hours until success.

---

# Folding@home Service

Service name:

```text
folding-at-home.service
```

Before the service starts, `foldingos-fah-prepare.service` validates the active
installation and renders effective FAH configuration under
`/run/foldingos/fah`.

The implementation records the exact executable path and arguments in the
embedded approved manifest after inspecting and validating the official FAH
8.5 artifact. They must not be inferred dynamically from downloaded content.

The service command is:

```ini
ExecStart=/usr/bin/foldingosctl fah run
```

`foldingosctl fah run` revalidates the active installation, drops no
privileges itself because systemd already runs it as `fah`, and replaces
itself with the exact manifest-defined executable and arguments. The manifest
path must lexically begin under `/data/apps/fah/current`; after resolving the
`current` symlink and all path components, the executable must remain beneath
the verified version directory referenced by `current`.

Required service properties:

```ini
User=fah
Group=fah
WorkingDirectory=/data/fah
Restart=on-failure
RestartSec=30s
StartLimitIntervalSec=10min
StartLimitBurst=5
NoNewPrivileges=yes
PrivateTmp=yes
ProtectHome=yes
ProtectSystem=strict
ReadWritePaths=/data/fah
ReadOnlyPaths=/data/apps/fah/current
ReadOnlyPaths=/run/foldingos/fah
```

The service starts only when `/data/apps/fah/current` references a verified
installation and Folding@home effective configuration is valid.

An already installed verified client may start without network availability or
successful time synchronization. The client handles its own inability to fetch
work and FahCores.

---

# Persistent Logging

`journald.conf`:

```ini
[Journal]
Storage=auto
SystemMaxUse=256M
SystemKeepFree=512M
MaxRetentionSec=14day
MaxFileSec=1day
RuntimeMaxUse=64M
RateLimitIntervalSec=30s
RateLimitBurst=1000
Compress=yes
Seal=no
```

`/data/logs/journal` is bind-mounted at `/var/log/journal` after `/data` is
mounted. Early logs remain volatile; `journalctl --flush` runs after the bind
mount becomes available.

No v0.1.0 service receives a journald rate-limit override.

Persistent-journal failure degrades to volatile logging and does not block FAH.

---

# Systemd Unit Graph

Required units:

```text
foldingos-data-expand.service
data.mount
foldingos-persistent-dirs.service
var-log-journal.mount
foldingos-journal-flush.service
foldingos-identity.service
foldingos-config-validate.service
foldingos-ssh-provision.service
sshd.service
systemd-networkd.service
systemd-networkd-wait-online.service
systemd-resolved.service
systemd-timesyncd.service
foldingos-fah-acquire.service
foldingos-fah-acquire.timer
foldingos-fah-prepare.service
folding-at-home.service
```

Required ordering:

```text
boot-efi.mount
  -> foldingos-data-expand.service
  -> data.mount
  -> foldingos-persistent-dirs.service
     -> var-log-journal.mount
        -> foldingos-journal-flush.service
     -> foldingos-identity.service
        -> foldingos-config-validate.service
     -> foldingos-ssh-provision.service
        -> sshd.service

foldingos-config-validate.service
  -> systemd-networkd.service
  -> systemd-networkd-wait-online.service
  -> network-online.target
     -> systemd-timesyncd.service
        -> systemd-time-wait-sync.service
           -> foldingos-fah-acquire.service

data.mount
  -> foldingos-fah-prepare.service

foldingos-config-validate.service
  -> foldingos-fah-prepare.service

foldingos-fah-prepare.service
  -> folding-at-home.service
```

`foldingos-fah-prepare.service` and `folding-at-home.service` do not require
`network-online.target` or successful time synchronization.

Acquisition failure does not make boot fail. When no verified client is
installed, the acquisition timer retries using the documented backoff state
under `/data/state`. When a verified client already exists, local preparation
and FAH startup proceed independently of acquisition.

There are no FoldOps units in v0.1.0.

---

# Service Failure Policy

| Failure | Required Behavior |
| --- | --- |
| Data expansion fails but existing data mounts safely | Continue with original capacity |
| Data cannot mount safely | Do not start persistent-data writers |
| Persistent journal unavailable | Use volatile journal and continue |
| No administrator key | Do not start SSH; continue boot |
| Invalid SSH provisioning candidate | Preserve existing keys |
| Network unavailable | Continue boot; acquisition retries |
| Time sync unavailable | Do not acquire new artifact; installed FAH may start |
| Invalid system configuration | Use last-known-good or image defaults |
| Invalid FAH configuration | Do not start FAH; preserve SSH recovery |
| FAH artifact verification fails | Never install or execute it |
| FAH repeatedly exits | Apply bounded restart policy and leave diagnostics |

---

# QEMU/OVMF Reference Test

The required automated reference environment uses:

```text
qemu-system-x86_64
OVMF UEFI firmware
virtio-blk storage
virtio-net Ethernet
at least 2 GiB RAM
at least 2 vCPUs
```

`scripts/test-qemu` must support:

```text
booting an exact-size 4 GiB disk
booting a larger disk and verifying expansion
injecting an EFI authorized_keys file
waiting for SSH
running acceptance checks over SSH
power-cut simulation
reboot and persistence verification
capturing serial console and journal diagnostics
```

The test harness must not modify the release image directly. It creates a
per-test copy before injecting provisioning data.

---

# Required Automated Tests

At minimum, CI and release verification implement:

## Build Tests

- clean Buildroot build
- no uncommitted or source-override inputs
- expected 4 GiB image size
- expected partition table, labels, GUIDs, and filesystems
- no FAH or FoldOps binaries in the image
- exact required artifact list

## Boot Tests

- QEMU/OVMF boot
- DHCP and DNS
- time synchronization
- SSH disabled without key
- valid key provisioning and SSH access
- root/password SSH rejection

## Storage Tests

- exact-size disk no-op expansion
- larger-disk successful expansion
- repeated expansion no-op
- existing file preservation
- invalid-layout safe failure

## Configuration Tests

- valid schemas
- malformed TOML rejection
- unknown-key rejection
- secret-value leakage rejection
- atomic activation interruption
- last-known-good recovery
- affected-service isolation

## Logging Tests

- persistence across reboot
- size and free-space limits
- rotation and vacuuming
- rate limiting
- volatile fallback
- secret redaction

## FAH Tests

- exact manifest validation
- wrong origin, size, hash, and architecture rejection
- verified install and atomic activation
- failed activation preserves previous version
- configuration and checkpoint preservation
- service runs as `fah`

## Reproducibility Tests

- clean build on dedicated Debian 13 builder
- clean build in disposable Debian 13 environment
- byte-identical image, version metadata, and checksum manifest

---

# Build Commands

Required user-facing commands:

```bash
./scripts/check-host-tools
./scripts/fetch-sources
./scripts/build
./scripts/test-qemu
./scripts/verify-reproducible
```

`scripts/check-host-tools` verifies the Debian 13 amd64 build-host baseline,
all required build and test tools, and the OVMF firmware files. It reports the
Debian package names needed for any missing requirements.

`scripts/build`:

1. Refuses a dirty worktree for release mode.
2. Verifies Buildroot and source hashes.
3. Derives deterministic timestamps from the Git source revision.
4. Creates an empty out-of-tree Buildroot output directory.
5. Applies `foldingos_x86_64_defconfig`.
6. Builds the complete image.
7. Produces version metadata and a SHA-256 artifact manifest.

The script supports an explicit developer mode that may use a writable download
cache but must not claim reproducibility.

---

# Release Artifacts

Required v0.1.0 artifacts:

```text
foldingos-x86_64-0.1.0.img
foldingos-x86_64-0.1.0.img.sha256
foldingos-x86_64-0.1.0.metadata.json
foldingos-x86_64-0.1.0.reproducibility.json
RELEASE_NOTES.md
```

Deterministic metadata JSON produced by both clean builds contains:

```text
FoldingOS version
Git commit and source timestamp
Buildroot version and digest
Linux kernel version
Build-host baseline Debian release and architecture
build-host package-manifest digest
Buildroot defconfig digest
source-input digest manifest
artifact digests
approved FAH manifest digest
```

Metadata key ordering and formatting must be deterministic.

Each clean build also emits a non-release verification record containing its
actual hostname-independent kernel version and installed package versions.
Those records are compared and retained for diagnostics but are not required
to be byte-identical release artifacts.

After both builds match, `verify-reproducible` creates the deterministic
reproducibility JSON report that records both required-artifact digest sets and
the pass result. The report must not contain hostnames, usernames, workspace
paths, or wall-clock timestamps.

---

# Release Gates

v0.1.0 publication is blocked until:

1. All placeholders in the approved FAH 8.5 manifest are replaced.
2. The FAH artifact runs successfully on FoldingOS and passes compatibility
   tests.
3. QEMU/OVMF acceptance tests pass.
4. Every claimed physical validated system passes its hardware acceptance test.
5. Security and failure-injection tests pass.
6. Two independent clean builds produce byte-identical required artifacts.
7. Documentation and release notes match the implementation.

---

# Implementation Sequence

Implementation should proceed in this order:

1. Buildroot bootstrap, defconfig, kernel, root filesystem, and QEMU boot
2. deterministic GPT image, EFI partition, and GRUB
3. systemd, persistent data mount, and expansion
4. DHCP networking, DNS, and time synchronization
5. persistent directories and journald
6. node identity, TOML validation, and configuration activation
7. administrator account, SSH policy, and EFI key provisioning
8. FAH manifest, acquisition, verification, activation, and service
9. automated QEMU acceptance suite
10. reproducibility verification and physical hardware validation

Each step must add its required automated tests before proceeding to dependent
steps.
