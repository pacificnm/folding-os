# FoldOps (Rust)

FoldOps fleet management applications for FoldingOS appliances.

**Status:** Planned import — source tree not yet present in this repository.

## Authority

Per [ADR-0022](../../doc/adr/0022-foldops-rust-source-in-foldingos-monorepo.md), the
authoritative FoldOps implementation for FoldingOS is the Rust workspace here.
The legacy Node.js repository is deprecated for appliance work.

## Runtime model

FoldOps is **not** embedded in the OS image. Appliances acquire verified layout
bundles at runtime via `foldingosctl foldops acquire` per
[ADR-0018](../../doc/adr/0018-foldops-package-acquisition-and-update-model.md) and
[ADR-0023](../../doc/adr/0023-runtime-foldops-and-foldingosctl-updates-without-os-reimage.md).

Install root: `/data/apps/foldops/<release>/`

## Build (target)

```bash
# From repository root after import:
./scripts/build-foldops-bundles
```

Build uses `cargo` and repository shell scripts only. FoldingOS does not use
Node.js or `npm` in the appliance or platform build path.

## Layout (target)

```text
packages/foldops/
  Cargo.toml
  crates/
    foldops-agent/
    foldops-supervisor/
    foldops-shared/
  packaging/appliance-bundle/
```

See [Milestone 4 appliance artifact and monorepo plan](../../doc/milestone/4-appliance-artifact-and-monorepo-plan.md).
