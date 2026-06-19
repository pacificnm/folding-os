import sys

from memory_common import database_url, vector_literal


query = " ".join(sys.argv[1:]).strip()

if not query:
    print('Usage: .venv/bin/python tools/search_memory.py "your query"')
    sys.exit(1)

try:
    import psycopg
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
            LIMIT 8
            """,
            (vector_literal(embedding),),
        ).fetchall()
except Exception as error:
    print(f"ERROR: memory search failed: {error}", file=sys.stderr)
    sys.exit(1)

for source_path, content in rows:
    print(f"\n--- {source_path} ---\n")
    print(content[:2000])
