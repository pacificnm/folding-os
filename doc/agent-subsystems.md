# Agent Subsystem Guide

Status: Living Document

This guide helps implementation agents find the right project context before
editing code. It summarizes navigation only. It does not override the project
charter, engineering principles, accepted ADRs, approved implementation
specifications, or subsystem documentation.

When this guide and a governing document disagree, the governing document wins.
When approved documents conflict, stop implementation and surface the conflict.

---

# Required Start

Before code changes:

1. Query project memory for the current milestone, affected subsystem, related
   architecture decisions, known issues, and build or packaging instructions.
2. Read [ai-context.md](ai-context.md) and this guide.
3. Read the subsystem documents and ADRs listed for the affected area.
4. Check document status. Proposed ADRs and proposed specifications are useful
   context, but do not silently promote them to accepted decisions.
5. Inspect the existing code and scripts.
6. Make focused changes and run the smallest verification set that covers the
   affected behavior.

Use this memory search shape as a starting point:

```text
current milestone <subsystem> architecture decisions known issues build tests packaging
```

---

# Source Precedence

Use the binding order in `AGENTS.md` and the document roles in
[README.md](README.md) and [ai-context.md](ai-context.md):

1. `doc/` specifications and governing project documents
2. Project memory
3. `AGENTS.md`
4. `DECISIONS.md`
5. `KNOWN_ISSUES.md`
6. Existing code behavior

Do not introduce a new architecture pattern because it is common practice. Do
not introduce Buildroot external-tree architecture unless an accepted ADR or
approved engineering specification authorizes it.

---

# Subsystem Map

## Buildroot Image And Release Artifacts

Use this area for image generation, Buildroot configuration, overlay contents,
release artifacts, and reproducibility.

Read:

- [build-system.md](build-system.md)
- [operations.md](operations.md)
- [milestone/1-engineering-spec.md](milestone/1-engineering-spec.md)
- [adr/0001-use-buildroot.md](adr/0001-use-buildroot.md)
- [adr/0012-reproducible-build-environment-and-verification.md](adr/0012-reproducible-build-environment-and-verification.md)

Primary paths:

- `configs/foldingos_x86_64_defconfig`
- `configs/linux-x86_64.config`
- `overlay/`
- `packages/*/Config.in`
- `scripts/build`
- `scripts/fetch-sources`
- `scripts/check-host-tools`

Verification:

```bash
./scripts/check-host-tools
./scripts/build
./scripts/verify-systemd-graph build/output/images/rootfs.tar
./scripts/verify-config build/output/images/rootfs.tar
./scripts/verify-reproducible
```

Run full reproducibility only when release eligibility or build output
determinism is in scope.

## Foundation Runtime

Use this area for boot status, data partition expansion, persistent directories,
bounded logging, SSH provisioning, base networking, and TOML configuration.

Read:

- [operations.md](operations.md)
- [boot-process.md](boot-process.md)
- [storage-layout.md](storage-layout.md)
- [networking.md](networking.md)
- [foldingosctl.md](foldingosctl.md)
- [adr/0004-partition-and-persistence-layout.md](adr/0004-partition-and-persistence-layout.md)
- [adr/0005-configuration-ownership-and-precedence.md](adr/0005-configuration-ownership-and-precedence.md)
- [adr/0007-first-boot-administrator-and-ssh-provisioning.md](adr/0007-first-boot-administrator-and-ssh-provisioning.md)
- [adr/0008-raw-image-size-and-data-expansion.md](adr/0008-raw-image-size-and-data-expansion.md)
- [adr/0010-persistent-logging-and-retention.md](adr/0010-persistent-logging-and-retention.md)
- [adr/0011-toml-configuration-validation-and-migration.md](adr/0011-toml-configuration-validation-and-migration.md)

Primary paths:

- `packages/foldingosctl/src/boot_cmd.rs`
- `packages/foldingosctl/src/config/`
- `packages/foldingosctl/src/config_cmd.rs`
- `packages/foldingosctl/src/identity.rs`
- `packages/foldingosctl/src/provision/ssh.rs`
- `packages/foldingosctl/src/storage.rs`
- `overlay/etc/foldingos/defaults/`
- `overlay/etc/systemd/`
- `overlay/usr/lib/systemd/system/`
- `overlay/usr/lib/tmpfiles.d/`

Verification:

```bash
cd packages/foldingosctl && cargo test
./scripts/verify-config build/output/images/rootfs.tar
./scripts/verify-persistent-logging build/output/images/rootfs.tar
./scripts/test-qemu
```

## Folding@home Runtime

Use this area for Folding@home manifest validation, acquisition, activation,
runtime configuration rendering, service startup, and physical acceptance.

Read:

