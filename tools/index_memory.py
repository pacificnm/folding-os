import hashlib
import sys
from pathlib import Path

from memory_common import database_url, vector_literal


PATHS = [
    "AGENTS.md",
    "DECISIONS.md",
    "KNOWN_ISSUES.md",
    "BUILD_COMMANDS.md",
    "README.md",
    "PRINCIPLES.md",
    "doc",
    "docs",
    "wiki",
]


def chunks(text, size=1800, overlap=200):
    i = 0
    while i < len(text):
        yield text[i : i + size]
        i += size - overlap


def embed(text):
    from openai import OpenAI

    client = OpenAI()
    result = client.embeddings.create(
        model="text-embedding-3-small",
        input=text,
    )
    return vector_literal(result.data[0].embedding)


def main() -> int:
    import psycopg

    project_root = Path(__file__).resolve().parents[1]

    with psycopg.connect(database_url()) as conn:
        for root in PATHS:
            path = project_root / root

            if path.is_dir():
                files = list(path.rglob("*.md"))
            elif path.exists():
                files = [path]
            else:
                continue

            for file in files:
                text = file.read_text(errors="ignore")
                source_path = file.relative_to(project_root).as_posix()

                for chunk in chunks(text):
                    content_hash = hashlib.sha256(
                        f"{source_path}:{chunk}".encode()
                    ).hexdigest()
                    vector = embed(chunk)

                    conn.execute(
                        """
                        INSERT INTO project_memory
                          (source_path, content, content_hash, embedding)
                        VALUES
                          (%s, %s, %s, %s::vector)
                        ON CONFLICT (content_hash) DO NOTHING
                        """,
                        (source_path, chunk, content_hash, vector),
                    )

        conn.commit()

    print("Memory indexing complete.")
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except Exception as error:
        print(f"ERROR: memory indexing failed: {error}", file=sys.stderr)
        sys.exit(1)
