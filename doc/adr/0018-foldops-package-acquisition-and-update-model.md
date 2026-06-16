# ADR-0018: FoldOps Package Acquisition And Update Model

**Status:** Accepted

**Version:** 1.0

**Date:** 2026-06-15

**Authors:** FoldingOS Project Contributors

**Amends:** [ADR-0014](0014-fixed-installation-roles.md) (FoldOps artifact integration)

**Amended by:**

- [ADR-0022](0022-foldops-rust-source-in-foldingos-monorepo.md) — FoldOps Rust source in `packages/foldops/`
- [ADR-0023](0023-runtime-foldops-and-foldingosctl-updates-without-os-reimage.md) — layout bundles, assigned manifests, `tools acquire`

The body of this ADR remains the Milestone 3 baseline. Appliance transport and
fleet assignment extensions are defined in the amending ADRs and
[milestone/4-appliance-artifact-and-monorepo-plan.md](../milestone/4-appliance-artifact-and-monorepo-plan.md).

---

# Context

FoldOps is distributed as Debian packages (`foldops-agent`, `foldops-supervisor`,
`foldops-web`) from official project infrastructure at
`https://deb.folding-os.com`, documented at
[https://www.folding-os.com/foldops](https://www.folding-os.com/foldops).
General Debian hosts install these packages with `apt` after configuring the
archive keyring and apt source.

[ADR-0014](0014-fixed-installation-roles.md) originally required FoldOps
payloads to be integrated at Buildroot build time so direct-flash and network
provisioning remained offline and reproducible. That model conflicts with two
project requirements:

1. FoldOps packages evolve quickly; rebaking every OS image for each FoldOps
   release is impractical for fleet management.
2. FoldingOS is an appliance and must not ship a general-purpose runtime package
   manager ([ADR-0014](0014-fixed-installation-roles.md)).

[ADR-0009](0009-fah-acquisition-and-update-model.md) already establishes a
precedent: the operating-system image ships a **pinned manifest and acquisition
service**, not the third-party binary. After networking and time synchronization,
the node downloads the exact artifact from an official HTTPS origin, verifies it,
installs into versioned persistent storage, and activates atomically.

[ADR-0017](0017-official-release-publication-and-supervisor-upstream-polling.md)
documents `deb.folding-os.com` as the official FoldOps package channel,
separate from FoldingOS disk images on `releases.folding-os.com`.

---

# Decision

FoldingOS release images will **not** embed FoldOps application binaries.

Each release image embeds:

- an approved **FoldOps acquisition manifest** at
  `/usr/share/foldingos/manifests/foldops.toml`
- the official **archive keyring** at `/usr/share/keyrings/foldops.gpg`
  (same trust anchor used by the Debian apt instructions)

After the installation role is validated and networking is available, a
**FoldOps acquisition service** (`foldingosctl foldops acquire`, invoked by
systemd) will:

1. Read the approved manifest for the node's role.
2. Download each required `.deb` directly from pinned HTTPS URLs on
   `deb.folding-os.com` (the same pool objects `apt` would install).
3. Verify each artifact's size and SHA-256 digest before installation.
4. Extract the Debian **data archive only** into versioned persistent storage
   (no arbitrary maintainer-script execution).
5. Activate the verified installation set atomically.
6. Enable role-appropriate FoldOps systemd units only after verification
   succeeds.

The service must never resolve or install an unpinned `latest` release.

## Role-specific package sets

| Role | Packages acquired |
| --- | --- |
| `agent` | `foldops-agent` |
| `supervisor` | `foldops-agent`, `foldops-supervisor`, `foldops-web` |

Role selection controls **which packages are acquired and which services may
start**. It is not arbitrary package selection.

## Parallel installation paths

| Environment | Install method |
| --- | --- |
| General Debian host | `apt` with `deb.folding-os.com` source and `foldops.gpg` keyring |
| FoldingOS appliance | `foldingosctl foldops acquire` with embedded manifest and HTTPS verification |

Both paths consume the **same official `.deb` artifacts** from
`deb.folding-os.com`.

## Persistent storage

Verified FoldOps installations live under:

```text
/data/apps/foldops/<manifest_release>/
```

Each package extracts into a role-specific subdirectory. Activation uses a
verified marker and a `current` pointer under `/data/apps/foldops/` analogous to
Folding@home activation in [ADR-0009](0009-fah-acquisition-and-update-model.md).

Runtime FoldOps configuration and state remain under the paths defined by
[ADR-0005](0005-configuration-ownership-and-precedence.md):

```text
/data/config/foldops/supervisor.env
/data/config/foldops/agent.env
/data/config/foldops/ingest-token
/data/config/foldops/supervisor-ca.pem
/data/foldops/
```

`/data/config/foldops.toml` is reserved for a future FoldOps TOML domain;
Milestone 3 uses the env files above instead.

Download staging and acquire retry state live under `/data/state/foldops/`.

## Trust model

Milestone 3 requires, for each pinned package entry in the manifest:

- exact HTTPS artifact URL on `deb.folding-os.com`
- expected artifact size in bytes
- SHA-256 digest

The embedded archive keyring enables future verification of signed apt
`Release` metadata when polling for newer FoldOps versions. Milestone 3 may
rely on manifest pins alone, matching the initial trust level used for
FoldingOS image registry import.

FoldingOS will not:

- include APT or `dpkg` as a runtime package manager
- configure apt sources with `trusted=yes`
- accept unpinned or unverified FoldOps artifacts
- execute Debian maintainer scripts unless a later approved specification
  explicitly defines and validates required post-install steps

## Systemd integration

FoldingOS ships **FoldingOS-owned systemd units** that invoke binaries under
`/data/apps/foldops/current/`. Units extracted from `.deb` payloads are not
enabled directly; their behavior is reproduced or wrapped by approved units in
the FoldingOS overlay.

FoldOps services must not start before:

1. installation role is validated and persisted
2. FoldOps acquisition for that role succeeds
3. on supervisor role: initial ingest-token and TLS provisioning succeeds
   ([ADR-0019](0019-foldops-supervisor-provisioning-and-tls.md))

## Boot and failure behavior

- Loss of network or `deb.folding-os.com` reachability must not prevent
  FoldingOS from booting.
- Failure to acquire FoldOps must not prevent Folding@home operation on agent
  nodes.
- FoldOps is not required for Folding@home client acquisition or continued
  operation ([ADR-0009](0009-fah-acquisition-and-update-model.md)).
- Acquisition retries use bounded backoff state under `/data/state/foldops/`,
  analogous to Folding@home acquisition.

## Updates

For Milestone 3, FoldOps package version pins change when a new FoldingOS release
embeds an updated manifest. Future work may add FoldOps-coordinated manifest
assignment without making FoldOps required for node boot.

Operating-system updates remain supervisor-mediated per
[ADR-0016](0016-network-provisioning-via-supervisor.md). FoldOps package updates
and FoldingOS image updates use separate official channels per
[ADR-0017](0017-official-release-publication-and-supervisor-upstream-polling.md).

---

# Approved Acquisition Manifest

The manifest must identify at least:

- manifest schema version
- manifest release label (activation directory name)
- supported architecture
- minimum compatible FoldingOS version
- for each required package: name, version, artifact URL, size, SHA-256,
  executable or unit paths needed for verification

The manifest must be version controlled, verified at build time, and recorded in
release metadata.

Concrete serialization and validation rules are defined in the
[Milestone 3 engineering specification](../milestone/3-engineering-spec.md).

---

# Alternatives Considered

## Embed FoldOps packages at Buildroot build time

Rejected for FoldOps because package release cadence is independent of the
operating-system image and frequent FoldOps updates should not require rebaking
the full 4 GiB appliance image. The OS image remains reproducible by pinning the
**manifest**, not the FoldOps binaries.

## Runtime APT against deb.folding-os.com

Rejected because a general-purpose package manager expands the appliance attack
surface, complicates deterministic validation, and is unnecessary when the same
`.deb` pool objects can be downloaded and verified directly.

## Build FoldOps from source inside Buildroot

Rejected because FoldOps remains an independent repository with its own release
process; FoldingOS consumes published Debian packages, not merged source trees.

---

# Consequences

## Positive

- Same official artifacts and keyring as Debian `apt` installs
- FoldOps can evolve without rebaking every FoldingOS image
- Deterministic pins preserved through the embedded manifest
- Consistent with the Folding@home acquisition model ([ADR-0009](0009-fah-acquisition-and-update-model.md))
- Clear separation from OS image updates ([ADR-0017](0017-official-release-publication-and-supervisor-upstream-polling.md))

## Negative

- First FoldOps availability requires network reachability to `deb.folding-os.com`
- Extract-only install may require FoldingOS to reproduce some `.deb` post-install
  layout expectations explicitly
- Release validation must cover acquisition, activation, and role-specific
  service graphs

## Tradeoffs

- Manifest-only trust in Milestone 3; signed `Release` polling is future work
- Extract-only FoldOps install requires explicit env/TLS bootstrap in
  `foldingosctl` ([ADR-0019](0019-foldops-supervisor-provisioning-and-tls.md))

---

# Required Follow-Up

Implementation specifications and `scripts/verify-systemd-graph` define the
approved FoldOps systemd unit graph and dependency ordering.

Supervisor ingest-token bootstrap, self-signed TLS, EFI staging, and the HTTPS
front end are defined and implemented per
[ADR-0019](0019-foldops-supervisor-provisioning-and-tls.md).

---

# Related Documents

- [ADR-0014: Fixed Installation Roles](0014-fixed-installation-roles.md)
- [ADR-0016: Network Provisioning Via Supervisor](0016-network-provisioning-via-supervisor.md)
- [ADR-0017: Official Release Publication And Supervisor Upstream Polling](0017-official-release-publication-and-supervisor-upstream-polling.md)
- [ADR-0009: Folding@home Acquisition And Update Model](0009-fah-acquisition-and-update-model.md)
- [FoldOps integration](../foldops-integration.md)
- [Milestone 3 engineering specification](../milestone/3-engineering-spec.md)
- [ADR-0019: FoldOps Supervisor Provisioning And TLS](0019-foldops-supervisor-provisioning-and-tls.md)
- [FoldOps installation](https://www.folding-os.com/foldops)
