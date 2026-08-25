//! `template.toml` — the on-disk form of a Template.
//!
//! **A separate shape from [`md2pdf_domain::Template`], on purpose.** The in-memory struct is
//! flat because that is convenient for the code that reads it; a person editing a file wants
//! the page separated from the type separated from the floors. Field names in a config file
//! are a user interface, and inheriting ten Rust field names would be choosing the compiler's
//! convenience over theirs.
//!
//! The mapping is written out rather than derived, which is the cost of that decision and is
//! also where validation naturally lives.

use md2pdf_domain::{Floors, Template};
use serde::{Deserialize, Serialize};

/// What a `template.toml` file contains.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TemplateFile {
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub page: Page,
    pub text: Text,
    #[serde(default)]
    pub floors: FloorTokens,
}

/// `deny_unknown_fields` is deliberate: a typo'd key is the commonest authoring mistake, and
/// silently ignoring `margin_pts` while the margin stays at its default is the worst possible
/// response — the author sees no change and no reason.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Page {
    pub width_pt: f64,
    pub height_pt: f64,
    pub margin_pt: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Text {
    pub base_size_pt: f64,
    pub font_body: String,
    pub font_mono: String,
}

/// Floors have defaults because most template authors should not think about them: they are
/// the ladder's limits, not the page's design, and T26c chose them by eye against real pages.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FloorTokens {
    #[serde(default = "default_table_pt")]
    pub table_pt: f64,
    #[serde(default = "default_comfort_pt")]
    pub table_comfort_pt: f64,
    #[serde(default = "default_image_scale")]
    pub image_scale: f64,
}

fn default_table_pt() -> f64 {
    Floors::default().table_pt
}
fn default_comfort_pt() -> f64 {
    Floors::default().table_comfort_pt
}
fn default_image_scale() -> f64 {
    Floors::default().image_scale
}

impl Default for FloorTokens {
    fn default() -> Self {
        Self {
            table_pt: default_table_pt(),
            table_comfort_pt: default_comfort_pt(),
            image_scale: default_image_scale(),
        }
    }
}

/// Why a template could not be used, in words an author can act on.
#[derive(Debug, Clone, PartialEq)]
pub struct Invalid(pub String);

impl std::fmt::Display for Invalid {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl TemplateFile {
    /// Parse and check. **Every rejection names the key and the problem.**
    pub fn parse(toml_text: &str) -> Result<Self, Invalid> {
        let file: TemplateFile =
            toml::from_str(toml_text).map_err(|e| Invalid(message_of(&e.to_string())))?;
        file.validate()?;
        Ok(file)
    }

    /// The checks a number cannot make for itself.
    ///
    /// Not exhaustive by design — this catches values that would produce a broken or blank
    /// page, not values that are merely unusual. A 400pt margin on an A4 page is absurd and is
    /// also the author's business.
    fn validate(&self) -> Result<(), Invalid> {
        if self.name.trim().is_empty() {
            return Err(Invalid("`name` is empty".into()));
        }
        for (key, v) in [
            ("page.width_pt", self.page.width_pt),
            ("page.height_pt", self.page.height_pt),
            ("text.base_size_pt", self.text.base_size_pt),
        ] {
            if !(v.is_finite() && v > 0.0) {
                return Err(Invalid(format!(
                    "`{key}` must be a positive number, got {v}"
                )));
            }
        }
        if !self.page.margin_pt.is_finite() || self.page.margin_pt < 0.0 {
            return Err(Invalid(format!(
                "`page.margin_pt` cannot be negative, got {}",
                self.page.margin_pt
            )));
        }
        // The margins must leave something to print on. Without this a template with a 300pt
        // margin on A4 compiles happily and every element is "too wide" forever.
        let available = self.page.width_pt - 2.0 * self.page.margin_pt;
        if available <= self.text.base_size_pt {
            return Err(Invalid(format!(
                "`page.margin_pt` of {} leaves {available}pt of width, which is narrower than \
                 one character at `text.base_size_pt` {}",
                self.page.margin_pt, self.text.base_size_pt
            )));
        }
        // A comfort floor above the base means every table reflows before it can shrink at
        // all, which is legal but is almost certainly a mistake worth naming.
        if self.floors.table_comfort_pt > self.text.base_size_pt {
            return Err(Invalid(format!(
                "`floors.table_comfort_pt` {} is above `text.base_size_pt` {} — no table could \
                 ever shrink",
                self.floors.table_comfort_pt, self.text.base_size_pt
            )));
        }
        if !(0.0..=1.0).contains(&self.floors.image_scale) {
            return Err(Invalid(format!(
                "`floors.image_scale` must be between 0 and 1, got {}",
                self.floors.image_scale
            )));
        }
        Ok(())
    }

    /// The domain value this file describes.
    pub fn to_template(&self) -> Template {
        Template {
            name: self.name.clone(),
            page_width_pt: self.page.width_pt,
            page_height_pt: self.page.height_pt,
            margin_pt: self.page.margin_pt,
            base_size_pt: self.text.base_size_pt,
            floors: Floors {
                table_pt: self.floors.table_pt,
                table_comfort_pt: self.floors.table_comfort_pt,
                image_scale: self.floors.image_scale,
            },
            font_body: self.text.font_body.clone(),
            font_mono: self.text.font_mono.clone(),
        }
    }

