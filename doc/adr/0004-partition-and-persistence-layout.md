# ADR-0004: Partition and Persistence Layout

**Status:** Accepted

**Version:** 1.0

**Date:** 2026-06-10

**Authors:** FoldingOS Project Contributors

---

# Context

FoldingOS is designed as an appliance operating system dedicated to
Folding@home.

The storage architecture must support:

* predictable deployment
* simple recovery
* preservation of scientific work
* future update mechanisms
* future rollback capability
* long-term maintainability

The storage model should remain understandable and avoid unnecessary
complexity.

---

# Decision

FoldingOS will separate operating system components from persistent node data.

The operating system should be considered disposable.

Persistent node state should not.

For Milestone 1, the recommended logical layout is:

```text
+------------------------------------------------------+
| EFI System Partition                                 |
+------------------------------------------------------+

+------------------------------------------------------+
| FoldingOS Root Filesystem                            |
+------------------------------------------------------+

+------------------------------------------------------+
| Persistent Data                                      |
|                                                      |
| - Node Configuration                                 |
| - Folding@home Configuration                         |
| - Folding@home Work Units                            |
| - Folding Checkpoints                                |
| - Logs                                               |
+------------------------------------------------------+
```

Future A/B root filesystems are explicitly anticipated but are not required
for the initial implementation.

For v0.1.0, the EFI partition is 512 MiB, the root partition is 2 GiB, and the
ext4 persistent data partition consumes the remaining capacity of the 4 GiB
release image. On boot, the final data partition expands to the maximum usable
capacity of the boot device as defined by
[ADR-0008](0008-raw-image-size-and-data-expansion.md).

---

# Design Philosophy

The operating system image may be replaced.

Persistent scientific work should survive.

Node identity should survive.

Configuration should survive.

This separation enables future image-based updates without unnecessary loss
of state.

---

# EFI System Partition

Responsibilities include:

* UEFI boot files
* GRUB
* kernel loading
* boot metadata

Users should rarely interact with this partition.

---

# Root Filesystem

The root filesystem contains:

* operating system
* runtime libraries
* system binaries
* service definitions
* core utilities

The root filesystem should be treated as immutable whenever practical.

Routine user modification is discouraged.

Future releases may enforce read-only operation.

---

# Persistent Data Partition

Persistent storage contains node-specific state.

Examples include:

## Configuration

* hostname

* networking

* SSH configuration

* local settings

Future FoldOps integration may add persistent FoldOps configuration and state.

---

## Folding@home State

* work units

* checkpoints

* runtime state

Scientific work should survive reboot whenever possible.

---

## Logs

Persistent logs should support:

* diagnostics

* troubleshooting

* recovery

Retention policies should balance operational usefulness with storage
consumption.

For v0.1.0, persistent journal storage and its bounded retention policy are
defined by [ADR-0010](0010-persistent-logging-and-retention.md).

---

## Future Metadata

Future persistent state may include:

* update history

* rollback history

* health history

* local inventory

* node certificates

---

# Persistence Philosophy

Persistent data should outlive operating system replacement.

Reinstalling FoldingOS should not require rebuilding node identity or losing
scientific work unless explicitly requested.

---

# Filesystem Philosophy

Filesystem implementation remains independent from logical architecture.

For v0.1.0:

* the root filesystem uses ext4
* the persistent data filesystem uses ext4
* the EFI System Partition uses vfat

Selection should prioritize:

* reliability

* maintainability

* operational simplicity

Future releases may select different filesystems through a superseding ADR.

---

# Mount Philosophy

Operating system components should remain separate from mutable runtime data.

Future implementations should avoid unnecessary writes to the operating
system partition.

Runtime state should be directed toward appropriate persistent or temporary
storage.

---

# Future A/B Support

Future releases may adopt:

```text
EFI

↓

Root A

↓

Root B

↓

Persistent Data
```

The inactive root filesystem would be updated while the active system
continues operating.

Successful validation would activate the new root.

Failure would automatically roll back to the previous version.

The current storage architecture intentionally preserves compatibility with
this future design.

---

# Recovery Philosophy

Recovery should preserve:

* node identity

* configuration

* Folding checkpoints

* Folding work units

* operational history where practical

Replacing the operating system should not unnecessarily destroy scientific
progress.

---

# Alternatives Considered

## Single Partition

Advantages:

* simple layout

Disadvantages:

* poor separation of concerns

* difficult upgrades

* difficult rollback

* difficult recovery

Decision:

Rejected.

---

## Full A/B Partition Layout for v1.0

Advantages:

* robust rollback

* image replacement

* strong update safety

Disadvantages:

* additional implementation complexity

* increased storage requirements

* delayed initial release

Decision:

Deferred.

Architecture should support future adoption without requiring immediate
implementation.

---

## Stateless Architecture

Advantages:

* simplified operating system

Disadvantages:

* loss of Folding checkpoints

* loss of configuration

* poor operational experience

Decision:

Rejected.

Persistent scientific work is a primary project objective.

---

# Consequences

## Positive

* preserves scientific work

* simplifies recovery

* supports future image updates

* supports future rollback

* separates immutable and mutable data

* aligns with appliance philosophy

## Negative

* additional partition complexity

* installer must understand persistent storage

* future migration planning required

These tradeoffs are acceptable.

---

# Future Review

Future ADRs may define:

* A/B implementation

* snapshot strategy

* rollback implementation

* persistent encryption

These decisions should extend rather than replace this architecture.

---

# Related Documents

* [Project charter](../../PROJECT_CHARTER.md)

* [Engineering principles](../../PRINCIPLES.md)

* [Storage layout](../storage-layout.md)

* [Update system](../update-system.md)

* [Installer](../installer.md)

* [ADR-0001: Use Buildroot](0001-use-buildroot.md)

* [ADR-0003: x86_64 Bootloader and Image Format](0003-x86_64-bootloader-and-image-format.md)

* [ADR-0008: Raw Image Size and Data-Partition Expansion](0008-raw-image-size-and-data-expansion.md)

* [ADR-0010: Persistent Logging And Retention](0010-persistent-logging-and-retention.md)

---

# Closing Statement

The operating system is replaceable.

Scientific work is not.

By separating immutable system components from persistent node state,
FoldingOS establishes a storage architecture that supports reliable
operation today while providing a clear path toward future image-based
updates and rollback capabilities without unnecessary complexity.
