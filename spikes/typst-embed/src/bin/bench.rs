//! Settles two open questions empirically.
//!
//! Q1 — Does the ProbePass need to compile the WHOLE document?
//!      A: full doc, every element wrapped in layout(), rendering discarded.
//!      B: measure-only harness — available width computed by us from the
//!         template (page - margins), elements measured but NEVER paginated.
//!      Do they agree, and how much cheaper is B?
//!
//! Q2 — What does an Override actually cost?
//!      cold render / warm render (comemo cache retained, nothing changed) /
//!      render after flipping ONE decision.

use std::cell::RefCell;
use std::time::Instant;

use typst::diag::{FileError, FileResult};
use typst::foundations::{Bytes, Datetime, Duration};
use typst::introspection::{Introspector, MetadataElem};
use typst::syntax::{FileId, RootedPath, Source, VirtualPath, VirtualRoot};
use typst::text::Font;
use typst::utils::LazyHash;
use typst::{Library, LibraryExt, World};
use typst_kit::fonts::FontStore;
use typst_layout::PagedDocument;

const N: usize = 120;
const PAGE_W: f64 = 220.0;
const MARGIN: f64 = 12.0;
const AVAIL: f64 = PAGE_W - 2.0 * MARGIN;

/// Mutable World so repeated compiles can share comemo's cache the way a
/// long-running app would, instead of starting cold every time.
struct BenchWorld {
    library: LazyHash<Library>,
    fonts: FontStore,
    main_id: FileId,
    main: RefCell<Source>,
    files: RefCell<Vec<(FileId, Bytes)>>,
}

// Single-threaded bench; typst requires World: Send + Sync.
unsafe impl Send for BenchWorld {}
unsafe impl Sync for BenchWorld {}

fn vpath(name: &str) -> FileId {
    RootedPath::new(VirtualRoot::Project, VirtualPath::new(name).expect("vpath")).intern()
}

impl BenchWorld {
    fn new() -> Self {
        let mut fonts = FontStore::new();
        fonts.extend(typst_kit::fonts::embedded());
        let main_id = vpath("main.typ");
        Self {
            library: LazyHash::new(Library::builder().build()),
            fonts,
            main_id,
            main: RefCell::new(Source::new(main_id, String::new())),
            files: RefCell::new(Vec::new()),
        }
    }
    fn set_main(&self, text: String) {
        *self.main.borrow_mut() = Source::new(self.main_id, text);
    }
    fn set_file(&self, name: &str, bytes: Vec<u8>) {
        let id = vpath(name);
        let mut files = self.files.borrow_mut();
        files.retain(|(fid, _)| *fid != id);
        files.push((id, Bytes::new(bytes)));
    }
}

impl World for BenchWorld {
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
            Ok(self.main.borrow().clone())
        } else {
            Err(FileError::NotFound(id.vpath().get_without_slash().into()))
        }
    }
    fn file(&self, id: FileId) -> FileResult<Bytes> {
        self.files
            .borrow()
            .iter()
            .find(|(fid, _)| *fid == id)
            .map(|(_, b)| b.clone())
            .ok_or_else(|| FileError::NotFound(id.vpath().get_without_slash().into()))
    }
    fn font(&self, index: usize) -> Option<Font> {
        self.fonts.font(index)
    }
    fn today(&self, _: Option<Duration>) -> Option<Datetime> {
        None
    }
}

fn compile(world: &BenchWorld) -> PagedDocument {
    compile_warned(world).0
}

fn compile_warned(world: &BenchWorld) -> (PagedDocument, usize, Vec<String>) {
    let r = typst::compile::<PagedDocument>(world);
    let warns: Vec<String> = r.warnings.iter().map(|w| w.message.to_string()).collect();
    let n = warns.len();
    match r.output {
        Ok(d) => (d, n, warns),
        Err(e) => {
            for err in &e {
                eprintln!("COMPILE ERROR: {}", err.message);
            }
            panic!("compilation failed");
        }
    }
}

