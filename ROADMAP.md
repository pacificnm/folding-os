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

- Bootstrap the first supervisor by direct flash to NVMe or SATA
- Add supervisor image registry and upstream release polling
- Provision agent nodes over UEFI PXE/iPXE with HTTP image transfer
- Register agents with the supervisor
- Assign fixed `agent` and `supervisor` roles at provision time
- Check desired image version on agent boot and stage updates
- Validate network provisioning on approved SATA and NVMe targets

See [ADR-0016](doc/adr/0016-network-provisioning-via-supervisor.md) and
[doc/milestone/3-engineering-spec.md](doc/milestone/3-engineering-spec.md).

The superseded combined-image USB installer is recorded in
[ADR-0013](doc/adr/0013-combined-appliance-and-installer-image.md).

## Milestone 4: FoldOps Integration

- Define node registration workflow
- Define metrics reporting API
- Integrate pinned and verified FoldOps package artifacts
- Activate the fixed agent and supervisor service graphs
- Provision initial supervisor administrator and TLS configuration
- Add FoldingOS agent
- Add FoldOps supervisor and web services
- Support remote configuration from FoldOps
- Report node health and FAH status

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
