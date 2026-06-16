# FoldOps (Rust)

FoldOps fleet management applications for FoldingOS appliances.

## Authority

Per [ADR-0022](../../doc/adr/0022-foldops-rust-source-in-foldingos-monorepo.md), the
authoritative FoldOps implementation for FoldingOS is the Rust workspace here.
The legacy Node.js repository at [pacificnm/foldops](https://github.com/pacificnm/foldops)
is deprecated for appliance work.

## Runtime model

FoldOps is **not** embedded in the OS image. Appliances acquire verified layout
bundles at runtime via `foldingosctl foldops acquire` per
[ADR-0018](../../doc/adr/0018-foldops-package-acquisition-and-update-model.md) and
[ADR-0023](../../doc/adr/0023-runtime-foldops-and-foldingosctl-updates-without-os-reimage.md).

Install root: `/data/apps/foldops/<release>/`

## Layout

```text
packages/foldops/
  Cargo.toml
  crates/
    foldops-agent/
    foldops-supervisor/
    foldops-types/
  web/                         # React dashboard (build host only)
  packaging/appliance-bundle/  # reserved for bundle helpers
```

FoldingOS-owned systemd units remain in the repository overlay; bundles ship
application binaries and static web assets only.

## Build host prerequisites

- `cargo` / `rustc` (Rust 1.85+ per workspace `rust-version`)
- `npm` (dashboard static assets only; not used by `./scripts/build`)
- `zstd`, `tar`, `sha256sum`

Optional publication: `rclone` configured for Cloudflare R2.

## Commands

From the repository root:

```bash
# Rust workspace
cd packages/foldops && cargo test --workspace

# Layout bundles + schema v2 manifest (build/output/foldops/<release>/)
./scripts/build-foldops-bundles

# foldingosctl binary for tools publication channel
./scripts/build-foldingosctl-release

# Upload to packages.folding-os.com (when rclone is configured)
./scripts/publish-foldops-bundles 0.1.0-1
```

The OS image build (`./scripts/build`) does **not** compile FoldOps. The embedded
bootstrap manifest in the overlay remains schema v1 until issue #84 lands
layout-tar-zst acquisition in `foldingosctl`.

## Related documents

- [Milestone 4 appliance artifact and monorepo plan](../../doc/milestone/4-appliance-artifact-and-monorepo-plan.md)
- [Issue #83](https://github.com/pacificnm/folding-os/issues/83)
