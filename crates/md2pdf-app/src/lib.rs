//! The desktop app, minus the drawing.
//!
//! Everything here builds and is tested in the container. The egui shell that cannot be
//! is `md2pdf-gui`, and the split is deliberate: **[measured]** `eframe` is not in the
//! offline registry, so code that depends on it cannot even be typechecked here. Keeping
//! every decision on this side of the line means the blind part is a few hundred lines of
//! drawing where a mistake is a compile error rather than a wrong answer.
//!
//! The rule, and it is mechanical: **`md2pdf-gui` contains no `if` that decides anything
//! about a document.** If a widget needs to know whether something is offerable, that
//! belongs in a read model here.
//!
//! See `design/plan-app.md`.

pub mod preview;
pub mod state;
pub mod worker;

pub use preview::Page;
pub use state::{App, Chosen, SourceState};
pub use worker::{Request, Update, Worker};
