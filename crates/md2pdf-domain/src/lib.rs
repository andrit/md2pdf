//! The pure domain. Values in, values out.
//!
//! Depends on `serde` and `thiserror` and nothing else — deliberately. This crate
//! cannot see typst, the filesystem, or a window, and the dependency graph is what
//! enforces that rather than a convention someone has to remember.
//!
//! Every type here is named from `design/GLOSSARY.md`. If a name here disagrees with
//! the glossary, the code is wrong.

pub mod collision;
pub mod decision;
pub mod diagnostic;
pub mod element;
pub mod escalation;
pub mod event;
pub mod hash;
pub mod job;
pub mod markup;
pub mod template;

pub use decision::{Decision, DecisionMap, Orientation, Reduction};
pub use diagnostic::{Compromise, CompromiseKind, Diagnostic};
pub use element::{Element, ElementClass, ElementId};
pub use hash::fnv1a;
pub use markup::Markup;
pub use template::{Floors, Template};
