# foldingosctl Command Reference

**Version:** 1.0

**Status:** Current for v0.1.0 appliance images

---

# Purpose

`foldingosctl` is the primary on-appliance control program for FoldingOS. It
implements first-boot provisioning, configuration management, storage expansion,
Folding@home client lifecycle, and Milestone 3 supervisor/agent fleet
provisioning.

The binary is installed at `/usr/bin/foldingosctl` and is invoked directly by
operators or by `systemd` units on boot and timer schedules.

Most commands are not interactive menus. They perform one operation, print a
short status line on success, and exit non-zero on failure.

---

# Invocation

```text
foldingosctl <group> <command> [arguments]
```

If the command is not recognized, the program prints:

```text
usage: foldingosctl <boot|config|fah|foldops|identity|provision|registry|storage> <command> [arguments]
```

There is no built-in `--help` flag. This document is the authoritative
reference.

---

# Installation Roles

Many commands are gated by the fixed installation role persisted at
`/data/config/installation-role`. Roles are provisioned at image flash or
network-install time per [ADR-0014](adr/0014-fixed-installation-roles.md).

| Role | Typical use |
| --- | --- |
| `supervisor` | First fleet node; hosts provisioning API, registry, PXE boot assistance |
| `agent` | Compute node; registers with supervisor and runs Folding@home |

Role-specific commands fail closed when the active role does not match.

---

# Command Summary

| Group | Command | Role | Typical invocation |
| --- | --- | --- | --- |
| `boot` | `status` | any | `systemd` on boot |
| `config` | `validate` | any | operator / automation |
| `config` | `effective` | any | operator |
| `config` | `activate` | any | operator |
| `identity` | `ensure` | any | `systemd` on boot |
| `storage` | `expand-data` | any | `systemd` on first boot |
| `provision` | `ssh` | any | `systemd` on first boot |
| `provision` | `role` | any | `systemd` on first boot |
| `provision` | `serve` | supervisor | `systemd` (long-running) |
| `provision` | `boot` | supervisor | `systemd` (long-running) |
| `provision` | `allow-boot` | supervisor | operator |
| `provision` | `list-enrollments` | supervisor | operator |
| `provision` | `assign` | supervisor | operator |
| `provision` | `install` | install initramfs | network-install boot path |
| `provision` | `enroll` | agent | `systemd` on first boot |
| `provision` | `check-version` | agent | `systemd` on boot |
| `provision` | `apply-update` | agent / update initramfs | `systemd` on boot |
| `registry` | `import-bootstrap` | supervisor | `systemd` on first boot |
| `registry` | `poll` | supervisor | timer |
| `registry` | `list` | supervisor | operator |
| `registry` | `show` | supervisor | operator |
| `fah` | `validate-manifest` | any | acceptance / diagnostics |
| `fah` | `acquire` | any | timer |
| `fah` | `verify-install` | any | `fah acquire` |
| `fah` | `activate` | any | `fah acquire` |
| `fah` | `prepare` | any | `systemd` before FAH service |
| `fah` | `run` | any | `systemd` (long-running) |
| `foldops` | `validate-manifest` | any | acceptance / diagnostics |
| `foldops` | `acquire` | agent / supervisor | `systemd` after role + network |
| `foldops` | `provision` | agent / supervisor | `systemd` after acquire |
| `foldops` | `serve-https` | supervisor | `systemd` after provision |

---

# boot

## `boot status`

Writes the local commissioning display to `tty1` and `/dev/console`.

On success the display shows the OS pretty name, a routable IPv4 address, and
the SSH login form `foldingos-admin@<address>`. If networking is not ready
within the retry window, the display shows a network failure message instead.

Invoked by `foldingos-boot-status.service` after network bring-up.

---

# config

Configuration domains are `system`, `network`, and `foldinghome`. Active
configuration lives under `/data/config/`. Image defaults live under
`/etc/foldingos/defaults/`. Effective merged configuration is written to
`/run/foldingos/effective/`.

## `config validate <domain|--all>`

Validates one domain or all domains. Checks schema, field constraints, and
cross-field rules (for example DHCP-required networking in v0.1.0).