/// Q3 — one pass: measure AND act, inside `context` rather than `layout()`.
/// `context` does not force a block-level container, so page(flipped:) is legal.
fn single_pass() -> String {
    format!(
        r#"#set page(width: {PAGE_W}pt, height: 320pt, margin: {MARGIN}pt)
#set text(size: 10pt)
{els}{ladder}
#for el in doc {{
  context {{
    let d = decide(el, {AVAIL}pt)
    [#metadata((id: el.id, ..d)) <d>]
    if d.rung == "rotate" {{ page(flipped: true, margin: {MARGIN}pt)[#el.body] }}
    else if d.rung == "shrink" {{ text(size: d.size * 1pt, el.body); v(6pt) }}
    else {{ el.body; v(6pt) }}
  }}
}}
"#,
        els = elements(),
        ladder = LADDER
    )
}

fn harvest(doc: &PagedDocument) -> Vec<serde_json::Value> {
    doc.introspector()
        .query_labelled()
        .iter()
        .filter_map(|c| c.to_packed::<MetadataElem>())
        .map(|m| serde_json::to_value(&m.value).unwrap())
        .collect()
}

/// The document body, shared by every variant so they measure the same thing.
fn elements() -> String {
    let mut s = String::from("#let doc = (\n");
    for i in 0..N {
        let e = match i % 5 {
            0 | 2 => format!("  (id: \"e{i}\", class: \"prose\",  body: lorem(60)),"),
            1 => format!(
                "  (id: \"e{i}\", class: \"table\",  body: table(columns: 6, ..range(12).map(j => [longvalue#j]))),"
            ),
            3 => format!(
                "  (id: \"e{i}\", class: \"code\",   body: raw(\"fn escalate(el: &Element) -> Decision {{ todo!() }}\", lang: \"rust\", block: true)),"
            ),
            _ => format!("  (id: \"e{i}\", class: \"figure\", body: rect(width: 400pt, height: 40pt)),"),
        };
        s.push_str(&e);
        s.push('\n');
    }
    s.push_str(")\n");
    s
}

const LADDER: &str = r#"
#let floors = (prose: 9pt, table: 7pt, code: 7pt, figure: 0pt)
#let atomic = ("table", "figure")
#let base = 10pt
#let decide(el, avail) = {
  if el.class not in atomic { return (rung: "none", size: none) }
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
"#;

/// A: current design — full document, layout() per element, rendering discarded.
fn probe_full() -> String {
    format!(
        r#"#set page(width: {PAGE_W}pt, height: 320pt, margin: {MARGIN}pt)
#set text(size: 10pt)
{els}{ladder}
#for el in doc {{
  layout(size => {{
    let d = decide(el, size.width)
    [#metadata((id: el.id, ..d)) <d>]
  }})
  el.body
  v(6pt)
}}
"#,
        els = elements(),
        ladder = LADDER
    )
}

/// B: proposed — available width known from the template; nothing paginated.
fn probe_measure_only() -> String {
    format!(
        r#"#set page(width: {PAGE_W}pt, height: auto, margin: {MARGIN}pt)
#set text(size: 10pt)
{els}{ladder}
#context {{
  for el in doc {{
    let d = decide(el, {AVAIL}pt)
    [#metadata((id: el.id, ..d)) <d>]
  }}
}}
"#,
        els = elements(),
        ladder = LADDER
    )
}

