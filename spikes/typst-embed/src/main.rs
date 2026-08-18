//! Stage 2 — re-prove the two-pass design through the EMBEDDED crate API.
//!
//! Stage 1 used the `typst` CLI, but md2pdf embeds the library and cannot shell
//! out. `typst eval` is a CLI feature; the question this spike answers is
//! whether the library exposes an equivalent way to pull ProbePass decisions
//! back out of a compiled document.
//!
//!   P1' — compile in-process, recover `metadata()` emitted inside `layout()`
//!   P5' — full ProbePass -> DecisionMap -> RenderPass loop, in Rust
//!   P7  — font book: 0.15.1 embeds no sans-serif; confirm we can supply one

use std::sync::Arc;

use typst::diag::{FileError, FileResult};
use typst::foundations::{Bytes, Datetime, Duration};
use typst::introspection::MetadataElem;
use typst::syntax::{FileId, RootedPath, Source, VirtualPath, VirtualRoot};
use typst::text::Font;
use typst::utils::LazyHash;
use typst::introspection::Introspector;
use typst::{Library, LibraryExt, World};
use typst_kit::fonts::FontStore;
use typst_layout::PagedDocument;

/// The smallest `World` that can compile an in-memory document.
/// md2pdf's real World will add: template files, image files resolved relative
/// to the Source, and its own font book — all behind PathBroker.
struct SpikeWorld {
    library: LazyHash<Library>,
    fonts: FontStore,
    main_id: FileId,
    main: Source,
    files: Vec<(FileId, Bytes)>,
}

impl SpikeWorld {
    fn new(text: String) -> Self {
        Self::with_files(text, Vec::new())
    }

    /// Extra virtual files. md2pdf's World will serve templates, images and the
    /// DecisionMap the same way — all of it behind PathBroker.
    fn with_files(text: String, extra: Vec<(&str, Vec<u8>)>) -> Self {
        let mut fonts = FontStore::new();
        fonts.extend(typst_kit::fonts::embedded());
        let main_id =
            RootedPath::new(VirtualRoot::Project, VirtualPath::new("main.typ").expect("valid vpath")).intern();
        let main = Source::new(main_id, text);
        let files = extra
            .into_iter()
            .map(|(name, bytes)| {
                let id = RootedPath::new(
                    VirtualRoot::Project,
                    VirtualPath::new(name).expect("valid vpath"),
                )
                .intern();
                (id, Bytes::new(bytes))
            })
            .collect();
        Self { library: LazyHash::new(Library::builder().build()), fonts, main_id, main, files }
    }
}

impl World for SpikeWorld {
    fn library(&self) -> &LazyHash<Library> {
        &self.library
    }
    fn book(&self) -> &LazyHash<typst::text::FontBook> {
        self.fonts.book()
    }
    fn main(&self) -> FileId {
        self.main_id
    }
    fn source(&self, id: FileId) -> FileResult<Source> {
        if id == self.main_id {
            Ok(self.main.clone())
        } else {
            Err(FileError::NotFound(id.vpath().get_without_slash().into()))
        }
    }
    fn file(&self, id: FileId) -> FileResult<Bytes> {
        self.files
            .iter()
            .find(|(fid, _)| *fid == id)
            .map(|(_, bytes)| bytes.clone())
            .ok_or_else(|| FileError::NotFound(id.vpath().get_without_slash().into()))
    }
    fn font(&self, index: usize) -> Option<Font> {
        self.fonts.font(index)
    }
    fn today(&self, _offset: Option<Duration>) -> Option<Datetime> {
        None
    }
}

/// Compile, then pull every labelled `metadata()` value back out as JSON.
/// This is the library equivalent of `typst query` / `typst eval`.
fn compile_and_harvest(
    text: &str,
    extra: Vec<(&str, Vec<u8>)>,
) -> (PagedDocument, Vec<serde_json::Value>) {
    let world = SpikeWorld::with_files(text.to_string(), extra);
    let result = typst::compile::<PagedDocument>(&world);
    let doc = match result.output {
        Ok(doc) => doc,
        Err(errors) => {
            for e in &errors {
                eprintln!("COMPILE ERROR: {}", e.message);
            }
            panic!("compilation failed");
        }
    };

    // The introspector is the in-process query surface.
    let harvested = doc
        .introspector()
        .query_labelled()
        .iter()
        .filter_map(|content| content.to_packed::<MetadataElem>())
        .map(|meta| serde_json::to_value(&meta.value).expect("metadata serialises"))
        .collect();

    (doc, harvested)
}

