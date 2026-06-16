# ADR-0023: Runtime FoldOps And foldingosctl Updates Without OS Reimage

**Status:** Proposed

**Date:** 2026-06-14

**Authors:** FoldingOS project

**Amends:** [ADR-0018](0018-foldops-package-acquisition-and-update-model.md),
[ADR-0017](0017-official-release-publication-and-supervisor-upstream-polling.md)

**Depends on:** [ADR-0022](0022-foldops-rust-source-in-foldingos-monorepo.md)

---

## Context

Milestone 3 requires FoldOps package version pins to change when a new FoldingOS
release embeds an updated manifest ([ADR-0018](0018-foldops-package-acquisition-and-update-model.md)).
That forces a **full 4 GiB image rebuild and fleet reflash** for routine FoldOps
or `foldingosctl` fixes.

`foldingosctl` is built into the root filesystem at image build time. FoldOps
is acquired at runtime but activation currently requires the active release to
match the **embedded** bootstrap manifest.

FoldingOS appliances do not and will not use `apt` or `dpkg` at runtime
([ADR-0018](0018-foldops-package-acquisition-and-update-model.md)). The current
`.deb` transport is a **Debian packaging convenience**. On appliances it is only
an `ar` container for `data.tar.*` extraction. That adds complexity without
matching how the OS already acquires Folding@home artifacts.

The project requires:

1. Update **FoldOps** on running supervisors and agents without OS reimage
2. Update **`foldingosctl`** on running nodes without OS reimage
3. Keep verified HTTPS acquisition, pinned manifests, and fail-closed behavior
4. Preserve separate channels for OS images, FoldOps bundles, and tools binaries

---

## Decision

### Fleet software update policy

**FoldOps and `foldingosctl` routine updates must not require a full operating-system
image reflash.**

A full OS image update remains required only for platform changes (kernel,
initramfs, base systemd graph, partition layout, or bootstrap-floor incompatibility).

### Appliance artifact transport

FoldingOS appliances adopt **layout bundles** as the primary FoldOps transport:

| Property | Value |
| --- | --- |
| Format | `layout-tar-zst` — zstd-compressed tar of install tree |
| Install root | `/data/apps/foldops/<manifest_release>/` |
| Activation | verified marker + `current` symlink (unchanged model) |
| Publication | `https://packages.folding-os.com/foldops/<release>/` |

Example bundle contents:

```text
foldops-agent-x86_64.tar.zst
  foldops-agent/usr/bin/foldops-agent
  foldops-agent/usr/share/...
```

`foldingosctl foldops acquire` extracts bundles in-process using existing Go
tar/zstd support. No `apt`, no `dpkg`, no maintainer script execution.

The legacy `artifact_format = "deb"` remains supported temporarily for migration.
New appliance releases publish `layout-tar-zst` only.

### Supervisor-assigned versions

FoldOps and tools versions are assigned by the supervisor, analogous to desired
OS image version:

| Assigned value | Persistent path | Consumer |
| --- | --- | --- |
| Desired FoldOps manifest release | `/data/config/foldops/assigned-manifest.toml` | `foldingosctl foldops acquire` |
| Desired tools version | `/data/config/tools/assigned-version.json` | `foldingosctl tools acquire` |

Bootstrap manifests embedded in the OS image at
`/usr/share/foldingos/manifests/foldops.toml` define the **floor** for first
boot only. When an assigned manifest is present and authorized, it takes
precedence over the embedded bootstrap pin.

`foldingosctl foldops acquire` must activate assigned releases without requiring
a new OS image build.

### foldingosctl tools acquire

Introduce `foldingosctl tools acquire` to update the control-plane binary
without reimaging:

1. Read assigned tools version from supervisor configuration on disk
2. Download pinned static binary (or minimal `layout-tar-zst` with `bin/foldingosctl`)
   from `https://packages.folding-os.com/foldingos-tools/<version>/`
3. Verify size and SHA-256
4. Atomically replace `/usr/bin/foldingosctl`
5. Record active version under `/data/state/tools/`
6. Restart affected long-running units that embed `foldingosctl`

Every released `foldingosctl` must retain the ability to acquire a newer tools
release that understands the current assignment and bundle schema.

### Manifest schema version 2

Acquisition manifests gain:

```toml
schema_version = 2
artifact_format = "layout-tar-zst"   # or "deb" during migration
install_prefix = "foldops-agent"     # top-level directory inside tar
```

### Official publication channels

| Channel | Content |
| --- | --- |
| `releases.folding-os.com` | FoldingOS disk images |
| `packages.folding-os.com/foldops/` | FoldOps layout bundles + manifest |
| `packages.folding-os.com/foldingos-tools/` | `foldingosctl` binaries |
| `deb.folding-os.com` | optional Debian packages for non-appliance hosts |

`deb.folding-os.com` is not used by FoldingOS appliances at runtime.

### What still requires OS reimage

- kernel or initramfs changes
- FoldingOS systemd unit graph changes not deliverable through tools acquire
- bootstrap floor newer than the running acquirer can understand
- partition layout or root filesystem platform changes

---

## Alternatives Considered

### Continue embedded-manifest-only FoldOps updates

Rejected. Does not meet the fleet update goal.

### Runtime apt against deb.folding-os.com

Rejected per [ADR-0018](0018-foldops-package-acquisition-and-update-model.md).

### Store foldingosctl only under /data/apps like FoldOps

Rejected for initial implementation. Atomic replace of `/usr/bin/foldingosctl`
is simpler for systemd units and bootstrapping; may revisit if dual-path causes
operational pain.

---

## Consequences

### Positive

- Routine FoldOps and `foldingosctl` fixes roll out without 4 GiB image rebuild
- Transport matches appliance tooling (Go extract, no deb parser long-term)
- Supervisor-centric fleet operations mirror OS desired-version assignment
- Monorepo CI can publish bundles and tools without `./scripts/build` OS image

### Negative

- Manifest v2, assignment APIs, and tools acquire require implementation work
- Bootstrap compatibility rules must be tested across skewed versions
- Publication infrastructure expands beyond `deb.folding-os.com`

---

## References

- [ADR-0009: Folding@home Acquisition And Update Model](0009-fah-acquisition-and-update-model.md)
- [ADR-0018: FoldOps Package Acquisition And Update Model](0018-foldops-package-acquisition-and-update-model.md)
- [ADR-0022: FoldOps Rust Source In FoldingOS Monorepo](0022-foldops-rust-source-in-foldingos-monorepo.md)
- [Milestone 4 appliance artifact and monorepo plan](../milestone/4-appliance-artifact-and-monorepo-plan.md)
