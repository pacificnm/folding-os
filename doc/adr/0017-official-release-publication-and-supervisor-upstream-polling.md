# ADR-0017: Official Release Publication And Supervisor Upstream Polling

**Status:** Accepted

**Version:** 1.1

**Date:** 2026-06-15

**Revision 1.1 (2026-06-15):** Official object prefix is `/release/` on
`releases.folding-os.com` (replaces `/foldingos/`).

**Authors:** FoldingOS Project Contributors

**Amends:** [ADR-0016](0016-network-provisioning-via-supervisor.md) (upstream release origin)

**Amended by:** [ADR-0023](0023-runtime-foldops-and-foldingosctl-updates-without-os-reimage.md)
(adds `packages.folding-os.com` publication channels for FoldOps bundles and
`foldingosctl` tools; OS image channel unchanged)

---

# Context

Milestone 3 supervisors maintain a local registry of approved FoldingOS release
images and poll an upstream server for newly published versions
([ADR-0016](0016-network-provisioning-via-supervisor.md)). Agent nodes never
download operating-system images directly from the public internet; they stage
updates from the supervisor registry or a supervisor-authorized redirect.

The project already publishes FoldOps Debian packages from project-controlled
HTTPS infrastructure on Cloudflare:

- `https://deb.folding-os.com` — apt repository and archive keyring
- documented at [https://www.folding-os.com/foldops](https://www.folding-os.com/foldops)

FoldingOS operating-system release images require a parallel, documented
publication channel. Prior documentation described an anonymous “upstream release
server” without naming the official hostname, manifest contract, or publication
workflow. `foldingosctl registry poll` is implemented, but no default upstream
URL ships on supervisor images and no publication pipeline is defined.

---

# Decision

FoldingOS will publish official release disk images and a machine-readable
releases manifest on Cloudflare-backed HTTPS infrastructure under the
`folding-os.com` domain family.

## Official upstream origin

All stable-channel objects live under the `/release/` path prefix on
`releases.folding-os.com`:

```text
releases.folding-os.com/release/
├── releases.json
├── foldingos-archive-keyring.gpg          (future signing)
└── images/
    ├── foldingos-x86_64-<version>.img
    ├── foldingos-x86_64-<version>.img.sha256   (optional)
    └── foldingos-x86_64-<version>.json         (optional metadata)
```

| Purpose | URL |
| --- | --- |
| Releases manifest | `https://releases.folding-os.com/release/releases.json` |
| Release disk image | `https://releases.folding-os.com/release/images/foldingos-x86_64-<version>.img` |
| Optional checksum sidecar | `https://releases.folding-os.com/release/images/foldingos-x86_64-<version>.img.sha256` |
| Future archive keyring | `https://releases.folding-os.com/release/foldingos-archive-keyring.gpg` |

The manifest URL is the single entry point for supervisor polling. Image URLs in
the manifest must be HTTPS URLs under `https://releases.folding-os.com/release/`
for stable-channel publications.

Supervisor appliances ship with the manifest URL recorded at:

```text
/data/config/provision/upstream-releases.url
```

`foldingos-registry-poll.timer` invokes `foldingosctl registry poll`, which
reads that file, fetches the manifest, downloads any new release entries, verifies
SHA-256, and stores verified images under `/data/registry/`.

## Manifest contract (schema version 1)

The manifest JSON matches the `upstreamReleasesManifest` structure implemented by
`foldingosctl registry poll`:

```json
{
  "schema_version": 1,
  "releases": [
    {
      "foldingos_version": "0.2.0",
      "git_revision": "<40-char-git-commit>",
      "image_url": "https://releases.folding-os.com/release/images/foldingos-x86_64-0.2.0.img",
      "image_sha256": "<lowercase-hex-sha256>",
      "image_size_bytes": 4294967296,
      "metadata_url": "https://releases.folding-os.com/release/images/foldingos-x86_64-0.2.0.json",
      "checksum_url": "https://releases.folding-os.com/release/images/foldingos-x86_64-0.2.0.img.sha256"
    }
  ]
}
```

Rules:

- `schema_version` must be `1`.
- Every listed release must include `foldingos_version`, `git_revision`,
  `image_url`, `image_sha256`, and `image_size_bytes`.
- `image_url` must use HTTPS. Redirects are not followed during download.
- Published `image_sha256` and `image_size_bytes` must match the object stored at
  `image_url`.
- The manifest lists all currently published stable releases. Supervisors import
  only entries not already present in the local registry (or with matching
  digest).

## Trust model (Milestone 3)

For Milestone 3 stable-channel publication:

1. Images are produced only from reproducible release builds that satisfy
   [ADR-0012](0012-reproducible-build-environment-and-verification.md).
2. Publication uses HTTPS to `releases.folding-os.com` (Cloudflare TLS).
3. Supervisors verify `image_sha256` and `image_size_bytes` before marking a
   registry entry `ready`.
4. Agents install only images present in the supervisor registry after
   supervisor-mediated authorization.

Detached image signatures and manifest signing are planned enhancements. They
are not required for Milestone 3 polling, but the keyring URL is reserved to
mirror the FoldOps apt trust model when signing is enabled.

## Publication workflow

Official stable releases are published by automation that:

1. completes the ADR-0012 reproducibility gate for required artifacts
2. uploads `foldingos-x86_64-<version>.img` and sidecars to
   `release/images/` in Cloudflare R2 or equivalent object storage behind
   `releases.folding-os.com`
3. updates `releases.json` atomically (replace object or versioned publish +
   CDN purge)
4. records the publication event in release metadata and changelog

Manual lab registration (for example `validate-agent-update-lab` copying a local
build image) remains a development-only path and is not part of the official
publication contract.

## Relationship to FoldOps apt packages

| Channel | Host | Consumer | Artifact |
| --- | --- | --- | --- |
| FoldOps packages | `deb.folding-os.com` | `apt` on Debian; `foldingosctl foldops acquire` on FoldingOS | `.deb` packages |
| FoldingOS images | `releases.folding-os.com` | supervisor `registry poll` | raw `.img` disk images |

FoldOps packages and FoldingOS images are distributed through separate URLs and
verification paths. A node may use both channels independently.

## Supervisor bootstrap unchanged

The first supervisor image still bootstraps its registry from the flashed
release through `registry import-bootstrap`. Upstream polling adds newer
published versions after the supervisor has network access.

---

# Alternatives Considered

## GitHub Releases as primary origin

Rejected as the stable production origin because fleet supervisors need a
fixed, operator-friendly hostname, predictable cache behavior, and publication
decoupled from source-hosting UI limits. GitHub Releases may remain a mirror or
build artifact archive, not the authoritative poll target.

## Agents pull directly from releases.folding-os.com

Rejected. [ADR-0016](0016-network-provisioning-via-supervisor.md) requires
supervisor-mediated staging so fleets can assign versions, fail closed, and
operate without per-node internet policy complexity.

## Embed only FoldOps apt and reuse apt for OS images

Rejected. FoldingOS ships raw GPT disk images, not Debian packages. The apt
infrastructure at `deb.folding-os.com` remains scoped to FoldOps packages.

---

# Consequences

## Positive

- Official upgrade path is explicit and aligned with existing Cloudflare hosting
- Supervisors can pull new versions without manual image copying
- Manifest contract matches implemented `registry poll` code
- Clear separation between FoldOps package updates and FoldingOS image updates

## Negative

- Requires publication automation and CDN object storage operations
- Milestone 3 trusts TLS + SHA-256 without detached signatures
- Manifest curation errors could expose a bad digest until verification fails

## Tradeoffs

- Stable channel only in v1; beta or per-fleet channels are future work
- Supervisors poll on a timer; immediate push notification is out of scope

---

# Implementation Plan

Work proceeds in this order within the Milestone 3 / issue #61 stream:

## Phase 1 — Documentation and defaults (this change)

- Accept ADR-0017 and cross-reference from ADR-0016, M3 spec, update-system,
  release-strategy, and `foldingosctl.md`
- Document manifest schema and URLs

## Phase 2 — Publication infrastructure

- Create `releases.folding-os.com` DNS and Cloudflare distribution (R2 or static
  bucket) with the `/release/` path layout above
- Add CI job that publishes verified release artifacts after ADR-0012 gate
- Publish `releases.json` and at least one stable image (for example `0.2.0`)

## Phase 3 — Supervisor image integration

- Ship default `/data/config/provision/upstream-releases.url` on supervisor role
  images pointing at the official manifest URL
- Confirm `foldingos-registry-poll.timer` imports new versions on a test
  supervisor
- Add automated test with fixture manifest (existing `registry poll` tests) plus
  integration test against staging manifest when available

## Phase 4 — Fleet rollout validation

- After #61 lab apply path passes: supervisor `registry poll` → `provision assign`
  → agent `check-version` / `apply-update` using an image published on
  `releases.folding-os.com` (not manual lab registration)

## Phase 5 — Signing (future)

- Publish `foldingos-archive-keyring.gpg`
- Extend manifest with signature fields
- Teach `registry poll` to verify detached signatures before import

---

# Future Considerations

- Additional channels (`beta`, `lts`) via separate manifest URLs
- FoldOps UI for operator approval before `rollout_state=ready`
- Supervisor air-gap import from removable media while retaining the same manifest
  schema

---

# Related Documents

- [ADR-0016: Network Provisioning Via Supervisor](0016-network-provisioning-via-supervisor.md)
- [ADR-0012: Reproducible Build Environment And Verification](0012-reproducible-build-environment-and-verification.md)
- [Update system](../update-system.md)
- [Release strategy](../release-strategy.md)
- [Milestone 3 engineering specification](../milestone/3-engineering-spec.md)
- [foldingosctl command reference](../foldingosctl.md)
- [ADR-0018: FoldOps Package Acquisition And Update Model](0018-foldops-package-acquisition-and-update-model.md)
- [FoldOps installation](https://www.folding-os.com/foldops)

---

# Closing Statement

Official FoldingOS upgrades reach fleets through supervisor-local registry
imports of HTTPS-published releases on `releases.folding-os.com`, using the same
project-controlled Cloudflare distribution model as FoldOps apt packages on
`deb.folding-os.com`.
