-- One-time setup for agent context memory.
-- Run as the PostgreSQL superuser, for example:
--   sudo -u postgres psql foldingos_memory -f tools/setup_context_memory.sql

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

GRANT SELECT, INSERT, UPDATE, DELETE ON agent_context_memory TO "jaimie";
GRANT USAGE, SELECT ON SEQUENCE agent_context_memory_id_seq TO "jaimie";
