# Host Hardware Profile Rollout

**Status:** Approved operator reference  
**Issue:** [#128](https://github.com/pacificnm/folding-os/issues/128)

This document describes what the host hardware profile feature delivers today,
which update channel each part uses, and how to roll out the remaining kernel
dependency on live agents **without USB reflash or network reprovision**.

---

# Feature Summary

Issue #128 adds:

- `foldingosctl inspect hardware` — read-only host inventory collection
- Agent ingest field `hardware` on delegated FoldingOS snapshots
- Supervisor persistence in `machines.hardware_profile`
- Admin machine detail **Hardware** tab

Routine delivery uses the **packages channel** (FoldOps bundles + `foldingosctl`
tools). See [ADR-0023](adr/0023-runtime-foldops-and-foldingosctl-updates-without-os-reimage.md)
and [wiki/Software-Updates.md](../wiki/Software-Updates.md).

---

# What Works After Packages Rollout Only

After FoldOps and tools assignment/apply on running nodes, the profile includes:

| Data | Source | Packages channel sufficient? |
| --- | --- | --- |
| CPU model, cores, threads | `/proc/cpuinfo` | Yes |
| Memory total | `/proc/meminfo` | Yes |
| Storage devices | `/sys/block/*` | Yes |
| Network adapters | `/sys/class/net/*` | Yes |
| PCI device IDs | `/sys/bus/pci/devices/*` | Yes |
| Board, BIOS, chassis, system product | `/sys/class/dmi/id/*` | **No** — needs kernel DMI sysfs |
| Memory module detail | `/sys/firmware/dmi/entries/17-*` | **No** — needs kernel DMI sysfs |

The admin Hardware tab shows whatever the agent persisted. When DMI sysfs is
missing, platform fields are empty and the UI may show **Hardware profile
pending** for detail sections even though CPU and capacity data are present.

---

# Kernel Requirement (OS Image Channel)

Full platform inventory requires explicit kernel options pinned in
`configs/linux-x86_64.config`:

```text
CONFIG_DMI=y
CONFIG_DMIID=y
CONFIG_DMI_SYSFS=y
```

These ship only in a **new OS disk image**. They are not delivered through
`foldingosctl tools acquire` or FoldOps bundle apply.

Per [update-system.md](update-system.md), in-place OS updates:

- replace EFI and root partitions only
- **preserve `/data`** (node identity, FAH work, FoldOps state, enrollment)
- do **not** use USB reflash or network install on enrolled agents

---

# Update Channels (Do Not Mix Up)

| Goal | Channel | Operator path |
| --- | --- | --- |
| `inspect hardware`, ingest, admin UI | Packages | Admin → Software updates |
| Kernel DMI sysfs on agents | OS image | Supervisor registry + `provision assign` + agent reboot |
| Supervisor OS/kernel change | OS image | **Not automated today** — agent boot services are agent-role only |

Issue #128 code can ship on packages alone. **Full DMI platform fields** remain
blocked until agents receive an OS image that includes the DMI kernel options.

---

# Lab Rollout Script

Use `./scripts/roll-agent-os-update-lab` from the build host to automate the
manual steps that block most operators today:

1. Verify the local build image exists
2. Copy the image into the supervisor registry under a **new version label**
3. Assign that version to enrolled agents (`--all` or `--node`)
4. Print reboot and verification steps

```bash
# After ./scripts/build on a branch with CONFIG_DMI* pinned:
./scripts/roll-agent-os-update-lab \
  --all \
  192.168.88.251 \
  "${HOME}/.ssh/id_ed25519" \
  0.1.1-lab
```

Important:

- The **registry version label must differ** from the agents' installed
  `VERSION_ID` in `/usr/lib/os-release` (typically `0.1.0`). Use labels such as
  `0.1.1-lab` even when the build output file is still
  `foldingos-x86_64-0.1.0.img`.
- Assignment alone does not update agents. Each agent must **reboot** so
  `foldingos-agent-version-check.service` runs `provision check-version`.
- Expect a large download, then **two reboots** (stage/schedule, then offline
  apply through the update initramfs).
- For end-to-end proof on one agent including apply, use
  `./scripts/validate-agent-update-lab`.

Production fleets that consume images from `releases.folding-os.com` should
publish a new release there and let `foldingos-registry-poll.timer` import it
before assignment. The lab script is for supervisor-local registry refresh during
development.

---

# Verification

On an agent after packages rollout:

```bash
foldingosctl inspect hardware --format json
foldingosctl inspect foldops --format json
foldingosctl inspect tools --format json
```

After OS image rollout (DMI enabled):

```bash
test -r /sys/class/dmi/id/product_name && echo "DMI sysfs present"
foldingosctl inspect hardware --format json
foldingosctl inspect update --format json
```

On the supervisor:

```bash
foldingosctl provision list-enrollments
foldingosctl registry list
```

In the dashboard: **Admin → Folding@home → machine → Hardware** tab.

---

# Known Gaps (Follow-Up Work)

Tracked separately from the #128 implementation merge:

- **OS image rollout in the admin UI** — assignment exists at
  `POST /api/fleet/assign` with `version`, but Software updates covers FoldOps and
  tools only
- **Supervisor in-place OS update** — `provision check-version` /
  `apply-update` require `agent` role; no supervisor boot pipeline
- **Unified fleet OS runbook** — architecture is documented across
  `update-system.md`, `foldingosctl.md`, and the wiki; a single operator checklist
  for live-fleet OS rollout remains thin
- **Optional richer inventory** — `dmidecode`, `pciutils` + hwdata, `ethtool`
  fallbacks (not required for MVP sysfs collection)

---

# Related Documents

- [update-system.md](update-system.md)
- [operations.md](operations.md) — lab registry and assign commands
- [foldingosctl.md](foldingosctl.md) — `inspect hardware`, `provision check-version`
- [wiki/Software-Updates.md](../wiki/Software-Updates.md)
- [Milestone 6 readiness review](milestone/6-readiness-review.md)
