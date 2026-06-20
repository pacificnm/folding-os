"""Shared helpers for FoldingOS project-memory tools."""

from __future__ import annotations

import os
from collections.abc import Sequence
from pathlib import Path


DEFAULT_DATABASE_URL = "postgresql:///foldingos_memory?host=/var/run/postgresql"
PROJECT_ROOT = Path(__file__).resolve().parents[1]


def load_project_env() -> None:
    """Load repo-local .env values without requiring python-dotenv."""
    env_path = PROJECT_ROOT / ".env"
    if not env_path.is_file():
        return

    for raw_line in env_path.read_text(encoding="utf-8").splitlines():
        line = raw_line.strip()
        if not line or line.startswith("#") or "=" not in line:
            continue

        key, value = line.split("=", 1)
        key = key.strip()
        value = value.strip().strip('"').strip("'")

        if key and value and key not in os.environ:
            os.environ[key] = value


def database_url() -> str:
    load_project_env()
    return os.environ.get("DATABASE_URL", DEFAULT_DATABASE_URL)


def vector_literal(values: Sequence[float]) -> str:
    """Return a pgvector text literal without requiring the pgvector Python package."""
    return "[" + ",".join(str(float(value)) for value in values) + "]"


load_project_env()
