try:
    from mcp.server.fastmcp import FastMCP
except ModuleNotFoundError as error:
    raise SystemExit(
        "Missing Python dependency for FoldingOS context memory MCP. "
        "Run it with .venv/bin/python or install the project Python dependencies."
    ) from error

from context_memory import format_entry, get_context, list_context, save_context, search_context


mcp = FastMCP("foldingos-context-memory")


@mcp.tool()
def save_context_memory(
    content: str,
    title: str = "",
    session_key: str = "",
    tags: list[str] | None = None,
) -> str:
    """Save agent session context for later retrieval after context compaction."""
    entry_id = save_context(
        content,
        title=title,
        session_key=session_key,
        tags=tags,
    )
    return f"Saved context memory entry id={entry_id}."


@mcp.tool()
def search_context_memory(
    query: str,
    limit: int = 8,
    session_key: str = "",
) -> str:
    """Search saved agent context by semantic similarity."""
    rows = search_context(query, limit=limit, session_key=session_key)
    if not rows:
        return "No matching context memory found."

    return "\n\n".join(format_entry(*row) for row in rows)


@mcp.tool()
def list_context_memory(limit: int = 20, session_key: str = "") -> str:
    """List recent saved agent context entries, newest first."""
    rows = list_context(limit=limit, session_key=session_key)
    if not rows:
        return "No context memory entries found."

    return "\n\n".join(format_entry(*row, content_limit=500) for row in rows)


@mcp.tool()
def get_context_memory(entry_id: int) -> str:
    """Retrieve one saved context entry by id."""
    row = get_context(entry_id)
    if row is None:
        return f"No context memory entry with id={entry_id}."

    return format_entry(*row, content_limit=10000)


if __name__ == "__main__":
    mcp.run()
