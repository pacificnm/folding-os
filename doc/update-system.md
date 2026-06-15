# FoldingOS Update System

**Version:** 0.2

**Status:** Approved Architecture (supervisor-mediated updates)

---

# Purpose

This document defines the scope, requirements, trust model, and update workflow
for FoldingOS operating-system image updates in supervisor-managed fleets.

FoldingOS updates must prioritize reliability, integrity, rollback safety, and
preservation of Folding@home work data.

Network fleet provisioning is defined by
[ADR-0016](adr/0016-network-provisioning-via-supervisor.md) and
[milestone/3-engineering-spec.md](milestone/3-engineering-spec.md).

---

# Scope

The update system covers:

- operating-system image updates for `agent` and `supervisor` nodes
- supervisor-local image registry and upstream release polling
- desired-version assignment and agent boot-time version checks
- update verification and staged activation
- preservation of approved persistent data
- update status reporting to the supervisor and FoldOps when available

The update system does not initially cover:

- general-purpose package management
- user-installed software
- desktop software updates
- arbitrary third-party packages

The Folding@home client remains a separately managed workload. Nodes download
approved, pinned client artifacts directly from official Folding@home
infrastructure. See
[ADR-0009](adr/0009-fah-acquisition-and-update-model.md).

---

# Update Philosophy

FoldingOS uses image-based updates, not runtime package management.

The operating-system root filesystem is replaceable.

Persistent data is not.

Persistent data includes:

- node identity
- installation role
- FoldOps registration state
- Folding@home configuration
- Folding@home work data and checkpoints
- bounded journal history under `/data/logs/journal`
- operator configuration under `/data/config`

---

# Trust Model

Nodes must install only images that are:

- produced by the official FoldingOS build process
- versioned and checksummed
- cryptographically signed when signing is enabled for the release channel
- present in the supervisor registry or fetched from an approved upstream origin
  named by the supervisor

Nodes must fail closed on checksum or signature mismatch.

The supervisor must not publish unverified images to agents.

---

# Supervisor-Mediated Workflow

## Upstream polling

The supervisor periodically polls the official FoldingOS releases manifest on
Cloudflare HTTPS infrastructure. See
[ADR-0017](adr/0017-official-release-publication-and-supervisor-upstream-polling.md).

| Item | URL |
| --- | --- |
| Manifest | `https://releases.folding-os.com/release/releases.json` |
| Disk images | `https://releases.folding-os.com/release/images/foldingos-x86_64-<version>.img` |
| Supervisor config | `/data/config/provision/upstream-releases.url` |

When a new approved image is available the supervisor:

1. downloads image and metadata from the manifest entry
2. verifies SHA-256 digest and declared size (Milestone 3)
3. stores the image in the local registry
4. marks it ready for operator-approved rollout

Detached image signatures are planned; Milestone 3 relies on HTTPS origin trust
and SHA-256 verification during import.

## Desired version assignment

The supervisor assigns a desired image version to each enrolled agent. Rollout
may be:

- fleet-wide
- per-node
- staged in batches

## Agent boot-time check

On boot each agent:

1. queries the supervisor for its desired image version
2. compares the desired version to the running image
3. downloads and verifies a newer image when assigned
4. stages the update
5. applies it on reboot

If the supervisor is unreachable, the agent continues running the current image.
Folding@home operation must not depend on supervisor availability after initial
installation.

---

# Milestone 3 Update Mechanism

Milestone 3 uses verified full-image replacement.

Required behavior:

- stage the new image before activation
- verify the staged image against registry metadata
- activate only after successful verification
- retain the previous bootable image until the new image is verified
- fail closed and keep the previous image on any error
- report update outcome to the supervisor

Staged update metadata at `/data/state/provision/staged-update.json` records an
`apply_state` lifecycle: `staged` → `boot_scheduled` → `applying` → success
(clear staged files) or `failed`. Normal-boot apply scheduling runs only while
`apply_state` is `staged`. After a failed offline apply, the agent must not
automatically schedule another update boot on subsequent normal boots.

The update initramfs has no network. Offline apply records outcome in
`/data/state/provision/pending-update-report.json`; the first normal-boot
`check-version` with network delivers that report to the supervisor.

See [Milestone 3 engineering specification](milestone/3-engineering-spec.md)
(Update Workflow) and [foldingosctl.md](foldingosctl.md) (`provision
apply-update`).

A/B root slots, automatic rollback, and signed update bundles remain planned
enhancements beyond Milestone 3.

---

# Supervisor Updates

The supervisor updates itself from the same registry and upstream polling model.

Supervisor self-update must not delete the only verified image while agents still
depend on that supervisor for provisioning or update coordination.

---

# Failure And Recovery

If update staging or activation fails:

- the currently bootable image remains active
- persistent configuration and Folding@home work remain intact where the update
  specification preserves them
- the failure is logged and reported
- `apply_state` becomes `failed` after offline apply failure
- the agent does not automatically retry offline apply on later normal boots
- retry requires a new supervisor assignment or operator recovery action

If update scheduling fails before the one-shot update boot (for example GRUB
`next_entry` could not be set), the agent remains on the current image with
`apply_state` still `staged` or `failed` as recorded in staged metadata.

If network provisioning fails before first boot:

- the target disk may be unbootable
- repeat network provisioning after correcting enrollment, networking, or image
  availability

Direct flash remains the recovery path for a single node.

---

# Related Documents

- [ADR-0016: Network Provisioning Via Supervisor](adr/0016-network-provisioning-via-supervisor.md)
- [Deployment and provisioning](installer.md)
- [Milestone 3 engineering specification](milestone/3-engineering-spec.md)
- [FoldOps integration](foldops-integration.md)
- [Release strategy](release-strategy.md)
