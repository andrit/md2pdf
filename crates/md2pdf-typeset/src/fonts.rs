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

/// Italic is a **separate file**, and this is not optional.
///
/// The variable `SourceSans3.ttf` carries a `wght` axis and nothing else — no `ital`,
/// no `slnt`. So `#emph` had no italic face to resolve and silently fell back to
/// upright: emphasis, one of the commonest markdown constructs, rendered identically
/// to plain text. No text-based test could see it, because `Compilation::text()`
/// reports characters, not styling; it was found by looking at a rendered page.
///
/// These are the **static** release faces rather than the variable italic, because the
/// variable one reports its family as `SourceSans3VF` while the roman reports
/// `Source Sans 3`. Typst groups faces by family name, so the mismatched pair would
/// never have been recognised as one family and the bug would have survived the fix.
const SOURCE_SANS_3_ITALIC: &[u8] = include_bytes!("../../../assets/fonts/SourceSans3-It.ttf");
const SOURCE_SANS_3_BOLD_ITALIC: &[u8] =
    include_bytes!("../../../assets/fonts/SourceSans3-BoldIt.ttf");

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
        for bytes in [
            SOURCE_SANS_3,
            SOURCE_SANS_3_ITALIC,
            SOURCE_SANS_3_BOLD_ITALIC,
            JETBRAINS_MONO,
        ] {
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

    /// Which of these characters no shipped face can draw.
    ///
    /// A character with no glyph renders as **tofu** — an empty box — and nothing in the
    /// text layer says so: extracted text carries the character either way, which is why
    /// ✅ and ❌ sat unnoticed in a fifth of the corpus until a page was looked at (T28).
    ///
    /// Asks every face rather than only the body family, because Typst falls back across
    /// the whole book: a character counts as covered if *anything* can draw it.
    pub fn uncovered(&self, chars: impl IntoIterator<Item = char>) -> Vec<char> {
        chars
            .into_iter()
            .filter(|c| {
                !(0..self.fonts.len())
                    .filter_map(|i| self.book.info(i))
                    .any(|info| info.coverage.contains(*c as u32))
            })
            .collect()
    }

    pub fn families(&self) -> Vec<String> {
        let mut v: Vec<String> = self.book.families().map(|(n, _)| n.to_string()).collect();
        v.sort_unstable();
        v.dedup();
        v
    }
}
