# ADR-0002: Use systemd for Init and Service Supervision

**Status:** Accepted

**Version:** 1.0

**Date:** 2026-06-10

**Authors:** FoldingOS Project Contributors

---

# Context

FoldingOS requires an init system responsible for:

* boot sequencing
* service startup
* dependency ordering
* service supervision
* restart policies
* shutdown sequencing
* operational reliability

Because FoldingOS is intended to operate unattended for extended periods,
the init system is a critical architectural component.

The chosen solution must prioritize reliability and maintainability over
minimal size alone.

---

# Decision

FoldingOS will use **systemd** as its init and service supervision system.

systemd will be responsible for:

* PID 1
* service management
* dependency ordering
* automatic service restart
* boot target management
* service status reporting
* shutdown coordination

The project will use standard systemd unit files rather than creating custom
service supervision infrastructure.

---

# Rationale

Although lightweight alternatives exist, systemd offers significant practical
advantages for the FoldingOS mission.

Advantages include:

* mature and widely deployed
* excellent Buildroot integration
* strong long-term maintenance
* robust dependency management
* reliable service supervision
* automatic restart capabilities
* comprehensive operational tooling
* predictable startup sequencing
* well-understood contributor ecosystem

The modest increase in footprint is acceptable given modern target hardware
and the project's emphasis on operational reliability.

---

# Intended Usage

FoldingOS will intentionally use only a limited subset of systemd
capabilities.

Expected usage includes:

* system boot
* service supervision
* dependency ordering
* restart policies
* service state inspection

FoldingOS should avoid unnecessary complexity simply because systemd
provides additional functionality.

The presence of a feature does not justify its use.

---

# Alternatives Considered

## s6

Advantages:

* extremely lightweight
* simple architecture
* excellent performance
* small footprint

Disadvantages:

* smaller contributor ecosystem
* additional contributor learning curve
* less common operational familiarity
* increased engineering burden

Decision:

Rejected for the initial implementation.

---

## OpenRC

Advantages:

* lightweight
* mature
* understandable

Disadvantages:

* less integrated supervision model
* smaller ecosystem
* fewer operational advantages for this project

Decision:

Rejected.

---

## BusyBox init

Advantages:

* extremely small
* minimal dependencies

Disadvantages:

* limited supervision features
* limited dependency management
* additional engineering effort required

Decision:

Rejected.

---

## Custom Init System

Advantages:

* complete control

Disadvantages:

* unnecessary engineering effort
* increased maintenance burden
* unnecessary project risk

Decision:

Rejected.

FoldingOS should not reinvent mature infrastructure without compelling
technical justification.

---

# Consequences

## Positive

* robust service supervision
* automatic restart handling
* mature implementation
* strong documentation
* large contributor familiarity
* predictable service ordering
* excellent Buildroot compatibility

## Negative

* larger runtime footprint
* additional functionality beyond project needs
* greater implementation complexity than minimal alternatives

These tradeoffs are considered acceptable.

---

# Initial Boot Ordering

The expected high-level startup sequence is:

Firmware

↓

Bootloader

↓

Linux Kernel

↓

systemd

↓

Filesystem Initialization

↓

Networking

↓

Time Synchronization

↓

FoldOps Agent (when enabled)

↓

Folding@home

↓

Operational State

Specific unit dependencies will be documented separately.

Folding@home must not depend on FoldOps availability.

---

# Service Philosophy

Every service should justify its existence.

Services should:

* have a single responsibility
* start deterministically
* fail visibly
* restart when appropriate
* log useful diagnostics

Unnecessary background services should not be enabled.

---

# Restart Policy

Critical services should support automatic restart where appropriate.

Repeated failures should be:

* observable
* logged
* diagnosable

Restart loops should be bounded and detectable.

---

# Logging Philosophy

Service logs should support:

* diagnostics
* troubleshooting
* operational visibility

Sensitive information should never be exposed through logs.

Persistent logging, retention, and disk-full behavior are defined by
[ADR-0010](0010-persistent-logging-and-retention.md).

---

# Future Review

This decision should be reconsidered only if:

* project requirements fundamentally change
* systemd becomes unsuitable for long-term maintenance
* measurable engineering evidence demonstrates a superior alternative

Changes should be justified by objective technical benefit rather than
personal preference or industry trends.

---

# Related Documents

* [Project charter](../../PROJECT_CHARTER.md)
* [Engineering principles](../../PRINCIPLES.md)
* [Architecture](../architecture.md)
* [Boot process](../boot-process.md)
* [Build system](../build-system.md)
* [Security model](../security.md)

---

# Closing Statement

FoldingOS values reliability over minimalism.

The project adopts systemd because it provides a mature, predictable, and
well-supported foundation for long-term unattended operation while allowing
the project to focus its engineering effort on scientific contribution rather
than infrastructure reinvention.
