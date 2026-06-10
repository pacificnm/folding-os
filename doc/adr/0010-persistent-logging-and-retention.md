# ADR-0010: Persistent Logging And Retention

**Status:** Accepted

**Version:** 1.0

**Date:** 2026-06-10

**Authors:** FoldingOS Project Contributors

---

# Context

FoldingOS must preserve enough diagnostic information to investigate boot,
service, networking, storage, and Folding@home failures across reboots.

Unbounded persistent logging can consume the data partition, disrupt
Folding@home work, and make an unattended node unreliable. Multiple independent
log-file mechanisms would also complicate retention, diagnostics, and
disk-full behavior.

Logging must remain useful without becoming a dependency for the primary
scientific workload.

---

# Decision

FoldingOS will use `systemd-journald` as the system logging implementation.

Services will write to the system journal through standard output, standard
error, syslog-compatible interfaces, or the native journal interface. Services
must not create independent persistent log files unless a future decision
documents a specific requirement.

Persistent journal storage will reside under:

```text
/data/logs/journal
```

FoldingOS will make that location available to `systemd-journald` at:

```text
/var/log/journal
```

using a bind mount or equivalent systemd-managed mount after `/data` is
available. Logs produced before persistent storage is available may begin in
the volatile journal and will be flushed to persistent storage when possible.

---

# Retention Policy

For v0.1.0, the persistent system journal will use all of the following limits:

- maximum persistent journal usage: 256 MiB
- minimum free space preserved on `/data`: 512 MiB
- maximum log age: 14 days
- maximum time span represented by one journal file: 1 day

`systemd-journald` will rotate and vacuum old journal files automatically.
Whichever configured size, free-space, or age limit is reached first governs
retention.

The volatile runtime journal will be capped at 64 MiB.

The corresponding journald settings are:

```ini
Storage=auto
SystemMaxUse=256M
SystemKeepFree=512M
MaxRetentionSec=14day
MaxFileSec=1day
RuntimeMaxUse=64M
```

`Storage=auto` allows the early journal to remain volatile until
`/data/logs/journal` is mounted at `/var/log/journal`; the journal is then
flushed to persistent storage. Persistent storage remains required during
normal operation.

If `/data` already has less than 512 MiB free for reasons unrelated to the
journal, journald cannot restore that reserve. It must not consume additional
space that would worsen the condition.

These defaults may be changed in a future release through an explicit,
validated configuration mechanism. v0.1.0 does not require user-configurable
retention.

---

# Disk-Full And Failure Behavior

Logging must never consume space reserved for Folding@home work and persistent
configuration.

When storage approaches its configured limits, journald must rotate and remove
old journal files. If new records still cannot be persisted, losing diagnostic
records is preferable to stopping Folding@home or damaging persistent data.

If `/data` or the persistent journal location is unavailable, invalid, or not
writable:

- boot continues when otherwise safe
- journald uses volatile storage
- the failure is reported through the available journal and service status
- Folding@home operation is not blocked solely because persistent logging is
  unavailable
- the system may retry persistent journal activation on a later boot

Logging failures must not trigger formatting, deletion of non-journal data, or
unbounded retry loops.

---

# Security And Privacy

Logs must not contain:

- passwords
- Folding@home passkeys
- private keys
- authentication tokens
- complete provisioning key material
- other secrets or credentials

Services are responsible for redacting sensitive values before logging.

Journal storage must not be writable by unprivileged service accounts. Normal
administrative access to logs is provided through the supported administrative
interface.

Rate limiting must remain enabled to reduce the impact of malfunctioning or
hostile services. Concrete v0.1.0 rate-limit values and service behavior are
defined by the
[v0.1.0 engineering specification](../milestone/1-engineering-spec.md).

---

# Operational Requirements

The journal must support:

- current and previous boot inspection
- filtering by systemd unit
- filtering by severity and time
- local SSH-based diagnostics
- future FoldOps log collection without making FoldOps required

Journal vacuuming and rotation must not require administrator intervention.

Future remote log collection must not disable local retention and must preserve
the same secret-redaction requirements.

---

# Alternatives Considered

## Volatile-Only Journal

Rejected because diagnostics needed after reboot or power loss would be lost.

## Traditional Syslog Daemon And Rotated Text Files

Rejected for v0.1.0 because it adds another logging service and duplicates
functionality already provided by systemd.

## Unbounded Persistent Journal

Rejected because logs could consume storage required for Folding@home work and
configuration.

## Separate Log Partition

Rejected for v0.1.0 because the data partition and journald limits provide
adequate isolation without additional partition complexity.

## Stop Workloads When Logging Fails

Rejected because loss of diagnostics is less harmful than unnecessarily
stopping scientific computation.

---

# Consequences

## Positive

- one consistent logging interface for system services
- diagnostics survive reboot under normal operation
- bounded disk consumption
- automatic rotation and vacuuming
- persistent logging failure does not stop Folding@home
- no additional syslog daemon is required

## Negative

- old diagnostics are deliberately discarded when limits are reached
- early-boot logs depend on successful volatile-to-persistent flushing
- journal files require journal-aware tools for inspection
- persistent logging consumes a bounded portion of `/data`

---

# Related Documents

- [ADR-0002: Init and Service Supervision](0002-init-and-service-supervision.md)
- [ADR-0004: Partition and Persistence Layout](0004-partition-and-persistence-layout.md)
- [ADR-0006: Folding@home Packaging and Privilege Model](0006-fah-packaging-and-privilege-model.md)
- [Security Model](../security.md)
- [Testing Strategy](../testing-strategy.md)
- [v0.1.0 Scope Specification](../milestone/1-implementation-spec.md)
