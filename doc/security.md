# FoldingOS Security Model

Version: 0.1

Status: Living Document

---

# Purpose

Security is a core design principle of FoldingOS, not an optional feature.

The objective is to minimize attack surface while providing a reliable platform
for scientific computation and centralized fleet management.

Vulnerability reporting, disclosure, and project security policy are defined in
[SECURITY.md](../SECURITY.md).

---

# Design Principles

Security decisions should prioritize:

- least privilege
- secure defaults
- minimal attack surface
- authenticated management
- encrypted communication
- deterministic behavior

Complexity should never be introduced solely in the name of security if it
reduces maintainability or reliability.

---

# Default Security Posture

A default FoldingOS installation should:

- expose the minimum required services
- avoid unnecessary packages
- avoid unnecessary users
- avoid unnecessary daemons
- minimize open ports
- require authenticated administration

---

# Remote Management

Primary remote administration:

- SSH

Requirements:

- encrypted communication
- authenticated access
- public-key authentication by default
- no direct root SSH login
- no SSH password authentication for v0.1.0
- future MFA support through external systems where appropriate

Initial administrator provisioning is defined by
[ADR-0007](adr/0007-first-boot-administrator-and-ssh-provisioning.md).

Installer-capable releases expose destructive installation capability only
through explicitly selected local-console installer mode. Installer mode must
exclude its source boot device, require target-specific destructive
confirmation, and write only to the selected target as defined by
[ADR-0013](adr/0013-combined-appliance-and-installer-image.md).

---

# Network Exposure

By default, only explicitly required services should be accessible.

Future versions should support:

- configurable firewall policies
- configurable management restrictions
- optional network segmentation

---

# Secrets

Secrets must never:

- be hardcoded
- be committed to source control
- appear in logs
- appear in diagnostics

Configuration should support secure secret injection where appropriate.

Structured TOML configuration must contain references to secrets rather than
secret values. Secrets reside under `/data/config/secrets/` or a future
designated secure store. Configuration updates must pass local validation and
cannot override security invariants. See
[ADR-0011](adr/0011-toml-configuration-validation-and-migration.md).

---

# Updates

Production update mechanisms must support:

- cryptographic signing
- authenticity verification
- integrity verification
- rollback capability

Unsigned update mechanisms should not be trusted.

Updates are not part of the initial bootable-base milestone. These requirements
become mandatory when production update capability is introduced.

---

# Logging

Logs should:

- support diagnostics
- avoid exposing secrets
- avoid exposing credentials
- avoid exposing authentication tokens

Sensitive information must be protected.

FoldingOS uses bounded persistent `systemd-journald` storage. Services must
redact sensitive values before logging, and persistent logging failure must not
cause unsafe data deletion or stop Folding@home. See
[ADR-0010](adr/0010-persistent-logging-and-retention.md).

---

# Principle of Least Privilege

Processes should execute with the minimum privileges required.

Unnecessary elevated privileges should be avoided.

---

# Dependency Philosophy

Every dependency increases:

- attack surface
- maintenance burden
- supply chain risk

Dependencies should be minimized and reviewed carefully.

---

# Cryptography

Where cryptography is required:

- use well-established algorithms
- use well-maintained libraries
- avoid custom cryptographic implementations

Cryptographic correctness should be preferred over novelty.

---

# Future Considerations

Potential future capabilities:

- Secure Boot

- TPM integration

- measured boot

- hardware-backed identity

These should only be adopted when they provide measurable value.

---

# Summary

Security is achieved through simplicity, minimalism, careful engineering, and
well-documented architecture rather than unnecessary complexity.
