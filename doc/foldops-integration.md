# FoldOps Integration

Version: 0.1

Status: Living Document

---

# Purpose

FoldOps provides centralized management, monitoring, and operational visibility
for FoldingOS deployments.

This document defines the intended architectural relationship between FoldingOS
and FoldOps.

---

# Design Philosophy

FoldingOS should remain lightweight.

Management complexity belongs in FoldOps rather than on individual nodes.

Nodes should remain simple appliances.

---

# Objectives

FoldOps should enable:

- centralized monitoring
- fleet management
- node inventory
- health reporting
- diagnostics
- update coordination
- remote configuration

without increasing unnecessary complexity on individual nodes.

---

# Node Identity

Each FoldingOS installation should possess a unique identity.

Future implementation details remain subject to ADR approval.

Potential identity sources include:

- generated UUID

- TPM identity

- hardware identity

- cryptographic key pairs

---

# Health Reporting

Potential metrics include:

- uptime

- CPU usage

- memory usage

- storage usage

- temperature

- Folding status

- work unit information

- estimated PPD

- software version

- update status

---

# Inventory

FoldOps should maintain inventory information including:

- hostname

- architecture

- operating system version

- hardware information

- CPU details

- memory

- storage

- network interfaces

---

# Remote Configuration

Future capabilities may include:

- configuration updates

- node naming

- grouping

- labels

- maintenance mode

Configuration changes should remain explicit and auditable.

---

# Updates

FoldOps may coordinate:

- update discovery

- update scheduling

- staged rollout

- rollback

- deployment status

Actual update implementation is documented separately.

---

# Security

Communication should be:

- authenticated

- encrypted

- verifiable

Nodes should never trust unauthenticated management requests.

---

# Failure Philosophy

Failure of FoldOps should not prevent:

- node boot

- Folding startup

- continued Folding operation

Nodes should continue contributing scientific computation independently.

---

# Long-Term Vision

A single FoldOps deployment should eventually manage:

- one node

- ten nodes

- hundreds of nodes

- thousands of nodes

with a consistent operational model.

---

# Summary

FoldOps exists to simplify management.

FoldingOS exists to perform computation.

The separation of responsibilities should remain clear throughout the lifetime
of the project.