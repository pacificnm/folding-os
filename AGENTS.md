# FoldingOS Implementation Rules

These rules apply to all implementation agents working in this repository.

1. Read the relevant approved documentation before making implementation
   changes.
2. Follow accepted ADRs and approved implementation specifications exactly.
3. Do not introduce or substitute an alternate architecture pattern unless an
   accepted ADR or approved engineering specification explicitly authorizes it.
4. Do not introduce Buildroot external-tree architecture unless an accepted
   ADR or approved engineering specification explicitly authorizes it.
5. Do not substitute common practice, convention, or a perceived best practice
   for a documented project decision.
6. When documentation and common practice conflict, the approved project
   documentation wins.
7. When approved documents conflict, or when a required architectural decision
   is undocumented, stop implementation and surface the issue for resolution.
8. Architecture changes require an ADR or specification update before
   implementation.

The governing source roles and precedence are defined in
`doc/ai-context.md` and `doc/README.md`.
