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
