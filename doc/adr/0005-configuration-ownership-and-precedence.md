# ADR-0005: Configuration Ownership and Precedence

**Status:** Accepted

**Version:** 1.0

**Date:** 2026-06-10

**Authors:** FoldingOS Project Contributors

---

# Context

FoldingOS requires a deterministic configuration model.

Without clear ownership and precedence rules, configuration can become
difficult to understand, debug, reproduce, and recover.

The project must define where configuration lives, which component owns each
configuration area, and what happens when multiple configuration sources
exist.

---

# Decision

FoldingOS will use an explicit layered configuration model.

Configuration precedence, from lowest to highest priority, is:

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

Higher layers override lower layers.

No hidden configuration precedence is allowed.

---

# Configuration Locations

## Built-in Defaults

Built-in defaults are compiled or packaged with FoldingOS.

These should represent safe operational defaults.

Examples:

- default service behavior
- default directory paths
- default logging behavior
- default startup behavior

---

## Image Defaults

Image defaults are shipped with the operating system image.

Recommended location:

```text
/etc/foldingos/defaults/
```

Examples:

- default hostname pattern
- default network behavior
- default service enablement
- default FoldingOS settings

Image defaults are part of the operating system image and may be replaced
during OS updates.

---

## Persistent Configuration

Persistent configuration survives operating system replacement.

Recommended location:

```text
/data/config/
```

Examples:

- hostname
- networking configuration
- SSH configuration
- Folding@home configuration
- FoldOps enrollment settings
- local node preferences

Persistent configuration is the primary source of node-specific truth.

---

## Administrator Overrides

Administrator overrides are explicit local modifications.

Recommended location:

```text
/data/config/overrides/
```

These override persistent defaults and image defaults.

Overrides should be documented and easy to inspect.

---

## Runtime Temporary Overrides

Runtime temporary overrides apply only to the current running system.

Examples:

- command-line debug flags
- temporary service overrides
- one-time recovery options

Runtime overrides should not silently become persistent.

---

# Ownership Model

Each configuration domain should have a clear owner.

## FoldingOS Core

Owns:

- boot behavior
- system service defaults
- default paths
- local platform behavior

---

## Networking

Owns:

- DHCP behavior
- static network configuration
- DNS configuration
- hostname application

---

## SSH

Owns:

- SSH enablement
- authorized keys
- root login policy
- password authentication policy

---

## Folding@home

Owns:

- Folding@home username
- team number
- passkey secret reference
- client configuration
- work directory configuration

Recommended persistent location:

```text
/data/config/foldinghome.toml
```

---

## FoldOps

This ownership domain applies when FoldOps integration is implemented after
v0.1.0. The initial release contains no FoldOps configuration or runtime state.

Owns:

- enrollment state
- API endpoint
- node identity used by FoldOps
- certificates or tokens
- remote management policy

Recommended persistent location:

```text
/data/config/foldops.toml
```

Runtime FoldOps state may live under:

```text
/data/foldops/
```

---

# Node Identity

Node identity must be generated once and persisted.

Recommended location:

```text
/data/config/node-id
```

The node identity should survive:

- reboot
- service restart
- operating system replacement
- image update

It should not change unless explicitly reset.

---

# Hostname Generation

If no hostname is configured, FoldingOS should generate one during first boot.

Recommended pattern:

```text
folding-XXXXXX
```

Where `XXXXXX` is generated from the node identity or another persistent
random value.

The generated hostname should be persisted in:

```text
/data/config/system.toml
```

---

# SSH Provisioning

SSH administrator and key provisioning behavior is defined by
[ADR-0007](0007-first-boot-administrator-and-ssh-provisioning.md).

Authorized keys should persist under:

```text
/data/config/ssh/authorized_keys
```

Direct root SSH login and SSH password authentication are disabled for v0.1.0.

---

# Folding@home Identity

Folding@home identity is separate from FoldingOS node identity.

Folding@home configuration may include:

- username
- team number
- passkey secret reference
- client options

Recommended location:

```text
/data/config/foldinghome.toml
```

This configuration should survive operating system replacement.

---

# FoldOps Enrollment

FoldOps enrollment is separate from local system configuration.

If no enrollment exists, the node should enter an unenrolled state while
continuing to operate locally where practical.

Enrollment data should be stored persistently.

Recommended location:

```text
/data/config/foldops.toml
```

---

# Factory Reset

Factory reset behavior must be explicit.

Potential reset modes:

## Configuration Reset

Deletes:

```text
/data/config/
```

May preserve:

```text
/data/fah/
```

so active Folding@home work is not unnecessarily lost.

## Full Reset

Deletes all persistent node data, including:

```text
/data/config/

/data/foldops/

/data/fah/

/data/state/

/data/logs/
```

Full reset should be clearly destructive.

---

# Recovery Behavior

If persistent configuration is missing or corrupted, FoldingOS should:

1. fall back to image defaults
2. regenerate missing non-secret identity where appropriate
3. log the recovery action
4. avoid destroying data automatically
5. continue booting where safe

Manual recovery should be possible through SSH or future recovery tooling.

---

# Configuration File Format

Structured FoldingOS configuration uses versioned TOML files separated by
ownership domain. Schema validation, unknown-key rejection, atomic activation,
last-known-good recovery, and migration behavior are defined by
[ADR-0011](0011-toml-configuration-validation-and-migration.md).

Opaque data such as SSH authorized keys, generated node identity, and secrets
remain in dedicated files or directories.

---

# Security Requirements

Configuration must not expose secrets through:

- logs
- diagnostics
- public repositories
- build artifacts

Sensitive values include:

- Folding@home passkeys
- FoldOps tokens
- private keys
- API credentials
- SSH private keys

Secrets should be stored only under `/data/config/secrets/` or in future
designated secure storage, never directly in TOML configuration.

---

# Consequences

## Positive

- deterministic behavior
- easier debugging
- clearer ownership
- better recovery
- safer updates
- improved maintainability

## Negative

- requires implementation discipline
- requires documentation updates when new configuration domains are added
- requires careful migration handling in future releases

These tradeoffs are acceptable.

---

# Alternatives Considered

## Single Global Configuration File

Rejected.

A single file becomes difficult to manage as the project grows and creates
unclear ownership boundaries.

## Fully Dynamic Remote Configuration

Rejected for initial releases.

Nodes must remain operational without FoldOps.

## Hardcoded Configuration

Rejected.

Hardcoded configuration prevents reliable fleet management and recovery.

---

# Future ADRs

Future ADRs may define:

- secret storage mechanism
- remote configuration synchronization
- FoldOps configuration authority
- factory reset command behavior

---

# Related Documents

- [Project charter](../../PROJECT_CHARTER.md)
- [Engineering principles](../../PRINCIPLES.md)
- [Security model](../security.md)
- [FoldOps integration](../foldops-integration.md)
- [Installer and first-boot scope](../installer.md)
- [ADR-0004: Partition and Persistence Layout](0004-partition-and-persistence-layout.md)
- [ADR-0011: TOML Configuration Validation And Migration](0011-toml-configuration-validation-and-migration.md)

---

# Closing Statement

Configuration must be boring, explicit, and predictable.

A FoldingOS node should always be able to explain where its configuration came
from and why a particular value is active.

Deterministic configuration is essential for reliable appliance behavior.
