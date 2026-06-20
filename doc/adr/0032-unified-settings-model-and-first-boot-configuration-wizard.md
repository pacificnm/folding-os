# ADR-0032: Unified Settings Model And First-Boot Configuration Wizard

**Status:** Proposed

**Date:** 2026-06-20

**Authors:** FoldingOS project

**Depends on:** [ADR-0005](0005-configuration-ownership-and-precedence.md),
[ADR-0026](0026-foldops-dashboard-operator-authentication.md),
[ADR-0031](0031-foldops-upgrades-dashboard-and-navigation.md)

**Related:** [Milestone 6 implementation specification](../milestone/6-implementation-spec.md),
[Milestone 6 engineering specification](../milestone/6-engineering-spec.md)

---

## Context

Milestone 5 still relies on scattered env files and bootstrap-time defaults for
FoldOps operator configuration. That is acceptable for a minimal update and
recovery release, but it is not a good long-term operator model.

Milestone 6 needs a single settings model that can:

- represent the operator-visible configuration for FoldOps and supervisor
- support first-boot initialization and later edits through the dashboard
- preserve the configuration ownership and precedence rules already established
- remain compatible with the existing runtime env-file consumers

The project already prefers schema-versioned TOML for structured configuration.
That model should be extended rather than replaced.

---

## Decision

FoldingOS will introduce a **unified, schema-versioned FoldOps settings model**
backed by TOML and rendered into derived runtime files.

### 1. Canonical settings store

The dashboard and supervisor will treat a single structured settings document as
the source of truth for operator-managed FoldOps configuration.

### 2. Derived runtime files

Runtime env files and compatibility fragments may still exist, but they will be
generated from the canonical settings model rather than edited independently.

### 3. First-boot configuration wizard

Milestone 6 will add a first-boot wizard that initializes the canonical
settings model and captures the minimum operator choices required for a usable
supervisor installation.

### 4. Feature flag handling

Feature flags belong in the unified settings model when they are operator-
meaningful and affect dashboard or service behavior. Transient process-specific
values remain out of scope.

### 5. Ownership and validation

Settings ownership stays explicit. Validation must fail closed on malformed or
incompatible values, and the dashboard must not partially activate invalid
configuration.

---

## Alternatives Considered

### Continue scattering configuration across env files

Rejected. It keeps the current bootstrap state but makes the dashboard harder
to reason about and harder to validate.

### Move everything into a generic key-value store

Rejected. That weakens the ownership model and makes validation less explicit.

---

## Consequences

### Positive

- reduces the number of configuration surfaces operators must understand
- supports first-boot setup and later edits with the same model
- keeps the runtime contract compatible with existing env-file consumers

### Negative

- requires a migration path from current env-file inputs
- adds a generation step between canonical settings and runtime files

