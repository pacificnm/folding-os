# ADR-0029: Packages Channel Publication Via rclone

**Status:** Proposed

**Date:** 2026-06-18

**Authors:** FoldingOS project

**Depends on:** [ADR-0017](0017-official-release-publication-and-supervisor-upstream-polling.md),
[ADR-0023](0023-runtime-foldops-and-foldingosctl-updates-without-os-reimage.md)

**Related:** [ADR-0028](0028-supervisor-fleet-software-update-workflow.md),
[Milestone 5 engineering specification](../milestone/5-engineering-spec.md)

---

## Context

FoldOps layout bundles and `foldingosctl` tools binaries publish to
`packages.folding-os.com`, backed by Cloudflare R2 object storage per
[ADR-0017](0017-official-release-publication-and-supervisor-upstream-polling.md).

The repository already includes:

- `scripts/build-foldops-bundles` — build `layout-tar-zst` bundles and
  `manifest.toml`
- `scripts/build-foldingosctl-release` — build static `foldingosctl` binary and
  `SHA256SUMS`
- `scripts/publish-foldops-bundles` — upload one FoldOps release via **rclone**

Operators maintain rclone configuration at `~/.config/rclone/rclone.conf` on
the build host. Milestone 5 requires a **repeatable, documented publication
pipeline** so new FoldOps and tools releases can be built and uploaded without
manual object-store steps, and so supervisor “check for updates” has stable
upstream catalogs to read.

---

## Decision

FoldingOS will standardize **packages-channel publication** on rclone from the
developer/build host, with repository scripts that wrap build + upload + index
update.

### 1. Publication channels

| Channel | Build script | Upload script | Public base URL |
| --- | --- | --- | --- |
| FoldOps bundles | `scripts/build-foldops-bundles` | `scripts/publish-foldops-bundles` | `https://packages.folding-os.com/foldops/<release>/` |
| foldingosctl tools | `scripts/build-foldingosctl-release` | `scripts/publish-foldingos-tools` (new) | `https://packages.folding-os.com/foldingos-tools/<version>/` |

FoldingOS disk images continue to publish through the `releases.folding-os.com`
channel separately. This ADR does not change OS image publication.

### 2. rclone configuration

Publication scripts assume:

- rclone installed on the build host
- remote configured in `~/.config/rclone/rclone.conf` (default remote name
  `r2`, overridable via `R2_REMOTE`)
- bucket and prefix overridable via environment variables documented in each
  script

Scripts must **not** embed secrets. Credentials live only in the operator’s
rclone config file.

Default environment variables (may be overridden):

| Variable | Default | Purpose |
| --- | --- | --- |
| `R2_REMOTE` | `r2` | rclone remote name |
| `FOLDOPS_R2_BUCKET` | `foldops-packages` | destination bucket |
| `FOLDOPS_R2_PREFIX` | `foldops` | FoldOps object prefix |
| `TOOLS_R2_PREFIX` | `foldingos-tools` | tools object prefix |

### 3. Required artifacts per release

**FoldOps** (`build/output/foldops/<manifest_release>/`):

- `manifest.toml` (schema v2, pinned URLs and digests)
- `SHA256SUMS`
- `foldops-agent-x86_64.tar.zst`
- `foldops-supervisor-x86_64.tar.zst`
- `foldops-web-x86_64.tar.zst`

**Tools** (`build/output/foldingos-tools/<version>/`):

- `foldingosctl-x86_64`
- `SHA256SUMS`

### 4. Upstream index files

Each channel publishes a machine-readable **`index.json`** at the channel root:

```text
https://packages.folding-os.com/foldops/index.json
https://packages.folding-os.com/foldingos-tools/index.json
```

Schema version 1 lists published releases with:

- release identifier (`manifest_release` or `tools_version`)
- publication timestamp (RFC 3339)
- manifest or checksum URL
- minimum compatible FoldingOS version when applicable

Publication scripts update the index atomically (write temp object, replace
root index, or upload versioned index + replace pointer). Exact mechanics are
defined in the Milestone 5 engineering specification.

Supervisor “check for updates” reads these indexes per
[ADR-0028](0028-supervisor-fleet-software-update-workflow.md).

### 5. Umbrella release script

Add `scripts/publish-packages-release` that:

1. accepts FoldOps manifest release id and tools version id
2. allows FoldOps and tools ids to differ so each channel can ship independently
3. runs channel build scripts when `--build` is passed (or expects prebuilt output)
4. builds tools releases with `build-foldingosctl-release --sync-overlay`
   when `--build --tools` is requested
5. invokes `publish-foldops-bundles` and `publish-foldingos-tools`
6. refreshes both channel `index.json` files
7. prints public URLs for operator verification

Dry-run mode must list planned uploads without writing objects.

### 6. Embedded bootstrap manifest policy

Publishing a new FoldOps release does **not** require an immediate OS image
rebuild. The embedded bootstrap manifest in the OS image remains the floor.
Supervisor assignment overrides the floor on running nodes per ADR-0023.

Publishing a new `foldingosctl` tools release also does **not** require an
immediate OS image rebuild. The tools artifact may be built by
`build-foldingosctl-release` and published directly to the tools channel. Passing
`--sync-overlay` writes `overlay/usr/share/foldingos/manifests/tools.json` so
the next OS image build embeds that tools pin as its bootstrap assignment.

Updating embedded overlay pins is a release hygiene step, not an image rebuild
requirement. Use `--sync-overlay` on `build-foldops-bundles` for FoldOps and on
`build-foldingosctl-release` for tools when the next image should inherit the
new package-channel release.

---

## Alternatives Considered

### GitHub Actions-only publication without local rclone

Rejected for Milestone 5. Operators already use local rclone; scripts must work
from the build host documented in [operations.md](../operations.md).

### Single combined manifest for FoldOps and tools

Rejected. Separate channels and indexes preserve independent release cadences
and match ADR-0023 assignment fields.

### Continue manual R2 console uploads

Rejected. Error-prone and blocks automated “check for updates”.

---

## Consequences

### Positive

- Repeatable release workflow aligned with existing rclone setup
- Supervisor can discover latest published versions
- Monorepo can ship FoldOps and tools fixes without `./scripts/build` OS image

### Negative

- Index schema and publication ordering must be kept consistent
- Mis-published index can confuse fleet update checks until corrected
- Build host requires rclone credentials management discipline

---

## References

- [Milestone 5 engineering specification](../milestone/5-engineering-spec.md)
- [packages/foldops/README.md](../../packages/foldops/README.md)
- [ADR-0023](0023-runtime-foldops-and-foldingosctl-updates-without-os-reimage.md)
