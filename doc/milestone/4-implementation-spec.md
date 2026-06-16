# FoldingOS Milestone 4 FoldOps Integration Implementation Specification

**Version:** 1.0

**Status:** Proposed

**Target Milestone:** Milestone 4, FoldOps Integration

---

# Purpose

This document defines the implementation scope for Milestone 4: full FoldOps
integration on FoldingOS appliances.

Milestone 3 delivered supervisor-led fleet provisioning, FoldOps package
acquisition, ingest-token/TLS bootstrap, and basic `foldops-agent` /
`foldops-supervisor` services. Milestone 4 completes the management contract so
FoldOps can operate a FoldingOS fleet using **`foldingosctl` as the local node
control plane** rather than reimplementing appliance behavior.

Concrete mechanisms are defined in [4-engineering-spec.md](4-engineering-spec.md).

---

# Milestone Goal

```text
Supervisor and agents boot

↓

FoldOps services start after foldingosctl foldops provision

↓

FoldOps agent collects node state by invoking foldingosctl inspect

↓

FoldOps supervisor correlates fleet inventory and health

↓

Operators manage the fleet from the FoldOps dashboard

↓

Approved actions invoke foldingosctl locally on the appropriate node

↓

Folding@home continues even if FoldOps is unavailable
```

---

# Scope

Milestone 4 adds:

- machine-readable `foldingosctl` output for automation ([ADR-0021](../adr/0021-machine-readable-foldingosctl-automation-interface.md))
- read-only `foldingosctl inspect` commands for inventory, health, FAH, and update state
- FoldOps agent refactors to delegate FoldingOS data collection to `foldingosctl`
- FoldOps ingest payloads correlated with FoldingOS `node-id` and installation role
- FoldOps supervisor integration with local `foldingosctl provision` and
  `registry` commands on the supervisor role
- operator workflows in FoldOps for fleet visibility, desired-version assignment,
  and selected configuration actions
- automated and physical validation of the integrated management path
- cross-repository contract documentation with [pacificnm/foldops](https://github.com/pacificnm/foldops)

Milestone 4 extends but does not replace:

- Milestone 3 provisioning enrollment (`foldingosctl provision enroll`)
- Milestone 3 supervisor HTTP provisioning API
- Milestone 2 Folding@home acquisition through `foldingosctl fah`

---

# Non-Goals

Milestone 4 does not implement:

- SSH-based remote command execution from FoldOps to agents
- replacing the FoldingOS supervisor provisioning API with FoldOps endpoints
- runtime APT or general package management on appliances
- FoldOps redistribution of FoldingOS or Folding@home binaries
- GPU management or non-CPU Folding@home features
- FoldOps-driven network PXE provisioning UI that bypasses `foldingosctl provision`
- full parity with legacy Debian FoldOps deployments in one release (FoldingOS path
  is the reference implementation)

---

# Architectural Principles

- **Delegate, don't duplicate.** FoldOps on FoldingOS invokes `foldingosctl`.
- **Fail soft for management, fail closed for mutation.** Ingest failures must
  not stop folding; invalid configuration changes must not apply.
- **Fixed roles.** Supervisor and agent capabilities remain role-gated per
  [ADR-0014](../adr/0014-fixed-installation-roles.md).
- **Separate channels.** FoldOps `.deb` updates and FoldingOS image updates remain
  separate per [ADR-0017](../adr/0017-official-release-publication-and-supervisor-upstream-polling.md)
  and [ADR-0018](../adr/0018-foldops-package-acquisition-and-update-model.md).
- **Coordinate cross-repo.** Schema and API changes require updates in both
  folding-os and foldops before release.

---

# Success Criteria

Milestone 4 is complete when:

1. FoldOps agent ingest on a FoldingOS agent is produced entirely from
   `foldingosctl inspect` and approved read commands
2. FoldOps dashboard shows fleet inventory keyed by FoldingOS `node-id`
3. FoldOps supervisor on a FoldingOS supervisor can list enrollments and assign
   desired image versions through `foldingosctl`
4. At least one remote configuration workflow uses `foldingosctl config activate`
   through an approved FoldOps action path
5. QEMU and physical validation records are committed
6. [4-readiness-review.md](4-readiness-review.md) is committed after all
   release-blocking issues close

---

# Implementation Sequence

See [4-engineering-spec.md](4-engineering-spec.md) for the ordered work breakdown.

High-level phases:

1. Machine-readable CLI foundation
2. FoldOps agent delegation on agents
3. FoldOps supervisor local fleet commands
4. Dashboard operator workflows
5. Remote configuration workflow
6. Validation and readiness review

---

# Related Documents

- [Milestone 3 engineering specification](3-engineering-spec.md)
- [Milestone 4 engineering specification](4-engineering-spec.md)
- [FoldOps integration](../foldops-integration.md)
- [ADR-0020](../adr/0020-foldops-delegates-node-operations-to-foldingosctl.md)
- [ADR-0021](../adr/0021-machine-readable-foldingosctl-automation-interface.md)
- [Roadmap](../../ROADMAP.md)
