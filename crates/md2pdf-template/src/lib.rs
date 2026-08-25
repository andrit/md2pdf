//! The template catalogue — every Template discovered on disk, and every one refused.
//!
//! `INV-11`: *templates are swappable config, discovered from a directory.* Until this crate
//! existed the invariant was false — `Template::default()` was a Rust constructor and
//! `templates/` was empty.
//!
//! ## Refusals are carried, not dropped
//!
//! A rejected template is reported with a reason (`GLOSSARY`, *TemplateCatalogue*). Template
//! authoring is a supported activity, and a folder that silently fails to appear is the worst
//! feedback available — the author has nothing to correct.
//!
//! ## What a Template is, today
//!
//! A folder holding `template.toml`. The GLOSSARY also specifies a `template.typ` carrying the
//! layout; that is **T33**, and its real content is a correctness risk rather than the loading
//! — a show-rule the RenderPass applies and the ProbePass does not would make every
//! measurement wrong. See `design/plan-template-catalogue.md`.

pub mod file;
pub mod roots;

use std::path::{Path, PathBuf};

use md2pdf_domain::Template;
use md2pdf_paths::PathBroker;

pub use file::{Invalid, TemplateFile};

/// A Template that loaded, and where it came from.
#[derive(Debug, Clone)]
pub struct Found {
    pub template: Template,
    pub description: String,
    pub folder: PathBuf,
}

/// A folder that looked like a Template and could not be used.
#[derive(Debug, Clone)]
pub struct Rejected {
    pub folder: PathBuf,
    pub reason: String,
}

/// Everything discovered, in precedence order.
#[derive(Debug, Clone, Default)]
pub struct TemplateCatalogue {
    pub found: Vec<Found>,
    pub rejected: Vec<Rejected>,
}

impl TemplateCatalogue {
    /// Search every root, in order, merging the results.
    ///
    /// **The first root to supply a name wins**, so a user copying `github-print` into their
    /// config directory shadows the shipped one — which is the workflow 3e exists for. Later
    /// roots still contribute names the earlier ones did not have, so the shipped template
    /// stays available to someone who has only added their own.
    pub fn discover(roots: &[PathBuf], broker: &PathBroker) -> Self {
        Self::discover_with_fonts(roots, broker, &|_| true)
    }

    /// The same, checking that named fonts exist.
    ///
    /// **The check is injected rather than performed here**, because knowing which fonts
    /// exist means holding the FontBook, and the FontBook lives in `md2pdf-typeset` — the
    /// one crate allowed to link typst (`check-boundaries.sh`). Taking a predicate keeps
    /// this crate free of that dependency while still putting the *rejection* in the
    /// catalogue, which is where an author will look for it.
    ///
    /// It matters because `INV-1` means a font is never fetched: a template naming
    /// "Helvetica" would otherwise render in a silent fallback and look like a bug in
    /// md2pdf rather than a typo in the template.
    pub fn discover_with_fonts(
        roots: &[PathBuf],
        broker: &PathBroker,
        has_font: &dyn Fn(&str) -> bool,
    ) -> Self {
        let mut catalogue = Self::default();
        for root in roots {
            catalogue.absorb(root, broker, has_font);
        }
        catalogue
    }

    fn absorb(&mut self, root: &Path, broker: &PathBroker, has_font: &dyn Fn(&str) -> bool) {
        // A root that does not exist is ordinary — a user with no config directory, a build
        // with no `templates/` beside it. Not a rejection: nothing claimed to be a template.
        let Ok(entries) = broker.list_dirs(root) else {
            return;
        };
        for folder in entries {
            let manifest = folder.join("template.toml");
            if !broker.exists(&manifest) {
                // Also not a rejection: an unrelated directory is not a broken template.
                continue;
            }
            match broker
                .read_to_string(&manifest)
                .map_err(|e| Invalid(e.to_string()))
                .and_then(|text| TemplateFile::parse(&text))
            {
                Ok(file) => {
                    let missing: Vec<&str> =
                        [file.text.font_body.as_str(), file.text.font_mono.as_str()]
                            .into_iter()
                            .filter(|f| !has_font(f))
                            .collect();
                    if !missing.is_empty() {
                        self.rejected.push(Rejected {
                            folder,
                            reason: format!(
                                "font not available: {} — md2pdf never fetches a font, so it \
                                 must be one it ships",
                                missing.join(", ")
                            ),
                        });
                        continue;
                    }
                    if self.found.iter().any(|f| f.template.name == file.name) {
                        // Shadowed by an earlier root. Not an error, and not silent either —
                        // it is visible as the folder that did not appear.
                        continue;
                    }
                    self.found.push(Found {
                        template: file.to_template(),
                        description: file.description.clone(),
                        folder,
                    });
                }
                Err(reason) => self.rejected.push(Rejected {
                    folder,
                    reason: reason.to_string(),
                }),
            }
        }
    }

    /// One Template by name.
    pub fn get(&self, name: &str) -> Option<&Found> {
        self.found.iter().find(|f| f.template.name == name)
    }

