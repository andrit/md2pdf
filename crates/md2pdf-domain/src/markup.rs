//! Markup — Typst markup text emitted from a parsed Source.
//!
//! Note the boundary this sits on. Typst *markup syntax* is a stable surface and
//! md2pdf-convert emits it freely. The typst *Rust crate* is an unstable surface
//! and only md2pdf-typeset may link it. Those are different things, and conflating
//! them would put an anti-corruption layer in the wrong place.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Markup(String);

impl Markup {
    pub fn new(text: impl Into<String>) -> Self {
        Self(text.into())
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
    pub fn into_string(self) -> String {
        self.0
    }
}

impl std::fmt::Display for Markup {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
