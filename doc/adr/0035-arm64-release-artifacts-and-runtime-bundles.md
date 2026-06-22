# ADR-0035: ARM64 Release Artifacts And Runtime Bundles

**Status:** Proposed

**Date:** 2026-06-22

**Authors:** FoldingOS project

**Depends on:** [ADR-0009](0009-fah-acquisition-and-update-model.md),
[ADR-0012](0012-reproducible-build-environment-and-verification.md),
[ADR-0017](0017-official-release-publication-and-supervisor-upstream-polling.md),
[ADR-0018](0018-foldops-package-acquisition-and-update-model.md),
[ADR-0023](0023-runtime-foldops-and-foldingosctl-updates-without-os-reimage.md),
[ADR-0029](0029-packages-channel-publication-via-rclone.md),
[ADR-0034](0034-raspberry-pi-5-boot-and-image-format.md)

**Related:** [Milestone 7 implementation specification](../milestone/7-implementation-spec.md),
[Milestone 7 engineering specification](../milestone/7-engineering-spec.md)

---

## Context

Milestone 7 adds an ARM64 platform target. The operating-system image is only
one part of the release surface. FoldOps layout bundles, `foldingosctl` tools
bundles, release indexes, checksums, signatures, and Folding@home acquisition
must become architecture-aware without breaking the x86_64 publication model.

Existing ADRs intentionally keep FoldOps and tools out of the OS image. That
model must continue for Pi 5. The Pi image should not embed FoldOps binaries,
`foldingosctl` runtime update payloads, or Folding@home client binaries just
because they are architecture-specific.

---

## Decision

Milestone 7 will make release publication and runtime acquisition explicitly
architecture-aware.

### 1. Architecture identifiers

Release metadata must distinguish at least:

- `x86_64`
- `aarch64`

The Pi 5 platform image should use both a platform identifier and an
architecture identifier where ambiguity matters:

```text
foldingos-raspberrypi5-aarch64-<version>.img
```

Generic runtime bundles may use only architecture when the same payload applies
to all platforms of that CPU architecture:

```text
foldops-supervisor-aarch64-<version>.tar.zst
foldops-agent-aarch64-<version>.tar.zst
foldingos-tools-aarch64-<version>.tar.zst
```

### 2. Release indexes

Official image and package indexes must include platform and architecture
fields. Supervisor polling must ignore artifacts that do not match the node or
fleet target architecture.

Minimum metadata fields for Milestone 7:

- artifact kind
- version
- platform when platform-specific
- architecture
- URL
- SHA-256 digest
- signature reference when signing is enabled
- minimum compatible OS version
- publication timestamp

### 3. Runtime acquisition

FoldOps and `foldingosctl` runtime updates continue to use the layout-bundle
model from ADR-0023. Milestone 7 adds `aarch64` builds and manifest entries.

Runtime acquisition must fail closed when an artifact architecture does not
match the running node.

### 4. Folding@home acquisition

The Folding@home client remains acquired from approved upstream infrastructure
per ADR-0009. Milestone 7 must verify whether a supported `aarch64` client and
required cores are available for the Pi 5 runtime before declaring Folding@home
operation complete.

If Folding@home ARM64 availability is incomplete, the Pi image may boot and
register for validation, but Milestone 7 cannot claim complete Pi Folding
support until the client acquisition path is documented and verified.

### 5. Reproducibility

ARM64 release images and ARM64 runtime bundles must meet the same reproducible
release expectations as existing x86_64 artifacts.

If the first ARM64 release cannot satisfy byte-identical independent clean
builds for all required artifacts, the exception must be documented in a
readiness review and must not be represented as a stable release artifact.

---

## Alternatives Considered

### Publish Pi artifacts outside the existing release channels

Rejected. A side channel would make supervisor polling, operator documentation,
and release verification inconsistent.

### Embed ARM64 FoldOps and tools in the Pi image

Rejected. This violates the runtime acquisition and update model accepted in
ADR-0018 and ADR-0023.

### Treat Raspberry Pi 5 as generic `aarch64` everywhere

Rejected for OS images because Pi boot assets are platform-specific. Accepted
for runtime bundles when the payload is truly architecture-only.

### Ship Pi image before Folding@home ARM64 verification

Allowed only as a development or validation artifact. It cannot satisfy
Milestone 7 readiness for a FoldingOS-supported Pi compute node.

---

## Consequences

### Positive

- preserves existing release architecture while adding ARM64
- prevents cross-architecture update mistakes
- keeps runtime acquisition consistent across platforms
- makes Folding@home ARM64 availability a visible release gate

### Negative

- release indexes and publication automation become more complex
- CI must build and verify additional artifacts
- supervisor update logic must reason about architecture compatibility
- Pi readiness may be blocked by upstream Folding@home ARM64 support

---

## Future Considerations

Future work may add:

- additional ARM64 platforms
- per-platform kernel or firmware compatibility metadata
- SBOM publication per architecture
- automated multi-architecture release promotion gates

---

## References

- [Issue #153: Plan Milestone 7](https://github.com/pacificnm/folding-os/issues/153)
- [ADR-0018: FoldOps Package Acquisition And Update Model](0018-foldops-package-acquisition-and-update-model.md)
- [ADR-0023: Runtime FoldOps And foldingosctl Updates Without OS Reimage](0023-runtime-foldops-and-foldingosctl-updates-without-os-reimage.md)
- [ADR-0029: Packages Channel Publication Via rclone](0029-packages-channel-publication-via-rclone.md)
