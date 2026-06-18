"""JSON validators for FoldingOS HTTP and foldingosctl automation acceptance tests."""

from __future__ import annotations

import json
from typing import Any


class CheckError(Exception):
    """Raised when a response fails structural validation."""


def parse_json(text: str, label: str = "response") -> Any:
    try:
        return json.loads(text)
    except json.JSONDecodeError as exc:
        raise CheckError(f"{label} is not valid JSON: {exc}") from exc


def require_object(doc: Any, label: str = "response") -> dict[str, Any]:
    if not isinstance(doc, dict):
        raise CheckError(f"{label} must be a JSON object")
    return doc


def require_array(obj: dict[str, Any], key: str) -> list[Any]:
    value = obj.get(key)
    if not isinstance(value, list):
        raise CheckError(f"missing or invalid array field: {key}")
    return value


def require_string(obj: dict[str, Any], key: str) -> None:
    if key not in obj or not isinstance(obj[key], str):
        raise CheckError(f"missing or invalid string field: {key}")


def require_bool(obj: dict[str, Any], key: str) -> None:
    if key not in obj or not isinstance(obj[key], bool):
        raise CheckError(f"missing or invalid boolean field: {key}")


def require_number(obj: dict[str, Any], key: str) -> None:
    if key not in obj or not isinstance(obj[key], (int, float)):
        raise CheckError(f"missing or invalid number field: {key}")


def validate_machines(doc: Any) -> None:
    obj = require_object(doc)
    machines = require_array(obj, "machines")
    require_number(obj, "farm_ppd")
    for index, machine in enumerate(machines):
        if not isinstance(machine, dict):
            raise CheckError(f"machines[{index}] must be an object")
        for key in (
            "hostname",
            "node_id",
            "installation_role",
            "foldingos_version",
            "first_seen",
            "last_seen",
            "online",
        ):
            if key not in machine:
                raise CheckError(f"machines[{index}] missing {key}")


def validate_fleet_enrollments(doc: Any) -> None:
    obj = require_object(doc)
    enrollments = require_array(obj, "enrollments")
    for index, row in enumerate(enrollments):
        if not isinstance(row, dict):
            raise CheckError(f"enrollments[{index}] must be an object")
        for key in (
            "node_id",
            "hostname",
            "installation_role",
            "current_image_version",
            "desired_image_version",
            "foldingos_version",
        ):
            if key not in row:
                raise CheckError(f"enrollments[{index}] missing {key}")


def validate_fleet_allow_boot(doc: Any) -> None:
    obj = require_object(doc)
    devices = require_array(obj, "devices")
    for index, device in enumerate(devices):
        if not isinstance(device, dict):
            raise CheckError(f"devices[{index}] must be an object")
        require_string(device, "mac_address")


def validate_fleet_registry(doc: Any) -> None:
    obj = require_object(doc)
    versions = require_array(obj, "versions")
    for index, entry in enumerate(versions):
        if not isinstance(entry, dict):
            raise CheckError(f"versions[{index}] must be an object")
        require_string(entry, "foldingos_version")


def validate_fleet_registry_show(doc: Any) -> None:
    obj = require_object(doc)
    require_string(obj, "foldingos_version")
    require_string(obj, "rollout_state")


def validate_software_updates(doc: Any) -> None:
    obj = require_object(doc)
    require_string(obj, "checked_at")
    upstream = require_object(obj, "upstream")
    require_object(upstream, "foldops")
    require_object(upstream, "tools")
    supervisor = require_object(obj, "supervisor")
    require_string(supervisor, "hostname")
    require_bool(supervisor, "foldops_update_available")
    require_bool(supervisor, "tools_update_available")
    agents = require_array(obj, "agents")
    for index, entry in enumerate(agents):
        if not isinstance(entry, dict):
            raise CheckError(f"agents[{index}] must be an object")
        require_string(entry, "hostname")
        require_string(entry, "node_id")
        require_bool(entry, "online")
        require_bool(entry, "foldops_apply_pending")
        require_bool(entry, "tools_apply_pending")


def validate_foldingosctl_success(doc: Any) -> None:
    obj = require_object(doc, "foldingosctl response")
    if obj.get("schema_version") != 1:
        raise CheckError("schema_version must be 1")
    require_bool(obj, "ok")
    if obj["ok"] is not True:
        raise CheckError("expected ok=true")
    require_string(obj, "command")
    if "data" not in obj:
        raise CheckError("missing data field")


def validate_foldingosctl_migration_status(doc: Any) -> None:
    validate_foldingosctl_success(doc)
    obj = require_object(doc)
    data = obj.get("data")
    if not isinstance(data, dict):
        raise CheckError("data must be an object")
    require_number(data, "phase")
    require_bool(data, "complete")
    require_string(data, "marker")
    require_string(data, "implementation")
