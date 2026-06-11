# ADR-0012: Reproducible Build Environment And Verification

**Status:** Accepted

**Version:** 1.2

**Date:** 2026-06-11

**Authors:** FoldingOS Project Contributors

---

# Context

FoldingOS requires reproducible release artifacts, but reproducibility is not
established merely by successfully rebuilding on one long-lived machine.

Buildroot uses tools supplied by the Linux host while also building most of the
target toolchain and host utilities it needs. Unrecorded host changes, source
changes, timestamps, caches, local overrides, or reused output directories can
therefore make builds differ.

The project has a dedicated Debian build system. That system is the primary
release builder, but the authoritative build definition must remain in the
repository rather than existing only as machine state.

---

# Decision

FoldingOS v0.1.0 will use Buildroot `2026.02.2`, selected and pinned from the
Buildroot 2026.02 LTS series.

The v0.1.0 build-host baseline is Debian 13 on x86_64 (`amd64`).

The initial dedicated builder reports:

```text
Linux 6.12.90+deb13.1-amd64
Debian kernel package 6.12.90-2
x86_64 GNU/Linux
```

The Debian major release and architecture are required build-host properties.
The exact running kernel and installed build-package versions are recorded in
release metadata. The verification environment need not use the identical
kernel patch version unless testing demonstrates that it affects required
release artifacts.

Release-candidate reproducibility is proven by two independent clean builds:

1. one build on the dedicated Debian release builder
2. one build in a clean disposable Debian environment created from the
   repository's documented build-host definition

Both builds must use:

- the same FoldingOS Git commit
- the same exact Buildroot release
- the same committed Buildroot configuration, overlays, patches, and scripts
- the same pinned and hash-verified source inputs
- Debian 13 on x86_64 with the documented build-host package manifest
- separate empty output directories
- no compiler cache
- no package source overrides
- no uncommitted source changes

The clean disposable Debian environment may be a virtual machine, physical
system, or digest-pinned Debian 13 x86_64 container. A container is an approved
independent verification environment even when its runtime executes on the
dedicated builder, provided it satisfies the container isolation requirements
below. The container supplies an independently created Debian userland and
must not reuse the dedicated builder's checkout, build outputs, or unverified
host state.

## Container Verification Environment

A container used as the second independent clean build must:

- use a Debian 13 x86_64 base image pinned by immutable image digest
- record the base-image reference and digest, container-runtime name and
  version, and installed Debian package versions
- install dependencies from the committed build-host package manifest
- build as an unprivileged user
- use a clean source checkout at the exact release-candidate Git commit
- use a new empty Buildroot output directory
- disable compiler caches and package source overrides
- receive no writable host checkout, prior build output, or developer-specific
  configuration
- receive only explicitly documented inputs and output locations

The container may receive the hash-verified source-download cache read-only.
Its completed artifact directory may be copied or written to the documented
verification handoff location.

The shared host kernel and container runtime do not invalidate independence for
v0.1.0. The Debian major release and architecture remain required build-host
properties; exact kernel and container-runtime versions are retained as
verification metadata for investigation.

---

# Dedicated Debian Builder

The dedicated Debian system is the primary release builder.

It must:

- use Debian 13 on x86_64
- install only documented build dependencies
- record the installed package versions used for every release build
- build as an unprivileged user
- use a clean source checkout at the release commit
- use a new empty Buildroot output directory for every verification build
- avoid developer-specific environment variables and local source overrides

The dedicated machine may keep a source-download cache, but may not reuse build
outputs for release verification.

---

# Buildroot Requirements

The Buildroot release tarball, signature, and digest must be pinned and
verified before use.

FoldingOS will:

- maintain project customizations outside the upstream Buildroot tree where
  practical
- commit the Buildroot defconfig and all project configuration
- use Buildroot's internal toolchain
- enable Buildroot reproducible-build support
- pin package versions and verify downloaded files with hashes
- use fixed numeric UIDs and GIDs for accounts that own persistent data
- prohibit package source override files in release builds
- prohibit `ccache` in reproducibility verification builds
- set build timestamps from the release source revision rather than wall-clock
  time
