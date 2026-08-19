//! The anti-corruption layer. **The only crate permitted to link the `typst` crate.**
//!
//! Everything crossing this boundary is a domain type — `Element`, `Template`,
//! `DecisionMap`, `Diagnostic`, `Compilation`. Callers never see a `PagedDocument`,
//! a `FileId`, a `Frame`, or an `Abs`.
//!
//! ## Why this crate exists
//!
//! Moving ~200 lines of spike code to typst 0.15.1 produced **eight** breaking API
//! changes: `FileId::new` became `RootedPath::new(..).intern()`, `VirtualPath::new`
//! started returning `Result`, `Library::default()` became `Library::builder().build()`,
//! `Introspector` became a trait needing import, `render()` took an options struct,
//! `as_rootless_path` was deprecated, `json.decode` was removed, and `today()` changed
//! parameter type. All eight were compile errors — the safe kind. The dangerous kind is
//! silent behavioural change, which is what `tests/contract.rs` exists to catch.
//!
//! Confined here, an upgrade touches one crate. See `docs/typst-upgrade.md`.
//!
//! ## The boundary is the crate graph, not a lint
//!
//! `md2pdf-domain` cannot import typst because typst is not in its manifest. That is
//! mechanical; a lint is something a person has to remember to keep passing.
//!
//! Note the distinction this layer does *not* draw: Typst **markup syntax** is a
//! stable surface and `md2pdf-convert` emits it freely as text. The typst **Rust
//! crate** is the unstable surface, and only this crate touches it.

mod export;
mod fonts;
mod harvest;
mod probe;
mod render;
mod world;

use md2pdf_domain::{DecisionMap, Diagnostic, Element, Template};
use typst_layout::PagedDocument;

pub use world::TypstWorld;

#[derive(Debug, thiserror::Error)]
pub enum TypesetError {
    #[error("typst compilation failed: {0}")]
    Compile(String),
    #[error("could not read probe results: {0}")]
    Harvest(String),
    #[error("export failed: {0}")]
    Export(String),
}

/// Page size as the rest of the app sees it. No typst types leak.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PageGeometry {
    pub number: u32,
    pub width_pt: f64,
    pub height_pt: f64,
}

impl PageGeometry {
    pub fn is_landscape(&self) -> bool {
        self.width_pt > self.height_pt
    }
}

/// A rendered document: PDF bytes on demand, page rasters on demand, and the
/// geometry — all from one compilation.
pub struct Compilation {
    doc: PagedDocument,
}

impl Compilation {
    pub fn pdf(&self) -> Result<Vec<u8>, TypesetError> {
        export::pdf(&self.doc)
    }
    /// RGBA8 pixels for one page. Preview *is* the output — same compilation.
    pub fn raster(&self, page: usize, pixels_per_point: f32) -> Option<(u32, u32, Vec<u8>)> {
        export::raster(&self.doc, page, pixels_per_point)
    }
    pub fn geometry(&self) -> Vec<PageGeometry> {
        export::geometry(&self.doc)
    }
    /// The plain text Typst actually rendered. The oracle for escaping round-trip
    /// tests: what came out must equal what went in.
    pub fn text(&self) -> String {
        export::text(&self.doc)
    }
    /// Each glyph run as `(text, font family, style)` — the oracle for **styling**,
    /// which `text()` cannot see. Style is `"normal"` / `"italic"` / `"oblique"`.
    pub fn text_runs(&self) -> Vec<(String, String, &'static str)> {
        export::text_runs(&self.doc)
    }
    pub fn page_count(&self) -> usize {
        self.doc.pages().len()
    }
}

/// The typesetting engine. Holds a long-lived `World` so `comemo` memoisation
/// survives across compilations — that is what makes an Override cost ~8ms rather
/// than a cold compile.
pub struct Typesetter {
    world: TypstWorld,
}

impl Default for Typesetter {
    fn default() -> Self {
        Self::new()
    }
}

impl Typesetter {
    pub fn new() -> Self {
        Self {
            world: TypstWorld::new(),
        }
    }

    /// Families available to Templates. Typst ships no sans-serif; ours are added.
    pub fn font_families(&self) -> Vec<String> {
        fonts::FontLibrary::shipped().families()
    }

    /// **ProbePass** — measure every Element, run the Escalation ladder, decide.
    /// Nothing is paginated and nothing is acted on.
    pub fn probe(
        &self,
        elements: &[Element],
        template: &Template,
    ) -> Result<(DecisionMap, Diagnostic), TypesetError> {
        self.world
            .set_source(probe::probe_source(elements, template));
        let doc = self.compile()?;
        let map = harvest::harvest(&doc, elements)?;
        let diagnostic = Diagnostic::from_decisions(&map);
        Ok((map, diagnostic))
    }

    /// **RenderPass** — apply the DecisionMap. Never measures, never decides.
    pub fn render(
        &self,
        elements: &[Element],
        template: &Template,
        map: &DecisionMap,
    ) -> Result<Compilation, TypesetError> {
        self.world
            .set_source(render::render_source(elements, template, map));
        Ok(Compilation {
            doc: self.compile()?,
        })
    }

    fn compile(&self) -> Result<PagedDocument, TypesetError> {
        let result = typst::compile::<PagedDocument>(&self.world);
        result.output.map_err(|errors| {
            let msg = errors
                .iter()
                .map(|e| e.message.to_string())
                .collect::<Vec<_>>()
                .join("; ");
            TypesetError::Compile(msg)
        })
    }
}
