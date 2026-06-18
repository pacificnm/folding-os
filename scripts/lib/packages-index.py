#!/usr/bin/env python3
"""Merge entries into packages.folding-os.com channel index.json catalogs."""

from __future__ import annotations

import argparse
import json
import sys
from datetime import datetime, timezone


def empty_index(channel: str) -> dict:
    return {"schema_version": 1, "channel": channel, "releases": []}


def load_index(raw: str, channel: str) -> dict:
    if not raw.strip():
        return empty_index(channel)
    index = json.loads(raw)
    if index.get("schema_version") != 1:
        raise SystemExit(f"unsupported schema_version: {index.get('schema_version')}")
    if index.get("channel") != channel:
        raise SystemExit(
            f"index channel mismatch: expected {channel!r}, got {index.get('channel')!r}"
        )
    index.setdefault("releases", [])
    return index


def merge_foldops(index: dict, entry: dict) -> dict:
    releases = [
        release
        for release in index["releases"]
        if release.get("manifest_release") != entry["manifest_release"]
    ]
    releases.append(entry)
    releases.sort(key=lambda release: release["published_at"], reverse=True)
    index["releases"] = releases
    return index


def merge_tools(index: dict, entry: dict) -> dict:
    releases = [
        release
        for release in index["releases"]
        if release.get("tools_version") != entry["tools_version"]
    ]
    releases.append(entry)
    releases.sort(key=lambda release: release["published_at"], reverse=True)
    index["releases"] = releases
    return index


def utc_now_rfc3339() -> str:
    return datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")


def cmd_merge_foldops(args: argparse.Namespace) -> None:
    index = load_index(sys.stdin.read(), "foldops")
    entry = {
        "manifest_release": args.manifest_release,
        "published_at": args.published_at or utc_now_rfc3339(),
        "manifest_url": args.manifest_url,
        "minimum_foldingos_version": args.minimum_foldingos_version,
    }
    json.dump(merge_foldops(index, entry), sys.stdout, indent=2)
    sys.stdout.write("\n")


def cmd_merge_tools(args: argparse.Namespace) -> None:
    index = load_index(sys.stdin.read(), "foldingos-tools")
    entry = {
        "tools_version": args.tools_version,
        "published_at": args.published_at or utc_now_rfc3339(),
        "binary_url": args.binary_url,
        "sha256_url": args.sha256_url,
        "minimum_foldingos_version": args.minimum_foldingos_version,
    }
    json.dump(merge_tools(index, entry), sys.stdout, indent=2)
    sys.stdout.write("\n")


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="command", required=True)

    foldops = sub.add_parser("merge-foldops", help="merge a FoldOps release entry")
    foldops.add_argument("manifest_release")
    foldops.add_argument("manifest_url")
    foldops.add_argument("minimum_foldingos_version")
    foldops.add_argument("--published-at")
    foldops.set_defaults(func=cmd_merge_foldops)

    tools = sub.add_parser("merge-tools", help="merge a tools release entry")
    tools.add_argument("tools_version")
    tools.add_argument("binary_url")
    tools.add_argument("sha256_url")
    tools.add_argument("minimum_foldingos_version")
    tools.add_argument("--published-at")
    tools.set_defaults(func=cmd_merge_tools)

    return parser


def main() -> None:
    parser = build_parser()
    args = parser.parse_args()
    args.func(args)


if __name__ == "__main__":
    main()