```bash
foldingosctl config validate system
foldingosctl config validate --all
```

## `config effective <domain>`

Prints the merged effective TOML for a domain to stdout.

```bash
foldingosctl config effective network
```

## `config activate <domain> <candidate-file>`

Atomically activates a candidate configuration file that already resides on
`/data`. The candidate must validate before replace. On failure the previous
active configuration is retained.

```bash
foldingosctl config activate system /data/config/candidates/system.toml
```

After activation, domain-specific apply logic runs (for example
`systemd-networkd` restart for the `network` domain).

---

# identity

## `identity ensure`

Ensures persistent node identity exists:

- `/data/config/node-id` (UUID)
- hostname from effective `system` configuration

Creates missing identity on first boot. Invoked by `foldingos-identity.service`.

---

# storage

## `storage expand-data`

Expands GPT partition 3 (`FOLDINGOS_DATA`) to consume remaining aligned disk
capacity, then runs `resize2fs` on the data filesystem.

 Preconditions:

- root and EFI layouts match the approved three-partition release image
- the data filesystem is not mounted during expansion
- the disk is at least as large as the release image requires

Invoked by `foldingos-data-expand.service` on first boot when the physical
disk is larger than the image.

---

# provision

Provisioning commands implement Milestone 3 supervisor-led fleet expansion.
See [installer.md](installer.md) and
[ADR-0016](adr/0016-network-provisioning-via-supervisor.md).

## `provision ssh`

Imports administrator SSH public keys staged on the EFI System Partition at:

```text
/boot/efi/foldingos/provision/authorized_keys
```

into persistent `/data/config/ssh/authorized_keys`, ensures the host key exists,
and removes the staged provisioning file. If no staged keys exist and persistent
keys are already valid, the command succeeds without changes.

Invoked by `foldingos-ssh-provision.service`.

## `provision role`

Activates or validates the installation role staged at:

```text
/boot/efi/foldingos/provision/installation-role
```

On first boot copies `supervisor` or `agent` into
`/data/config/installation-role`. Conflicts between staged and persisted roles
fail closed.

Invoked by `foldingos-installation-role.service`.

## `provision serve` (supervisor)

Starts the supervisor HTTP provisioning API. Default listen address is
`0.0.0.0:8743` unless overridden by `/data/config/provision/listen.url`.

Endpoints include:

| Path | Purpose |
| --- | --- |
| `POST /v1/agents/register` | Agent enrollment |
| `GET /v1/agents/desired-version` | Assigned image version lookup |
| `POST /v1/agents/update/authorize` | Agent update stream authorization |
| `POST /v1/agents/update/status` | Agent update status reporting |
| `POST /v1/rollouts/assign` | Desired-version assignment |
| `POST /v1/provision/authorize` | Install-session authorization |
| `GET /v1/provision/images/{version}/stream` | Release image streaming |
| `GET /boot/ipxe/bootstrap.ipxe` | iPXE bootstrap script |
| `GET /boot/ipxe/script.ipxe` | iPXE install script |
| `GET /boot/vmlinuz` | Install kernel |
| `GET /boot/install-initramfs.cpio.gz` | Install initramfs |

Ensures an enrollment token exists at
`/data/config/provision/enrollment-token` before serving.

Long-running. Invoked by `foldingos-provision.service`.

## `provision boot` (supervisor)

Starts proxy-DHCP, TFTP, and dnsmasq network-boot assistance for blank agent
machines. Stages `ipxe.efi` and `autoexec.ipxe` under `/data/provision/boot/tftp`
and writes `/data/config/provision/dnsmasq.conf`.

Selects the wired interface automatically from `networkctl`, or uses
`/data/config/provision/boot.interface` when pinned.

Long-running. Invoked by `foldingos-provision-boot.service`.

## `provision allow-boot <mac>` (supervisor)

Adds a client MAC address to the network-boot allowlist at
`/data/config/provision/boot-allowlist`.

Accepts colon- or hyphen-separated MAC forms. Normalizes to lowercase
`aa:bb:cc:dd:ee:ff`. Idempotent when the MAC is already listed.

