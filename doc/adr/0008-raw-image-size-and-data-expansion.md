# ADR-0008: Raw Image Size and Data-Partition Expansion

**Status:** Accepted

**Version:** 1.0

**Date:** 2026-06-10

**Authors:** FoldingOS Project Contributors

---

# Context

FoldingOS is distributed as a complete raw disk image that must boot from
storage devices of different capacities.

A fixed-size release image is required for reproducible builds and predictable
flashing. Leaving all storage at the image size would waste capacity needed for
Folding@home work, logs, configuration, and future operational state.

The persistent data partition is the final partition in the v0.1.0 layout, so it
can safely grow without moving the EFI or root partitions.

---

# Decision

The FoldingOS v0.1.0 x86_64 raw image will be 4 GiB.

Its initial GPT partition layout will be:

| Partition | Filesystem | Initial Size |
| --- | --- | --- |
| EFI System Partition | vfat | 512 MiB |
| Root filesystem | ext4 | 2 GiB |
| Persistent data | ext4 | Remaining image capacity |

The persistent data partition must be the final partition.

On boot, FoldingOS will expand the persistent data partition and its ext4
filesystem to the maximum usable aligned capacity of the boot device.

Target devices smaller than the raw image are unsupported.

---

# Expansion Behavior

Expansion will run automatically after the EFI partition is available and
before services that require persistent data start.

The expansion process will:

1. Identify the boot device and persistent data partition without assuming a
   fixed device name such as `/dev/sda`.
2. Confirm that the persistent data partition is the expected final partition.
3. Compare the current partition end with the device's maximum usable aligned
   sector.
4. Perform no action when the partition already occupies the available space.
5. Grow the partition without changing its start sector.
6. Preserve the partition identity and ext4 filesystem identity.
7. Grow the ext4 filesystem to fill the expanded partition.
8. Log the result without exposing sensitive data.

The implementation must never shrink a partition or filesystem.

---

# Safety And Failure Behavior

Expansion must be idempotent and safe to run on every boot.

Before changing the partition table, the implementation must validate:

- the expected GPT layout is present
- the persistent data partition exists
- the persistent data partition is the final partition
- its start sector matches the expected installed layout
- the target device is larger than or equal to the release image

If validation or expansion fails:

- the original partition start and filesystem data must remain intact
- FoldingOS must not format or recreate the persistent filesystem
- the failure must be logged clearly
- the node may continue using the original data-partition capacity when it can
  be mounted safely
- repeated boots may retry expansion

If safe continuation is not possible, services that write persistent data must
not start.

---

# Build And Release Requirements

The raw image must contain a valid, mountable initial persistent data
filesystem.

Release validation must verify:

- the image size is exactly 4 GiB
- the image boots without expansion on a 4 GiB virtual target
- the data partition expands on larger targets
- expansion consumes all available aligned capacity
- the expanded filesystem remains mountable
- existing files survive expansion
- repeated expansion attempts make no further changes

The concrete partition and filesystem resize tooling is defined by the
[v0.1.0 engineering specification](../milestone/1-engineering-spec.md).

---

# Recovery

Expansion changes only the final persistent data partition and its filesystem.

Recovery tooling should be able to inspect the GPT and ext4 filesystem using
standard Linux utilities.

Expansion failure must not trigger automatic formatting or deletion of
persistent data.

---

# Alternatives Considered

## Fixed-Size Data Partition

Rejected because it wastes available storage and unnecessarily constrains
Folding@home work and logs.

## Expand The Root Filesystem

Rejected because mutable capacity belongs in the persistent data partition, not
the operating-system partition.

## Require Users To Expand Storage Manually

Rejected because it conflicts with the appliance deployment model.

## Build Images For Multiple Disk Sizes

Rejected because it increases release complexity without improving the runtime
architecture.

## Create The Data Partition On First Boot

Rejected because release images should contain a valid fallback data filesystem
that remains usable if expansion cannot occur.

---

# Consequences

## Positive

- one release image works across different device capacities
- all available storage becomes usable for persistent data
- release-image size remains deterministic
- root and EFI partitions remain fixed and predictable
- failed expansion can fall back to the original data capacity

## Negative

- first boot modifies the partition table and filesystem
- resize tooling becomes part of the runtime image
- expansion behavior requires destructive-operation safety testing
- future layouts must preserve or deliberately supersede the final-partition
  assumption

---

# Future Considerations

Future A/B layouts or encrypted data partitions may require a new expansion
decision.

Any replacement must preserve the principles that expansion is automatic,
idempotent, non-shrinking, and protective of persistent data.

---

# Related Documents

- [ADR-0003: x86_64 Bootloader and Image Format](0003-x86_64-bootloader-and-image-format.md)
- [ADR-0004: Partition and Persistence Layout](0004-partition-and-persistence-layout.md)
- [Installer](../installer.md)
- [v0.1.0 Scope Specification](../milestone/1-implementation-spec.md)

---

# Closing Statement

FoldingOS ships one small, reproducible image and automatically makes all
remaining device capacity available to persistent data.