    /// Every name that loaded — what to show when the requested one is not here.
    pub fn names(&self) -> Vec<&str> {
        self.found
            .iter()
            .map(|f| f.template.name.as_str())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use md2pdf_paths::testing::TempDir;

    fn write_template(dir: &TempDir, folder: &str, toml: &str) -> PathBuf {
        // Through the fixture helper rather than `std::fs`: `INV-9` holds in tests too,
        // and `check-boundaries.sh` enforces it.
        dir.write(&format!("{folder}/template.toml"), toml.as_bytes());
        dir.join(folder)
    }

    fn shipped(name: &str, base: f64) -> String {
        format!(
            r#"
name = "{name}"
[page]
width_pt = 595.0
height_pt = 842.0
margin_pt = 56.0
[text]
base_size_pt = {base}
font_body = "Source Sans 3"
font_mono = "JetBrains Mono"
"#
        )
    }

    #[test]
    fn a_folder_with_a_manifest_is_discovered() {
        let dir = TempDir::new("catalogue-finds");
        write_template(&dir, "mine", &shipped("mine", 11.0));
        let c = TemplateCatalogue::discover(&[dir.path().to_path_buf()], &PathBroker::new());
        assert_eq!(c.names(), vec!["mine"]);
        assert_eq!(c.get("mine").unwrap().template.base_size_pt, 11.0);
        assert!(c.rejected.is_empty());
    }

    #[test]
    fn a_broken_manifest_is_reported_with_a_reason_and_the_rest_still_load() {
        // The whole point of carrying rejections: one bad folder must not cost the user
        // their other templates, and must not vanish without explanation.
        let dir = TempDir::new("catalogue-rejects");
        write_template(&dir, "good", &shipped("good", 12.0));
        write_template(&dir, "broken", "name = \"broken\"\nthis is not toml");
        let c = TemplateCatalogue::discover(&[dir.path().to_path_buf()], &PathBroker::new());
        assert_eq!(c.names(), vec!["good"]);
        assert_eq!(c.rejected.len(), 1);
        assert!(c.rejected[0].folder.ends_with("broken"));
        assert!(
            !c.rejected[0].reason.is_empty(),
            "a rejection with no reason"
        );
    }

    #[test]
    fn a_directory_that_is_not_a_template_is_ignored_rather_than_rejected() {
        // An unrelated folder is not a broken template, and reporting it as one would train
        // the user to ignore the rejection list.
        let dir = TempDir::new("catalogue-ignores");
        dir.write("notes/README.md", b"not a template");
        write_template(&dir, "real", &shipped("real", 12.0));
        let c = TemplateCatalogue::discover(&[dir.path().to_path_buf()], &PathBroker::new());
        assert_eq!(c.names(), vec!["real"]);
        assert!(c.rejected.is_empty(), "{:?}", c.rejected);
    }

    #[test]
    fn a_missing_root_is_not_an_error() {
        let c = TemplateCatalogue::discover(
            &[PathBuf::from("/definitely/not/here")],
            &PathBroker::new(),
        );
        assert!(c.found.is_empty() && c.rejected.is_empty());
    }

    #[test]
    fn an_earlier_root_shadows_a_later_one_by_name() {
        // The workflow 3e exists for: copy the shipped template into your config directory,
        // edit it, and have yours win — while everything you did not copy still appears.
        let user = TempDir::new("catalogue-user");
        let shipped_dir = TempDir::new("catalogue-shipped");
        write_template(&user, "github-print", &shipped("github-print", 14.0));
        write_template(&shipped_dir, "github-print", &shipped("github-print", 12.0));
        write_template(&shipped_dir, "other", &shipped("other", 12.0));

        let c = TemplateCatalogue::discover(
            &[user.path().to_path_buf(), shipped_dir.path().to_path_buf()],
            &PathBroker::new(),
        );
        assert_eq!(
            c.get("github-print").unwrap().template.base_size_pt,
            14.0,
            "the user's copy should win"
        );
        assert!(
            c.names().contains(&"other"),
            "a later root must still contribute names the earlier one lacked"
        );
    }

    #[test]
    fn a_template_naming_a_font_we_do_not_ship_is_refused() {
        // `INV-1` means md2pdf never fetches a font. Without this the template loads, the
        // document renders in whatever Typst falls back to, and it looks like a bug in
        // md2pdf rather than a typo in the template.
        let dir = TempDir::new("catalogue-fonts");
        write_template(
            &dir,
            "helvetica",
            &shipped("helvetica", 12.0).replace("Source Sans 3", "Helvetica"),
        );
        let c = TemplateCatalogue::discover_with_fonts(
            &[dir.path().to_path_buf()],
            &PathBroker::new(),
            &|f| f == "JetBrains Mono",
        );
        assert!(c.found.is_empty());
        assert_eq!(c.rejected.len(), 1);
        assert!(
            c.rejected[0].reason.contains("Helvetica"),
            "the rejection does not name the font: {}",
            c.rejected[0].reason
        );
    }
}