Required before a blank machine can fetch the install iPXE script unless a
valid enrollment token is supplied in the iPXE URL.

```bash
sudo foldingosctl provision allow-boot 00:be:43:e7:59:5e
```

## `provision list-enrollments` (supervisor)

Lists enrolled agents from `/data/provision/enrollments/` with hostname and
current/desired image versions.

## `provision assign` (supervisor)

Assigns a desired release image version to enrolled agents.

```bash
# entire fleet
foldingosctl provision assign --version 0.1.0 --all

# one node
foldingosctl provision assign --version 0.1.0 --node <node-uuid>
```

## `provision install` (install initramfs)

Streams a supervisor-authorized release image onto a target disk over HTTP.
Used by the network-install initramfs, not normal appliance operation.

After the image is written, network install resets inherited persistent state
from the copied release image, then stages agent-only provisioning files:

- remove inherited runtime trees under `/data/config/`, `/data/registry/`,
  `/data/provision/`, and `/data/state/` on the target data partition
- clear `next_entry` from EFI `grubenv` when present
- `/data/config/installation-role`
- `/data/config/provision/supervisor.url`
- `/data/config/provision/enrollment-token`
- EFI provisioning files under `/boot/efi/foldingos/provision/`

See [Milestone 3 engineering specification](milestone/3-engineering-spec.md)
(Inherited state reset during network install).

```bash
foldingosctl provision install --auto-disk \
  --supervisor-url http://192.168.4.17:8743 \
  --enrollment-token <token>
```

| Flag | Required | Description |
| --- | --- | --- |
| `--disk <device>` | one of disk flags | Target block device (for example `/dev/nvme0n1`) |
| `--auto-disk` | one of disk flags | Select the internal install target automatically |
| `--version <ver>` | no | Requested image version; supervisor may authorize a default |
| `--supervisor-url <url>` | no | Supervisor base URL; initramfs normally passes kernel cmdline value |
| `--enrollment-token <token>` | no | Enrollment token; initramfs normally passes kernel cmdline value |

## `provision enroll` (agent)

Registers the local agent with the supervisor configured in
`/data/config/provision/supervisor.url` using the enrollment token at
`/data/config/provision/enrollment-token`.

No-op with an informational message when supervisor URL is not configured and
no enrollment token is present. Fails closed when an enrollment token is
present but the supervisor URL is missing.
Idempotent when already enrolled with the same node identity.

Invoked by `foldingos-agent-register.service`.

## `provision check-version` (agent)

Queries the supervisor for the desired image version assigned to this node.
When a newer approved version is assigned, **downloads and verifies the full
release image** (typically 4 GiB) from the supervisor into
`/data/state/provision/staged-update.img` with metadata at
`/data/state/provision/staged-update.json`. Progress lines are written while
the download runs; a silent hang usually means a large download is still in
progress.

Staged metadata includes `apply_state=staged` and the assigned version. When
`apply_state` is `boot_scheduled`, `applying`, or `failed`, `check-version` does
not overwrite existing staged update files.

**Stdout** (for scripts and `systemd`):

| Output | Meaning |
| --- | --- |
| `current` | No assigned update, or already on the assigned version |
| `<version>` | Assigned update is staged (or pending apply); not yet installed |

When stdout prints a version string such as `0.1.1-lab`, the image has been
**downloaded to `/data/state/provision/`** — it is not installed until
`provision apply-update` runs. Status and progress messages go to the console
and stdout during staging.

Supervisor connectivity failures are non-fatal and print `current` so boot
continues on the installed image.

Requires prior enrollment. Invoked by `foldingos-agent-version-check.service`.

## `provision apply-update` (agent)

Activates a verified staged update. In normal appliance boot, runs while
`staged-update.json` has `apply_state=staged` or retries while
`apply_state=boot_scheduled`: sets `apply_state=boot_scheduled`, stages update
boot assets under `/boot/efi/foldingos/update/`, sets GRUB `next_entry` to `1`
(the `foldingos-update` menu entry), and reboots once.

