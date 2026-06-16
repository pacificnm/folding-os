# Milestone 3 Readiness Review

**Version:** 1.0

**Status:** Approved

**Review date:** 2026-06-14

**Target release:** v0.1.0 network fleet provisioning scope

---

# Purpose

This document records the Milestone 3 Network Fleet Provisioning readiness review
required by issue #65.

It reconciles implemented supervisor-led provisioning, agent enrollment, staged
updates, and FoldOps integration with [ADR-0016](../adr/0016-network-provisioning-via-supervisor.md),
[ADR-0014](../adr/0014-fixed-installation-roles.md),
[ADR-0017](../adr/0017-official-release-publication-and-supervisor-upstream-polling.md),
[ADR-0018](../adr/0018-foldops-package-acquisition-and-update-model.md),
[ADR-0019](../adr/0019-foldops-supervisor-provisioning-and-tls.md), and the
approved Milestone 3 engineering specification. It records validation evidence
and states the remaining gates before a public v0.1.0 release.

---

# Completion Status

```text
Milestone 3 network fleet provisioning implementation: COMPLETE
Milestone 3 network fleet provisioning validation:      COMPLETE
v0.1.0 fleet provisioning readiness:                    SATISFIED
Public v0.1.0 release eligibility:                      BLOCKED (metadata workflow)
```

Milestone 3 network fleet provisioning work is complete when issues #56 through
#65 are closed with matching implementation and validation evidence, and this
readiness review is committed.

