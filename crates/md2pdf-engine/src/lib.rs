//! The imperative shell, and the UI contract.
//!
//! Commands in, events out — both plain serializable data, no lifetimes, no
//! closures, no trait objects in the API surface. That is what lets an adapter
//! live in another process or another language later without redesigning the
//! contract. In-process it is a channel; across a boundary it is line-delimited
//! JSON, with the same shape either way.
//!
//! The engine does not know what a window is.

// Template discovery, token parsing, rejection reasons
pub mod catalogue;
// Command / Event — the whole public surface
pub mod contract;
// orchestration: convert -> probe -> harvest -> render -> write
pub mod job;
// collision detection, Resolution, BlanketResolution, writing
pub mod output;
// Diagnostic -> AttentionList; Override application
pub mod review;

pub use contract::{Command, Emit, Event};
pub use job::{handle, Deps, JobError};
