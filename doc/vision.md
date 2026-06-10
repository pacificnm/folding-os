# FoldingOS Vision

## Purpose

This document describes the intended user experience, operational outcomes, and
long-term direction of FoldingOS.

Project scope and mission are defined in the
[project charter](../PROJECT_CHARTER.md). Engineering decisions are guided by
the [engineering principles](../PRINCIPLES.md).

---

# Vision Statement

> **To become the simplest, most reliable, and most maintainable operating
> system dedicated exclusively to advancing Folding@home scientific research.**

FoldingOS should feel like a purpose-built scientific appliance rather than a
traditional Linux distribution.

---

# Intended Experience

The ideal deployment process should resemble:

```text
Download

↓

Flash

↓

Boot

↓

Configure

↓

Fold
```

Users should not need extensive Linux knowledge or ongoing administration.

Nodes should operate unattended for extended periods and recover automatically
from routine failures whenever practical.

---

# Operational Outcomes

FoldingOS should provide:

- predictable deployment
- reliable Folding@home operation
- automatic recovery after reboot and temporary service interruption
- preservation of Folding@home work across expected failures
- secure remote administration
- authenticated and integrity-verified production updates once updates are
  enabled
- clear diagnostics when automatic recovery is not possible

The system should remain understandable and maintainable by an individual
engineer.

---

# Fleet-Oriented Direction

FoldingOS should work well for a single machine while scaling naturally to:

- home laboratories
- educational environments
- research clusters
- enterprise-scale Folding@home deployments

FoldOps integration should reduce per-node administration without making node
operation dependent on FoldOps availability.

---

# Long-Term Direction

The project should steadily improve:

- ease of deployment
- unattended reliability
- recoverability
- hardware validation
- fleet visibility
- update safety
- documentation quality

New capability should support these outcomes without turning FoldingOS into a
general-purpose operating system.

---

# Success Criteria

Success should be measured by:

- scientific contribution
- operational stability
- ease of deployment
- ease of maintenance
- recovery from expected failures
- security
- long-term reliability
- code quality
- documentation quality

Package count, feature count, benchmark scores, and desktop usability are not
success metrics.

---

# Closing Statement

FoldingOS succeeds when a node can be deployed with confidence, contribute
scientific work reliably, and demand little ongoing attention.