- generate release artifacts only from clean complete builds

The v0.1.0 Buildroot direction and wrapper commands are defined by the
[v0.1.0 engineering specification](../milestone/1-engineering-spec.md).

---

# Source Acquisition

All required build sources must be fetched before reproducibility verification.

The source set must be:

- version pinned
- hash verified
- recorded in release metadata
- sufficient for both verification builds

After source acquisition, verification builds should run without network access
where practical. A shared read-only download cache is permitted because source
integrity is verified, but each build must use its own empty output directory.

Unpinned branches, moving tags, unverified archives, and developer-local source
directories are prohibited in release builds.

---

# Measurable Success Condition

A release candidate is reproducible only when both independent clean builds
complete successfully and produce byte-identical required release artifacts.

Required matching artifacts for v0.1.0 are:

- `foldingos-x86_64-<version>.img`
- release version metadata
- artifact checksum manifest

The verification process will calculate SHA-256 digests for each required
artifact. Corresponding digests from both builds must match exactly.

The raw image must match byte for byte. Matching extracted files or equivalent
runtime behavior is not sufficient.

Any required-artifact mismatch blocks release publication.

---

# Difference Investigation

When required artifacts differ, the release remains blocked until the cause is
understood and corrected.

The verification process should retain:

- both artifact sets
- SHA-256 manifests
- source revision
- Buildroot version and digest
- Debian release and installed build-package versions
- Buildroot configuration
- build logs
- environment allowlist values
- a binary-difference report where practical

Approved releases must not use a mismatch allowlist.

---

# Release Metadata

Deterministic release metadata must record:

- FoldingOS version and Git commit
- source revision timestamp used for deterministic timestamps
- Buildroot version and verified digest
- Buildroot defconfig digest
- Debian major release
- build-host baseline architecture
- build-host package manifest
- required source-input digests
- required release-artifact digests

Hostnames, usernames, absolute workspace paths, and wall-clock build times must
not affect required release artifacts.

Each verification build must separately record its actual build-host kernel and
installed package versions. These per-build records are retained for
investigation but are not required byte-identical release artifacts.

The final reproducibility report records the verification result after both
required-artifact sets have matched.

---

# Failure Behavior

Build failure or artifact mismatch fails closed:

- no release is published
- mismatched artifacts are not signed as an official release
- diagnostics and metadata are retained for investigation
- correcting the issue requires two new independent clean builds

A successful build on the dedicated Debian system alone is not sufficient to
claim reproducibility.

---

# Alternatives Considered

## One Successful Build

Rejected because it demonstrates buildability, not reproducibility.

## Two Builds Reusing One Output Directory

Rejected because cached outputs can hide nondeterminism and missing
dependencies.

## Two Builds Only On The Dedicated Builder

Rejected as the final proof because hidden machine state can influence both
builds identically.

## Require A Separate Physical System Or Virtual Machine

Rejected because contributors may not have a second physical machine or enough
resources for a second full virtual machine. A digest-pinned container with an
independently created Debian userland, clean checkout, and empty output
directory provides the required independent clean-build check for v0.1.0.

## Permit Known Artifact Differences

Rejected for v0.1.0 required artifacts because the release criterion must remain
simple and measurable.

---

# Consequences

## Positive

- reproducibility has an objective pass/fail condition
- the dedicated Debian builder remains useful without becoming undocumented
  infrastructure
- clean independent builds expose hidden host dependencies
- release artifacts are traceable to source and build inputs
- mismatches block publication rather than becoming accepted ambiguity

## Negative

- release verification requires two full builds
- a disposable Debian verification environment must be maintained
- nondeterministic upstream packages may require patches
- strict byte-identical images require careful timestamp and filesystem-image
  control
- Buildroot LTS updates require deliberate pin updates and revalidation

---

# Related Documents

- [ADR-0001: Use Buildroot As The FoldingOS Build System](0001-use-buildroot.md)
- [Build System](../build-system.md)
- [Release Strategy](../release-strategy.md)
- [Testing Strategy](../testing-strategy.md)
- [v0.1.0 Scope Specification](../milestone/1-implementation-spec.md)
