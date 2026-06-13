# ADR-0015: Local Commissioning Display

**Status:** Accepted

**Version:** 1.0

**Date:** 2026-06-13

**Authors:** FoldingOS Project Contributors

---

# Context

FoldingOS is a headless appliance. Normal administration uses SSH with
public-key authentication. Production nodes are not expected to keep a monitor
or keyboard attached.

During commissioning, installation, and field troubleshooting, operators still
need a simple way to confirm that a node finished booting and to learn its
current network address without router access or a working SSH client.

The initial foundation image targeted minimal kernel drivers and did not enable
a local framebuffer console. On many physical UEFI systems, including validated
Dell hardware, that produced a blank display after GRUB even when the node was
healthy and reachable over SSH.

The project does not need:

- a desktop environment
- local keyboard interaction
- password-based console login
- full GPU driver support

The project does need:

- visible kernel and service boot messages on a temporarily attached monitor
- a final commissioning message showing the node address and SSH entry point

---

# Decision

FoldingOS will provide a **local commissioning display** on UEFI x86_64
systems using the firmware-provided EFI framebuffer.

## Display stack

The kernel must enable a minimal built-in display path sufficient for text
output on `tty1`:

- EFI framebuffer support
- simpledrm or equivalent firmware-backed framebuffer driver
- virtual terminal and framebuffer console support

Full GPU drivers such as `i915`, `amdgpu`, or `nouveau` are out of scope for
this decision.

## Boot messages

Kernel and service boot messages continue to use the existing appliance console
configuration:

```text
console=tty1 console=ttyS0,115200
```

Operators who temporarily attach a monitor should see normal boot progress on
`tty1`.

## Final commissioning message

After the node reaches `network-online.target` and has a routable DHCP IPv4
address on wired Ethernet, a one-shot appliance service writes a final status
message to `/dev/tty1` and `/dev/console`:

```text
FoldingOS 0.1.0 ready
Address: 192.168.4.32
SSH: foldingos-admin@192.168.4.32
```

Rules:

- `FoldingOS 0.1.0` comes from `/usr/lib/os-release`
- `192.168.4.32` is an example; the message must print the actual selected
  routable IPv4 address
- if no routable IPv4 address is available, the service writes a failure status
  instead of a fabricated address
- the message is informational only and does not enable local login
- the message may be rewritten on later successful network-state changes during
  the same boot, but the ready format remains the same

## No local login

This decision does not add:

- `getty` login on `tty1`
- auto-login shells
- console passwords
- keyboard-driven administration

SSH with provisioned public keys remains the only supported remote-administration
path, as defined by
[ADR-0007](0007-first-boot-administrator-and-ssh-provisioning.md).

---

# Alternatives Considered

## SSH-only commissioning

Rejected as the sole model because it forces router inspection or another
machine during initial field setup, and it made healthy nodes appear failed when
a monitor was attached.

## Full GPU drivers

Rejected for v0.1.0 because it increases image size, firmware exposure, and
hardware-validation burden without improving the commissioning use case.

## Local console login or auto-login

Rejected because production nodes will not keep keyboards attached, and
auto-login would create an unnecessary physical-access administrative path.

## Persistent on-screen status application

Rejected as over-engineering. A one-shot boot-status service is sufficient.

---

# Consequences

## Positive

- operators can confirm successful boot with a temporarily attached monitor
- DHCP address discovery does not require router or SSH access
- implementation remains small and aligned with the appliance model
- Milestone 3 installer work can reuse the same local display path

## Negative

- the display path depends on UEFI GOP behavior and may not work on every
  unsupported platform
- unsupported hardware may still show no local output
- the ready message exposes the node IPv4 address to anyone with physical
  monitor access

## Tradeoffs

- commissioning convenience is preferred over strict minimization of kernel
  display support
- physical monitor access is treated as a commissioning aid, not an
  administration interface

---

# Future Considerations

- installer mode may add additional local-console interaction on the same
  display path
- serial-console commissioning remains supported through `ttyS0`
- full GPU drivers may be considered later only with explicit hardware-support
  justification

---

# References

- [ADR-0003: x86_64 Bootloader and Image Format](0003-x86_64-bootloader-and-image-format.md)
- [ADR-0007: First-Boot Administrator and SSH-Key Provisioning](0007-first-boot-administrator-and-ssh-provisioning.md)
- [Boot process](../boot-process.md)
- [Operations](../operations.md)
