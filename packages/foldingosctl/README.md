# foldingosctl (Rust)

On-appliance control program for FoldingOS. Implements provisioning, configuration,
Folding@home lifecycle, FoldOps acquisition, fleet registry, recovery, and
machine-readable inspection for automation.

Installed at `/usr/bin/foldingosctl` on appliance images (setuid root). FoldOps
services delegate to this binary; they do not perform privileged OS operations
directly.

## Documentation

| Document | Audience |
| --- | --- |
| [doc/foldingosctl.md](../../doc/foldingosctl.md) | Operators — command syntax, roles, workflows |
| [doc/foldingosctl-components.md](../../doc/foldingosctl-components.md) | Developers/agents — module map, dispatch, FoldOps delegation |
| [doc/foldops-components.md](../../doc/foldops-components.md) | FoldOps app (supervisor, agent, dashboard) |

## Layout

```text
packages/foldingosctl/
  Cargo.toml
  src/
    main.rs, cli.rs           # entry + command dispatch
    automation*.rs            # JSON envelopes + policy
    setuid_privilege.rs       # privilege model
    paths.rs                  # AppliancePaths (/data constants)
    boot/, config/, fah/, foldops/, inspect/, provision/,
    recovery/, registry/, services/, tools/  # command groups
  VENDOR/                     # vendored crates (not project source)
```

## Build host prerequisites

- `cargo` / `rustc` (Rust 1.85+ per `rust-version` in `Cargo.toml`)

## Commands

From the repository root:

```bash
# Unit tests
cd packages/foldingosctl && cargo test

# JSON automation contract smoke test
./scripts/test-api-json --foldingosctl packages/foldingosctl/target/debug/foldingosctl

# Release binary for packages channel / overlay sync
./scripts/build-foldingosctl-release --version <version> --sync-overlay
```

Publishing the tools channel does not require a full OS image build when using
`--sync-overlay` to update overlay bootstrap pins.

## Command groups

```text
foldingosctl <boot|config|fah|foldops|identity|inspect|provision|recovery|registry|services|storage|tools> <command>
```

Use `--format json` on automation-facing commands for structured output. See
[foldingosctl.md](../../doc/foldingosctl.md) for per-command syntax.

## MCP project memory

Component docs under `doc/` are indexed by `tools/index_memory.py` for
`search_project_memory` queries in Cursor.

```bash
.venv/bin/python tools/index_memory.py
```

## Related documents

- [doc/foldingosctl.md](../../doc/foldingosctl.md)
- [doc/foldingosctl-components.md](../../doc/foldingosctl-components.md)
- [doc/agent-subsystems.md](../../doc/agent-subsystems.md) — foldingosctl CLI section
- [ADR-0025](../../doc/adr/0025-implement-foldingosctl-in-rust.md)
