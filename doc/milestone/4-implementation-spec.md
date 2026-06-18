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
- supervisor fleet mutation authorization for the `foldops` service user per
  [ADR-0024](../adr/0024-foldops-supervisor-fleet-mutation-authorization.md)
- `foldingosctl` reimplementation in Rust per
  [ADR-0025](../adr/0025-implement-foldingosctl-in-rust.md) (issue #101)
- dashboard operator authentication and first-boot bootstrap per
  [ADR-0026](../adr/0026-foldops-dashboard-operator-authentication.md)
- remote operator API model (supervisor gateway and agent proxy) per
  [ADR-0027](../adr/0027-foldops-remote-operator-api.md)
- Rust FoldOps source in `packages/foldops/` per
  [ADR-0022](../adr/0022-foldops-rust-source-in-foldingos-monorepo.md)
- runtime FoldOps and `foldingosctl` updates without OS reimage per
  [ADR-0023](../adr/0023-runtime-foldops-and-foldingosctl-updates-without-os-reimage.md)
  (`layout-tar-zst`, supervisor-assigned manifests, `tools acquire`)
- automated and physical validation of the integrated management path
- contract documentation in this repository (`packages/foldops/`, manifests, ADRs)

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
- **Separate channels.** FoldOps bundles, `foldingosctl` tools binaries, and
  FoldingOS image updates remain separate per
  [ADR-0017](../adr/0017-official-release-publication-and-supervisor-upstream-polling.md),
  [ADR-0018](../adr/0018-foldops-package-acquisition-and-update-model.md), and
  [ADR-0023](../adr/0023-runtime-foldops-and-foldingosctl-updates-without-os-reimage.md).
- **Monorepo contracts.** Schema and API changes land in this repository
  (`packages/foldops/`, `packages/foldingosctl/`, docs) before release.

---

# Success Criteria

Milestone 4 is complete when:

1. FoldOps agent ingest on a FoldingOS agent is produced entirely from
   `foldingosctl inspect` and approved read commands
2. FoldOps dashboard shows fleet inventory keyed by FoldingOS `node-id`
3. FoldOps supervisor on a FoldingOS supervisor can list enrollments and assign
   desired image, FoldOps, and tools versions through `foldingosctl`
4. At least one remote configuration workflow uses `foldingosctl config activate`
   through an approved FoldOps action path
5. Dashboard operator authentication and remote operator API protect supervisor
   management paths per ADR-0026 and ADR-0027
6. `foldingosctl` is implemented in Rust and the Go tree is removed per ADR-0025
7. QEMU and physical validation records are committed
8. [4-readiness-review.md](4-readiness-review.md) is committed after all
   release-blocking issues close

---

# Implementation Sequence

See [4-engineering-spec.md](4-engineering-spec.md) for the ordered work breakdown.

High-level phases:

1. Appliance artifact transport and monorepo foundation
2. Machine-readable CLI foundation
3. FoldOps agent delegation on agents
4. FoldOps supervisor local fleet commands
5. Dashboard operator authentication and first-run bootstrap
6. Dashboard operator workflows and remote operator API routes
7. `foldingosctl` Rust migration (issue #101)
8. Remote configuration workflow
9. Validation and readiness review

---

# Related Documents

- [Milestone 4 appliance artifact and monorepo plan](4-appliance-artifact-and-monorepo-plan.md)
- [Milestone 3 engineering specification](3-engineering-spec.md)
- [Milestone 4 engineering specification](4-engineering-spec.md)
- [FoldOps integration](../foldops-integration.md)
- [ADR-0020](../adr/0020-foldops-delegates-node-operations-to-foldingosctl.md)
- [ADR-0021](../adr/0021-machine-readable-foldingosctl-automation-interface.md)
- [ADR-0022](../adr/0022-foldops-rust-source-in-foldingos-monorepo.md)
- [ADR-0023](../adr/0023-runtime-foldops-and-foldingosctl-updates-without-os-reimage.md)
- [ADR-0024](../adr/0024-foldops-supervisor-fleet-mutation-authorization.md)
- [ADR-0025](../adr/0025-implement-foldingosctl-in-rust.md)
- [ADR-0026](../adr/0026-foldops-dashboard-operator-authentication.md)
- [ADR-0027](../adr/0027-foldops-remote-operator-api.md)
- [Roadmap](../../ROADMAP.md)
