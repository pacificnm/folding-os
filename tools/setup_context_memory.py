"""Create the agent_context_memory table and indexes."""

import sys
from pathlib import Path

from memory_common import database_url, PROJECT_ROOT

SCHEMA_SQL = """
CREATE EXTENSION IF NOT EXISTS vector;

CREATE TABLE IF NOT EXISTS agent_context_memory (
    id bigserial PRIMARY KEY,
    session_key text NOT NULL DEFAULT '',
    title text NOT NULL DEFAULT '',
    content text NOT NULL,
    tags text[] NOT NULL DEFAULT '{}',
    embedding vector(1536) NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS agent_context_memory_embedding_idx
    ON agent_context_memory
    USING hnsw (embedding vector_cosine_ops);

CREATE INDEX IF NOT EXISTS agent_context_memory_session_created_idx
    ON agent_context_memory (session_key, created_at DESC);

CREATE INDEX IF NOT EXISTS agent_context_memory_created_idx
    ON agent_context_memory (created_at DESC);
"""

SQL_FILE = PROJECT_ROOT / "tools" / "setup_context_memory.sql"


def main() -> int:
    import psycopg

    try:
        with psycopg.connect(database_url()) as conn:
            conn.execute(SCHEMA_SQL)
            conn.commit()
    except psycopg.errors.InsufficientPrivilege:
        print(
            "ERROR: current database user cannot create tables in schema public.\n"
            "Run the one-time setup as postgres instead:\n"
            f"  sudo -u postgres psql foldingos_memory -f {SQL_FILE}",
            file=sys.stderr,
        )
        return 1

    print("agent_context_memory schema ready.")
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except Exception as error:
        print(f"ERROR: context memory setup failed: {error}", file=sys.stderr)
        sys.exit(1)
