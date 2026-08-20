//! PathBroker — the ONLY crate permitted to touch the filesystem.
//!
//! Exists so that a future Mac App Store requirement (security-scoped bookmarks
//! instead of raw persisted paths) is a swap inside one crate rather than a
//! rewrite across the codebase. Nothing else calls `std::fs`.
//!
//! Adapters hand the engine already-resolved handles. This crate never opens a
//! native file dialog — that is UI-technology-specific and belongs to the adapter.

// open / read / write / exists, all path access
pub mod broker;
// OutputPath = Destination + (Source relative to SourceRoot)
pub mod mirror;
// platform config dir via `directories`, keyed off BundleId
pub mod settings;
// SourceSet discovery; records the SourceRoot
pub mod walk;
// temp-dir helper other crates borrow, so std::fs stays in this crate
pub mod testing;

pub use broker::{PathBroker, PathError};
pub use mirror::output_path;
pub use walk::SourceSet;
