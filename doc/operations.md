# FoldingOS Operations

**Version:** 1.0

**Status:** Approved for v0.1.0 appliance operation

---

# Purpose

This document describes how operators and developers build, deploy, administer,
diagnose, and recover a FoldingOS v0.1.0 appliance, including the Folding@home
runtime introduced in Milestone 2.

FoldingOS is a headless appliance. Normal administration uses SSH. A monitor
and keyboard are not required in production.

During commissioning, a temporarily attached monitor shows kernel and service
boot messages on `tty1` and a final ready message with the DHCP IPv4 address
and SSH entry point. See
[ADR-0015](adr/0015-local-commissioning-display.md).

For the full on-appliance command surface, see
[foldingosctl.md](foldingosctl.md).

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

Network fleet provisioning acceptance (supervisor-led PXE install, enrollment,
and staged update) on the QEMU/OVMF reference platform:

```bash
./scripts/test-provision-qemu
```

Requires KVM when available; pure TCG network installs may take hours.
See [issue #63](https://github.com/pacificnm/folding-os/issues/63).

Additional static verification helpers:

```bash
./scripts/verify-systemd-graph build/output/images/rootfs.tar
./scripts/verify-config build/output/images/rootfs.tar
./scripts/verify-persistent-logging build/output/images/rootfs.tar
./scripts/verify-fah-manifest build/output/images/rootfs.tar
./scripts/verify-foldops-manifest build/output/images/rootfs.tar
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
  --role supervisor \
  --foldops-ingest-token /path/to/foldops-ingest-token \
  /dev/sdX \
  build/output/images/foldingos-x86_64-0.1.0.img
```

Generate the ingest token with `openssl rand -hex 32`. The token file must
contain a single line of 64 lowercase hex characters. See
[ADR-0019](adr/0019-foldops-supervisor-provisioning-and-tls.md).

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

The appliance reaches normal operation when it acquires DHCP on wired Ethernet,
accepts SSH from a provisioned administrator key, and writes the commissioning
ready message to the local display when a monitor is attached.

## Commissioning display

When a monitor is temporarily attached during setup, the node shows:

1. kernel and service boot messages on `tty1`
2. a final ready message after DHCP networking is online:

```text
FoldingOS 0.1.0 ready
Address: 192.168.4.32
SSH: foldingos-admin@192.168.4.32
```

The version string comes from `/usr/lib/os-release`. The address is the actual
routable IPv4 address acquired on wired Ethernet. The example above is
illustrative only.

This display is informational. It does not provide local login, and production
nodes are not expected to keep a monitor or keyboard attached. See
[ADR-0015](adr/0015-local-commissioning-display.md).

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

# Network Fleet Provisioning

Supervisor-led fleet expansion is defined by
[ADR-0016](adr/0016-network-provisioning-via-supervisor.md) and
[installer.md](installer.md). The first node is always a `supervisor` installed
by direct flash. Additional `agent` nodes network boot from the supervisor path.

## Supervisor services

On a provisioned supervisor:

```bash
systemctl status foldingos-provision.service foldingos-provision-boot.service
foldingosctl registry list
foldingosctl provision list-enrollments
```

`foldingos-provision.service` serves the HTTP provisioning API and image
streaming endpoints. `foldingos-provision-boot.service` runs proxy-DHCP, TFTP,
and HTTP boot assistance for blank agent machines.

## Allow a blank agent to network boot

Before a machine can fetch the install iPXE script, add its Ethernet MAC to the
boot allowlist:

```bash
sudo foldingosctl provision allow-boot 00:be:43:e7:59:5e
```

On dual-disk agents where automatic target selection would pick the wrong
internal disk, pin the install target at the same time:

```bash
sudo foldingosctl provision allow-boot --disk /dev/sda 00:be:43:e7:59:5e
```

The pinned mapping is stored at
`/data/config/provision/boot-install-disk-allowlist` and passed to the install
initramfs as `foldingos.install-disk=` in the iPXE kernel command line.

## Registry refresh during lab work

When iterating on a local build, replace the supervisor registry image with the
candidate from `build/output/images/`:

```bash
./scripts/refresh-supervisor-registry-lab <supervisor-host> <ssh-private-key>
```

## Assign and validate agent updates

Assign a desired image version on the supervisor:

```bash
foldingosctl provision assign --version 0.1.0 --all
# or
foldingosctl provision assign --version 0.1.0 --node <node-uuid>
```

Validate staged update behavior on physical lab hardware:

```bash
./scripts/validate-agent-update-lab <supervisor-host> <agent-host> <ssh-private-key>
```

## Automated QEMU acceptance

```bash
./scripts/test-provision-qemu
```

See [testing-strategy.md](testing-strategy.md) and
[milestone/3-readiness-review.md](milestone/3-readiness-review.md).

## Network install recovery

If network installation fails before the agent reaches first boot:

1. Correct enrollment, registry, networking, or target-disk faults on the
   supervisor.
2. Re-run `allow-boot` for the client MAC when needed.
3. Network boot the blank machine again.

Interrupted installation may leave the target disk unbootable. Repeating network
provisioning is the supported recovery path. Direct flash remains available for
single-node emergencies.

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

---

# Folding@home Runtime

FoldingOS does not redistribute the Folding@home client or FahCore binaries.
Release images contain only the approved acquisition manifest. After deployment,
the node downloads the exact pinned artifact from official Folding@home HTTPS
infrastructure, verifies it, installs it into versioned persistent storage, and
runs it as the dedicated `fah` service account.

References:

- [ADR-0006](adr/0006-fah-packaging-and-privilege-model.md)
- [ADR-0009](adr/0009-fah-acquisition-and-update-model.md)
- [Milestone 2 readiness review](milestone/2-readiness-review.md)

## Approved v0.1.0 client

| Item | Value |
| --- | --- |
| Client version | `8.5.6` |
| Package | `fah-client_8.5.6_amd64.deb` |
| Manifest | `/usr/share/foldingos/manifests/fah.toml` |
| Upstream origin | `download.foldingathome.org` |
| License / terms | GPL-3.0-or-later; see manifest `terms_url` and [foldingathome.org](https://foldingathome.org/faq/opensource/) |

FahCore binaries are not part of the FoldingOS image. The running client may
download them separately from Folding@home infrastructure during normal
operation. FoldingOS does not mirror, cache, or proxy those downloads.

FoldOps is not required for acquisition or continued Folding@home operation.

## Boot and service sequence

After networking and time synchronization:

1. `foldingos-fah-acquire.timer` triggers `foldingos-fah-acquire.service`
2. `foldingosctl fah acquire` reads the embedded manifest and, when needed,
   downloads and verifies the pinned artifact
3. A verified install is activated under `/data/apps/fah/<version>/` with
   `/data/apps/fah/current` pointing at the active version
4. `foldingos-fah-prepare.service` renders `/run/foldingos/fah/config.xml`
5. `folding-at-home.service` execs the manifest-defined `fah-client` as user
   `fah` (UID/GID `200`)

An already verified active client skips re-download on later acquire attempts.

## Persistent locations

| Path | Purpose |
| --- | --- |
| `/data/apps/fah/<version>/` | Verified client installation |
| `/data/apps/fah/current` | Relative symlink to active version |
| `/data/fah/` | Client work, checkpoints, and logs |
| `/data/config/foldinghome.toml` | Operator Folding@home configuration |
| `/data/config/secrets/` | Passkey and other secrets referenced by config |
| `/run/foldingos/fah/config.xml` | Rendered runtime configuration |
| `/data/state/fah-acquire.state` | Acquisition retry state after failure |

## Acquisition retries

When download, verification, or activation fails:

- the partial artifact and staging directory are removed
- the last verified installed client remains available
- retry state is written to `/data/state/fah-acquire.state`
- the timer retries using this schedule: `1m`, `5m`, `15m`, `1h`, then `6h`
  indefinitely

Successful acquisition clears retry state.

## Runtime acceptance

After the node reaches multi-user operation and time synchronization:

```bash
./scripts/run-physical-acceptance <host> <ssh-private-key> [port]
```

The command waits for `folding-at-home.service`, then verifies the verified
client install, rendered runtime configuration, and `fah` process execution.

## Useful inspection commands

```bash
foldingosctl fah validate-manifest
systemctl status foldingos-fah-acquire.timer foldingos-fah-acquire.service
systemctl status foldingos-fah-prepare.service folding-at-home.service
readlink /data/apps/fah/current
sudo test -f /data/apps/fah/8.5.6/.foldingos-verified
sudo test -f /run/foldingos/fah/config.xml
sudo cat /data/state/fah-acquire.state
```

To inspect the running client without `ps` on the appliance image:

```bash
main_pid="$(systemctl show folding-at-home.service -p MainPID --value)"
sudo grep -a -Fq 'fah-client' "/proc/${main_pid}/cmdline"
```

Administrators with passwordless `sudo` may trigger acquisition manually:

```bash
sudo foldingosctl fah acquire
```

Manual acquisition follows the same verification rules as the scheduled
service and does not install unpinned or unverified artifacts.

---

# FoldOps Runtime

FoldOps packages are not embedded in release images. After deployment, the node
downloads pinned verified artifacts, extracts them under `/data/apps/foldops/`,
and starts role-appropriate services only after ingest-token and TLS
provisioning succeed.

On Milestone 3 appliances, acquisition uses embedded bootstrap manifest schema
v1 and `.deb` extract from `deb.folding-os.com`. Milestone 4 targets
`layout-tar-zst` bundles from `packages.folding-os.com/foldops/` with optional
supervisor-assigned manifest at `/data/config/foldops/assigned-manifest.toml`.
Assigned manifests override the bootstrap floor without OS reimage.

References:

- [ADR-0018](adr/0018-foldops-package-acquisition-and-update-model.md)
- [ADR-0019](adr/0019-foldops-supervisor-provisioning-and-tls.md)
- [ADR-0023](adr/0023-runtime-foldops-and-foldingosctl-updates-without-os-reimage.md)
- [milestone/4-appliance-artifact-and-monorepo-plan.md](milestone/4-appliance-artifact-and-monorepo-plan.md)
- [foldingosctl.md](foldingosctl.md)

## Supervisor USB staging

Supervisor direct flash requires both administrator SSH keys and the fleet
ingest token on the EFI System Partition before first boot:

```bash
sudo ./scripts/make-bootable-usb \
  --ssh-public-key /path/to/admin-key.pub \
  --role supervisor \
  --foldops-ingest-token /path/to/foldops-ingest-token \
  /dev/sdX \
  build/output/images/foldingos-x86_64-0.1.0.img
```

## Boot and service sequence

After installation role validation, networking, and time synchronization:

1. `foldingos-foldops-acquire.timer` triggers `foldingos-foldops-acquire.service`
2. `foldingosctl foldops acquire` downloads, verifies, and activates pinned
   FoldOps packages under `/data/apps/foldops/current` (from assigned manifest
   when present, otherwise bootstrap manifest)
3. `foldingos-foldops-provision.service` runs `foldingosctl foldops provision`
   to import the ingest token, render env files, generate supervisor TLS, and
   write `/data/state/foldops/provisioned.json`
4. On supervisor role: `foldingos-foldops-serve-https.service` and
   `foldingos-foldops-supervisor.service` start the HTTPS front end on
   `:3443` and loopback supervisor on `:3000`
5. `foldingos-foldops-agent.service` starts `foldops-agent` after provision
   and after `folding-at-home.service` when Folding@home is active

FoldOps failure must not block boot or Folding@home.

## Useful inspection commands

```bash
foldingosctl foldops validate-manifest
foldingosctl inspect foldops --format json
foldingosctl inspect tools --format json
systemctl status foldingos-foldops-acquire.timer foldingos-foldops-acquire.service
systemctl status foldingos-foldops-provision.service
systemctl status foldingos-foldops-serve-https.service foldingos-foldops-supervisor.service
systemctl status foldingos-foldops-agent.service
readlink /data/apps/foldops/current
sudo test -f /data/state/foldops/provisioned.json
sudo test -f /data/config/foldops/ingest-token
curl -k https://127.0.0.1:3443/
```

## foldingosctl tools updates (Milestone 4)

Routine `foldingosctl` fixes must not require OS image reflash. When supervisor
assignment is enabled:

1. supervisor writes `/data/config/tools/assigned-version.json`
2. `foldingosctl tools acquire` downloads the pinned binary from
   `packages.folding-os.com/foldingos-tools/`
3. verifies SHA-256 and atomically replaces `/usr/bin/foldingosctl`
4. records active version under `/data/state/tools/`

Manual acquire:

```bash
sudo foldingosctl tools acquire
foldingosctl inspect tools --format json
```

See [ADR-0023](adr/0023-runtime-foldops-and-foldingosctl-updates-without-os-reimage.md).

## Packages channel publication (Milestone 5)

FoldOps layout bundles and `foldingosctl` tools binaries publish to
`packages.folding-os.com` using **rclone** from the build host. OS disk images
continue to publish through `releases.folding-os.com` separately.

### Prerequisites

- `rclone` installed on the build host
- R2 remote configured at `~/.config/rclone/rclone.conf` (default remote name
  `r2`, overridable via `R2_REMOTE`)
- Build outputs under `build/output/foldops/<manifest_release>/` and/or
  `build/output/foldingos-tools/<version>/`

Default destination environment variables (override when needed):

| Variable | Default | Purpose |
| --- | --- | --- |
| `R2_REMOTE` | `r2` | rclone remote name |
| `FOLDOPS_R2_BUCKET` | `foldops-packages` | destination bucket |
| `FOLDOPS_R2_PREFIX` | `foldops` | FoldOps object prefix |
| `TOOLS_R2_PREFIX` | `foldingos-tools` | tools object prefix |
| `PACKAGES_PUBLIC_BASE` | `https://packages.folding-os.com` | public URL base |

Scripts do **not** embed credentials. Credentials live only in the operator rclone
config file per [ADR-0029](adr/0029-packages-channel-publication-via-rclone.md).

### Build and publish

Build only:

```bash
./scripts/build-foldops-bundles --manifest-release 0.1.0-2 --sync-overlay
./scripts/build-foldingosctl-release --version 0.1.1 --sync-overlay
```

Publish one channel:

```bash
./scripts/publish-foldops-bundles 0.1.0-2
./scripts/publish-foldingos-tools 0.1.1
```

Build and publish a tools-only update:

```bash
./scripts/build-foldingosctl-release --version 0.1.1 --sync-overlay
./scripts/publish-foldingos-tools 0.1.1
```

Build and publish both channels (umbrella script):

```bash
./scripts/publish-packages-release --foldops 0.1.0-2 --tools 0.1.1 --build
```

Dry-run (list planned uploads and index updates without writing objects):

```bash
./scripts/publish-packages-release --foldops 0.1.0-2 --tools 0.1.1 --dry-run
```

Each publication refreshes the channel **`index.json`** at:

- `https://packages.folding-os.com/foldops/index.json`
- `https://packages.folding-os.com/foldingos-tools/index.json`

Supervisor “check for updates” reads these indexes per
[ADR-0028](adr/0028-supervisor-fleet-software-update-workflow.md).

Publishing a new FoldOps or `foldingosctl` tools release does **not** require an
immediate OS image rebuild. `--sync-overlay` updates the repository overlay pin
so the next `./scripts/build` embeds the current bootstrap manifest or tools
assignment, while supervisor assignment overrides the floor on running nodes per
[ADR-0023](adr/0023-runtime-foldops-and-foldingosctl-updates-without-os-reimage.md).

References:

- [ADR-0029](adr/0029-packages-channel-publication-via-rclone.md)
- [Milestone 5 engineering spec](milestone/5-engineering-spec.md)

## Supervisor recovery export and restore (Milestone 5)

Supervisor nodes hold fleet-critical state: FoldOps SQLite database, configuration,
enrollment records, and assigned software pins. Before risky updates or migration,
create an on-demand backup and download it through the HTTPS API or `foldingosctl`.

### Create and download a backup

Via supervisor admin UI (recommended):

Open `https://<supervisor>:3443/admin/recovery` from a browser on the same
network. Create and download backups without pasting an ingest token — the
supervisor serves the admin UI over HTTPS like the other Milestone 5 admin
screens.

Via HTTPS API (localhost or automation):

```bash
curl -k -X POST https://127.0.0.1:3443/api/recovery/export \
  -H "Content-Type: application/json" \
  -d '{"include_secrets": false}'

curl -k -L -o foldingos-supervisor-backup.tar.zst \
  https://127.0.0.1:3443/api/recovery/export/latest
```

Via SSH:

```bash
sudo foldingosctl recovery export
sudo foldingosctl recovery export --output /tmp/supervisor-backup.tar.zst
```

Exports are retained under `/data/foldops/backups/` (last three archives). Private
TLS keys under `/data/foldops/tls/` are excluded unless the operator passes
`--include-secrets` or `"include_secrets": true` in the API request.

### Restore from a backup

Restore is operator-guided and fail-closed: manifest validation must pass before
any file is overwritten.

1. Copy the `.tar.zst` archive to the supervisor.
2. Validate without writing:

```bash
sudo foldingosctl recovery import /path/to/archive.tar.zst --dry-run
```

3. Restore and restart supervisor services:

```bash
sudo foldingosctl recovery import /path/to/archive.tar.zst
```

4. Confirm FoldOps supervisor, provisioning, and HTTPS services are healthy:

```bash
systemctl status foldingos-foldops-supervisor.service
curl -k https://127.0.0.1:3443/
```

See [ADR-0030](adr/0030-supervisor-recovery-backup-and-export.md).

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

## Folding@home runtime

```bash
foldingosctl fah validate-manifest
systemctl status foldingos-fah-acquire.timer folding-at-home.service
sudo cat /data/state/fah-acquire.state
sudo journalctl -u foldingos-fah-acquire.service -u folding-at-home.service -b --no-pager
```

`journalctl` may require `sudo` on the appliance image.

## Build and release artifacts

```bash
sha256sum -c build/output/images/foldingos-x86_64-0.1.0.img.sha256
./scripts/verify-physical-validation-record \
  validation/appliance-physical-0.1.0.json \
  build/output/images/foldingos-x86_64-0.1.0.img
```

---

# Recovery

## Failed network provisioning

If a blank machine fails during PXE network install before first agent boot, see
[Network Fleet Provisioning](#network-fleet-provisioning) (network install
recovery). Direct flash remains the single-node emergency recovery path.

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

## Folding@home acquisition failing

1. Confirm DNS and time synchronization:

   ```bash
   resolvectl query download.foldingathome.org
   timedatectl show -p NTPSynchronized --value
   ```

2. Inspect acquisition state and recent service output:

   ```bash
   sudo cat /data/state/fah-acquire.state
   sudo journalctl -u foldingos-fah-acquire.service -b --no-pager
   ```

3. Wait for the timer retry or run a manual attempt:

   ```bash
   sudo foldingosctl fah acquire
   ```

An already verified active client continues running while acquisition retries.
Failed attempts do not execute or activate unverified artifacts.

## Folding@home service not running

1. Confirm a verified active install exists:

   ```bash
   readlink /data/apps/fah/current
   test -f /data/apps/fah/8.5.6/.foldingos-verified
   ```

2. Confirm runtime configuration was rendered:

   ```bash
   sudo test -f /run/foldingos/fah/config.xml
   systemctl status foldingos-fah-prepare.service
   ```

3. Inspect the runtime service:

   ```bash
   systemctl status folding-at-home.service
   sudo journalctl -u folding-at-home.service -b --no-pager
   ```

`folding-at-home.service` restarts on failure with bounded burst limits.

## Unexpected power loss

Boot again and rerun:

```bash
./scripts/run-physical-acceptance <host> <ssh-private-key>
```

The appliance is designed to recover persistent configuration, verified Folding@home
client state, journal records, and node identity across reboot and unclean
shutdown when storage remains intact.

## Full appliance replacement

v0.1.0 updates use full-image reflashing. Reflash the release image, provision
a new administrator key on EFI, and boot. Preservation of an existing data
partition across reflashes is not guaranteed in v0.1.0.

---

# Known Limitations

- No local GUI or installer UI in v0.1.0
- No package manager on the target image
- Agent FoldOps HTTPS trust uses `SUPERVISOR_TLS_CA` in the Rust agent
  (`packages/foldops/`); FoldingOS stages the CA and terminates TLS on the
  supervisor
- No Folding@home client embedded in the release image
- CPU-only Folding@home in v0.1.0; GPU support is out of scope
- First client acquisition requires upstream HTTPS reachability
- Local display is for commissioning only; no keyboard or console login is provided
- Unsupported hardware may still show no local output if UEFI framebuffer is unavailable
- Only documented validated hardware carries a support claim

Validated physical systems are listed in [hardware-support.md](hardware-support.md).

---

# Related Documents

- [Build system](build-system.md)
- [Boot process](boot-process.md)
- [Physical validation](physical-validation.md)
- [Milestone 3 readiness review](milestone/3-readiness-review.md)
- [Deployment and provisioning](installer.md)
- [Milestone 3 engineering specification](milestone/3-engineering-spec.md)
- [Milestone 2 readiness review](milestone/2-readiness-review.md)
- [Security model](security.md)
- [Testing strategy](testing-strategy.md)
