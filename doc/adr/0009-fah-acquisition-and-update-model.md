# ADR-0009: Folding@home Acquisition and Update Model

**Status:** Accepted

**Version:** 1.0

**Date:** 2026-06-10

**Authors:** FoldingOS Project Contributors

---

# Context

FoldingOS must install and run a known, verified Folding@home client while
remaining reproducible and suitable for unattended operation.

Including the Folding@home client in the FoldingOS image would make FoldingOS
a redistributor of that software. Building the client on a deployed node would
require a compiler and development dependencies that conflict with the minimal
appliance model.

Installing whichever release upstream currently labels as latest would make
deployments non-reproducible and could install an untested or incompatible
version.

FoldOps may coordinate workload versions, but FoldingOS must also remain
operational without FoldOps.

---

# Decision

FoldingOS release images will not contain or redistribute the Folding@home
client or FahCore binaries.

FoldingOS will contain a small workload acquisition service and an approved
Folding@home workload manifest. After networking and time synchronization are
available, the acquisition service will:

1. Read the approved manifest.
2. Download the exact pinned client artifact directly from an official
   Folding@home HTTPS origin.
3. Verify the artifact's size and SHA-256 digest before installation.
4. Install the verified client into versioned persistent application storage.
5. Atomically activate the installed version.
6. Start the client as the unprivileged `fah` service account.

The service must never resolve or install an unpinned `latest` release.

For v0.1.0, the approved manifest is embedded in the FoldingOS image and is
trusted as part of that image. Future update mechanisms may deliver externally
signed manifests.

FahCore binaries remain managed and downloaded directly by the Folding@home
client from Folding@home infrastructure. FoldingOS will not bundle, mirror, or
proxy them.

---

# Approved Workload Manifest

The manifest must identify at least:

- manifest schema version
- Folding@home client version
- supported architecture
- official upstream artifact URL
- expected artifact size
- SHA-256 digest
- required FoldingOS compatibility version
- upstream license or terms reference

The manifest must be version controlled and included in release metadata.

An engineering specification will define its serialization format, signature
format, and exact validation procedure.

---

# Storage And Activation

Downloaded client binaries will be stored separately from mutable
Folding@home work:

```text
/data/apps/fah/<version>/
```

The active version will be selected atomically through an implementation-defined
reference under:

```text
/data/apps/fah/
```

Configuration, work units, checkpoints, and runtime state remain under their
existing persistent locations, including:

```text
/data/config/foldinghome/
/data/fah/
```

Installing or changing a client version must not erase or recreate those
locations.

---

# FoldOps Role

FoldOps may:

- report available approved workload manifests
- assign an approved manifest to a node or rollout group
- coordinate staged client-version rollouts
- schedule activation
- monitor acquisition, activation, and rollback status

FoldOps must not:

- authorize an unsigned or otherwise untrusted manifest
- replace the node's local verification
- cause installation of an unpinned `latest` version
- become required for continued Folding@home operation

To preserve the non-redistribution model, FoldOps will not host, mirror, cache,
or proxy Folding@home client or FahCore binaries unless a future decision
explicitly permits redistribution.

Nodes download approved artifacts directly from Folding@home infrastructure.

---

# Standalone Operation

FoldOps is not required for initial acquisition or continued operation.

A standalone node uses the approved manifest embedded in its FoldingOS image.
If FoldOps or the manifest service is unavailable, the node continues running
the last verified installed client.

An administrator may trigger reacquisition or select another approved,
locally available manifest through the supported administrative interface.

---

# Failure And Rollback Behavior

If download or verification fails:

- the unverified artifact must not be installed or executed
- the failure must be logged
- retries must use bounded backoff
- an already installed verified client must continue running

If activation of a newly installed client fails:

- the previous verified client version must remain available
- Folding@home persistent work and configuration must remain intact
- the node should reactivate the previous verified version when safe
- FoldOps should receive failure status when available

The acquisition service must not delete the last known-good client.

---

# Licensing And Redistribution

Because nodes download client artifacts directly from Folding@home
infrastructure, FoldingOS does not redistribute those artifacts.

FoldingOS documentation must disclose:

- that the client is acquired from Folding@home after deployment
- the exact approved version and upstream origin
- the applicable upstream license or terms
- that the client may download separately governed FahCore binaries

FoldingOS must comply with the licenses of the acquisition service and any
libraries it ships. A future decision to mirror, cache, proxy, modify, or
bundle Folding@home software requires a new licensing and redistribution
review.

---

# Security Requirements

- Only official Folding@home HTTPS origins may appear in approved manifests.
- Artifact verification must occur before installation or execution.
- Hash mismatch, size mismatch, unsupported architecture, or unsupported
  manifest schema must fail closed.
- External manifests must be authenticated before they can become approved.
- FoldOps transport authentication does not replace manifest authentication.
- Downloaded artifacts must not execute with acquisition-service privileges.
- The acquisition service must have only the privileges required to install
  versioned workload files and change the active-version reference.

---

# Alternatives Considered

## Include The Client In The FoldingOS Image

Rejected for the initial implementation because it makes FoldingOS a
redistributor and couples client updates to operating-system releases.

## Build The Client On Each Deployed Node

Rejected because it requires a compiler and development dependencies, increases
attack surface, consumes node resources, and makes build reproducibility harder
to audit.

## Install The Current Latest Version

Rejected because the result changes over time and bypasses compatibility
testing and controlled rollout.

## Have FoldOps Serve The Client Artifact

Rejected for the non-redistribution model because hosting or proxying the
artifact is still redistribution and makes FoldOps part of the artifact trust
path.

---

# Consequences

## Positive

- FoldingOS images do not redistribute Folding@home software
- deployments use an exact tested client version
- client updates do not require an operating-system image replacement
- standalone nodes remain supported
- FoldOps can coordinate controlled fleet rollouts
- failed updates can retain the last known-good client

## Negative

- a new node cannot begin Folding until it reaches the official upstream
  artifact origin
- official upstream artifact availability becomes a deployment dependency
- persistent storage must hold installed client versions
- acquisition, verification, activation, and rollback require dedicated
  implementation and testing
- upstream artifact format and runtime compatibility must be validated

---

# Related Documents

- [ADR-0005: Configuration Ownership and Precedence](0005-configuration-ownership-and-precedence.md)
- [ADR-0006: Folding@home Packaging and Privilege Model](0006-fah-packaging-and-privilege-model.md)
- [FoldOps Integration](../foldops-integration.md)
- [Update System](../update-system.md)
- [v0.1.0 Scope Specification](../milestone/1-implementation-spec.md)

