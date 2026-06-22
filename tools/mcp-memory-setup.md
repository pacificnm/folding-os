# FoldingOS Memory MCP Setup

For the full setup guide, including Cursor configuration, see
[`MCP-SETUP.md`](MCP-SETUP.md).

This document describes the local setup for the FoldingOS project-memory MCP
server in `tools/mcp_memory_server.py`.

The memory tools are development-time helpers only. They index repository
documentation into PostgreSQL with `pgvector`, then expose semantic search
through the MCP tool `search_project_memory`.

## Components

| File | Purpose |
| --- | --- |
| `tools/memory_common.py` | Shared environment loading, default database URL, and pgvector literal formatting. |
| `tools/index_memory.py` | Reads approved project context files and Markdown docs, creates OpenAI embeddings, and inserts chunks into PostgreSQL. |
| `tools/search_memory.py` | Command-line semantic search against the local `project_memory` table. |
| `tools/mcp_memory_server.py` | FastMCP stdio server exposing `search_project_memory(query, limit)`. |
| `.env.example` | Template for `DATABASE_URL` and `OPENAI_API_KEY`. |

## Required Services

- PostgreSQL with the `pgvector` extension available.
- Python 3 with the `openai`, `psycopg`, and `mcp` packages installed.
- An OpenAI API key in the environment or in repo-local `.env`.

The default connection string is:

```text
postgresql:///foldingos_memory?host=/var/run/postgresql
```

Set `DATABASE_URL` in `.env` when using a TCP connection, a different user, or a
different database name.

## PostgreSQL Setup

Install PostgreSQL and `pgvector` using the package names for the host OS. On a
Debian-family workstation the packages are typically:

```bash
sudo apt install postgresql postgresql-contrib postgresql-XX-pgvector
```

Replace `XX` with the installed PostgreSQL major version.

Create the database and enable `pgvector`:

```bash
sudo -u postgres createdb foldingos_memory
sudo -u postgres psql foldingos_memory
```

Run this SQL in the `foldingos_memory` database:

```sql
CREATE EXTENSION IF NOT EXISTS vector;

CREATE TABLE IF NOT EXISTS project_memory (
    id bigserial PRIMARY KEY,
    source_path text NOT NULL,
    content text NOT NULL,
    content_hash text NOT NULL UNIQUE,
    embedding vector(1536) NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS project_memory_embedding_idx
    ON project_memory
    USING hnsw (embedding vector_cosine_ops);

CREATE INDEX IF NOT EXISTS project_memory_source_path_idx
    ON project_memory (source_path);
```

`tools/index_memory.py` uses OpenAI `text-embedding-3-small`, which produces the
1536-dimension vectors expected by this table design.

For a local Unix-socket setup, the operating-system user running the tools must
be able to connect to the database. One simple development setup is to create a
matching PostgreSQL role:

```bash
sudo -u postgres createuser "$USER"
sudo -u postgres psql -d foldingos_memory -c "GRANT ALL PRIVILEGES ON DATABASE foldingos_memory TO \"$USER\";"
sudo -u postgres psql -d foldingos_memory -c "GRANT ALL PRIVILEGES ON ALL TABLES IN SCHEMA public TO \"$USER\";"
sudo -u postgres psql -d foldingos_memory -c "GRANT ALL PRIVILEGES ON ALL SEQUENCES IN SCHEMA public TO \"$USER\";"
```

If the role already exists, keep it and only apply the grants.

## Python Setup

Create and activate a repository-local virtual environment:

```bash
python3 -m venv .venv
. .venv/bin/activate
python -m pip install --upgrade pip
python -m pip install openai psycopg "mcp[cli]"
```

Create `.env` from the example and fill in the local API key:

```bash
cp .env.example .env
```

Required values:

```env
DATABASE_URL="postgresql:///foldingos_memory?host=/var/run/postgresql"
OPENAI_API_KEY="sk-..."
```

Do not commit `.env`.

## Index Project Memory

Run the indexer from the repository root:

```bash
.venv/bin/python tools/index_memory.py
```

The indexer reads:

- `AGENTS.md`
- `DECISIONS.md`
- `KNOWN_ISSUES.md`
- `BUILD_COMMANDS.md`
- `README.md`
- `doc/**/*.md`
- `docs/**/*.md`, when present

Each document is split into overlapping chunks. The indexer stores
`source_path`, `content`, a `content_hash`, and the embedding. Existing chunks
are skipped by `content_hash`, so repeated indexing is safe.

To rebuild the memory table from scratch:

```sql
TRUNCATE project_memory RESTART IDENTITY;
```

Then rerun:

```bash
.venv/bin/python tools/index_memory.py
```

## Test Local Search

Before wiring MCP into an agent, verify direct search:

