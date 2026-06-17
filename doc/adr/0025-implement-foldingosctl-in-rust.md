# ADR-0025: Implement foldingosctl In Rust

**Status:** Proposed

**Date:** 2026-06-14

**Authors:** FoldingOS project

**Depends on:** [ADR-0020](0020-foldops-delegates-node-operations-to-foldingosctl.md),
[ADR-0021](0021-machine-readable-foldingosctl-automation-interface.md),
[ADR-0022](0022-foldops-rust-source-in-foldingos-monorepo.md),
[ADR-0023](0023-runtime-foldops-and-foldingosctl-updates-without-os-reimage.md),
[ADR-0024](0024-foldops-supervisor-fleet-mutation-authorization.md)

**Related:** [ADR-0026](0026-foldops-dashboard-operator-authentication.md),
[ADR-0027](0027-foldops-remote-operator-api.md)

**Implements:** GitHub issue #101

---

## Context

`foldingosctl` is the sole supported local control interface on FoldingOS
appliances ([ADR-0020](0020-foldops-delegates-node-operations-to-foldingosctl.md)).
FoldOps services in `packages/foldops/` are implemented in Rust per
[ADR-0022](0022-foldops-rust-source-in-foldingos-monorepo.md).

The current `foldingosctl` implementation is written in Go under
`packages/foldingosctl/src/` and is built into release images through Buildroot's
`golang-package` integration.

Maintaining two language toolchains for the appliance stack creates avoidable
cost:

- CI, developer environments, and release validation must install and cache both
  Go and Rust
- error handling, configuration parsing, testing, and release discipline diverge
  between platform and fleet code
- FoldOps already shells out to `foldingosctl`; two languages increase contract
  drift risk without improving the operator boundary

FoldingOS is a small appliance project that prioritizes static binaries,
predictable resource use, safe system interaction, and long-term maintainability.
Consolidating platform control-plane code in Rust matches those goals and aligns
`packages/foldops/` with `packages/foldingosctl/`.

The repository remains early enough that replacing the Go tree before Milestone 4
completion is lower risk than carrying dual toolchains through fleet integration,
dashboard workflows, and runtime `tools acquire` updates.

---

## Decision

FoldingOS will **reimplement `foldingosctl` in Rust** and **remove the Go
implementation** once parity is verified.

### 1. Canonical implementation

- The authoritative `foldingosctl` source moves to a **Rust crate** under
  `packages/foldingosctl/`.
- The installed appliance binary remains:

  ```text
  /usr/bin/foldingosctl
  ```

- The human and automation CLI contract documented in
  [foldingosctl.md](../foldingosctl.md) and
  [ADR-0021](0021-machine-readable-foldingosctl-automation-interface.md) is
  **stable across the migration**. JSON schemas, exit codes, and role checks must
  not change meaning without explicit ADR or specification updates.

### 2. Build integration

- Buildroot builds `foldingosctl` with the Rust toolchain (`rust-package` or
  equivalent approved mechanism), not `golang-package`.
- The FoldingOS image build **drops the Go toolchain dependency** used only for
  `foldingosctl`.
- Release images continue to ship a statically linked control-plane binary
  suitable for appliance use.

### 3. FoldOps integration unchanged

- FoldOps agent and supervisor continue to invoke `/usr/bin/foldingosctl` as a
  **subprocess** with `--format json` per
  [ADR-0020](0020-foldops-delegates-node-operations-to-foldingosctl.md) and
  [ADR-0027](0027-foldops-remote-operator-api.md).
- This migration does **not** introduce an in-process library API between
  FoldOps and `foldingosctl`.

### 4. Phased migration

Migration proceeds command-group by command-group with tests at each step:

| Phase | Command surface |
| --- | --- |
| 1 | Crate skeleton, CLI dispatch, JSON automation envelope, build wiring |
| 2 | `inspect` and read-only automation commands |
| 3 | Supervisor fleet reads and mutators authorized by [ADR-0024](0024-foldops-supervisor-fleet-mutation-authorization.md) |
| 4 | Provisioning, enrollment, registry, update, config, storage, identity |
| 5 | FAH, FoldOps acquire/provision/serve-https, tools acquire |
| 6 | Remove Go sources, vendor tree, and `golang-package` build rules |

Each phase must keep existing QEMU and script-based acceptance checks passing
before the next phase begins.

### 5. Runtime updates

[ADR-0023](0023-runtime-foldops-and-foldingosctl-updates-without-os-reimage.md)
remains in force. The Rust binary must remain replaceable through
`foldingosctl tools acquire` without OS reimage.

### 6. Repository layout

Target layout:

```text
packages/
  foldingosctl/              # Rust crate — appliance control plane
    Cargo.toml
    src/
  foldops/                   # Rust workspace — fleet management (unchanged)
```

The Go tree at `packages/foldingosctl/src/` is **removed** when migration
completes. Until then, Rust development may live beside Go in a transitional
layout defined by the engineering specification update for issue #101.

---

## Alternatives Considered

### Keep Go for foldingosctl

Rejected. Perpetuates dual toolchains and splits platform conventions from
FoldOps for no architectural benefit.

### Rewrite FoldOps in Go

Rejected. FoldOps is already Rust-only on appliances; reverting fleet code would
discard completed Milestone 3–4 work.

### In-process Rust library shared by FoldOps and foldingosctl

Rejected for this migration. Subprocess delegation preserves a single operator
entry point, distinct service identities, and independent testing. See
[ADR-0027](0027-foldops-remote-operator-api.md).

### Separate `foldingosctl` repository

Rejected. Platform control plane and appliance documentation already live in
this repository; monorepo changes remain coordinated per
[ADR-0022](0022-foldops-rust-source-in-foldingos-monorepo.md).

---

## Consequences

### Positive

- One primary language for appliance application and control-plane code
- Simpler CI and developer setup
- Rust matches static-binary, reliability, and safety goals for OS tooling
- FoldOps delegation contract stays stable while implementation moves

### Negative

- Large one-time porting effort across provisioning, config, FAH, and update paths
- Transitional period may require dual-tree discipline until Go removal
- Buildroot Rust integration must be validated on the pinned Buildroot baseline

### Tradeoffs

- Phased migration is slower than a big-bang rewrite but preserves acceptance
  evidence and production lab continuity
- CLI stability is preferred over internal Rust API elegance during the port

---

## Future Considerations

- Publish machine-readable JSON Schema under `doc/schemas/foldingosctl/` when the
  Rust implementation stabilizes
- Evaluate shared internal crates only if duplication becomes measurable after Go
  removal

---

## References

- [foldingosctl command reference](../foldingosctl.md)
- [Milestone 4 engineering specification](../milestone/4-engineering-spec.md)
- [Build system](../build-system.md)
- [ADR-0020](0020-foldops-delegates-node-operations-to-foldingosctl.md)
- [ADR-0021](0021-machine-readable-foldingosctl-automation-interface.md)
- [ADR-0022](0022-foldops-rust-source-in-foldingos-monorepo.md)
- [ADR-0023](0023-runtime-foldops-and-foldingosctl-updates-without-os-reimage.md)
- [ADR-0024](0024-foldops-supervisor-fleet-mutation-authorization.md)
- GitHub issue #101
