import os
import psycopg
from openai import OpenAI
from mcp.server.fastmcp import FastMCP

mcp = FastMCP("foldingos-memory")
client = OpenAI()

DB_URL = os.environ.get(
    "DATABASE_URL",
    "postgresql:///foldingos_memory?host=/var/run/postgresql",
)

@mcp.tool()
def search_project_memory(query: str, limit: int = 8) -> str:
    """Search FoldingOS project memory for relevant specs, decisions, known issues, and build notes."""
    embedding = client.embeddings.create(
        model="text-embedding-3-small",
        input=query,
    ).data[0].embedding

    with psycopg.connect(DB_URL) as conn:
        rows = conn.execute(
            """
            SELECT source_path, content
            FROM project_memory
            ORDER BY embedding <=> %s::vector
            LIMIT %s
            """,
            (embedding, limit),
        ).fetchall()

    if not rows:
        return "No matching project memory found."

    output = []
    for source_path, content in rows:
        output.append(f"--- {source_path} ---\n{content[:2000]}")

    return "\n\n".join(output)

if __name__ == "__main__":
    mcp.run()
