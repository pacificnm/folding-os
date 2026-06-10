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

- Build first x86_64 image
- Boot to shell
- Enable DHCP networking
- Enable SSH access
- Establish persistent storage layout
- Add basic service supervision

Milestones 1 and 2 together define the first working v0.1.0 release scope.

## Milestone 2: Folding@home Integration

- Implement verified Folding@home client acquisition
- Configure automatic startup
- Persist work unit and checkpoint data
- Define FAH configuration management
- Add FAH health checks

## Milestone 3: FoldOps Integration

- Define node registration workflow
- Define metrics reporting API
- Add FoldingOS agent
- Support remote configuration from FoldOps
- Report node health and FAH status

## Milestone 4: Update System

- Define update model
- Evaluate A/B root filesystem design
- Add signed update bundles
- Add rollback behavior
- Report update status to FoldOps

## Milestone 5: Raspberry Pi Support

- Add ARM64 build target
- Support Raspberry Pi 5 boot process
- Validate Ethernet and storage
- Produce flashable Pi image

## Milestone 6: v1.0 Release

- Publish first stable x86_64 release
- Complete installation documentation
- Publish hardware compatibility list
- Publish security model
- Validate IPv4-only, IPv6-only, and dual-stack networking
- Verify stable release signatures
- Publish release images
