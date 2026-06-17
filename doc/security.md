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

Primary operator interface (Milestone 4 target):

- FoldOps supervisor HTTPS dashboard and `/api/*` routes per
  [ADR-0027](adr/0027-foldops-remote-operator-api.md)
- dashboard operator login with mandatory password change on first use per
  [ADR-0026](adr/0026-foldops-dashboard-operator-authentication.md)

Break-glass and development administration:

- SSH public-key access for `foldingos-admin` per
  [ADR-0007](adr/0007-first-boot-administrator-and-ssh-provisioning.md)

Requirements:

- encrypted communication for remote management
- authenticated access for dashboard and machine ingest
- public-key authentication for SSH by default
- no direct root SSH login
- no SSH password authentication for v0.1.0
- no release-image embedded administrator SSH keys
- future MFA support through external systems where appropriate

Machine-to-machine fleet authentication uses `INGEST_TOKEN` / `AGENT_TOKEN` per
[ADR-0019](adr/0019-foldops-supervisor-provisioning-and-tls.md). Human operator
sessions are separate from ingest secrets per
[ADR-0026](adr/0026-foldops-dashboard-operator-authentication.md).

Initial SSH key provisioning is defined by
[ADR-0007](adr/0007-first-boot-administrator-and-ssh-provisioning.md). SSH is
optional for operators who manage the fleet primarily through the web UI.

Network fleet provisioning exposes destructive installation only through
authenticated supervisor-approved requests. Provisioning must reject
unverified images, authenticate enrollment, and write only to the selected
target as defined by
[ADR-0016](adr/0016-network-provisioning-via-supervisor.md).

Supervisor-role installations enable the FoldOps web interface by default, but
it must not become remotely available until TLS provisioning succeeds and
operator authentication is configured per
[ADR-0019](adr/0019-foldops-supervisor-provisioning-and-tls.md) and
[ADR-0026](adr/0026-foldops-dashboard-operator-authentication.md).

---

# Network Exposure

By default, only explicitly required services should be accessible.

Milestone 3 supervisor appliances expose:

| Service | Port | When |
| --- | --- | --- |
| FoldingOS provisioning API | `8743` (default) | After supervisor bootstrap |
| FoldOps HTTPS front end | `3443` | After `foldops provision` succeeds |
| SSH | `22` | After administrator key import |

The FoldOps dashboard listens on `0.0.0.0:3443` only after ingest-token import
and self-signed TLS generation succeed. The FoldOps supervisor process itself
binds to loopback `:3000` only.

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
