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
