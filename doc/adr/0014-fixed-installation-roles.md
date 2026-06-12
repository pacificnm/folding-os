# ADR-0014: Fixed Installation Roles

**Status:** Accepted

**Version:** 1.0

**Date:** 2026-06-11

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

FoldingOS will provide one combined appliance and installer image containing
the approved FoldOps runtime payloads required by all supported installation
roles.

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

Role selection controls service activation from payloads already present in
the release image. It is not arbitrary package selection and does not install
packages from the network.

---

# FoldOps Artifact Integration

Approved FoldOps package artifacts will be acquired and integrated at
Buildroot build time using a pinned and cryptographically verified process.

FoldingOS will not:

- include APT as a runtime package manager
- contact the FoldOps Debian repository during installation
- use `trusted=yes` as a production trust model
- accept unpinned or unauthenticated FoldOps package artifacts
- execute Debian maintainer scripts unless an approved implementation
  specification explicitly defines and validates that behavior

The exact package versions, artifact hashes or signatures, extraction process,
and service-unit integration require an approved FoldOps implementation
specification before implementation.

---

# Installer And Direct-Flash Provisioning

The combined-image installer must collect an explicit role selection and
provision that selection onto the installed target. The installed appliance
must validate and persist the role before starting role-specific services.

Direct-flash deployment remains supported, but the exact mechanism used to
select a role before first boot is not defined by this ADR. It must be defined
before role-specific services are implemented.

The exact target EFI provisioning path, persistent role path, validation
rules, and first-boot consumption transaction require an approved amendment to
the installer engineering specification.

---

# Supervisor Provisioning

Supervisor installation must establish:

- an initial FoldOps administrator
- TLS identity and private-key material
- encrypted access to the enabled web interface

The exact administrator authentication method, credential-input channel,
certificate source, hostname and subject-name rules, secret-storage paths, and
certificate renewal or replacement process are not defined by this ADR. They
require an approved security and implementation specification before the
supervisor role may be implemented.

---

# Service And Failure Behavior

Role-specific services must not start in installer mode.

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

## Install FoldOps Packages From The Network

Rejected because installation is offline and FoldingOS does not provide a
runtime package manager.

## Supported In-Place Role Changes

Rejected for the initial architecture because role changes complicate service
state, persistent data ownership, security provisioning, and validation.
Reinstallation provides an explicit and reproducible transition.

---

# Consequences

## Positive

- one operating system and release image supports both roles
- the supervisor includes an agent without a separate node deployment
- installation remains offline and reproducible
- role activation is explicit and testable
- unsupported role drift is avoided

## Negative

- the release image contains payloads unused by the selected role
- role changes require destructive reinstallation
- supervisor installation requires additional secure provisioning
- release validation must cover both role-specific service graphs

---

# Required Follow-Up Decisions

Implementation is blocked until approved specifications define:

- exact FoldOps package versions and artifact verification
- role provisioning, persistence, and direct-flash behavior
- initial supervisor administrator provisioning
- TLS certificate and private-key provisioning
- supervisor persistent-state paths and storage requirements
- whether the supervisor role runs Folding@home

---

# Related Documents

- [ADR-0013: Combined Appliance And Installer Image](0013-combined-appliance-and-installer-image.md)
- [FoldingOS Installer](../installer.md)
- [FoldOps Integration](../foldops-integration.md)
- [FoldingOS Security Model](../security.md)

