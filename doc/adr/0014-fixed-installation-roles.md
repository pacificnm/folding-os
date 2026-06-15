# ADR-0014: Fixed Installation Roles

**Status:** Accepted

**Amended by:**

- [ADR-0016](0016-network-provisioning-via-supervisor.md) (role assignment and
  provisioning mechanism)
- [ADR-0018](0018-foldops-package-acquisition-and-update-model.md) (FoldOps
  runtime acquisition from `deb.folding-os.com`)

**Version:** 1.1

**Date:** 2026-06-11

**Revised:** 2026-06-15 (FoldOps acquisition model)

**Authors:** FoldingOS Project Contributors

---

# Context

FoldingOS must support both compute nodes managed by FoldOps and systems that
host FoldOps management services. Maintaining separate operating-system images
for those purposes would duplicate the build, release, security, and
validation work that the combined appliance and installer image is intended to
avoid.

FoldOps is distributed as three Debian package artifacts:

```text
foldops-agent
foldops-supervisor
foldops-web
```

FoldingOS is an appliance rather than a general-purpose Debian system. It does
not provide runtime APT package management, and installation must remain
offline and reproducible.

---

# Decision

FoldingOS will provide one combined appliance image for all supported
installation roles. FoldOps runtime payloads are **not** embedded in the image;
they are acquired at runtime from official Debian packages on
`deb.folding-os.com` per
[ADR-0018](0018-foldops-package-acquisition-and-update-model.md).

The installer will offer exactly two fixed roles:

```text
agent
supervisor
```

The `agent` role enables the FoldOps agent and does not enable the FoldOps
supervisor or web interface.

The `supervisor` role includes and enables:

```text
foldops-agent
foldops-supervisor
foldops-web
```

The FoldOps web interface is enabled by default for the supervisor role.

The selected role is fixed for the life of an installation. FoldingOS will not
support an in-place role change. Changing roles requires a fresh destructive
reinstallation.

Supervisor installation requires initial administrator and TLS provisioning.
The supervisor web interface must not become remotely available until that
provisioning succeeds.

Role selection controls which FoldOps packages are acquired and which services
may start. It is not arbitrary package selection. Acquisition uses pinned
manifest entries and official HTTPS artifacts; see
[ADR-0018](0018-foldops-package-acquisition-and-update-model.md).

---

# FoldOps Artifact Integration

FoldOps integration is defined by
[ADR-0018](0018-foldops-package-acquisition-and-update-model.md).

Summary:

- the release image embeds a pinned acquisition manifest and archive keyring,
  not FoldOps application binaries
- after role validation and network availability, `foldingosctl foldops acquire`
  downloads the same `.deb` artifacts that `apt` installs from
  `deb.folding-os.com`, verifies them, and activates them under
  `/data/apps/foldops/`
- FoldingOS does not ship runtime APT; general Debian hosts continue to use
  `apt` against the same repository

Service-unit integration and acquisition implementation details are defined in
the [Milestone 3 engineering specification](../milestone/3-engineering-spec.md).

---

# Installer, Direct-Flash, And Network Provisioning

[ADR-0016](0016-network-provisioning-via-supervisor.md) supersedes the
combined-image USB installer defined by ADR-0013.

Role assignment occurs during provisioning:

- the **supervisor** role is assigned when the first node is direct-flashed
- **agent** roles are assigned by the supervisor during network provisioning

The installed appliance must validate and persist the role before starting
role-specific services. Direct-flash deployment remains supported for
supervisor bootstrap and emergency recovery.

The exact target EFI provisioning path, persistent role path, validation rules,
network-boot transaction, and first-boot consumption behavior are defined by the
[Milestone 3 engineering specification](../milestone/3-engineering-spec.md).

---

# Supervisor Provisioning

Supervisor installation must establish:

- an initial FoldOps administrator
- TLS identity and private-key material
- encrypted access to the enabled web interface

The exact administrator authentication method, credential-input channel,
certificate source, hostname and subject-name rules, secret-storage paths, and
certificate renewal or replacement process are defined by
[ADR-0019](0019-foldops-supervisor-provisioning-and-tls.md).

---

# Service And Failure Behavior

Role-specific FoldOps services must not start before the installation role is
validated and persisted.

In appliance mode:

- the agent role must not start `foldops-supervisor` or `foldops-web`
- the supervisor role must start the FoldOps agent, supervisor, and web
  services after required provisioning succeeds
- failure of FoldOps services must not prevent FoldingOS from booting
- failure of FoldOps services on a compute node must not prevent continued
  Folding@home operation

Whether a supervisor installation also runs the Folding@home workload is not
decided by this ADR.

---

# Storage

Both roles use the existing FoldingOS partition layout.

The supervisor's persistent state belongs under the FoldingOS persistent data
area. Exact paths, retention policy, backup requirements, and any minimum
supervisor storage capacity remain unresolved and require an approved
implementation specification.

---

# Alternatives Considered

## Separate Agent And Supervisor Images

Rejected because separate images would duplicate the operating system, release
pipeline, validation matrix, and maintenance burden.

## Runtime APT against deb.folding-os.com

Rejected because a general-purpose package manager is incompatible with the
appliance model. [ADR-0018](0018-foldops-package-acquisition-and-update-model.md)
instead downloads the same pinned `.deb` pool objects over HTTPS with
`foldingosctl`-controlled verification and extract-only installation.

## Embed all FoldOps binaries at Buildroot build time

Rejected because FoldOps releases independently of the operating-system image;
rebaking the full appliance image for every FoldOps update is impractical.
Direct-flash and network provisioning remain offline for the **operating-system
image**; FoldOps acquisition runs after first boot when network is available.

## Supported In-Place Role Changes

Rejected for the initial architecture because role changes complicate service
state, persistent data ownership, security provisioning, and validation.
Reinstallation provides an explicit and reproducible transition.

---

# Consequences

## Positive

- one operating system and release image supports both roles
- the supervisor includes an agent without a separate node deployment
- the operating-system image remains offline-flashable and reproducible
- role activation is explicit and testable
- unsupported role drift is avoided

## Negative

- FoldOps acquisition requires network reachability after first boot
- role changes require destructive reinstallation
- supervisor installation requires additional secure provisioning
- release validation must cover both role-specific service graphs

---

# Required Follow-Up Decisions

Implementation is blocked until approved specifications define:

- whether the supervisor role runs Folding@home

FoldOps package acquisition is defined by
[ADR-0018](0018-foldops-package-acquisition-and-update-model.md).
Supervisor ingest-token bootstrap, TLS, and EFI staging are defined by
[ADR-0019](0019-foldops-supervisor-provisioning-and-tls.md).

---

# Related Documents

- [ADR-0018: FoldOps Package Acquisition And Update Model](0018-foldops-package-acquisition-and-update-model.md)
- [ADR-0016: Network Provisioning Via Supervisor](0016-network-provisioning-via-supervisor.md)
- [FoldingOS deployment and provisioning](../installer.md)
- [FoldOps Integration](../foldops-integration.md)
- [ADR-0019: FoldOps Supervisor Provisioning And TLS](0019-foldops-supervisor-provisioning-and-tls.md)
- [FoldingOS Security Model](../security.md)

