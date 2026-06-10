# ADR-0006: Folding@home Packaging and Privilege Model

**Status:** Accepted

**Version:** 1.0

**Date:** 2026-06-10

**Authors:** FoldingOS Project Contributors

---

# Context

FoldingOS exists for one primary purpose:

To provide a reliable operating system dedicated to running Folding@home.

Although Folding@home is the primary workload, it should remain an
application running on top of the operating system rather than becoming part
of the operating system itself.

The execution model must prioritize:

* reliability
* security
* maintainability
* observability
* least privilege
* recoverability

---

# Decision

The Folding@home client will be packaged as a managed system service.

It will:

* execute under systemd
* execute as a dedicated service account
* not execute as root during normal operation
* have only the minimum required filesystem permissions
* have only the minimum required runtime privileges

The operating system remains responsible for service supervision.

The Folding@home client remains an application workload.

The client is acquired after deployment and is not included in FoldingOS
release images. Acquisition, verification, activation, and update behavior are
defined by
[ADR-0009](0009-fah-acquisition-and-update-model.md).

---

# Service Account

Milestone 1 creates a dedicated system user:

```text
User: fah

Group: fah
```

The account should:

* have no interactive login
* have no password
* have no home directory intended for user interaction
* exist solely to execute Folding@home services

Example:

```text
UID: system assigned

Shell:
/usr/sbin/nologin
```

(or equivalent implementation)

---

# Service Supervision

The Folding@home client will be managed through systemd.

Responsibilities include:

* startup
* shutdown
* restart
* dependency management
* health observation
* log integration

Normal operation should not require manual supervision.

---

# Startup Ordering

Expected startup sequence:

```text
Firmware

↓

Bootloader

↓

Kernel

↓

systemd

↓

Filesystem

↓

Networking

↓

Time Synchronization

↓

Folding@home Acquisition (when no verified client is installed)

↓

Folding@home
```

Folding@home should not start before required runtime dependencies are
available. FoldOps is not a required dependency for acquisition or Folding@home
startup.

---

# Filesystem Ownership

Operating system binaries remain under:

```text
/
```

Versioned Folding@home client binaries acquired after deployment reside under:

```text
/data/apps/fah
```

Persistent Folding data resides under:

```text
/data/fah
```

Typical contents:

* work units
* checkpoints
* runtime state
Ownership:

```text
fah:fah
```

The service should have write access only to the locations it requires.

---

# Logging

Service logs should integrate with system logging.

Persistent service logs are managed by `systemd-journald` under:

```text
/data/logs/journal
```

Sensitive information should never be logged.

Retention and disk-full behavior are defined by
[ADR-0010](0010-persistent-logging-and-retention.md).

---

# Configuration

Configuration should be stored separately from binaries.

Recommended location:

```text
/data/config/foldinghome.toml
```

Configuration should survive operating system replacement.

Configuration ownership and precedence are defined by
[ADR-0005](0005-configuration-ownership-and-precedence.md).

TOML format, validation, and secret separation are defined by
[ADR-0011](0011-toml-configuration-validation-and-migration.md).

---

# Network Access

The Folding@home client requires outbound network connectivity.

No inbound public service should be exposed solely for Folding@home
operation.

Management interfaces should remain under explicit FoldingOS control.

---

# Security Philosophy

The Folding@home client should operate under the principle of least
privilege.

It should:

* not require root
* not modify operating system binaries
* not write outside designated locations
* not require unnecessary capabilities

Additional privileges should require explicit engineering justification.

---

# Update Philosophy

Updating Folding@home should not require replacing persistent node state.

Future update mechanisms should preserve:

* work units
* checkpoints
* configuration
* identity

where practical.

---

# Packaging Philosophy

Folding@home is treated as:

* an application
* a managed service
* a supervised workload

It is not considered part of the immutable operating system image from an
architectural perspective and is not distributed within official FoldingOS
release images.

This separation simplifies future updates and maintenance.

---

# Failure Handling

If the Folding@home service exits unexpectedly:

* the failure should be logged
* restart policy should apply
* repeated failures should be observable
* diagnostics should be available through FoldOps where applicable

Failure should not require rebooting the operating system.

---

# Alternatives Considered

## Run as root

Advantages:

* simple implementation

Disadvantages:

* unnecessary privilege
* increased security risk
* violates least privilege

Decision:

Rejected.

---

## Embed Folding@home into operating system components

Advantages:

* tighter integration

Disadvantages:

* harder maintenance
* harder updates
* poorer architectural separation

Decision:

Rejected.

Folding@home remains an application workload.

---

## User-managed execution

Advantages:

* flexibility

Disadvantages:

* inconsistent deployments
* operational variability
* reduced reliability

Decision:

Rejected.

Managed services are preferred.

---

# Consequences

Positive:

* improved security

* improved maintainability

* better operational consistency

* simpler supervision

* cleaner architecture

Negative:

* requires dedicated service account

* requires permission management

* requires service definitions

These tradeoffs are acceptable.

---

# Future Enhancements

Future releases may introduce:

* tighter sandboxing

* Linux namespaces

* seccomp

* capability reduction

* read-only filesystem protections

* additional service hardening

Such features should be adopted incrementally and based on measurable
engineering value.

---

# Related Documents

* [Project charter](../../PROJECT_CHARTER.md)

* [Engineering principles](../../PRINCIPLES.md)

* [Security model](../security.md)

* [Storage layout](../storage-layout.md)

* [Architecture](../architecture.md)

* [ADR-0002: Init and Service Supervision](0002-init-and-service-supervision.md)

* [ADR-0004: Partition and Persistence Layout](0004-partition-and-persistence-layout.md)

* [ADR-0009: Folding@home Acquisition and Update Model](0009-fah-acquisition-and-update-model.md)

* [ADR-0010: Persistent Logging And Retention](0010-persistent-logging-and-retention.md)

* [ADR-0011: TOML Configuration Validation And Migration](0011-toml-configuration-validation-and-migration.md)

---

# Closing Statement

FoldingOS exists to run Folding@home.

However, Folding@home should remain a well-contained, supervised application
running with the minimum privileges necessary to accomplish its task.

The operating system provides the platform.

The Folding@home client provides the scientific computation.

Maintaining that separation improves security, maintainability, and
long-term reliability.