    /// The file form of a Template — used to write the shipped one and to prove round-tripping.
    pub fn from_template(t: &Template, description: &str) -> Self {
        Self {
            name: t.name.clone(),
            description: description.into(),
            page: Page {
                width_pt: t.page_width_pt,
                height_pt: t.page_height_pt,
                margin_pt: t.margin_pt,
            },
            text: Text {
                base_size_pt: t.base_size_pt,
                font_body: t.font_body.clone(),
                font_mono: t.font_mono.clone(),
            },
            floors: FloorTokens {
                table_pt: t.floors.table_pt,
                table_comfort_pt: t.floors.table_comfort_pt,
                image_scale: t.floors.image_scale,
            },
        }
    }
}

/// Pull the sentence out of a TOML error.
///
/// `toml`'s Display is a multi-line span diagram — a location header, a caret, then the
/// actual complaint. The first line is *"TOML parse error at line 6, column 1"*, which
/// tells an author where but never what. The complaint is the last non-empty line, and it
/// is the one that names the key they mistyped.
fn message_of(s: &str) -> String {
    let last = s
        .lines()
        .map(str::trim)
        .rfind(|l| !l.is_empty())
        .unwrap_or(s);
    let head = s.lines().next().unwrap_or("").trim();
    if last == head {
        head.to_string()
    } else {
        format!("{last} ({head})")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SHIPPED: &str = r#"
name = "github-print"
description = "GitHub's rendering, sized for print"

[page]
width_pt = 595.0
height_pt = 842.0
margin_pt = 56.0

[text]
base_size_pt = 12.0
font_body = "Source Sans 3"
font_mono = "JetBrains Mono"

[floors]
table_pt = 7.0
table_comfort_pt = 10.0
image_scale = 0.25
"#;

    /// The file that actually ships, not a copy of it in a test.
    ///
    /// `include_str!` rather than a literal: a test that pins its own copy of the values
    /// proves the parser works and nothing about the file the user gets.
    const SHIPPED_FILE: &str = include_str!("../../../templates/github-print/template.toml");

    #[test]
    fn the_file_that_ships_is_the_rust_default() {
        // **The load-bearing test of 3e.** `Template::default()` stays as the fallback for
        // when no catalogue is available, so if the file and the constructor disagree the
        // same document renders differently depending on whether a directory happened to
        // exist — and the goldens would move for a reason that looks like a rendering
        // change and is not.
        let file = TemplateFile::parse(SHIPPED_FILE).expect("the shipped template must parse");
        assert_eq!(file.to_template(), Template::default());
    }

    #[test]
    fn the_shipped_values_round_trip_to_the_rust_default() {
        // The load-bearing one. `Template::default()` stays as the fallback, so if the file
        // and the constructor disagree the same document renders differently depending on
        // whether a directory happened to exist — and the goldens would move for a reason
        // that looks like a rendering change and is not.
        let file = TemplateFile::parse(SHIPPED).expect("shipped template parses");
        assert_eq!(file.to_template(), Template::default());
    }

    #[test]
    fn a_template_survives_a_round_trip_through_the_file_form() {
        let t = Template::default();
        let back = TemplateFile::from_template(&t, "").to_template();
        assert_eq!(back, t);
    }

    #[test]
    fn floors_are_optional_because_most_authors_should_not_touch_them() {
        let without = SHIPPED.split("[floors]").next().unwrap();
        let file = TemplateFile::parse(without).expect("floors should be optional");
        assert_eq!(file.to_template().floors, Floors::default());
    }

    #[test]
    fn a_mistyped_key_is_refused_rather_than_ignored() {
        // The commonest authoring mistake. Ignoring it means the author edits a value, sees
        // no change, and has nothing to go on.
        let typo = SHIPPED.replace("margin_pt", "margin_pts");
        let err = TemplateFile::parse(&typo).expect_err("a typo must be refused");
        assert!(
            err.0.contains("margin_pts") || err.0.contains("unknown field"),
            "the error does not name the key: {err}"
        );
    }

    #[test]
    fn margins_must_leave_something_to_print_on() {
        let squeezed = SHIPPED.replace("margin_pt = 56.0", "margin_pt = 295.0");
        let err = TemplateFile::parse(&squeezed).expect_err("no width left");
        assert!(err.0.contains("margin_pt"), "{err}");
    }

    #[test]
    fn a_comfort_floor_above_the_base_is_refused() {
        let odd = SHIPPED.replace("table_comfort_pt = 10.0", "table_comfort_pt = 14.0");
        let err = TemplateFile::parse(&odd).expect_err("comfort above base");
        assert!(err.0.contains("table_comfort_pt"), "{err}");
    }

    #[test]
    fn nonsense_numbers_are_refused_by_name() {
        for (from, to, key) in [
            ("width_pt = 595.0", "width_pt = 0.0", "page.width_pt"),
            (
                "base_size_pt = 12.0",
                "base_size_pt = -3.0",
                "text.base_size_pt",
            ),
            (
                "image_scale = 0.25",
                "image_scale = 2.0",
                "floors.image_scale",
            ),
        ] {
            let err = TemplateFile::parse(&SHIPPED.replace(from, to))
                .expect_err("should be refused: {to}");
            assert!(err.0.contains(key), "wrong reason for {to}: {err}");
        }
    }

    #[test]
    fn an_empty_name_is_refused_because_the_catalogue_is_keyed_by_it() {
        let err = TemplateFile::parse(&SHIPPED.replace(r#"name = "github-print""#, r#"name = """#))
            .expect_err("empty name");
        assert!(err.0.contains("name"), "{err}");
    }
}
