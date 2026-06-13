# Milestone 2 Readiness Review

**Version:** 1.0

**Status:** Approved

**Review date:** 2026-06-13

**Target release:** v0.1.0 Folding@home runtime scope

---

# Purpose

This document records the Milestone 2 Folding@home Runtime readiness review
required by issue #22.

It reconciles implemented acquisition, verification, activation, and service
behavior with [ADR-0006](../adr/0006-fah-packaging-and-privilege-model.md),
[ADR-0009](../adr/0009-fah-acquisition-and-update-model.md), and the approved
v0.1.0 scope and engineering specifications. It records validation evidence and
states the remaining gates before a public v0.1.0 release.

---

# Completion Status

```text
Milestone 2 Folding@home runtime implementation: COMPLETE
Milestone 2 Folding@home runtime validation:      COMPLETE
v0.1.0 runtime readiness:                         SATISFIED
Public v0.1.0 release eligibility:                BLOCKED (metadata workflow)
```

Milestone 2 Folding@home runtime work is complete when issues #13 through #21
are closed with matching implementation and validation evidence, and this
readiness review is committed. FoldOps integration remains future roadmap
scope and is not part of this milestone.

---

# Governing Specifications

GitHub Milestone 2 is titled **Folding@home Runtime**. The governing approved
documents for this milestone are:

| Document | Role |
| --- | --- |
| [1-implementation-spec.md](1-implementation-spec.md) | v0.1.0 scope including Folding@home integration |
| [1-engineering-spec.md](1-engineering-spec.md) | Concrete acquisition, activation, service, and validation contract |
| [ADR-0006](../adr/0006-fah-packaging-and-privilege-model.md) | Least-privilege `fah` service model |
| [ADR-0009](../adr/0009-fah-acquisition-and-update-model.md) | Non-redistribution acquisition and update model |

The draft files [2-engineering-spec.md](2-engineering-spec.md) and
[2-implementation-spec.md](2-implementation-spec.md) describe FoldOps
managed-node scope. They are not the governing specification for this
milestone and were not modified as part of issue #22.

---

# Issue Closure Matrix

| Issue | Title | State | Primary evidence |
| --- | --- | --- | --- |
| #13 | Approve and commit the pinned Folding@home 8.5 acquisition manifest | Closed | `overlay/usr/share/foldingos/manifests/fah.toml`, `scripts/verify-fah-manifest` |
| #14 | Implement strict Folding@home manifest parsing and validation | Closed | `packages/foldingosctl/src/fah.go`, `fah_test.go` |
| #15 | Implement verified Folding@home artifact download and staging | Closed | `packages/foldingosctl/src/fah_acquire.go` |
| #16 | Implement Folding@home artifact extraction and installed-version verification | Closed | `packages/foldingosctl/src/fah_extract.go`, `fah_verify_install.go` |
| #17 | Implement atomic Folding@home activation and rollback preservation | Closed | `packages/foldingosctl/src/fah_activate.go` |
| #18 | Render validated Folding@home runtime configuration and secrets | Closed | `packages/foldingosctl/src/fah_prepare.go`, `overlay/.../foldingos-fah-prepare.service` |
| #19 | Implement least-privilege Folding@home systemd service | Closed | `overlay/.../folding-at-home.service`, `fah_run.go` |
| #20 | Implement Folding@home acquisition scheduling, retries, and standalone operation | Closed | `foldingos-fah-acquire.{service,timer}`, `fah_acquire_state.go` |
| #21 | Complete Folding@home runtime automated and physical validation | Closed | `scripts/test-qemu`, `scripts/run-physical-acceptance`, `validation/appliance-physical-0.1.0.json` |
| #22 | Finalize Milestone 2 Folding@home runtime documentation and readiness review | Closed | This document |

No Milestone 2 release-blocking issue remains open or deferred through an
approved document change.

---

# Approved Upstream Artifact

The committed v0.1.0 manifest pins the exact approved client:

