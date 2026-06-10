# Contributing to FoldingOS

First, thank you for your interest in contributing to FoldingOS.

Whether you are fixing a typo, improving documentation, testing hardware,
reviewing code, or implementing new functionality, your contribution is
appreciated.

Our goal is to build a reliable, maintainable, and purpose-built operating
system dedicated to advancing Folding@home scientific research.

---

# Project Philosophy

FoldingOS is intentionally conservative.

We value:

- reliability
- simplicity
- security
- maintainability
- documentation
- reproducibility

over rapid feature development.

Engineering quality is more important than engineering speed.

---

# Before You Contribute

Please read:

- README.md

- PROJECT_CHARTER.md

- PRINCIPLES.md

- docs/vision.md

- docs/design-principles.md

- docs/architecture.md

These documents define the project's philosophy and architectural direction.

---

# Documentation First

Documentation is considered part of the implementation.

Significant changes should update:

- documentation

- ADRs

- comments where appropriate

Architecture should never exist only in source code.

---

# Simplicity Matters

When proposing changes, prefer:

- fewer dependencies

- fewer abstractions

- fewer moving parts

- simpler implementations

Feature count is not a project objective.

---

# Engineering Standards

Contributions should strive to be:

- readable

- maintainable

- deterministic

- well documented

- testable

Readable code is preferred over clever code.

---

# Pull Requests

Pull requests should:

- have a clear purpose

- remain reasonably focused

- avoid unrelated changes

- include documentation updates where required

- include tests where practical

Large architectural changes should be discussed before implementation.

---

# Architecture Decisions

Significant design decisions should be documented through Architecture
Decision Records (ADRs).

The objective is to document not only what was decided, but why.

---

# Dependencies

Every dependency should justify its existence.

Before introducing a dependency, consider:

- maintenance cost

- security implications

- build complexity

- long-term support

When practical, simpler solutions should be preferred.

---

# Security

Security-sensitive issues should be reported responsibly.

Contributors should avoid:

- committing secrets

- committing credentials

- committing private keys

- introducing insecure defaults

See SECURITY.md for additional guidance.

---

# Testing

Whenever practical:

- add tests

- improve tests

- automate tests

Every resolved defect should ideally reduce the likelihood of future
regressions.

---

# Coding Style

Please follow:

- docs/coding-standards.md

Consistency throughout the codebase is considered an important project goal.

---

# Respect Existing Design Principles

Before proposing significant changes, ask:

Does this improve:

- reliability?

- scientific contribution?

- maintainability?

- simplicity?

- security?

If not, reconsider whether the change belongs in FoldingOS.

---

# Community

Constructive discussion is encouraged.

Disagreement is expected.

Engineering decisions should be based on technical merit, evidence, and
project philosophy rather than personal preference.

Respectful collaboration is expected from all contributors.

---

# Thank You

FoldingOS exists because people are willing to contribute their time,
knowledge, and engineering expertise in support of scientific research.

Thank you for helping make the project better.