In update initramfs boot (`foldingos.update-apply=1`), sets
`apply_state=applying`, copies the staged image EFI and root partitions onto the
boot disk while preserving the persistent data partition, records outcome in
`/data/state/provision/pending-update-report.json`, clears staged files on
success, and reboots. The update initramfs has no network; `check-version` on
the first normal boot with network delivers the pending report to the supervisor.

On offline apply failure, sets `apply_state=failed`, records a pending `failed`
report, and reboots into the normal boot path without scheduling another update
boot automatically.

## `provision report-update-status` (agent)

Reports a missed update outcome directly to the supervisor when network is
available. Operator recovery when an offline apply succeeded but the pending
report file is missing.

```bash
foldingosctl provision report-update-status --status applied --version 0.1.1-lab
```

| Flag | Required | Description |
| --- | --- | --- |
| `--status <status>` | yes | `applied` or `failed` |
| `--version <ver>` | yes | Assigned image version to report |

No-op with an informational message when no staged update is pending or when
`apply_state` is not `staged` (normal boot) or the update initramfs boot path is
not active.

Invoked by `foldingos-agent-apply-update.service` while `apply_state` is
`staged` or `boot_scheduled`, and by the update initramfs for offline apply.

---

# registry

Supervisor-local release image registry under `/data/registry/`.

## `registry import-bootstrap` (supervisor)

Imports the running supervisor's own release image into the local registry so
agents can be provisioned from it.

Invoked by `foldingos-registry-bootstrap.service`.

## `registry poll` (supervisor)

Polls the upstream releases manifest configured at
`/data/config/provision/upstream-releases.url` and stages newly approved
images into the registry.

Supervisor appliances use the official stable manifest by default:

```text
https://releases.folding-os.com/release/releases.json
```

Manifest schema, image URLs, and trust model are defined in
[ADR-0017](adr/0017-official-release-publication-and-supervisor-upstream-polling.md).

Invoked by `foldingos-registry-poll.timer`.

## `registry list` (supervisor)

Lists known registry versions.

## `registry show <version>` (supervisor)

Shows the registry entry JSON for one version.

---

# fah

Folding@home client lifecycle commands. The embedded approved manifest ships in
the image.

## `fah validate-manifest`

Validates the embedded Folding@home acquisition manifest.

## `fah acquire`

Downloads and stages an approved Folding@home client version when the manifest
requires acquisition.

## `fah verify-install <version>`

Verifies an extracted client version under `/data/apps/fah/<version>/`.

## `fah activate <version>`

Atomically activates a verified version by updating the `/data/apps/fah/current`
symlink.

## `fah prepare`

Renders runtime configuration and secrets for the active client into
`/run/foldingos/fah/`.

Invoked by `foldingos-fah-prepare.service`.

## `fah run`

Execs the active Folding@home client with validated runtime configuration.
Drops privileges to the `fah` user.

Long-running. Invoked by `folding-at-home.service`.

---

# FoldOps Commands

FoldOps package lifecycle commands. The embedded approved manifest ships in the
image. See [ADR-0018](adr/0018-foldops-package-acquisition-and-update-model.md).

## `foldops validate-manifest`

Validates the embedded FoldOps acquisition manifest at
`/usr/share/foldingos/manifests/foldops.toml`.

## `foldops acquire`

Downloads pinned `.deb` artifacts from `deb.folding-os.com`, verifies size and
SHA-256, extracts into `/data/apps/foldops/<manifest_release>/`, and activates
`/data/apps/foldops/current`. Required packages depend on installation role:

| Role | Packages |
| --- | --- |
| `agent` | `foldops-agent` |
| `supervisor` | `foldops-agent`, `foldops-supervisor`, `foldops-web` |

Invoked by `foldingos-foldops-acquire.service` after role validation and network
availability. FoldOps systemd units must not start until acquisition succeeds.

## `foldops provision`

Imports the fleet ingest token from EFI, configures FoldOps environment files,
and completes TLS bootstrap on supervisor role. See
[ADR-0019](adr/0019-foldops-supervisor-provisioning-and-tls.md).

**EFI input** (before first boot or written by supervisor during network install):