The superseded combined-image USB installer issues (#23 through #35) are closed
and recorded in [ADR-0013](../adr/0013-combined-appliance-and-installer-image.md).
They are not part of the Milestone 3 closure matrix.

---

# Governing Specifications

GitHub Milestone 3 is titled **Network Fleet Provisioning**. The governing
approved documents for this milestone are:

| Document | Role |
| --- | --- |
| [3-engineering-spec.md](3-engineering-spec.md) | Concrete provisioning, registry, update, and validation contract |
| [ADR-0016](../adr/0016-network-provisioning-via-supervisor.md) | Supervisor-led network provisioning architecture |
| [ADR-0014](../adr/0014-fixed-installation-roles.md) | Fixed `supervisor` and `agent` roles |
| [ADR-0017](../adr/0017-official-release-publication-and-supervisor-upstream-polling.md) | Upstream release origin and supervisor registry polling |
| [ADR-0018](../adr/0018-foldops-package-acquisition-and-update-model.md) | Runtime FoldOps package acquisition |
| [ADR-0019](../adr/0019-foldops-supervisor-provisioning-and-tls.md) | Supervisor ingest token and TLS provisioning |

---

# Issue Closure Matrix

| Issue | Title | State | Primary evidence |
| --- | --- | --- | --- |
| #56 | Persist and validate installation role on direct-flash bootstrap | Closed | `packages/foldingosctl/src/provision_role.go`, `scripts/make-bootable-usb --role` |
| #57 | Implement supervisor image registry and upstream release polling | Closed | `packages/foldingosctl/src/registry*.go`, `foldingos-registry-poll.timer` |
| #58 | Implement agent registration and desired-version API | Closed | `provision_enrollment.go`, `provision_serve.go`, `foldingos-agent-register.service` |
| #59 | Implement supervisor HTTP image streaming to provisioned targets | Closed | `provision_stream_handlers.go`, `provision_install.go` |
| #60 | Implement PXE/iPXE network boot and enrollment workflow | Closed | `provision_boot.go`, `foldingos-provision-boot.service`, `doc/foldingosctl.md` |
| #61 | Implement agent staged OS update and reboot apply | Closed | `provision_update.go`, `foldingos-agent-apply-update.service`, GRUB update entry |
| #62 | Implement FoldOps supervisor administrator and TLS provisioning | Closed | `foldops_provision.go`, `foldops_tls.go`, `make-bootable-usb --foldops-ingest-token` |
| #72 | Implement FoldOps extract-only deb install layout and system user | Closed | `foldops_acquire.go`, `foldingos-foldops-acquire.timer` |
| #73 | Add pinned foldops.toml manifest and verify-foldops-manifest build gate | Closed | `overlay/usr/share/foldingos/manifests/foldops.toml`, `scripts/verify-foldops-manifest` |
| #63 | Implement QEMU network provisioning acceptance suite | Closed | `scripts/test-provision-qemu`, [validation/network-provision-qemu-0.1.0.json](../../validation/network-provision-qemu-0.1.0.json) |
| #64 | Validate network provisioning on physical SATA and NVMe targets | Closed | [validation/network-provision-physical-0.1.0.json](../../validation/network-provision-physical-0.1.0.json), `scripts/validate-agent-update-lab` |
| #65 | Finalize Milestone 3 network fleet provisioning documentation and readiness review | Closed | This document |

No Milestone 3 release-blocking issue remains open or deferred through an
approved document change.

---

# Architecture Reconciliation

Review confirms implementation matches [ADR-0016](../adr/0016-network-provisioning-via-supervisor.md):

- the first node is a `supervisor` installed by direct flash to internal NVMe or
  SATA
- additional nodes are `agent` nodes provisioned over UEFI PXE/iPXE
- TFTP carries only the iPXE bootstrap chain; release images stream over HTTP
- the supervisor maintains a local verified image registry
- agents register with the supervisor and receive SSH keys during install
- agents check desired image version on boot and apply staged full-image updates
- direct flash remains the recovery path when network provisioning fails
- USB-source network provisioning for internal disks is not implemented

Fixed roles match [ADR-0014](../adr/0014-fixed-installation-roles.md). Role
changes require destructive reinstallation.

No unresolved provisioning architecture decisions remain outside documented
future work (detached image signatures, A/B root slots, USB-source internal
install).

---

# Implementation Reconciliation

## Supervisor bootstrap

Implemented behavior matches [installer.md](../installer.md) and
[3-engineering-spec.md](3-engineering-spec.md):

- `scripts/make-bootable-usb` stages SSH keys, optional `--role supervisor`, and
  required `--foldops-ingest-token`
- first boot persists `installation-role`, imports SSH keys, acquires FoldOps
  packages, completes FoldOps TLS provisioning, and starts the provisioning
  control plane
- `foldingosctl registry import-bootstrap` and upstream polling populate the
  local registry from `releases.folding-os.com`

## Network agent provisioning

- `foldingosctl provision boot` serves proxy-DHCP, TFTP, and HTTP boot assets
- `foldingosctl provision allow-boot [--disk <device>] <mac>` gates blank-machine
  install script access and optionally pins the install target on multi-disk
  agents
- install initramfs runs `foldingosctl provision install` with supervisor
  authorization, target-disk validation, verified image streaming, inherited-state
  reset, and agent staging files
- internal SATA and NVMe targets require non-empty serial numbers; sysfs fallback
  supplements `lsblk` in the install initramfs

## Agent registration and updates

- `foldingos-agent-register.service` runs after `foldingos-identity.service`
- `foldingosctl provision enroll` registers the agent with the supervisor
- `foldingosctl provision check-version` stages verified updates
- `foldingosctl provision apply-update` schedules and applies offline update
  boots through the update initramfs
- failed offline apply records `apply_state=failed` and does not auto-retry on
  later normal boots

## FoldOps integration

- FoldOps packages are acquired at runtime from `deb.folding-os.com` per
  [ADR-0018](../adr/0018-foldops-package-acquisition-and-update-model.md)
- supervisor ingest token and self-signed TLS are provisioned per
  [ADR-0019](../adr/0019-foldops-supervisor-provisioning-and-tls.md)
- network-provisioned agents receive EFI ingest token and data-partition
  `supervisor-ca.pem` during install

---

# Documentation Reconciliation

| Topic | Document |
| --- | --- |
| Deployment workflow | [installer.md](../installer.md) |
| Operator procedures | [operations.md](../operations.md) |
| `foldingosctl` commands | [foldingosctl.md](../foldingosctl.md) |
| Update trust model and recovery | [update-system.md](../update-system.md) |
| Automated validation | [testing-strategy.md](../testing-strategy.md) |
| Foundation physical validation | [physical-validation.md](../physical-validation.md) |
| Validated hardware | [hardware-support.md](../hardware-support.md) |
| FoldOps runtime | [foldops-integration.md](../foldops-integration.md) |

Operator documentation now describes supervisor direct flash, network boot
allowlisting, multi-disk install pinning, registry refresh, agent update
validation, and network-install recovery.

---

# Validation Evidence

## Unit tests

```bash
cd packages/foldingosctl/src
go test ./...
```

## QEMU/OVMF network provisioning acceptance

```bash
./scripts/test-provision-qemu
```

Committed record:

```text
validation/network-provision-qemu-0.1.0.json
```

The suite validates supervisor registry bootstrap, enrollment gating, undersized
and corrupt image rejection, selected-disk-only network install, agent
registration, `/data` expansion, staged update apply, and Folding@home runtime
after update.

## Physical network provisioning acceptance

Validated platform:

```text
Dell OptiPlex Micro (wired Ethernet, UEFI)
```

Committed record:

```text
validation/network-provision-physical-0.1.0.json
```

Physical coverage:

| Test | Result |
| --- | --- |
| `supervisor_direct_flash` | pass |
| `agent_provision_nvme` | pass |
| `agent_provision_sata` | pass |
| `agent_registration_ssh` | pass |
| `desired_version_update` | pass |
| `fah_runtime_after_update` | pass |

Physical agents validated:

- NVMe target via automatic internal-disk selection (`folding-9da275`)
- SATA target on a dual-disk OptiPlex with install pinned to `/dev/sda`
  (`folding-6eac14`)

Lab helpers used during physical validation:

```bash
./scripts/refresh-supervisor-registry-lab <supervisor-host> <ssh-private-key>
./scripts/validate-agent-update-lab <supervisor-host> <agent-host> <ssh-private-key>
./scripts/run-physical-acceptance <agent-host> <ssh-private-key>
```

Foundation physical validation from Milestone 1 remains recorded separately in
`validation/appliance-physical-0.1.0.json`.

---

# Release Gate Status

| Gate | Milestone 3 provisioning | Public v0.1.0 release |
| --- | --- | --- |
| Supervisor bootstrap and registry | Satisfied | Required |
| Network agent provisioning | Satisfied | Required |
| Agent registration and staged updates | Satisfied | Required |
| FoldOps acquire and supervisor TLS | Satisfied | Required |
| QEMU network provisioning acceptance | Satisfied | Required |
| Physical SATA and NVMe provisioning | Satisfied | Required |
| Milestone 1 foundation validation | Satisfied (separate record) | Required |
| Milestone 2 Folding@home runtime validation | Satisfied (separate record) | Required |
| Documentation matches implementation | Satisfied by this review | Required |
| Release metadata `release_eligible: true` workflow | Not automated in `generate-build-artifacts` | Still manual |

Milestone 3 satisfies the network fleet provisioning readiness criteria for
v0.1.0. Publication still requires the project's release process to mark a tagged
revision and candidate image as publicly release eligible.

---

# Known Limitations

- CPU-only Folding@home in v0.1.0; GPU support is out of scope
- Milestone 3 update mechanism uses verified full-image replacement without A/B
  slots or automatic rollback
- Detached release-image signatures are planned; import trusts HTTPS origin and
  SHA-256 in v0.1.0
- Multi-disk agents require `allow-boot --disk` when automatic selection would
  choose the wrong internal target
- QEMU network installs without KVM may take hours on pure TCG
- FoldOps agent HTTPS trust to the supervisor depends on staged
  `supervisor-ca.pem` until upstream FoldOps accepts custom CA configuration
- Interrupted network installation may leave a target unbootable; re-run network
  provisioning after correcting the fault

---

# Milestone Boundary

Milestone 3 ends with a supervisor-managed fleet where:

- one `supervisor` node is installed by direct flash
- additional `agent` nodes network boot, install to internal SATA or NVMe, and
  register automatically
- the supervisor polls upstream releases and assigns desired image versions
- agents stage and apply verified operating-system updates across reboot
- FoldOps packages and Folding@home runtime continue operating after provisioning
  and update

Milestone 3 does not retroactively change the Milestone 1 foundation or Milestone
2 Folding@home runtime contracts.

---

# Review Outcome

| Acceptance criterion | Result |
| --- | --- |
| Documentation accurately describes supervisor bootstrap and network agent provisioning | Pass |
| All Milestone 3 release-blocking issues are closed with validation evidence | Pass |
| QEMU and physical validation results are recorded | Pass |
| Implementation reconciles with ADR-0016 and the Milestone 3 engineering specification | Pass |
| No unresolved provisioning architecture decisions remain | Pass |
| Milestone 3 completion and v0.1.0 fleet provisioning readiness are recorded | Pass |

**Milestone 3 Network Fleet Provisioning readiness review: PASS**

---

# Related Documents

- [Milestone 3 engineering specification](3-engineering-spec.md)
- [Milestone 1 readiness review](1-readiness-review.md)
- [Milestone 2 readiness review](2-readiness-review.md)
- [Deployment and provisioning](../installer.md)
- [Operations](../operations.md)
- [Documentation index](../README.md)
