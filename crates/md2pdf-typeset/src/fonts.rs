//! The shipped FontBook.
//!
//! Typst embeds four families and NO sans-serif, so md2pdf ships its own. Bundling
//! rather than resolving from the system is what makes PDF output identical across
//! macOS, Windows and Linux. See `assets/fonts/README.md`.

use typst::foundations::Bytes;
use typst::text::{Font, FontBook, FontInfo};

/// Vendored at build time — not fetched, so builds are reproducible and offline.
const SOURCE_SANS_3: &[u8] = include_bytes!("../../../assets/fonts/SourceSans3.ttf");
const JETBRAINS_MONO: &[u8] = include_bytes!("../../../assets/fonts/JetBrainsMono.ttf");

pub struct FontLibrary {
    pub book: FontBook,
    pub fonts: Vec<Font>,
}

impl FontLibrary {
    /// Typst's own faces plus ours. Typst's are kept because math and the default
    /// serif still resolve through them.
    pub fn shipped() -> Self {
        let mut book = FontBook::new();
        let mut fonts = Vec::new();

        for (font, info) in typst_kit::fonts::embedded() {
            book.push(info);
            fonts.push(font);
        }
        for bytes in [SOURCE_SANS_3, JETBRAINS_MONO] {
            let data = Bytes::new(bytes.to_vec());
            for (i, info) in FontInfo::iter(&data).enumerate() {
                if let Some(font) = Font::new(data.clone(), i as u32) {
                    book.push(info);
                    fonts.push(font);
                }
            }
        }
        Self { book, fonts }
    }

    pub fn families(&self) -> Vec<String> {
        let mut v: Vec<String> = self.book.families().map(|(n, _)| n.to_string()).collect();
        v.sort_unstable();
        v.dedup();
        v
    }
}
