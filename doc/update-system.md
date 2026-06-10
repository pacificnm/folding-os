# FoldingOS Update System

Version: 0.1
Status: Draft

## Purpose

This document defines the scope, requirements, trust model, and unresolved decisions for the FoldingOS update system.

FoldingOS updates must prioritize reliability, integrity, rollback safety, and preservation of Folding@home work data.

## Scope

The update system covers:

- Operating system image updates
- FoldingOS agent updates
- Approved Folding@home workload-manifest updates
- Build/version metadata
- Update verification
- Rollback behavior
- Preservation of persistent data

The update system does not initially cover:

- General-purpose package management
- User-installed software
- Desktop software updates
- Arbitrary third-party packages

The Folding@home client is a separately managed workload. Nodes download
approved, pinned client artifacts directly from official Folding@home
infrastructure. FoldOps may coordinate approved manifest rollout but does not
serve the binaries. See
[ADR-0009](adr/0009-fah-acquisition-and-update-model.md).

## Requirements

The update system should:

- Verify update authenticity
- Verify update integrity
- Preserve node configuration
- Preserve Folding@home work/checkpoint data
- Support rollback after failed updates
- Report update status to FoldOps when available
- Avoid leaving nodes in a partially updated state

## Trust Model

FoldingOS nodes should only trust updates that are:

- Produced by the official build process
- Versioned
- Checksummed
- Cryptographically signed
- Retrieved from a trusted source

Nodes must not install unsigned or unverifiable updates.

## Update Philosophy

FoldingOS should prefer image-based updates over in-place package updates.

The operating system image is replaceable.

Persistent data is not.

Persistent data includes:

- Node identity
- FoldOps registration
- Folding@home configuration
- Folding@home work data
- Checkpoints
- Logs, depending on retention policy

## Future A/B Update Model

A future update system may use:

```text
boot
rootfs-a
rootfs-b
data
```

The node boots from one root filesystem while updating the inactive root filesystem.

If the updated system boots successfully and passes health checks, it is marked good.

If it fails, the system rolls back to the previous root filesystem.

## Health Checks

Post-update health checks may include:

- Successful boot
- Network availability
- Time synchronization
- FoldOps agent startup
- Folding@home startup
- Persistent data availability

## Rollback Requirements

Rollback should occur when:

- The updated system fails to boot
- Required services fail repeatedly
- Persistent data cannot be mounted
- Health checks fail

Rollback should not erase configuration or Folding work data.

## Unresolved Decisions

The following decisions require ADRs:

- Exact update framework
- A/B partition implementation
- Signing mechanism
- Update server model
- FoldOps-controlled rollout behavior
- External workload-manifest signing and publication mechanism
- Whether automatic updates are enabled by default
- Rollback health-check timeout
- Whether updates can be manually triggered over SSH

## Summary

The FoldingOS update system must be boring, safe, and predictable.

A failed update should not destroy a node.

A failed update should not destroy scientific work.

Reliability is more important than update convenience.
