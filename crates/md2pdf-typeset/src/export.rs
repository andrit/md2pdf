//! PDF bytes and page rasters — both from the same Compilation.
//!
//! Preview is the output, by construction: one compile serves both.

use typst_layout::PagedDocument;

use crate::{PageGeometry, TypesetError};

pub fn pdf(doc: &PagedDocument) -> Result<Vec<u8>, TypesetError> {
    typst_pdf::pdf(doc, &typst_pdf::PdfOptions::default())
        .map_err(|e| TypesetError::Export(format!("{e:?}")))
}

/// RGBA8 raster of one page, for the preview surface. Returns pixels plus size, so
/// no typst type crosses the boundary.
pub fn raster(doc: &PagedDocument, page: usize, ppp: f32) -> Option<(u32, u32, Vec<u8>)> {
    let p = doc.pages().get(page)?;
    let opts = typst_render::RenderOptions {
        pixel_per_pt: typst::utils::Scalar::new(ppp as f64),
        ..Default::default()
    };
    let pixmap = typst_render::render(p, &opts);
    Some((pixmap.width(), pixmap.height(), pixmap.data().to_vec()))
}

/// The plain text actually rendered, in frame order.
///
/// This is the oracle for `md2pdf-convert`'s escaping round-trip: text in, compile,
/// text out, compare. `TextItem::text` is Typst's own record of what a glyph run
/// represents, so it survives ligatures and shaping.
///
/// Returns a `String` — a `Frame` must not cross this boundary.
pub fn text(doc: &PagedDocument) -> String {
    let mut out = String::new();
    for page in doc.pages() {
        collect_text(&page.frame, &mut out);
    }
    out
}

/// Each rendered glyph run as `(text, family, style)` — the oracle for **styling**.
///
/// `text()` reports characters and so cannot see a styling failure. Italic silently
/// not rendering (no italic face in the FontBook, `#emph` falling back to upright) was
/// invisible to every text-based test and had to be found by eye. This makes that
/// class of defect assertable: the run carrying "italic" must report style `Italic`.
///
/// Style crosses as a plain `&'static str`, not a typst enum.
pub fn text_runs(doc: &PagedDocument) -> Vec<(String, String, &'static str)> {
    let mut out = Vec::new();
    for page in doc.pages() {
        collect_runs(&page.frame, &mut out);
    }
    out
}

fn collect_runs(frame: &typst::layout::Frame, out: &mut Vec<(String, String, &'static str)>) {
    for (_, item) in frame.items() {
        match item {
            typst::layout::FrameItem::Text(t) => {
                let info = t.font.font().info();
                let style = match info.variant.style {
                    typst::text::FontStyle::Normal => "normal",
                    typst::text::FontStyle::Italic => "italic",
                    typst::text::FontStyle::Oblique => "oblique",
                };
                out.push((t.text.to_string(), info.family.clone(), style));
            }
            typst::layout::FrameItem::Group(g) => collect_runs(&g.frame, out),
            _ => {}
        }
    }
}

fn collect_text(frame: &typst::layout::Frame, out: &mut String) {
    for (_, item) in frame.items() {
        match item {
            typst::layout::FrameItem::Text(t) => out.push_str(&t.text),
            typst::layout::FrameItem::Group(g) => collect_text(&g.frame, out),
            _ => {}
        }
    }
}

pub fn geometry(doc: &PagedDocument) -> Vec<PageGeometry> {
    doc.pages()
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let s = p.frame.size();
            PageGeometry {
                number: i as u32 + 1,
                width_pt: s.x.to_pt(),
                height_pt: s.y.to_pt(),
            }
        })
        .collect()
}
