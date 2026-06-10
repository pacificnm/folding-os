# ADR-0007: First-Boot Administrator and SSH-Key Provisioning

**Status:** Accepted

**Version:** 1.0

**Date:** 2026-06-10

**Authors:** FoldingOS Project Contributors

---

# Context

FoldingOS requires a secure and usable administration path before FoldOps is
available.

The initial release is a headless appliance installed by flashing a complete
disk image. It must not ship default passwords, embedded administrator keys, or
an unauthenticated remote-management interface.

The provisioning workflow must support both initial access and recovery of
administrative access while remaining simple and deterministic.

---

# Decision

FoldingOS will create one dedicated administrative account:

```text
foldingos-admin
```

The account will:

- have no usable password credential
- authenticate to SSH using public keys only
- receive administrative privileges through an explicit sudo policy
- be the only interactive administrative account enabled by default

The account will use:

```text
Home: /home/foldingos-admin
Shell: /bin/sh
```

Direct root SSH login and SSH password authentication will be disabled.

No administrator public or private key will be embedded in release images.

FoldingOS v0.1.0 will use the OpenSSH server.

---

# Provisioning Channel

Administrator public keys will be provisioned through a file on the EFI System
Partition:

```text
/foldingos/provision/authorized_keys
```

After flashing the image, an administrator places one or more valid OpenSSH
public keys in this file before booting the target node.

When FoldingOS is running, the same file is visible at:

```text
/boot/efi/foldingos/provision/authorized_keys
```

The EFI System Partition is used because it can be accessed from another
machine without mounting the Linux root or data filesystems.

---

# Import Behavior

After the EFI and data partitions are mounted, and before OpenSSH starts, a
provisioning service will:

1. Detect the provisioning file.
2. Validate that it contains at least one supported public key.
3. Reject private keys and malformed entries.
4. Copy valid keys to persistent configuration:

   ```text
   /data/config/ssh/authorized_keys
   ```

5. Set the persistent key directory to `root:root` ownership and mode `0700`.
6. Set the persistent key file to `root:root` ownership and mode `0600`.
7. Configure OpenSSH to use the persistent key file for `foldingos-admin`.
8. Remove the provisioning file after a successful import.
9. Log the outcome without logging key material.

An invalid provisioning file must not replace an existing valid persistent key
set.

Key import replaces the persistent authorized-key set. Adding or rotating keys
therefore requires supplying the complete desired set.

---

# SSH Availability

SSH must not accept remote administrative access until at least one valid
administrator public key exists in persistent configuration.

If no valid key exists:

- the node may continue booting and running Folding@home
- the OpenSSH service must not start
- the missing-key condition must be visible through local diagnostics

Provisioning is checked on every boot so physical access to the EFI System
Partition can recover or rotate administrator keys.

---

# Administrative Privileges

The `foldingos-admin` account may perform required system-administration tasks
through sudo.

The v0.1.0 sudo policy permits passwordless full administrative access because
possession of an authorized administrator key represents administrative
authority.

The policy must:

- apply only to `foldingos-admin`
- require no password
- remain explicit and version controlled
- be reviewed as administrative tooling becomes more structured

Interactive login for service accounts, including `fah`, remains prohibited.

---

# Security Model

The workflow assumes that physical write access to the boot device grants the
ability to recover administrative access.

This is acceptable for the initial appliance model because physical access
already permits replacement or modification of the installed image.

The provisioning workflow must never:

- accept private keys
- create default passwords
- enable direct root SSH login
- enable SSH password authentication
- expose key material in logs
- silently discard an existing valid key set after failed validation

---

# Recovery

Administrative access is recovered by:

1. Powering down the node.
2. Accessing the boot device from a trusted machine.
3. Writing the desired public-key set to
   `/foldingos/provision/authorized_keys` on the EFI System Partition.
4. Booting the node.
5. Allowing the provisioning service to validate and import the keys.

This recovery path does not require FoldOps, an existing SSH credential, or a
default password.

---

# Alternatives Considered

## Default Administrator Password

Rejected because shared or predictable credentials create unacceptable risk.

## Direct Root SSH Login

Rejected because a named administrator account provides clearer policy and
future auditability.

## Interactive Local First-Boot Wizard

Rejected for v0.1.0 because it complicates unattended and headless deployment.

## FoldOps-Only Provisioning

Rejected because nodes must remain deployable and recoverable without FoldOps.

## Embedding Keys During Image Build

Rejected because release images must remain generic and must not contain
deployment-specific credentials.

---

# Consequences

## Positive

- no default credentials
- usable headless provisioning
- deterministic persistent key location
- recovery without FoldOps
- direct root SSH remains disabled
- release images remain deployment-independent

## Negative

- users must modify the EFI System Partition after flashing
- physical access can rotate administrator keys
- sudo becomes an additional runtime dependency
- malformed provisioning files require physical correction

---

# Future Considerations

Future ADRs may define:

- FoldOps-mediated key provisioning
- hardware-backed enrollment
- signed provisioning bundles
- multi-role administrator accounts
- restricted command-specific sudo policies
- local recovery-console policy

---

# Related Documents

- [Security model](../security.md)
- [Installer](../installer.md)
- [ADR-0005: Configuration Ownership and Precedence](0005-configuration-ownership-and-precedence.md)
- [v0.1.0 Scope Specification](../milestone/1-implementation-spec.md)

---

# Closing Statement

FoldingOS must be remotely manageable without shipping credentials or depending
on FoldOps.

Public-key provisioning through the EFI System Partition provides a simple,
explicit, and recoverable first-boot administration workflow.
