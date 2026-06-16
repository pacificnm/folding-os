# ADR-0021: Machine-Readable foldingosctl Automation Interface

**Status:** Proposed

**Date:** 2026-06-14

**Authors:** FoldingOS project

**Depends on:** [ADR-0020](0020-foldops-delegates-node-operations-to-foldingosctl.md)

---

## Context

[ADR-0020](0020-foldops-delegates-node-operations-to-foldingosctl.md) requires
FoldOps to invoke `foldingosctl` for node-local FoldingOS operations.

The current CLI is optimized for operators and systemd:

- success prints human-readable lines to stdout
- failures print `foldingosctl: …` to stderr and exit non-zero
- several commands are side-effectful or role-gated
- there is no stable JSON schema for automation consumers

FoldOps agents need predictable structured output to build ingest payloads.
FoldOps supervisor services need structured enrollment, registry, and assignment
data for dashboard and coordination features.

---

## Decision

FoldingOS will add a **machine-readable automation interface** to `foldingosctl`
with the following rules:

1. **JSON is the canonical automation format.** Automated callers use
   `--format json` (long form) on supported commands. JSON objects include
   `schema_version` and `ok` fields at the top level.

2. **Milestone 4 introduces an `inspect` command group** for read-only
   aggregation used by FoldOps ingest and dashboards:

   | Command | Purpose |
   | --- | --- |
   | `inspect node` | Identity, role, hostname, FoldingOS version |
   | `inspect system` | Uptime, load, memory, root filesystem, primary NIC |
   | `inspect fah` | Verified client version, service state, runtime summary |
   | `inspect commissioning` | Boot readiness checks currently used by commissioning display |
   | `inspect update` | Current/desired image version and staged update state |

   `inspect` commands are read-only and safe for periodic polling.

3. **Existing mutating commands gain JSON output where already used by
   automation**, starting with:

   | Command | JSON support |
   | --- | --- |
   | `provision list-enrollments` | full row objects |
   | `registry list` | version entries |
   | `registry show <version>` | registry entry document |
   | `config effective <domain>` | structured effective config |
   | `config validate <domain\|--all>` | validation result list |

4. **Human and JSON modes are mutually exclusive per invocation.** When
   `--format json` is set:

   - success writes one JSON document to stdout
   - failure writes one JSON error document to stdout
   - stderr contains only diagnostic logs if explicitly enabled later; by default
     stderr is empty on failure so parsers have a single stream

5. **Automation authorization uses the `foldops` service identity.** Read-only
   `inspect` commands and approved read commands must succeed when invoked by
   the `foldops` user without granting general root privileges. Mutating
   commands remain restricted to `foldingos-admin` or systemd unless a future
   ADR defines an explicit automation policy file.

6. **Schema changes are versioned.** Increment `schema_version` in JSON output
   when fields are removed or change meaning. Additive fields are permitted within
   the same major schema version.

7. **FoldOps must treat unsupported commands as absent capability**, not as a
   signal to fall back to direct OS inspection on FoldingOS appliances.

---

## Alternatives Considered

### Parse existing human output with regular expressions

Rejected. Current stdout is not a stable contract and varies with formatting
changes.

### Separate `foldingosctl-json` binary

Rejected. Splitting the control plane increases drift risk between human and
automation paths.

### Expose all commands to the foldops user

Rejected. Mutating commands such as `provision install` and `config activate`
require stricter authorization and audit boundaries.

---

## Consequences

### Positive

- FoldOps can integrate without embedding FoldingOS filesystem knowledge
- JSON schemas can be tested in `foldingosctl` unit tests independent of FoldOps
- Operators can use the same commands with `--format json` for scripting

### Negative

- Every automated command requires explicit JSON implementation and tests
- FoldOps and FoldingOS must coordinate schema changes across repositories

### Tradeoffs

- Subcommand-specific JSON is preferred over one giant `inspect all` blob so
  consumers fetch only needed data and failures are isolated

---

## Future Considerations

- Publish JSON Schema files under `doc/schemas/foldingosctl/` when the surface
  stabilizes
- Consider `--format json` for additional commands after Milestone 4 baseline
- Event streaming or push-based telemetry remains out of scope

---

## References

- [foldingosctl command reference](../foldingosctl.md)
- [Milestone 4 engineering specification](../milestone/4-engineering-spec.md)
- [ADR-0020](0020-foldops-delegates-node-operations-to-foldingosctl.md)
