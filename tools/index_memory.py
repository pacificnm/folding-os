import hashlib
import os
from pathlib import Path

import psycopg
from openai import OpenAI

client = OpenAI()

DB_URL = os.environ.get(
    "DATABASE_URL",
    "postgresql://postgres@localhost:5432/foldingos_memory",
)

PATHS = [
    "AGENTS.md",
    "DECISIONS.md",
    "KNOWN_ISSUES.md",
    "BUILD_COMMANDS.md",
    "README.md",
    "doc",
    "docs",
]


def chunks(text, size=1800, overlap=200):
    i = 0
    while i < len(text):
        yield text[i : i + size]
        i += size - overlap


def embed(text):
    result = client.embeddings.create(
        model="text-embedding-3-small",
        input=text,
    )
    return result.data[0].embedding


with psycopg.connect(DB_URL) as conn:
    for root in PATHS:
        path = Path(root)

        if path.is_dir():
            files = list(path.rglob("*.md"))
        elif path.exists():
            files = [path]
        else:
            continue

        for file in files:
            text = file.read_text(errors="ignore")

            for chunk in chunks(text):
                content_hash = hashlib.sha256(
                    f"{file}:{chunk}".encode()
                ).hexdigest()

                vector = embed(chunk)

                conn.execute(
                    """
                    INSERT INTO project_memory
                      (source_path, content, content_hash, embedding)
                    VALUES
                      (%s, %s, %s, %s)
                    ON CONFLICT (content_hash) DO NOTHING
                    """,
                    (str(file), chunk, content_hash, vector),
                )

    conn.commit()

print("Memory indexing complete.")