| Field | Value |
| --- | --- |
| Client version | `8.5.6` |
| Architecture | `x86_64` |
| Artifact | `fah-client_8.5.6_amd64.deb` |
| Origin | `https://download.foldingathome.org/` |
| Artifact URL | `https://download.foldingathome.org/releases/beta/fah-client/debian-10-64bit/release/fah-client_8.5.6_amd64.deb` |
| Size | `3205180` bytes |
| SHA-256 | `643de04033a1cb972a81e3a193d710e919a4f34634a987f11adc4cee61fdaefe` |
| Terms | `https://foldingathome.org/faq/opensource/` |

Builds run `scripts/verify-fah-manifest` before image generation and record
`approved_fah_manifest_sha256` in release metadata.

---

# Licensing And Non-Redistribution

Confirmed by implementation and documentation review:

- FoldingOS release images contain **no** Folding@home client or FahCore
  binaries.
- Nodes download the pinned client artifact directly from official Folding@home
  HTTPS infrastructure after deployment.
- FoldingOS does not mirror, cache, proxy, or redistribute upstream client or
  FahCore artifacts.
- The approved client is GPL-3.0-or-later; upstream terms are referenced by the
  manifest `terms_url`.
- FahCore binaries remain separately governed downloads managed by the client
  at runtime.

Operator disclosure and procedures are documented in
[operations.md](../operations.md).

---

# Implementation Reconciliation

## Acquisition and activation

Implemented behavior matches [ADR-0009](../adr/0009-fah-acquisition-and-update-model.md)
and [1-engineering-spec.md](1-engineering-spec.md):

- embedded approved manifest at `/usr/share/foldingos/manifests/fah.toml`
- `foldingosctl fah acquire` downloads only from approved HTTPS origins
- size and SHA-256 verification before install
- versioned storage under `/data/apps/fah/<version>/`
- atomic activation through `/data/apps/fah/current`
- verified install marker at `.foldingos-verified`
- failed partial downloads and staging directories are removed; last known-good
  versions are retained

## Scheduling, retries, and standalone operation

- `foldingos-fah-acquire.timer` schedules acquisition after boot and after each
  attempt
- verified active client causes acquire to exit successfully without
  re-downloading
- retry state persists at `/data/state/fah-acquire.state`
- retry delays: `1m`, `5m`, `15m`, `1h`, then `6h` indefinitely
- FoldOps is not required for acquisition or continued operation

## Runtime service and privilege model

Implemented behavior matches [ADR-0006](../adr/0006-fah-packaging-and-privilege-model.md):

- `foldingos-fah-prepare.service` renders `/run/foldingos/fah/config.xml`
- `folding-at-home.service` runs as UID/GID `200` (`fah`)
- `ExecStart=/usr/bin/foldingosctl fah run` execs the manifest-defined client
  under `/data/apps/fah/current`
- systemd sandboxing: `NoNewPrivileges`, `ProtectSystem=strict`, constrained
  read/write paths
- service restart policy: `Restart=on-failure`, bounded burst limits

## FoldOps boundary

Review confirms no unapproved FoldOps dependency:

- no FoldOps agent, enrollment, or supervisor services in the v0.1.0 image
- no FoldOps-hosted client mirrors or proxies
- no unpinned `latest` acquisition path

---

# Documentation Reconciliation

| Topic | Document |
| --- | --- |
| Operator procedures | [operations.md](../operations.md) |
| Physical runtime validation | [physical-validation.md](../physical-validation.md) |
| Automated validation | [testing-strategy.md](../testing-strategy.md) |
| Acquisition architecture | [ADR-0009](../adr/0009-fah-acquisition-and-update-model.md) |
| Service privilege model | [ADR-0006](../adr/0006-fah-packaging-and-privilege-model.md) |
| Boot ordering | [boot-process.md](../boot-process.md) |
| Storage layout | [storage-layout.md](../storage-layout.md) |

Operational and failure-recovery procedures in [operations.md](../operations.md)
match implemented acquisition retry behavior, service ordering, and
least-privilege execution constraints verified on the Dell test node.

