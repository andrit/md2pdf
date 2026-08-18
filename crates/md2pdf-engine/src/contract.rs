//! `Command` / `Event` — the whole public surface of the engine.
//!
//! Plain serializable data: no lifetimes, no closures, no trait objects. In-process
//! this rides a channel; across a process boundary it is line-delimited JSON, and
//! the shape is identical either way.
//!
//! Not yet implemented.
