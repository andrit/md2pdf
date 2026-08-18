//! Integration tests that need the real Typst compiler as their oracle.
//!
//! Deliberately **one binary, several modules**. Each integration-test file becomes its
//! own executable, and each of these statically links the whole ~250-crate typst graph;
//! linking two at once got `ld` killed by the OOM killer on a 4 GB machine. One entry
//! point keeps the link count down without giving up the separation of concerns.

mod emission;
mod escaping;
