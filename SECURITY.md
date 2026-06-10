# FoldingOS Security Policy

Version: 0.1

Status: Active

---

# Overview

The FoldingOS project takes security seriously.

Our objective is to build a reliable, secure, and maintainable operating system
dedicated to advancing Folding@home scientific research.

We welcome responsible disclosure of security issues and will work with
reporters to investigate, validate, and resolve legitimate vulnerabilities.

This file defines project security policy, vulnerability reporting, and secure
development expectations. The technical security architecture is defined in the
[security model](doc/security.md).

---

# Reporting Security Vulnerabilities

If you believe you have discovered a security vulnerability in FoldingOS,
please report it responsibly.

Please include as much information as possible:

- description of the issue
- affected component
- affected version
- reproduction steps
- proof of concept if available
- potential impact
- suggested mitigation (optional)

---

# Responsible Disclosure

Please do not publicly disclose security vulnerabilities until the project has
had a reasonable opportunity to investigate and address the issue.

Responsible disclosure helps protect users and contributors while fixes are
developed.

---

# Secure Development

Contributors are encouraged to:

- validate input
- check return values
- handle errors explicitly
- avoid undefined behavior
- avoid unnecessary privilege escalation
- document security-sensitive decisions

Readable code is considered a security advantage.

---

# Security Updates

When security issues are addressed:

- documentation should be updated

- release notes should identify fixes when appropriate

- affected versions should be documented

- regression testing should be considered

---

# Scope

This policy currently applies to:

- FoldingOS source code

- build system

- release artifacts

- official documentation

- project-maintained tooling

Third-party software remains subject to its own security policies and licensing.

---

# Contact

Project security contact information may be updated as the project evolves.

Until a dedicated security contact is established, project maintainers should
coordinate responsible handling of reported vulnerabilities.

---

# Commitment

The FoldingOS project is committed to continuous improvement in security,
documentation, and engineering quality.

Security is not considered a one-time activity.

It is an ongoing engineering responsibility shared by every contributor.
