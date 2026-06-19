import os
import sys

import psycopg
from openai import OpenAI

client = OpenAI()

DB_URL = os.environ.get(
    "DATABASE_URL",
    "postgresql://postgres@localhost:5432/foldingos_memory",
)

query = " ".join(sys.argv[1:]).strip()

if not query:
    print('Usage: python tools/search_memory.py "your query"')
    sys.exit(1)

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
        LIMIT 8
        """,
        (embedding,),
    ).fetchall()

for source_path, content in rows:
    print(f"\n--- {source_path} ---\n")
    print(content[:2000])
