//! Template tokens. The subset the typesetting layer needs.

use serde::{Deserialize, Serialize};

use crate::element::ElementClass;

/// Per-class minimum font size. Never global — body prose is read linearly and
/// fatigues; tables are scanned.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Floors {
    pub prose_pt: f64,
    pub table_pt: f64,
    pub code_pt: f64,
    /// Smallest scale an image may be reduced to before rotating instead (0..1).
    ///
    /// Not a point size — images shrink by scale factor. Without a floor here shrink
    /// would always succeed and rotation would never fire, however unreadable the
    /// result. A quarter size is the starting point; rotation buys roughly 1.5x the
    /// width, which is the better trade below that. Tunable by eye like the others.
    pub image_scale: f64,
}

impl Default for Floors {
    fn default() -> Self {
        Self {
            prose_pt: 9.0,
            table_pt: 7.0,
            code_pt: 7.0,
            image_scale: 0.25,
        }
    }
}

impl Floors {
    /// The point-size floor for a text-bearing class.
    ///
    /// Meaningless for `Image`, which shrinks by scale — use [`Floors::image_scale`].
    pub fn for_class(&self, class: ElementClass) -> f64 {
        match class {
            ElementClass::Table => self.table_pt,
            ElementClass::Code => self.code_pt,
            ElementClass::Image => 0.0,
            _ => self.prose_pt,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Template {
    pub name: String,
    pub page_width_pt: f64,
    pub page_height_pt: f64,
    pub margin_pt: f64,
    pub base_size_pt: f64,
    pub floors: Floors,
    pub font_body: String,
    pub font_mono: String,
}

impl Default for Template {
    fn default() -> Self {
        Self {
            name: "github-print".into(),
            page_width_pt: 595.0, // A4
            page_height_pt: 842.0,
            margin_pt: 56.0,
            base_size_pt: 10.0,
            floors: Floors::default(),
            font_body: "Source Sans 3".into(),
            font_mono: "JetBrains Mono".into(),
        }
    }
}

impl Template {
    /// Available width, computed directly rather than asked of Typst.
    ///
    /// This is what lets the ProbePass skip `layout()` entirely — 3.6x cheaper for
    /// identical decisions. See `design/spike-typst-measure-findings.md`.
    pub fn available_pt(&self) -> f64 {
        self.page_width_pt - 2.0 * self.margin_pt
    }
}
