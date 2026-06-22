"""Read/write helpers for agent session context stored in PostgreSQL."""

from __future__ import annotations

from memory_common import database_url, vector_literal


EMBEDDING_MODEL = "text-embedding-3-small"


def embed_text(text: str) -> str:
    from openai import OpenAI

    client = OpenAI()
    result = client.embeddings.create(model=EMBEDDING_MODEL, input=text)
    return vector_literal(result.data[0].embedding)


def save_context(
    content: str,
    *,
    title: str = "",
    session_key: str = "",
    tags: list[str] | None = None,
) -> int:
    import psycopg

    content = content.strip()
    if not content:
        raise ValueError("content must not be empty")

    embedding = embed_text(f"{title}\n{content}".strip())
    tag_list = tags or []

    with psycopg.connect(database_url()) as conn:
        row = conn.execute(
            """
            INSERT INTO agent_context_memory
              (session_key, title, content, tags, embedding)
            VALUES
              (%s, %s, %s, %s, %s::vector)
            RETURNING id
            """,
            (session_key.strip(), title.strip(), content, tag_list, embedding),
        ).fetchone()
        conn.commit()

    return int(row[0])


def search_context(
    query: str,
    *,
    limit: int = 8,
    session_key: str = "",
) -> list[tuple[int, str, str, str, list[str], str]]:
    import psycopg

    embedding = embed_text(query)
    session_key = session_key.strip()

    with psycopg.connect(database_url()) as conn:
        if session_key:
            rows = conn.execute(
                """
                SELECT id, session_key, title, content, tags, created_at
                FROM agent_context_memory
                WHERE session_key = %s
                ORDER BY embedding <=> %s::vector
                LIMIT %s
                """,
                (session_key, embedding, limit),
            ).fetchall()
        else:
            rows = conn.execute(
                """
                SELECT id, session_key, title, content, tags, created_at
                FROM agent_context_memory
                ORDER BY embedding <=> %s::vector
                LIMIT %s
                """,
                (embedding, limit),
            ).fetchall()

    return rows


def list_context(
    *,
    limit: int = 20,
    session_key: str = "",
) -> list[tuple[int, str, str, str, list[str], str]]:
    import psycopg

    session_key = session_key.strip()

    with psycopg.connect(database_url()) as conn:
        if session_key:
            rows = conn.execute(
                """
                SELECT id, session_key, title, content, tags, created_at
                FROM agent_context_memory
                WHERE session_key = %s
                ORDER BY created_at DESC
                LIMIT %s
                """,
                (session_key, limit),
            ).fetchall()
        else:
            rows = conn.execute(
                """
                SELECT id, session_key, title, content, tags, created_at
                FROM agent_context_memory
                ORDER BY created_at DESC
                LIMIT %s
                """,
                (limit,),
            ).fetchall()

    return rows


def get_context(entry_id: int) -> tuple[int, str, str, str, list[str], str] | None:
    import psycopg

    with psycopg.connect(database_url()) as conn:
        row = conn.execute(
            """
            SELECT id, session_key, title, content, tags, created_at
            FROM agent_context_memory
            WHERE id = %s
            """,
            (entry_id,),
        ).fetchone()

    return row


def format_entry(
    entry_id: int,
    session_key: str,
    title: str,
    content: str,
    tags: list[str],
    created_at: str,
    *,
    content_limit: int = 2000,
) -> str:
    tag_text = ", ".join(tags) if tags else "(none)"
    session_text = session_key or "(none)"
    preview = content if len(content) <= content_limit else content[:content_limit] + "..."
    return (
        f"id={entry_id} created_at={created_at} session_key={session_text}\n"
        f"title={title or '(untitled)'} tags={tag_text}\n"
        f"{preview}"
    )