```text
/boot/efi/foldingos/provision/foldops-ingest-token
```

Single line: 64 hex characters (`openssl rand -hex 32`).

**Supervisor role:**

1. import token → `/data/config/foldops/ingest-token`
2. generate self-signed TLS under `/data/foldops/tls/`
3. write `/data/config/foldops/supervisor.env` (`HOST=127.0.0.1`, `PORT=3000`)
4. write local `/data/config/foldops/agent.env`
5. mark `/data/state/foldops/provisioned.json`
6. remove EFI staging file

**Agent role:**

1. import token from EFI
2. derive `SUPERVISOR_URL` from `/data/config/provision/supervisor.url`:
   parse host, set `https://<host>:3443`
3. write `/data/config/foldops/agent.env` with `AGENT_TOKEN` and
   `SUPERVISOR_TLS_CA=/data/config/foldops/supervisor-ca.pem`
4. mark provisioned (CA is already on disk from network install, or copied on
   supervisor direct-flash paths)

Until provision succeeds, the HTTPS dashboard must not listen on `0.0.0.0`.
Invoked by `foldingos-foldops-provision.service` after `foldops acquire`.

## `foldops serve-https`

Supervisor-only TLS terminator. Listens on `0.0.0.0:3443`, terminates HTTPS
using `/data/foldops/tls/cert.pem` and `key.pem`, and reverse-proxies to
`http://127.0.0.1:3000` where `foldops-supervisor` runs.

Long-running. Invoked by `foldingos-foldops-serve-https.service` only after
`/data/state/foldops/provisioned.json` exists.

---

# Key Paths

| Path | Purpose |
| --- | --- |
| `/data/config/installation-role` | Active `supervisor` or `agent` role |
| `/data/config/node-id` | Persistent node UUID |
| `/data/config/provision/enrollment-token` | Fleet enrollment token |
| `/data/config/provision/supervisor.url` | Agent supervisor base URL |
| `/data/config/provision/listen.url` | Supervisor API listen URL |
| `/data/config/provision/boot-allowlist` | PXE/iPXE client MAC allowlist |
| `/data/config/provision/boot.interface` | Optional pinned NIC for PXE service |
| `/data/config/provision/dnsmasq.conf` | Generated proxy-DHCP/TFTP config |
| `/data/provision/boot/tftp/` | Staged `ipxe.efi` and `autoexec.ipxe` |
| `/data/state/provision/staged-update.img` | Verified agent update image staging file |
| `/data/state/provision/staged-update.json` | Staged update metadata and verification state |
| `/data/provision/enrollments/` | Agent enrollment records |
| `/data/registry/` | Supervisor release image registry |
| `/usr/share/foldingos/manifests/foldops.toml` | Embedded FoldOps acquisition manifest |
| `/usr/share/keyrings/foldops.gpg` | Official FoldOps apt archive keyring |
| `/data/apps/foldops/current` | Active verified FoldOps installation tree |
| `/data/state/foldops/` | FoldOps download staging and acquire retry state |
| `/data/config/foldops.toml` | FoldOps runtime configuration |
| `/data/foldops/` | FoldOps persistent runtime state |
| `/data/config/foldops/ingest-token` | Fleet ingest secret (supervisor) |
| `/data/config/foldops/supervisor.env` | Supervisor FoldOps environment |
| `/data/config/foldops/agent.env` | Agent FoldOps environment |
| `/data/config/foldops/supervisor-ca.pem` | Agent trust anchor for supervisor HTTPS |
| `/data/foldops/tls/` | Supervisor self-signed TLS material |
| `/data/state/foldops/provisioned.json` | FoldOps provision completion marker |
| `/boot/efi/foldingos/provision/foldops-ingest-token` | Staged fleet ingest token (EFI) |

---

# Related Documentation

- [Operations](operations.md) — build, deploy, diagnose, recover
- [Deployment and provisioning](installer.md) — supervisor bootstrap and PXE workflow
- [Boot process](boot-process.md) — boot sequence and systemd graph
- [Milestone 3 engineering specification](milestone/3-engineering-spec.md) — fleet provisioning contract