- [operations.md](operations.md#foldinghome-runtime)
- [foldingosctl.md](foldingosctl.md)
- [milestone/2-engineering-spec.md](milestone/2-engineering-spec.md)
- [milestone/2-readiness-review.md](milestone/2-readiness-review.md)
- [adr/0006-fah-packaging-and-privilege-model.md](adr/0006-fah-packaging-and-privilege-model.md)
- [adr/0009-fah-acquisition-and-update-model.md](adr/0009-fah-acquisition-and-update-model.md)

Primary paths:

- `packages/foldingosctl/src/fah/`
- `overlay/usr/share/foldingos/manifests/fah.toml`
- `overlay/usr/lib/systemd/system/foldingos-fah-*`
- `overlay/usr/lib/systemd/system/folding-at-home.service`

Verification:

```bash
cd packages/foldingosctl && cargo test
./scripts/verify-fah-manifest build/output/images/rootfs.tar
./scripts/test-qemu
./scripts/run-physical-acceptance <host> <ssh-private-key> [port]
```

## foldingosctl CLI And Automation

Use this area for the Rust CLI, command reference behavior, setuid policy,
automation JSON, inspection commands, and FoldOps delegation boundaries.

Read:

- [foldingosctl.md](foldingosctl.md)
- [foldops-integration.md](foldops-integration.md)
- [milestone/4-engineering-spec.md](milestone/4-engineering-spec.md)
- [adr/0020-foldops-delegates-node-operations-to-foldingosctl.md](adr/0020-foldops-delegates-node-operations-to-foldingosctl.md)
- [adr/0021-machine-readable-foldingosctl-automation-interface.md](adr/0021-machine-readable-foldingosctl-automation-interface.md)
- [adr/0024-foldops-supervisor-fleet-mutation-authorization.md](adr/0024-foldops-supervisor-fleet-mutation-authorization.md)
- [adr/0025-implement-foldingosctl-in-rust.md](adr/0025-implement-foldingosctl-in-rust.md)

Primary paths:

- `packages/foldingosctl/src/main.rs`
- `packages/foldingosctl/src/cli.rs`
- `packages/foldingosctl/src/automation.rs`
- `packages/foldingosctl/src/automation_policy.rs`
- `packages/foldingosctl/src/inspect/`
- `packages/foldingosctl/src/setuid_privilege.rs`
- `overlay/usr/share/foldingos/foldops-*-automation.toml`
- `packages/foldingosctl/Cargo.toml`

Verification:

```bash
cd packages/foldingosctl && cargo test
./scripts/test-api-json --foldingosctl packages/foldingosctl/target/debug/foldingosctl
./scripts/test-qemu
```

## Network Fleet Provisioning And OS Updates

Use this area for supervisor direct flash, network boot, agent enrollment,
registry image management, desired OS image assignment, and agent apply-update.

Read:

- [installer.md](installer.md)
- [update-system.md](update-system.md)
- [operations.md](operations.md#network-fleet-provisioning)
- [milestone/3-engineering-spec.md](milestone/3-engineering-spec.md)
- [milestone/3-readiness-review.md](milestone/3-readiness-review.md)
- [adr/0014-fixed-installation-roles.md](adr/0014-fixed-installation-roles.md)
- [adr/0016-network-provisioning-via-supervisor.md](adr/0016-network-provisioning-via-supervisor.md)
- [adr/0017-official-release-publication-and-supervisor-upstream-polling.md](adr/0017-official-release-publication-and-supervisor-upstream-polling.md)

Primary paths:

- `packages/foldingosctl/src/provision/`
- `packages/foldingosctl/src/registry_*`
- `packages/foldingosctl/src/registry_cmd.rs`
- `packages/foldingosctl/src/enrollment.rs`
- `packages/foldingosctl/src/assignments.rs`
- `overlay/usr/lib/systemd/system/foldingos-provision*`
- `overlay/usr/lib/systemd/system/foldingos-agent-*`
- `scripts/test-provision-qemu`
- `scripts/validate-agent-update-lab`

Verification:

```bash
cd packages/foldingosctl && cargo test
./scripts/test-provision-qemu
./scripts/validate-agent-update-lab <supervisor-host> <agent-host> <ssh-private-key>
```

## FoldOps Runtime And Dashboard

Use this area for the Rust FoldOps agent, supervisor, shared types, dashboard,
bundle layout, ingest, machine controls, logs, and admin pages.

Read:

- [foldops-integration.md](foldops-integration.md)
- [foldops-components.md](foldops-components.md) — Rust/React module and API map
- [packages/foldops/README.md](../packages/foldops/README.md)
- [milestone/4-appliance-artifact-and-monorepo-plan.md](milestone/4-appliance-artifact-and-monorepo-plan.md)
- [milestone/4-engineering-spec.md](milestone/4-engineering-spec.md)
- [adr/0018-foldops-package-acquisition-and-update-model.md](adr/0018-foldops-package-acquisition-and-update-model.md)
- [adr/0019-foldops-supervisor-provisioning-and-tls.md](adr/0019-foldops-supervisor-provisioning-and-tls.md)
- [adr/0022-foldops-rust-source-in-foldingos-monorepo.md](adr/0022-foldops-rust-source-in-foldingos-monorepo.md)
- [adr/0023-runtime-foldops-and-foldingosctl-updates-without-os-reimage.md](adr/0023-runtime-foldops-and-foldingosctl-updates-without-os-reimage.md)
- [adr/0026-foldops-dashboard-operator-authentication.md](adr/0026-foldops-dashboard-operator-authentication.md)
- [adr/0027-foldops-remote-operator-api.md](adr/0027-foldops-remote-operator-api.md)

Primary paths:

- `packages/foldops/crates/foldops-agent/`
- `packages/foldops/crates/foldops-supervisor/`
- `packages/foldops/crates/foldops-types/`
- `packages/foldops/web/src/`
- `packages/foldops/packaging/appliance-bundle/`
- `scripts/build-foldops-bundles`
- `overlay/usr/lib/systemd/system/foldingos-foldops-*`

Verification:

```bash
cd packages/foldops && cargo test --workspace
cd packages/foldops/web && npm run build
./scripts/build-foldops-bundles --manifest-release <release>
./scripts/verify-foldops-manifest build/output/images/rootfs.tar
```

## Milestone 5 Software Updates, Publication, And Recovery

Use this area for supervisor-led FoldOps/tools update discovery, assignment,
apply APIs, rclone publication, package indexes, supervisor backup export, and
recovery import.

Read:

- [milestone/5-implementation-spec.md](milestone/5-implementation-spec.md)
- [milestone/5-engineering-spec.md](milestone/5-engineering-spec.md)
- [update-system.md](update-system.md#fleet-software-updates-foldops-and-foldingosctl)
- [operations.md](operations.md#packages-channel-publication-milestone-5)
- [adr/0028-supervisor-fleet-software-update-workflow.md](adr/0028-supervisor-fleet-software-update-workflow.md)
- [adr/0029-packages-channel-publication-via-rclone.md](adr/0029-packages-channel-publication-via-rclone.md)
- [adr/0030-supervisor-recovery-backup-and-export.md](adr/0030-supervisor-recovery-backup-and-export.md)

Primary paths:

- `packages/foldops/crates/foldops-supervisor/`
- `packages/foldops/crates/foldops-agent/`
- `packages/foldops/web/src/pages/admin/`
- `packages/foldingosctl/src/tools/`
- `packages/foldingosctl/src/recovery/`
- `packages/foldingosctl/src/inspect/tools.rs`
- `packages/foldingosctl/src/inspect/foldops.rs`
- `scripts/build-foldops-bundles`
- `scripts/build-foldingosctl-release`
- `scripts/publish-foldops-bundles`
- `scripts/publish-foldingos-tools`
- `scripts/publish-packages-release`
- `scripts/test-tools-acquire-qemu`
- `overlay/usr/lib/tmpfiles.d/foldingos.conf`

Verification:

```bash
cd packages/foldingosctl && cargo test
cd packages/foldops && cargo test --workspace
cd packages/foldops/web && npm run build
./scripts/publish-packages-release --foldops <release> --tools <version> --dry-run
./scripts/test-api-json --foldingosctl packages/foldingosctl/target/debug/foldingosctl
./scripts/test-tools-acquire-qemu
```

Use real rclone publication commands only when the task explicitly covers live
publication and the operator environment is configured.

## Supervisor USB And Physical Validation

Use this area for live supervisor USB preparation, direct-flash validation, and
lab acceptance.

Read:

- [operations.md](operations.md)
- [physical-validation.md](physical-validation.md)
- [BUILD_COMMANDS.md](../BUILD_COMMANDS.md)
- [adr/0013-combined-appliance-and-installer-image.md](adr/0013-combined-appliance-and-installer-image.md)
- [adr/0016-network-provisioning-via-supervisor.md](adr/0016-network-provisioning-via-supervisor.md)

Primary paths:

- `scripts/make-bootable-usb`
- `scripts/run-physical-acceptance`
- `scripts/verify-physical-validation-record`
- `validation/`

Verification:

```bash
./scripts/run-physical-acceptance <host> <ssh-private-key> [port]
./scripts/verify-physical-validation-record \
  validation/appliance-physical-0.1.0.json \
  build/output/images/foldingos-x86_64-0.1.0.img
```

`scripts/make-bootable-usb` is destructive to the selected whole-disk device.
Do not run it unless the user explicitly asks for media preparation and confirms
the target device.

---

# Verification Selector

For documentation-only changes, run a lightweight link/path sanity check and
inspect the rendered Markdown when practical.

For Rust-only changes:

```bash
cd packages/foldingosctl && cargo test
cd packages/foldops && cargo test --workspace
```

Run the command for the workspace you touched.

For FoldOps web changes:

```bash
cd packages/foldops/web && npm run build
```

For overlay or systemd changes:

```bash
./scripts/build
./scripts/verify-systemd-graph build/output/images/rootfs.tar
./scripts/test-qemu
```

For package publication changes:

```bash
./scripts/publish-packages-release --foldops <release> --tools <version> --dry-run
```

For runtime update and recovery behavior, combine the unit/build checks with the
QEMU or lab test listed in the affected subsystem.
