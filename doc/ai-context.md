# FoldingOS AI Context

> This document provides the authoritative context for AI assistants working on
> the FoldingOS project.

Version: 0.1
Status: Living Document

---

# Project Overview

FoldingOS is an open-source, purpose-built operating system designed exclusively
for running Folding@home compute nodes.

The objective is not to create another Linux distribution.

The objective is to create the best possible platform for contributing
computational resources to Folding@home.

---

# Mission

Maximize scientific contribution through:

- simplicity
- reliability
- security
- maintainability
- reproducibility

while minimizing:

- complexity
- administration
- attack surface
- resource consumption

---

# Core Philosophy

FoldingOS is an appliance.

It should behave more like:

- a router
- a firewall
- an embedded controller

than a desktop operating system.

Users should deploy it and let it run.

---

# Primary Goals

- Fast deployment
- Stable operation
- Minimal maintenance
- Secure by default
- Fleet management
- Long-term reliability

---

# Non Goals

FoldingOS is NOT:

- Desktop Linux
- Ubuntu replacement
- Debian replacement
- Development workstation
- Gaming platform
- NAS
- Media server
- Docker platform
- Kubernetes platform
- Browser platform

Do not recommend features that move the project toward these goals.

---

# Engineering Priorities

Priority order:

1. Reliability

2. Scientific contribution

3. Security

4. Simplicity

5. Maintainability

6. Performance

7. Features

Feature count is NOT a success metric.

---

# Documentation Philosophy

Documentation is authoritative.

Implementation follows documentation.

Architecture changes require documentation updates.

Major decisions require ADRs.

Never introduce undocumented behavior.

---

# Coding Philosophy

Prefer:

- simple code

- readable code

- maintainable code

- deterministic behavior

- explicit behavior

Avoid:

- unnecessary abstraction

- unnecessary dependencies

- clever implementations

- framework bloat

- magic behavior

Humans should always be able to understand the code.

---

# Security Philosophy

Default to:

- least privilege

- minimal services

- authenticated management

- encrypted communications

- secure defaults

Security is not optional.

---

# Build Philosophy

Builds should be:

- reproducible

- deterministic

- automated

- version controlled

No undocumented build process should exist.

---

# Runtime Philosophy

The operating system should:

Boot

↓

Initialize

↓

Connect

↓

Fold

↓

Report Status

↓

Continue Folding

No unnecessary runtime components should exist.

---

# Failure Philosophy

Recover whenever possible.

Log failures clearly.

Do not lose Folding progress unnecessarily.

Graceful degradation is preferred over catastrophic failure.

---

# AI Assistant Rules

Before making architectural changes:

- Read relevant documentation.

- Read relevant ADRs.

- Preserve project philosophy.

- Avoid feature creep.

- Explain tradeoffs.

- Keep solutions simple.

If uncertain:

Ask.

Do not invent architecture.

Do not assume requirements.

---

# Ultimate Objective

Create the simplest, most reliable operating system dedicated to advancing
Folding@home scientific research.

Every design decision should reinforce that mission.