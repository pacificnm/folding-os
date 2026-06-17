//! Shared types for the FoldOps agent ↔ supervisor API contract.
//!
//! Field names match [`packages/shared/src/schema.ts`](../../../packages/shared/src/schema.ts).

mod control;
mod ingest;
mod validate;

pub use control::{is_control_action, ControlAction, CONTROL_ACTIONS};
pub use ingest::{
    Disk, Fah, FahSystemdStatus, IngestPayload, Maintenance, Memory, Network, NodeLogs, System,
};
pub use validate::{
    parse_ingest_json, validate_ingest_payload, IngestJsonError, IngestValidationError,
};