```bash
.venv/bin/python tools/search_memory.py "current milestone and FoldOps architecture"
```

Expected behavior:

- The command prints matching source paths and snippets.
- A missing `OPENAI_API_KEY` fails during embedding creation.
- A missing table or extension fails with a PostgreSQL error.

## Run The MCP Server

The MCP server runs over stdio:

```bash
.venv/bin/python tools/mcp_memory_server.py
```

It exposes one tool:

| Tool | Arguments | Result |
| --- | --- | --- |
| `search_project_memory` | `query: str`, `limit: int = 8` | Matching project-memory snippets grouped by source path. |

Example MCP client command configuration:

```json
{
  "mcpServers": {
    "foldingos-memory": {
      "command": "/home/jaimie/folding-os/.venv/bin/python",
      "args": [
        "/home/jaimie/folding-os/tools/mcp_memory_server.py"
      ],
      "env": {
        "DATABASE_URL": "postgresql:///foldingos_memory?host=/var/run/postgresql",
        "OPENAI_API_KEY": "sk-..."
      }
    }
  }
}
```

If the client launches the server from a shell where `.env` is readable from the
repository root, the `env` block can omit `DATABASE_URL` and `OPENAI_API_KEY`.
Providing explicit environment values is usually clearer for editor and agent
integrations.

## Agent Usage

Implementation agents should query project memory before changing code. Useful
queries include:

- `current milestone`
- the affected subsystem, such as `FoldOps dashboard settings`
- related ADR numbers or architecture terms
- known issue text or build failure text
- build, release, recovery, or packaging instructions

The memory result does not override approved documentation. Follow the binding
order in `AGENTS.md`: `/doc` specifications first, then MCP project memory, then
the remaining project notes and existing code behavior.

## Troubleshooting

| Symptom | Likely cause | Fix |
| --- | --- | --- |
| `Missing Python dependency for FoldingOS memory MCP` | The MCP server is not running inside the virtual environment, or dependencies are not installed. | Run with `.venv/bin/python` and install `openai psycopg "mcp[cli]"`. |
| `relation "project_memory" does not exist` | The table has not been created in the configured database. | Run the SQL table setup in this document. |
| `type "vector" does not exist` | `pgvector` is not installed or `CREATE EXTENSION vector` has not been run. | Install the PostgreSQL `pgvector` package and enable the extension in `foldingos_memory`. |
| Authentication or peer errors | The local PostgreSQL role does not match the OS user or lacks grants. | Create/grant the role or set `DATABASE_URL` to a connection string with explicit credentials. |
| OpenAI authentication errors | `OPENAI_API_KEY` is missing or invalid. | Set the key in `.env` or the MCP client's environment configuration. |
| Empty search results | The database is empty. | Run `.venv/bin/python tools/index_memory.py`. |
| `permission denied for schema public` during context setup | The app user can write rows but cannot create tables. | Run `sudo -u postgres psql foldingos_memory -f tools/setup_context_memory.sql`. |
| `relation "agent_context_memory" does not exist` | Context table setup has not been run. | Run `tools/setup_context_memory.py` or the postgres SQL setup above. |

## Agent Context Memory

The context-memory MCP stores agent session notes in the same PostgreSQL
database. Use it to survive Cursor context compaction and to search prior work.

### Schema Setup

Create the context table once:

```bash
.venv/bin/python tools/setup_context_memory.py
```

If the current database user cannot create tables, run the one-time SQL setup
as postgres:

```bash
sudo -u postgres psql foldingos_memory -f tools/setup_context_memory.sql
```

This creates the `agent_context_memory` table and indexes in the configured
database.

### Run The Context MCP Server

```bash
.venv/bin/python tools/mcp_context_memory_server.py
```

Tools:

| Tool | Arguments | Result |
| --- | --- | --- |
| `save_context_memory` | `content: str`, `title: str = ""`, `session_key: str = ""`, `tags: list[str] = []` | Stores one context entry and returns its id. |
| `search_context_memory` | `query: str`, `limit: int = 8`, `session_key: str = ""` | Semantic search over saved context. |
| `list_context_memory` | `limit: int = 20`, `session_key: str = ""` | Recent saved entries, newest first. |
| `get_context_memory` | `entry_id: int` | Full content for one entry. |

Example MCP client configuration is in `.cursor/mcp.json` under
`foldingos-context-memory`.

### Agent Usage

Save context when:

- a multi-step task reaches a stable checkpoint
- important decisions, file paths, or commands would be lost after compaction
- you are handing off to a later agent turn on the same task

Search or list context when:

- earlier chat detail is missing after compaction
- you need prior decisions, blockers, or verification results for the task

Use a stable `session_key` such as the branch name, issue number, or task slug
so related entries stay grouped.
