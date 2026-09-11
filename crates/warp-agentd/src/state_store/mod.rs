//! Local state-store skeletons.
//!
//! LEFTOVER: these helpers still surface raw `std::io::Error` via `io::Result`.
//! The runtime layer (exec / reporting / scheduler / daemon / telemetry) converts
//! them with `source_err(RuntimeReason::Io, ...)` at its own boundary. Convert this
//! module to a dedicated `StateStoreReason` once the upper layers are fully migrated.

pub mod agent_runtime;
pub mod execution_queue;
pub mod history;
pub(crate) mod log_checkpoint_state;
pub mod log_checkpoints;
pub mod planner_candidates;
pub mod reporting;
pub mod running;
