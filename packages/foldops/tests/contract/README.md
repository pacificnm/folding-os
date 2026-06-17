# Golden JSON fixtures for API contract tests between Node and Rust implementations.
#
# Run: cargo test -p foldops-types --test contract
#
# When adding fixtures:
# 1. Capture real payloads from the Node supervisor/agent where possible.
# 2. Name files by endpoint or payload type (e.g. ingest-minimal.json).
# 3. Tests in crates/foldops-types/tests/contract.rs load from this directory.