---

# Validation Evidence

## Unit and manifest validation

```bash
cd packages/foldingosctl/src
go test ./...
```

```bash
./scripts/verify-fah-manifest build/output/images/rootfs.tar
```

## QEMU/OVMF wiring checks

`scripts/test-qemu` verifies foundation behavior plus:

- `foldingosctl fah validate-manifest`
- `foldingos-fah-acquire.timer` enabled
- acquire and runtime unit files present

Live HTTPS client acquisition in QEMU user networking is intentionally not part
of the automated QEMU gate. Runtime acquisition and service behavior are
validated on approved physical hardware.

## Physical runtime acceptance

Validated system:

```text
Dell OptiPlex Micro
```

Committed record:

```text
validation/appliance-physical-0.1.0.json
```

Runtime tests recorded:

| Test | Result |
| --- | --- |
| `fah_acquisition` | pass |
| `fah_runtime_service` | pass |
| `fah_runtime_reboot` | pass |

Verification:

```bash
./scripts/verify-physical-validation-record \
  validation/appliance-physical-0.1.0.json \
  build/output/images/foldingos-x86_64-0.1.0.img
```

Remote acceptance command:

```bash
./scripts/run-physical-acceptance <host> <ssh-private-key> [port]
```

---

# Release Gate Status

| Gate | Milestone 2 runtime | Public v0.1.0 release |
| --- | --- | --- |
| Approved Folding@home manifest committed and verified | Satisfied | Required |
| Folding@home acquisition and runtime validation | Satisfied | Required |
| QEMU/OVMF acceptance | Satisfied (foundation + FAH wiring) | Required |
| Foundation physical validation | Satisfied | Required |
| Reproducible required artifacts | Satisfied (Milestone 1 evidence) | Required |
| Documentation matches implementation | Satisfied by this review | Required |
| Release metadata `release_eligible: true` workflow | Not automated in `generate-build-artifacts` | Still manual |

Milestone 2 satisfies the Folding@home runtime readiness criteria for v0.1.0.
Publication still requires the project's release process to mark a tagged
revision and candidate image as publicly release eligible.

---

# Known Limitations

- CPU-only Folding@home in v0.1.0; GPU support is out of scope
- First acquisition requires reachability of `download.foldingathome.org` and
  working DNS/time synchronization
- Transient DNS races immediately after reboot may delay upstream client
  connectivity for a short period; the service retries automatically
- QEMU live acquisition is not used as an automated release gate
- FoldOps fleet coordination remains future scope

---

# Milestone Boundary

Milestone 2 ends with a bootable v0.1.0 appliance that, after network and time
synchronization:

- acquires the pinned Folding@home 8.5.6 client from official upstream HTTPS
  infrastructure
- verifies and activates the client into versioned persistent storage
- renders validated runtime configuration
- runs `fah-client` as the unprivileged `fah` account under systemd
- preserves verified client state across reboot
- continues operating without FoldOps availability

Milestone 2 does not retroactively change the Milestone 1 foundation contract.

---

# Review Outcome

| Acceptance criterion | Result |
| --- | --- |
| Documentation accurately describes the exact approved upstream artifact and terms | Pass |
| Documentation clearly states FoldingOS does not redistribute client or FahCore binaries | Pass |
| Operational and failure-recovery procedures match implemented behavior | Pass |
| No unapproved FoldOps dependency, redistribution, or unpinned acquisition path exists | Pass |
| All Milestone 2 release-blocking issues are closed or explicitly deferred | Pass |
| Milestone 2 completion and v0.1.0 runtime readiness are recorded | Pass |

**Milestone 2 Folding@home Runtime readiness review: PASS**

---

# Related Documents

- [v0.1.0 scope specification](1-implementation-spec.md)
- [v0.1.0 engineering specification](1-engineering-spec.md)
- [Milestone 1 readiness review](1-readiness-review.md)
- [Operations](../operations.md)
- [Physical validation](../physical-validation.md)
- [Documentation index](../README.md)
