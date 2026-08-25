//! The Preview read model — *is* the output, by construction.
//!
//! Not a rendering of what the PDF will look like: the same `Compilation` that writes the
//! PDF is the one rastered here, so a preview that disagrees with the output is not
//! possible. That property is why `Preview` was a read model in the event storm rather
//! than a feature.

use md2pdf_typeset::Compilation;

/// One rastered page, ready to become a texture.
///
/// Plain RGBA and dimensions: no GUI type appears here, which is what keeps this crate
/// buildable without a display.
#[derive(Clone, PartialEq, Eq)]
pub struct Page {
    pub index: usize,
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

impl std::fmt::Debug for Page {
    /// The pixels are megabytes; printing them helps nobody.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Page")
            .field("index", &self.index)
            .field("width", &self.width)
            .field("height", &self.height)
            .field("bytes", &self.rgba.len())
            .finish()
    }
}

impl Page {
    /// Raster one page. `None` when the index is past the end.
    pub fn raster(compilation: &Compilation, index: usize, scale: f32) -> Option<Self> {
        let (width, height, rgba) = compilation.raster(index, scale)?;
        Some(Self {
            index,
            width,
            height,
            rgba,
        })
    }

    /// Roughly how much memory this page holds.
    ///
    /// Exists because the obvious way to make the app feel slow is to raster every page
    /// of a long document at 2x and keep them all: an A4 page at 2x is ~8 MB, so a
    /// 24-page document is 200 MB of textures nobody is looking at. The app keeps the
    /// visible page and its neighbours — see `plan-app.md` D2.
    pub fn bytes(&self) -> usize {
        self.rgba.len()
    }
}
