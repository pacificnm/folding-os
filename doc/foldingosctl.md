# foldingosctl Command Reference

**Version:** 1.0

**Status:** Current for v0.1.0 appliance images

---

# Purpose

`foldingosctl` is the primary on-appliance control program for FoldingOS. It
implements first-boot provisioning, configuration management, storage expansion,
Folding@home client lifecycle, and Milestone 3 supervisor/agent fleet
provisioning.

The binary is installed at `/usr/bin/foldingosctl` with mode **`4755`
(setuid root)** and is invoked directly by operators, FoldOps services, or
`systemd` units on boot and timer schedules.

On appliance images the binary drops to the real invoking UID on startup and
re-elevates to root only for policy-approved privileged commands. See
[Privilege model](#privilege-model) and
[ADR-0024](adr/0024-foldops-supervisor-fleet-mutation-authorization.md).

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

# Privilege model

Appliance images install `/usr/bin/foldingosctl` as **`root:root` mode
`4755`**.

| Invoker | Behavior |
| --- | --- |
| `systemd` (root) | runs with real UID 0; no drop |
| `foldingos-admin` | drops to operator UID; may re-elevate for privileged commands |
| `foldops` (FoldOps services) | drops to `foldops`; re-elevates only for commands listed in the automation policy file |

Privileged commands include `foldops acquire`, `tools acquire`, `recovery
export`, `recovery import`, and `config activate` (agent policy). Read-only
commands and fleet assignment to group-writable paths run without
re-elevation.

FoldOps HTTP services **must not** invoke `sudo` or perform privileged OS
operations directly. They subprocess `foldingosctl` only; elevation is enforced
inside the binary per [ADR-0024](adr/0024-foldops-supervisor-fleet-mutation-authorization.md).

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
| `provision` | `allow-boot` | supervisor | operator / foldops automation |
| `provision` | `list-allow-boot` | supervisor | operator / automation |
| `provision` | `list-enrollments` | supervisor | operator / automation |
| `provision` | `assign` | supervisor | operator / foldops automation |
| `provision` | `install` | install initramfs | network-install boot path |
| `provision` | `enroll` | agent | `systemd` on first boot |
| `provision` | `check-version` | agent | `systemd` on boot |
| `provision` | `apply-update` | agent / update initramfs | `systemd` on boot |
| `provision` | `report-update-status` | agent | operator recovery |
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
| `foldops` | `acquire` | agent / supervisor | timer |
| `foldops` | `provision` | agent / supervisor | `systemd` on boot |
| `foldops` | `serve-https` | supervisor | `systemd` after provision |

---

# boot

## `boot status`

Writes the local commissioning display to `tty1` and `/dev/console`.

On success the display shows a boxed summary, the ADR-0015 ready lines
(`<pretty-name> ready`, routable IPv4 address, and SSH login form), role and
version metadata, and a service checklist with `✓` / `✗` markers for:

- SSH provisioning, installation role, FoldOps acquire/provision, and runtime
  units (role-specific)
- Folding@home client status

The command waits up to three minutes for optional services to become ready
after network bring-up, then writes the final display. FoldOps provisioning
refreshes the display when runtime services start.

The same checklist is printed to stdout for journal inspection:

```text
Commissioning service status:
  Network online: ready
  FoldOps HTTPS (port 3443): ready
```

If networking is not ready within the retry window, the display shows a network
failure message instead.

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

## `provision allow-boot [--disk <device>] <mac>` (supervisor)

Adds a client MAC address to the network-boot allowlist at
`/data/config/provision/boot-allowlist`.

Accepts colon- or hyphen-separated MAC forms. Normalizes to lowercase
`aa:bb:cc:dd:ee:ff`. Idempotent when the MAC is already listed.

Optional `--disk` pins the network install target for that MAC. The value is
stored in `/data/config/provision/boot-install-disk-allowlist` and passed to
the install initramfs as `foldingos.install-disk=` in the iPXE kernel command
line. Use this when a machine has multiple internal disks and automatic target
selection would pick the wrong one.

Required before a blank machine can fetch the install iPXE script unless a
valid enrollment token is supplied in the iPXE URL.

```bash
sudo foldingosctl provision allow-boot 00:be:43:e7:59:5e
sudo foldingosctl provision allow-boot --disk /dev/sda 00:be:43:e7:59:5e
foldingosctl provision allow-boot --format json 00:be:43:e7:59:5e
```

FoldOps supervisor exposes the same operations at `GET/POST /api/fleet/allow-boot`
when running on the supervisor role. The `foldops` service user may invoke this
command only when authorized by
`/usr/share/foldingos/foldops-supervisor-automation.toml`; see
[ADR-0024](adr/0024-foldops-supervisor-fleet-mutation-authorization.md).
without requiring CLI access from operators.

## `provision list-allow-boot` (supervisor)

Lists MAC addresses allowed for PXE/iPXE network install from
`/data/config/provision/boot-allowlist`, including optional install-disk pins
from `/data/config/provision/boot-install-disk-allowlist`.

```bash
foldingosctl provision list-allow-boot
foldingosctl provision list-allow-boot --format json
```

Human output is one MAC per line; when a disk is pinned, the line is
`mac<TAB>disk=/dev/sdX`.

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

FoldOps supervisor exposes assignment at `POST /api/fleet/assign`. The `foldops`
service user may invoke this command only when authorized by
`/usr/share/foldingos/foldops-supervisor-automation.toml`; see
[ADR-0024](adr/0024-foldops-supervisor-fleet-mutation-authorization.md).

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
- `/data/config/foldops/supervisor-ca.pem` (copied from the supervisor TLS CA)
- EFI provisioning files under `/boot/efi/foldingos/provision/`, including
  `foldops-ingest-token` (copied from the supervisor imported ingest token)

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

## `inspect fah --format json`

Reports machine-readable Folding@home acquisition and runtime status for
FoldOps and operators. The response includes:

- `installed` and `active_client_version` for `/data/apps/fah/current`
- `expected_client_version` from the embedded manifest
- `verified`, based on the `.foldingos-verified` marker and manifest digest
- `acquisition.consecutive_failures`, `next_attempt_unix`, and
  `last_failure_reason` from `/data/state/fah-acquire.state`
- `service_active`, runtime project/progress fields, and recent log errors
- `log_path` and `log_readable`

This inspection path is read-only. It does not download or install the client;
automatic acquisition remains owned by `foldingos-fah-acquire.service` and
`foldingos-fah-acquire.timer`.

---

# FoldOps Commands

FoldOps package lifecycle and runtime bootstrap commands. The embedded bootstrap
manifest ships in the image; supervisor-assigned manifests override the bootstrap
floor when present per
[ADR-0023](adr/0023-runtime-foldops-and-foldingosctl-updates-without-os-reimage.md).
See
[ADR-0018](adr/0018-foldops-package-acquisition-and-update-model.md),
[ADR-0019](adr/0019-foldops-supervisor-provisioning-and-tls.md), and
[foldops-integration.md](foldops-integration.md).

Supervisor bootstrap order:

```text
role → foldops acquire → foldops provision → serve-https + foldops-supervisor
  → provisioning control plane → registry import
```

Agents run `foldops acquire` → `foldops provision` before `foldops-agent`.

## `foldops validate-manifest`

Validates the embedded bootstrap FoldOps acquisition manifest at
`/usr/share/foldingos/manifests/foldops.toml`. When
`/data/config/foldops/assigned-manifest.toml` exists, assigned pins are validated
separately during acquire.

## `foldops acquire`

Downloads pinned verified artifacts, verifies size and SHA-256, extracts payload
only (no `dpkg` install or maintainer scripts) into
`/data/apps/foldops/<manifest_release>/<package>/`, writes a
`.foldingos-verified` marker per package, and activates
`/data/apps/foldops/current`.

**Milestone 3 (shipped):** schema v1, `artifact_format = deb`, download from
`deb.folding-os.com`.

**Milestone 4 (target):** schema v2, `artifact_format = layout-tar-zst`, download
from `packages.folding-os.com/foldops/`, with assigned manifest precedence over
the embedded bootstrap manifest.

Required packages depend on installation role:

| Role | Packages |
| --- | --- |
| `agent` | `foldops-agent` |
| `supervisor` | `foldops-agent`, `foldops-supervisor`, `foldops-web` |

Idempotent when the manifest release is already verified and active for the
current role. Failed attempts are deferred with backoff; state is persisted
under `/data/state/foldops/`.

Prerequisites: active installation role, data partition mounted, network
online, and NTP synchronized.

Invoked by `foldingos-foldops-acquire.service`, scheduled by
`foldingos-foldops-acquire.timer` (first attempt 1 minute after boot, then
every minute while acquisition is incomplete).

## `tools acquire` (Milestone 4)

Downloads the supervisor-assigned `foldingosctl` binary from
`packages.folding-os.com/foldingos-tools/`, verifies SHA-256, atomically
replaces `/usr/bin/foldingosctl`, and records state under `/data/state/tools/`.
Does not require OS image reflash. See
[ADR-0023](adr/0023-runtime-foldops-and-foldingosctl-updates-without-os-reimage.md).

Assignment file: `/data/config/tools/assigned-version.json`

## `foldops provision`

Imports the fleet ingest token, renders FoldOps environment files, generates TLS
on supervisor role, and writes `/data/state/foldops/provisioned.json`. See
[ADR-0019](adr/0019-foldops-supervisor-provisioning-and-tls.md).

**Preconditions:** verified FoldOps packages are active at
`/data/apps/foldops/current` for the installation role.

**Idempotency:** when `/data/state/foldops/provisioned.json` already exists and
validates, the command ensures supervisor fleet-automation permissions per
[ADR-0024](adr/0024-foldops-supervisor-fleet-mutation-authorization.md), prints
an informational message, and exits successfully without re-importing secrets.

**Token import order:**

1. use `/data/config/foldops/ingest-token` when already present
2. otherwise import from EFI, persist to `/data/config/foldops/ingest-token`
   (`0600`), then remove the EFI staging file

**EFI input** (supervisor direct flash or written by supervisor during network
install):

```text
/boot/efi/foldingos/provision/foldops-ingest-token
```

Single line: 64 lowercase hex characters (`openssl rand -hex 32`). On
supervisor USB preparation, `scripts/make-bootable-usb --foldops-ingest-token
FILE` writes this path.

**Supervisor role:**

1. import and persist ingest token
2. generate self-signed ECDSA TLS under `/data/foldops/tls/` when missing
   (`cert.pem`, `key.pem`, `ca.pem`; 365-day lifetime; hostname + `127.0.0.1`
   SAN)
3. copy TLS CA to `/data/config/foldops/supervisor-ca.pem`
4. write `/data/config/foldops/supervisor.env`:

   ```env
   HOST=127.0.0.1
   PORT=3000
   INGEST_TOKEN=<imported-secret>
   DB_PATH=/data/foldops/foldops.db
   WEB_ROOT=/data/apps/foldops/current/foldops-web/usr/share/foldops/web
   ```

5. write `/data/config/foldops/agent.env` for the co-located agent (hostname
   from effective `system` configuration, same token and CA paths as remote
   agents)
6. ensure supervisor fleet-automation state permissions for the `foldops` user
   (enrollment store, boot allowlists, supervisor self-assignment files) per
   [ADR-0024](adr/0024-foldops-supervisor-fleet-mutation-authorization.md)
7. write `/data/state/foldops/provisioned.json`
8. remove EFI staging file when import came from EFI

**Agent role:**

1. import ingest token (EFI or persistent)
2. require `/data/config/foldops/supervisor-ca.pem` (staged on the data
   partition during network install)
3. derive `SUPERVISOR_URL` from `/data/config/provision/supervisor.url`: parse
   host, ignore scheme and port, set `https://<host>:3443`
4. write `/data/config/foldops/agent.env`:

   ```env
   SUPERVISOR_URL=https://<supervisor-host>:3443
   SUPERVISOR_TLS_CA=/data/config/foldops/supervisor-ca.pem
   AGENT_TOKEN=<imported-secret>
   FAH_LOG_PATH=/data/fah/log.txt
   FAH_DB_PATH=/data/fah/client.db
   FAH_WORK_DIR=/data/fah/work
   ```

5. write `/data/state/foldops/provisioned.json`
6. remove EFI staging file when import came from EFI

Until provision succeeds, FoldOps HTTPS must not listen on `0.0.0.0`, and
`foldops-supervisor` / `foldops-agent` must not start with incomplete env.
Invoked by `foldingos-foldops-provision.service` after acquisition completes
(`ConditionPathExists=/data/apps/foldops/current`).

## `foldops serve-https`

Supervisor-only TLS terminator. Listens on `0.0.0.0:3443`, terminates HTTPS
using `/data/foldops/tls/cert.pem` and `key.pem`, and reverse-proxies to
`http://127.0.0.1:3000` where `foldops-supervisor` runs.

Fails closed when the active role is not `supervisor`, when
`/data/state/foldops/provisioned.json` is missing, or when TLS material is
incomplete.

Long-running. Invoked by `foldingos-foldops-serve-https.service` (supervisor
role only; requires provisioned marker).

## FoldOps runtime services

| Unit | Role | Purpose |
| --- | --- | --- |
| `foldingos-foldops-acquire.timer` | any | Schedule package acquisition |
| `foldingos-foldops-provision.service` | any | One-shot env/TLS bootstrap |
| `foldingos-foldops-serve-https.service` | supervisor | TLS front end on `:3443` |
| `foldingos-foldops-supervisor.service` | supervisor | Loopback `foldops-supervisor` on `:3000` |
| `foldingos-foldops-agent.service` | any | `foldops-agent` after provision |

Supervisor-only units use `ExecCondition` so they no-op on agent appliances.
`foldingos-foldops-agent.service` starts after `folding-at-home.service`.
FoldOps service failure must not block boot or Folding@home
([ADR-0014](adr/0014-fixed-installation-roles.md)).

Agent HTTPS trust for self-signed supervisor TLS uses `SUPERVISOR_TLS_CA` in the
Rust FoldOps agent (`packages/foldops/`). The HTTPS terminator and CA staging
are owned by FoldingOS; the supervisor process remains HTTP on loopback.

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
| `/usr/share/foldingos/manifests/foldops.toml` | Embedded bootstrap FoldOps acquisition manifest |
| `/data/config/foldops/assigned-manifest.toml` | Supervisor-assigned FoldOps manifest (Milestone 4) |
| `/data/config/tools/assigned-version.json` | Supervisor-assigned `foldingosctl` version (Milestone 4) |
| `/data/state/tools/` | Active and last `tools acquire` state (Milestone 4) |
| `/usr/share/keyrings/foldops.gpg` | Official FoldOps apt archive keyring |
| `/data/apps/foldops/current` | Active verified FoldOps installation tree |
| `/data/state/foldops/` | FoldOps acquire retry state and provision marker |
| `/data/state/foldops/acquire.state` | FoldOps acquisition backoff state |
| `/data/state/foldops/provisioned.json` | FoldOps provision completion marker |
| `/data/config/foldops.toml` | Reserved FoldOps TOML domain (Milestone 3 uses env files) |
| `/data/foldops/` | FoldOps persistent runtime state (database, TLS, working files) |
| `/data/foldops/foldops.db` | FoldOps supervisor database |
| `/data/config/foldops/ingest-token` | Fleet ingest secret (supervisor) |
| `/data/config/foldops/supervisor.env` | Supervisor FoldOps environment |
| `/data/config/foldops/agent.env` | Agent FoldOps environment |
| `/data/config/foldops/supervisor-ca.pem` | Agent trust anchor for supervisor HTTPS |
| `/data/foldops/tls/` | Supervisor self-signed TLS material |
| `/boot/efi/foldingos/provision/foldops-ingest-token` | Staged fleet ingest token (EFI) |

---

# Related Documentation

- [foldingosctl component reference](foldingosctl-components.md) — module map and FoldOps delegation (developers/agents)
- [Operations](operations.md) — build, deploy, diagnose, recover
- [Deployment and provisioning](installer.md) — supervisor bootstrap and PXE workflow
- [FoldOps integration](foldops-integration.md) — fleet monitoring bootstrap
- [Boot process](boot-process.md) — boot sequence and systemd graph
- [Milestone 3 engineering specification](milestone/3-engineering-spec.md) — fleet provisioning contract