const PROBE_PASS: &str = r#"
#set page(width: 220pt, height: 320pt, margin: 12pt)
#set text(size: 10pt)

#let doc = (
  (id: "e0", class: "prose",  body: lorem(18)),
  (id: "e1", class: "table",  body: table(columns: 6, ..range(6).map(i => [longvalue#i]))),
  (id: "e2", class: "table",  body: table(columns: 3, [a], [b], [c])),
  (id: "e3", class: "figure", body: rect(width: 400pt, height: 30pt)),
)
#let floors = (prose: 9pt, table: 7pt, figure: 0pt)
#let atomic = ("table", "figure")
#let base = 10pt

#for el in doc {
  layout(size => {
    let avail = size.width
    let decision = if el.class not in atomic {
      (rung: "none", size: none)
    } else {
      let floor = floors.at(el.class)
      let chosen = none
      let s = base
      while s >= floor and chosen == none {
        if measure(text(size: s, el.body)).width <= avail { chosen = s }
        s = s - 0.5pt
      }
      if chosen == base { (rung: "none", size: none) }
      else if chosen != none { (rung: "shrink", size: chosen.pt()) }
      else { (rung: "rotate", size: floor.pt()) }
    }
    [#metadata((id: el.id, class: el.class, natural: measure(el.body).width.pt(), ..decision)) <d>]
  })
}
"#;

fn render_pass() -> String {
    format!(
        r#"
#set page(width: 220pt, height: 320pt, margin: 12pt)
#set text(size: 10pt)
#let decisions = json("decisions.json")
#let doc = (
  (id: "e0", class: "prose",  body: lorem(18)),
  (id: "e1", class: "table",  body: table(columns: 6, ..range(6).map(i => [longvalue#i]))),
  (id: "e2", class: "table",  body: table(columns: 3, [a], [b], [c])),
  (id: "e3", class: "figure", body: rect(width: 400pt, height: 30pt)),
)
#for el in doc {{
  let d = decisions.find(x => x.id == el.id)
  if d == none or d.rung == "none" {{ el.body; v(6pt) }}
  else if d.rung == "shrink" {{ text(size: d.size * 1pt, el.body); v(6pt) }}
  else {{
    // legal here: top level, not inside layout()
    page(flipped: true, margin: 12pt)[#el.body]
  }}
}}
"#
    )
}

fn main() {
    // ---- P7: what fonts does the embedded book actually have? ----
    let world = SpikeWorld::new(String::new());
    let book = world.book();
    let mut families: Vec<&str> = book.families().map(|(name, _)| name).collect();
    families.sort_unstable();
    families.dedup();
    println!("P7  embedded font families ({}): {:?}", families.len(), families);

    // ---- P1': recover metadata emitted inside layout(), in-process ----
    let t0 = std::time::Instant::now();
    let (_probe_doc, decisions) = compile_and_harvest(PROBE_PASS, Vec::new());
    let probe_ms = t0.elapsed().as_millis();
    println!("\nP1' ProbePass compiled in {probe_ms} ms");
    println!("P1' harvested {} decisions via introspector:", decisions.len());
    for d in &decisions {
        println!("      {d}");
    }
    assert!(!decisions.is_empty(), "P1' FAILED: no metadata recovered");

    // ---- P5': inject decisions, render at top level ----
    let t1 = std::time::Instant::now();
    let (doc, _) = compile_and_harvest(
        &render_pass(),
        vec![("decisions.json", serde_json::to_vec(&decisions).expect("serialise"))],
    );
    let render_ms = t1.elapsed().as_millis();
    println!("\nP5' RenderPass compiled in {render_ms} ms -> {} pages", doc.pages().len());
    for (i, page) in doc.pages().iter().enumerate() {
        let size = page.frame.size();
        let landscape = size.x > size.y;
        println!(
            "      page {}: {:.0}x{:.0}pt {}",
            i + 1,
            size.x.to_pt(),
            size.y.to_pt(),
            if landscape { "LANDSCAPE" } else { "portrait" }
        );
    }

    // ---- PDF + raster, both from the same compilation ----
    let pdf = typst_pdf::pdf(&doc, &typst_pdf::PdfOptions::default()).expect("pdf export");
    println!("\n    PDF export: {} bytes", pdf.len());

    let pixmap = typst_render::render(&doc.pages()[0], &typst_render::RenderOptions::default());
    println!("    Raster preview page 1: {}x{} px", pixmap.width(), pixmap.height());

    println!("\n    total two-pass: {} ms (embedded, no process startup)", probe_ms + render_ms);
    let _ = Arc::new(());
}
