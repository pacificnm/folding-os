# FoldingOS Coding Standards

Version: 0.1

Status: Living Document

---

# Purpose

This document defines the coding standards and engineering expectations for
all FoldingOS source code.

Contributor workflow and community expectations are defined in
[CONTRIBUTING.md](../CONTRIBUTING.md).

Project-wide decision guidance is defined in the
[engineering principles](../PRINCIPLES.md).

Consistency is considered a feature.

Readable code is preferred over clever code.

---

# Primary Principles

Code should be:

- Correct

- Readable

- Maintainable

- Predictable

- Well documented

- Testable

Future maintainers should understand the implementation without unnecessary
effort.

---

# Simplicity

Prefer:

Simple code.

Simple interfaces.

Simple APIs.

Simple algorithms.

Avoid unnecessary abstraction.

---

# Readability

Code should optimize for human readers.

Variable names should clearly communicate intent.

Avoid abbreviations unless universally understood.

Prefer:

currentTemperature

instead of:

tmp

or:

ct

---

# Functions

Functions should:

- perform one responsibility

- be easy to understand

- avoid unnecessary side effects

Prefer smaller functions over very large implementations.

---

# Classes

Classes should have one clear responsibility.

Large "god objects" should be avoided.

Composition should generally be preferred over excessive inheritance.

---

# Comments

Comments should explain:

WHY

rather than

WHAT

Bad:

// increment i

Good:

// Retry count is incremented to prevent infinite reconnect loops.

Code should remain readable without excessive comments.

---

# Error Handling

Errors should:

- be detected

- be logged

- provide useful diagnostics

Silent failure is discouraged.

Recoverable errors should recover gracefully.

---

# Logging

Logs should:

- provide operational value

- avoid unnecessary noise

- contain actionable information

Sensitive information must never be logged.

---

# Dependencies

Every dependency must justify its existence.

Before adding a dependency ask:

- Can existing code solve this?

- Is the maintenance burden justified?

- Is the security risk justified?

Prefer fewer dependencies.

---

# Configuration

Configuration should be:

- explicit

- documented

- version controlled

Avoid hidden configuration.

Avoid undocumented behavior.

---

# Security

Never:

- hardcode secrets

- hardcode credentials

- disable security validation

- bypass authentication

Security shortcuts are prohibited.

---

# Performance

Correctness first.

Reliability second.

Performance third.

Optimize only after measurement.

Avoid premature optimization.

---

# Testing

Critical functionality should be testable.

Tests should be:

- deterministic

- repeatable

- understandable

Testing requirements are defined in the
[testing strategy](testing-strategy.md).

---

# Documentation

Significant implementation changes should update:

- documentation

- ADRs

- comments where appropriate

Documentation should never be allowed to drift from implementation.

---
