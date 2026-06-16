# FoldingOS Roadmap

FoldingOS is being developed as a purpose-built appliance operating system for Folding@home compute nodes.

## Milestone 0: Documentation & Architecture

- Define the project vision
- Establish design principles
- Document system architecture
- Define the build approach
- Define initial hardware targets
- Define security and update strategy

## Milestone 1: Bootable Base System

Status: **Complete** (foundation scope, 2026-06-12)

- Build first x86_64 image
- Boot to shell
- Enable DHCP networking
- Enable SSH access
- Establish persistent storage layout
- Add basic service supervision

See [doc/milestone/1-readiness-review.md](doc/milestone/1-readiness-review.md)
for validation evidence and release-gate status.

Milestones 1 and 2 together define the first working v0.1.0 runtime scope.
Milestone 3 adds supervisor-led network fleet provisioning. Public v0.1.0
release publication remains blocked until all mandatory release gates in
[doc/milestone/1-engineering-spec.md](doc/milestone/1-engineering-spec.md) are
satisfied and release metadata finalization is implemented.

## Milestone 2: Folding@home Integration

Status: **Complete** (2026-06-13)

- Implement verified Folding@home client acquisition
- Configure automatic startup
- Persist work unit and checkpoint data
- Define FAH configuration management
- Add FAH health checks

See [doc/milestone/2-readiness-review.md](doc/milestone/2-readiness-review.md).

## Milestone 3: Network Fleet Provisioning

Status: **Complete** (2026-06-14)

- Bootstrap the first supervisor by direct flash to NVMe or SATA
- Add supervisor image registry and upstream release polling
- Provision agent nodes over UEFI PXE/iPXE with HTTP image transfer
- Register agents with the supervisor
- Assign fixed `agent` and `supervisor` roles at provision time
- Check desired image version on agent boot and stage updates
- Acquire FoldOps packages at runtime per [ADR-0018](doc/adr/0018-foldops-package-acquisition-and-update-model.md)
  (Milestone 3: `.deb` from `deb.folding-os.com`; Milestone 4: layout bundles from
  `packages.folding-os.com` per [ADR-0023](doc/adr/0023-runtime-foldops-and-foldingosctl-updates-without-os-reimage.md))
- Provision initial supervisor administrator and TLS configuration
- Validate network provisioning on approved SATA and NVMe targets

See [ADR-0016](doc/adr/0016-network-provisioning-via-supervisor.md),
[doc/milestone/3-engineering-spec.md](doc/milestone/3-engineering-spec.md), and
[doc/milestone/3-readiness-review.md](doc/milestone/3-readiness-review.md).

The superseded combined-image USB installer is recorded in
[ADR-0013](doc/adr/0013-combined-appliance-and-installer-image.md).

## Milestone 4: FoldOps Integration

Status: **In planning** (2026-06-14)

- Delegate node-local operations to `foldingosctl` instead of duplicating OS logic in FoldOps
- Add machine-readable `foldingosctl inspect` and `--format json` automation output
- Import FoldOps Rust source into `packages/foldops/` (monorepo; runtime still on `/data`)
- Adopt `layout-tar-zst` appliance transport and supervisor-assigned FoldOps/tools versions
- Add `foldingosctl tools acquire` for control-plane updates without OS reimage
- Refactor FoldOps agent ingest to collect inventory, health, FAH, and update state via `foldingosctl`
- Correlate FoldOps ingest with FoldingOS `node-id` and installation role
- Integrate FoldOps supervisor with local `foldingosctl provision` and `registry` fleet commands
- Support approved remote configuration workflows (starting with Folding@home policy)
- Validate integrated management on QEMU and physical hardware

See [ADR-0020](doc/adr/0020-foldops-delegates-node-operations-to-foldingosctl.md),
[ADR-0021](doc/adr/0021-machine-readable-foldingosctl-automation-interface.md),
[ADR-0022](doc/adr/0022-foldops-rust-source-in-foldingos-monorepo.md),
[ADR-0023](doc/adr/0023-runtime-foldops-and-foldingosctl-updates-without-os-reimage.md),
[doc/milestone/4-appliance-artifact-and-monorepo-plan.md](doc/milestone/4-appliance-artifact-and-monorepo-plan.md),
[doc/milestone/4-implementation-spec.md](doc/milestone/4-implementation-spec.md), and
[doc/milestone/4-engineering-spec.md](doc/milestone/4-engineering-spec.md).

Package acquisition, role-specific service activation, and supervisor TLS
provisioning were delivered in Milestone 3 per
[ADR-0018](doc/adr/0018-foldops-package-acquisition-and-update-model.md).
Runtime assignment and layout-bundle transport extend that model in Milestone 4.

## Milestone 5: Update System

- Define update model
- Evaluate A/B root filesystem design
- Add signed update bundles
- Add rollback behavior
- Report update status to FoldOps

## Milestone 6: Raspberry Pi Support

- Add ARM64 build target
- Support Raspberry Pi 5 boot process
- Validate Ethernet and storage
- Produce flashable Pi image

## Milestone 7: v1.0 Release

- Publish first stable x86_64 release
- Complete installation documentation
- Publish hardware compatibility list
- Publish security model
- Validate IPv4-only, IPv6-only, and dual-stack networking
- Verify stable release signatures
- Publish release images