fn render_src() -> String {
    format!(
        r#"#set page(width: {PAGE_W}pt, height: 320pt, margin: {MARGIN}pt)
#set text(size: 10pt)
{els}
#let decisions = json("decisions.json")
#for el in doc {{
  let d = decisions.find(x => x.id == el.id)
  if d == none or d.rung == "none" {{ el.body; v(6pt) }}
  else if d.rung == "shrink" {{ text(size: d.size * 1pt, el.body); v(6pt) }}
  else {{ page(flipped: true, margin: {MARGIN}pt)[#el.body] }}
}}
"#,
        els = elements()
    )
}

/// comemo's memo cache is GLOBAL, not per-World. Without evicting, whichever
/// variant runs second looks artificially fast. Every "cold" number below is
/// taken after a full evict.
fn cold<T>(label: &str, f: impl FnOnce() -> T) -> T {
    comemo::evict(0);
    time(label, f)
}

fn time<T>(label: &str, f: impl FnOnce() -> T) -> T {
    let t = Instant::now();
    let r = f();
    println!("  {label:<46} {:>7.1} ms", t.elapsed().as_secs_f64() * 1000.0);
    r
}

fn main() {
    let world = BenchWorld::new();
    println!("Document: {N} elements (prose / table / code / figure), {PAGE_W}pt page, {AVAIL}pt available\n");

    // ---------------- Q1 ----------------
    println!("Q1 — does the ProbePass need a full document compile?");
    world.set_main(probe_full());
    let a = cold("A  full doc: layout() + measure + paginate", || harvest(&compile(&world)));
    world.set_main(probe_measure_only());
    let b = cold("B  measure-only: known width, never paginated", || harvest(&compile(&world)));

    let norm = |v: &Vec<serde_json::Value>| {
        let mut m: Vec<String> = v.iter().map(|x| x.to_string()).collect();
        m.sort();
        m
    };
    let agree = norm(&a) == norm(&b);
    println!("  decisions: A={} B={}  identical: {}", a.len(), b.len(), if agree { "YES" } else { "NO" });
    if !agree {
        for (x, y) in a.iter().zip(b.iter()).filter(|(x, y)| x != y).take(5) {
            println!("    A {x}\n    B {y}");
        }
    }

    // ---------------- Q2 ----------------
    println!("\nQ2 — what does an Override cost?");
    let mut decisions = b.clone();
    world.set_file("decisions.json", serde_json::to_vec(&decisions).unwrap());
    world.set_main(render_src());

    let d1 = cold("render: cold", || compile(&world));
    println!("        -> {} pages", d1.pages().len());
    time("render: warm (nothing changed, cache retained)", || compile(&world));

    // Flip exactly one element's decision — the Override hot path.
    let idx = decisions
        .iter()
        .position(|d| d["rung"] == "rotate")
        .expect("some element rotated");
    decisions[idx]["rung"] = serde_json::json!("none");
    world.set_file("decisions.json", serde_json::to_vec(&decisions).unwrap());
    let d2 = time("render: after ONE Override flipped", || compile(&world));
    println!("        -> {} pages", d2.pages().len());

    time("render: warm again after the change", || compile(&world));

    // And what a full re-probe would cost if an Override forced one.
    world.set_main(probe_measure_only());
    time("re-probe (measure-only), cache warm", || harvest(&compile(&world)));

    // ---------------- Q3 ----------------
    println!("\nQ3 — is the two-pass split needed at all?");
    let w2 = BenchWorld::new();
    w2.set_main(probe_measure_only());
    comemo::evict(0);
    let t = Instant::now();
    let probe = harvest(&compile(&w2));
    w2.set_file("decisions.json", serde_json::to_vec(&probe).unwrap());
    w2.set_main(render_src());
    let two = compile(&w2);
    let two_ms = t.elapsed().as_secs_f64() * 1000.0;
    println!("  two-pass  (probe -> harvest -> render)         {two_ms:>7.1} ms  -> {} pages", two.pages().len());

    let w3 = BenchWorld::new();
    w3.set_main(single_pass());
    comemo::evict(0);
    let t = Instant::now();
    let (one, nwarn, warns) = compile_warned(&w3);
    let one_ms = t.elapsed().as_secs_f64() * 1000.0;
    let one_dec = harvest(&one);
    println!("  single-pass (measure AND act in one compile)   {one_ms:>7.1} ms  -> {} pages", one.pages().len());
    println!("  decisions: two={} one={}  same pages: {}", probe.len(), one_dec.len(), two.pages().len() == one.pages().len());
    println!("  warnings: {nwarn}");
    for w in warns.iter().take(3) { println!("    {w}"); }
    println!("  ratio (two/one): {:.2}x", two_ms / one_ms);

    // Override on the single-pass design: overrides are read from a file the
    // context block consults, so the same hot path exists.
    time("  single-pass: warm recompile, nothing changed", || compile(&w3));
}
