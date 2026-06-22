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

# FoldingOS / FoldOps Agent Instructions

Before making code changes, retrieve project context for the current task.

If the MCP tool `search_project_memory` is available, use it first. Search
project memory for:

- current milestone
- affected subsystem
- related architecture decisions
- known issues
- build or packaging instructions

If `search_project_memory` is not available, do not stop. Fall back to local
repository search and read the relevant documents directly.

If the context-memory MCP tools are available, use them to preserve and recover
session state across context compaction:

- Before compaction or at major task boundaries, call `save_context_memory`
  with a short `title`, optional `session_key`, and the decisions, file paths,
  blockers, and next steps the next agent turn will need.
- After compaction or when prior chat detail is missing, call
  `search_context_memory` or `list_context_memory`, then `get_context_memory`
  for full entries.
- Use the same `session_key` within one task thread when possible.

Recommended fallback searches:

```bash
grep -RIn "<subsystem-or-error-text>" doc packages overlay configs scripts
grep -RIn "Milestone 5\|Update and Recovery\|FoldOps\|Folding@home" doc
grep -RIn "ADR-00\|Status: Accepted\|Status: Proposed" doc/adr
```

Relevant fallback files:

- `doc/ai-context.md`
- `doc/README.md`
- `doc/operations.md`
- `doc/foldingosctl.md`
- affected `doc/adr/*.md`
- affected `doc/milestone/*.md`
- `BUILD_COMMANDS.md` for the live build and supervisor USB workflow

Binding order:
1. /doc specifications
2. MCP project memory
3. AGENTS.md
4. DECISIONS.md
5. KNOWN_ISSUES.md
6. Existing code behavior

Do not invent architecture.
Do not change public APIs unless the spec requires it.
Run build/tests before final response.
Report changed files and verification results.
