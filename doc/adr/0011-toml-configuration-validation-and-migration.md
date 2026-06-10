# ADR-0011: TOML Configuration Validation And Migration

**Status:** Accepted

**Version:** 1.0

**Date:** 2026-06-10

**Authors:** FoldingOS Project Contributors

---

# Context

ADR-0005 defines configuration ownership, locations, and precedence, but does
not define the file format or the lifecycle used to validate and safely apply
configuration.

FoldingOS configuration must be understandable over SSH, deterministic,
strictly validated, safe during power loss, and evolvable across future
releases. A malformed configuration for one optional subsystem must not
unnecessarily prevent the node from booting or Folding@home from operating.

---

# Decision

FoldingOS will use TOML for structured configuration.

Configuration will be separated by ownership domain rather than stored in one
global file. The v0.1.0 persistent structured configuration files are:

```text
/data/config/system.toml
/data/config/network.toml
/data/config/foldinghome.toml
```

FoldOps may add `/data/config/foldops.toml` when FoldOps integration is
implemented in a later milestone.

Image defaults use matching domain filenames under:

```text
/etc/foldingos/defaults/
```

Administrator overrides use matching domain filenames under:

```text
/data/config/overrides/
```

Every structured configuration file must contain:

```toml
schema_version = 1
```

The v0.1.0 supported fields, types, defaults, constraints, and owning services
are defined by the
[v0.1.0 engineering specification](../milestone/1-engineering-spec.md).

---

# Non-TOML Persistent Data

Opaque or single-purpose data that is not naturally structured configuration
will remain in dedicated files or directories.

Examples include:

```text
/data/config/node-id
/data/config/ssh/authorized_keys
/data/config/secrets/
```

These files remain subject to explicit validation, ownership, permission, and
atomic-update requirements appropriate to their content.

Secrets must not be stored directly in TOML configuration files. TOML files
may contain references to secret names or paths under:

```text
/data/config/secrets/
```

The exact secret storage mechanism and future hardware-backed secret support
remain separate decisions.

---

# Schema Validation

Each configuration domain must have one explicit versioned schema.

Validation must reject:

- malformed TOML
- missing `schema_version`
- unsupported schema versions
- unknown keys
- missing required fields
- incorrect value types
- values outside documented constraints
- references to missing or inaccessible required secrets
- settings that violate security invariants

Unknown keys are rejected so misspellings and obsolete settings cannot be
silently ignored.

Defaults must be explicit and documented. Validation and effective
configuration generation must be deterministic.

---

# Candidate Validation And Atomic Activation

Automated configuration writers must never edit an active file in place.

The activation process is:

1. Write the complete candidate file to a temporary file on `/data`.
2. Apply required ownership and permissions.
3. Parse and validate the candidate against its domain schema.
4. Generate and validate the resulting effective configuration after applying
   precedence.
5. Flush the candidate and containing directory as required for durable
   activation.
6. Preserve the previous valid active file as the domain's last-known-good
   configuration.
7. Atomically rename the validated candidate over the active file.
8. Reload or restart only the affected service when required.
9. Confirm service health and report the result.

The active configuration must never be replaced by a candidate that fails
validation.

The v0.1.0 temporary-file, last-known-good, locking, durability, and
service-health mechanisms are defined by the
[v0.1.0 engineering specification](../milestone/1-engineering-spec.md).

---

# Validation Failure Behavior

Configuration validation occurs before affected application services start.

If a candidate update fails validation:

- the active configuration remains unchanged
- the failure is logged without exposing secrets
- the update is reported as rejected
- the currently running service continues using its existing configuration

If active persistent configuration is missing or invalid during boot:

1. use the domain's last-known-good configuration when valid
2. otherwise use valid image defaults when safe
3. otherwise prevent only the affected service from starting
4. keep the operating system and SSH recovery access available when safe

A failure in optional FoldOps configuration must not stop Folding@home.

A failure in Folding@home configuration may prevent Folding@home from starting
when no safe valid configuration exists, but must not prevent system boot or
SSH recovery.

Security invariants, including disabled direct root SSH login and disabled SSH
password authentication, cannot be overridden by ordinary configuration.

---

# Migration

Every structured configuration file has an independent integer
`schema_version`.

FoldingOS v0.1.0 supports schema version `1` only.

Future forward migrations must:

1. retain the original valid file
2. write migrated output to a separate temporary file
3. validate the migrated result against the target schema
4. atomically activate it only after successful validation
5. preserve the original if migration or activation fails
6. log the outcome without exposing secrets

Migration must never silently discard unknown or unsupported values.

Automatic downgrade migration is not required. A configuration with a schema
version newer than the running software supports must fail validation without
being modified.

---

# Effective Configuration

Effective configuration is produced using the precedence defined by ADR-0005:

```text
Built-in defaults

↓

Image defaults

↓

Persistent configuration

↓

Administrator overrides

↓

Runtime temporary overrides
```

Each layer must validate against the same domain schema before it participates
in effective configuration generation.

Diagnostic tooling should be able to report the effective non-secret
configuration and identify which layer supplied each value.

---

# Security Requirements

- TOML configuration files must not contain secret values.
- Configuration and secret files must use restrictive ownership and
  permissions appropriate to their consumers.
- Validation errors and diagnostics must redact sensitive values.
- Unprivileged services must not modify configuration outside their owned
  domain.
- Configuration updates received from future remote-management systems must
  pass the same local validation and atomic activation process.
- A remote source cannot bypass local security invariants.

---

# Alternatives Considered

## YAML

Rejected because its complex parsing rules, implicit types, and broad feature
set make strict and predictable appliance configuration harder.

## JSON

Rejected because it is less convenient for administrators to read and edit and
does not support comments.

## Simple Key-Value Files

Rejected because they become difficult to structure, type, validate, and evolve
as configuration domains grow.

## One Global TOML File

Rejected because it combines unrelated ownership domains and increases the
impact of one malformed update.

## Edit Active Files In Place

Rejected because interrupted writes can leave partial or corrupted
configuration.

## Ignore Unknown Keys

Rejected because misspellings and obsolete options would appear to succeed
while having no effect.

---

# Consequences

## Positive

- human-readable configuration
- deterministic parsing and validation
- clear ownership by domain
- strict detection of mistakes and obsolete keys
- power-loss-safe activation
- service-specific failure isolation
- explicit path for future schema evolution

## Negative

- requires a TOML parser and domain-schema validation implementation
- every supported field must be documented and tested
- strict unknown-key rejection requires deliberate schema migrations
- last-known-good state and atomic activation add implementation work

---

# Related Documents

- [ADR-0005: Configuration Ownership And Precedence](0005-configuration-ownership-and-precedence.md)
- [ADR-0007: First-Boot Administrator And SSH-Key Provisioning](0007-first-boot-administrator-and-ssh-provisioning.md)
- [Security Model](../security.md)
- [Boot Process](../boot-process.md)
- [Testing Strategy](../testing-strategy.md)
- [v0.1.0 Scope Specification](../milestone/1-implementation-spec.md)
