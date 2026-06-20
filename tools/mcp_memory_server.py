try:
    import psycopg
    from mcp.server.fastmcp import FastMCP
except ModuleNotFoundError as error:
    raise SystemExit(
        "Missing Python dependency for FoldingOS memory MCP. "
        "Run it with .venv/bin/python or install the project Python dependencies."
    ) from error

from memory_common import database_url, vector_literal


mcp = FastMCP("foldingos-memory")


@mcp.tool()
def search_project_memory(query: str, limit: int = 8) -> str:
    """Search FoldingOS project memory for specs, decisions, issues, and build notes."""
    from openai import OpenAI

    client = OpenAI()
    embedding = client.embeddings.create(
        model="text-embedding-3-small",
        input=query,
    ).data[0].embedding

    with psycopg.connect(database_url()) as conn:
        rows = conn.execute(
            """
            SELECT source_path, content
            FROM project_memory
            ORDER BY embedding <=> %s::vector
            LIMIT %s
            """,
            (vector_literal(embedding), limit),
        ).fetchall()

    if not rows:
        return "No matching project memory found."

    output = []
    for source_path, content in rows:
        output.append(f"--- {source_path} ---\n{content[:2000]}")

    return "\n\n".join(output)


if __name__ == "__main__":
    mcp.run()
