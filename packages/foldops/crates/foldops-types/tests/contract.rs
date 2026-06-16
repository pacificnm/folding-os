//! Contract tests: golden JSON fixtures must parse and validate against foldops-types.
//!
//! Fixtures live in [`tests/contract/fixtures/`](../../../tests/contract/fixtures/).

use std::fs;
use std::path::PathBuf;

use foldops_types::{parse_ingest_json, validate_ingest_payload, IngestPayload};

fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/contract/fixtures")
        .join(name)
}

fn load_fixture(name: &str) -> String {
    fs::read_to_string(fixture_path(name))
        .unwrap_or_else(|e| panic!("failed to read fixture {name}: {e}"))
}

#[test]
fn ingest_minimal_fixture_parses_and_validates() {
    let json = load_fixture("ingest-minimal.json");
    let payload = parse_ingest_json(&json).expect("fixture should parse and validate");
    assert_eq!(payload.hostname, "fah-01");
    assert_eq!(payload.fah.systemdStatus, foldops_types::FahSystemdStatus::Active);
}

#[test]
fn ingest_minimal_fixture_round_trips_json() {
    let json = load_fixture("ingest-minimal.json");
    let payload: IngestPayload =
        serde_json::from_str(&json).expect("fixture should deserialize");
    validate_ingest_payload(&payload).expect("fixture should validate");

    let again: IngestPayload =
        serde_json::from_str(&serde_json::to_string(&payload).unwrap()).unwrap();
    assert_eq!(payload, again);
}

#[test]
fn ingest_minimal_optional_fields_absent() {
    let json = load_fixture("ingest-minimal.json");
    let value: serde_json::Value = serde_json::from_str(&json).unwrap();
    let without_logs = {
        let mut obj = value.as_object().unwrap().clone();
        obj.remove("logs");
        serde_json::Value::Object(obj)
    };
    let payload: IngestPayload = serde_json::from_value(without_logs).unwrap();
    validate_ingest_payload(&payload).unwrap();
    assert!(payload.logs.is_none());
}
