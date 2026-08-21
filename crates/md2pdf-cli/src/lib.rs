//! Adapter #1 — headless.
//!
//! Not a throwaway. A contract with one implementation is not a contract, and this is
//! the cheap second one: it runs in CI with no display, it is how the conversion core
//! is tested end to end, and it forces the engine to stay honest about not knowing what
//! a window is (`INV-8`).
//!
//! The egui adapter is a separate crate in this workspace, added on the macOS host,
//! where a window actually exists.
//!
//! Split into a library so the argument parsing and the reporting can be tested without
//! spawning a process; `main.rs` is the composition root and nothing else.

pub mod args;
pub mod report;
