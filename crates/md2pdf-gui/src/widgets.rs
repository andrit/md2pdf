//! The read models, drawn.
//!
//! **Every function here is named for the read model it draws** — `attention_list()`, not
//! `WarningsPanel` (`GLOSSARY`, naming). The names came from the event storm before any of
//! this existed, and keeping them is what makes a conversation about "the attention list"
//! mean one thing in the design, the domain and the window.
//!
//! **Nothing here decides anything about a document.** A widget returns what was clicked
//! as a [`Request`]; whether an offer exists, what it is called, and what a Compromise
//! means are all settled in `md2pdf-app`. That rule is what makes this the only file in
//! the project written against an API the container cannot run — a mistake here is a
//! compile error or a misplaced label, never a wrong decision about a page.

use md2pdf_app::state::{App, SourceState};
use md2pdf_app::worker::Request;
use md2pdf_domain::Override;

/// The chosen Sources and what happened to each.
pub fn source_list(ui: &mut egui::Ui, app: &App) -> Option<Request> {
    let mut intent = None;
    ui.heading("Documents");
    if app.sources.is_empty() {
        ui.label("Drop a markdown file or a folder onto this window.");
        return None;
    }
    egui::ScrollArea::vertical()
        .id_salt("sources")
        .max_height(220.0)
        .show(ui, |ui| {
            for (path, state) in &app.sources {
                let name = path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| path.display().to_string());
                ui.horizontal(|ui| {
                    // Colour is a hint, never the only signal — the word is always there.
                    let (mark, tint) = match state {
                        SourceState::Pending => ("·", ui.visuals().weak_text_color()),
                        SourceState::Converted | SourceState::Written => {
                            ("ok", ui.visuals().text_color())
                        }
                        SourceState::Flagged => ("needs attention", ui.visuals().warn_fg_color),
                        SourceState::Skipped(_) => ("skipped", ui.visuals().warn_fg_color),
                        SourceState::Failed(_) => ("failed", ui.visuals().error_fg_color),
                    };
                    ui.colored_label(tint, mark);
                    if ui.link(name).clicked() {
                        intent = Some(app.open_request(path));
                    }
                });
                if let SourceState::Failed(why) | SourceState::Skipped(why) = state {
                    ui.small(why);
                }
            }
        });
    intent
}

/// Templates found on disk — **including the ones that were refused, and why**.
///
/// Showing rejections is the whole reason 3e carries them: a template that silently fails
/// to appear leaves its author with nothing to correct.
pub fn template_catalogue(ui: &mut egui::Ui, app: &mut App) {
    ui.heading("Template");
    let current = app
        .chosen
        .template
        .clone()
        .unwrap_or_else(|| "github-print (built in)".to_string());
    egui::ComboBox::from_id_salt("template")
        .selected_text(current)
        .show_ui(ui, |ui| {
            for found in &app.catalogue.found {
                let name = found.template.name.clone();
                ui.selectable_value(&mut app.chosen.template, Some(name.clone()), name);
            }
        });
    for rejected in &app.catalogue.rejected {
        ui.colored_label(
            ui.visuals().warn_fg_color,
            format!(
                "{} was not loaded: {}",
                rejected.folder.display(),
                rejected.reason
            ),
        );
    }
}

/// Where output goes, and what to do about files already there.
///
/// The collision answer is asked **before** the run, because the engine resolves every
/// collision up front and never asks mid-Job — `plan-app.md` D6.
pub fn job_settings(ui: &mut egui::Ui, app: &mut App) -> Option<Request> {
    use md2pdf_domain::BlanketResolution as R;
    ui.heading("Output");

    let shown = app
        .chosen
        .destination
        .as_ref()
        .map(|d| d.display().to_string())
        .unwrap_or_else(|| "drop a folder, or type a path".into());
    let mut typed = shown.clone();
    if ui.text_edit_singleline(&mut typed).changed() && !typed.trim().is_empty() {
        app.chosen.destination = Some(typed.trim().into());
    }

    ui.horizontal(|ui| {
        ui.label("If a PDF already exists:");
        for (label, value) in [
            ("skip it", R::SkipAll),
            ("write alongside", R::RenameAll),
            ("replace it", R::OverwriteAll),
        ] {
            ui.selectable_value(&mut app.chosen.on_collision, Some(value), label);
        }
    });

    let is_directory = app
        .chosen
        .path
        .as_ref()
        .map(|p| p.is_dir())
        .unwrap_or(false);
    match app.chosen.command(is_directory) {
        Ok(command) => {
            let go = ui.add_enabled(!app.running, egui::Button::new("Convert"));
            go.clicked().then_some(Request::Run(command))
        }
        Err(missing) => {
            // The button is disabled *and says why*. A greyed-out control with no
            // explanation is the commonest way a UI wastes someone's afternoon.
            ui.add_enabled(false, egui::Button::new("Convert"));
            ui.small(missing);
            None
        }
    }
}

/// What md2pdf conceded, and what can be allowed instead.
pub fn attention_list(ui: &mut egui::Ui, app: &App) -> Option<Request> {
    let Some(list) = &app.attention else {
        return None;
    };
    let mut intent = None;
    ui.heading("Needs your attention");
    if list.is_empty() {
        ui.label("Nothing — this document converted cleanly.");
        return None;
    }
    egui::ScrollArea::vertical()
        .id_salt("attention")
        .show(ui, |ui| {
            for item in &list.items {
                ui.group(|ui| {
                    ui.label(describe(&item.what));
                    if let Some(request) = adjustment_panel(ui, app, item) {
                        intent = Some(request);
                    }
                });
            }
        });
    intent
}

/// The offers for one Element.
///
/// **Only where the Diagnostic named an Element** — md2pdf is not a layout editor, and it
/// offers a fix exactly where the engine already admitted it compromised. An entry with
/// no offers (a missing image file) draws nothing here rather than an inert button.
pub fn adjustment_panel(
    ui: &mut egui::Ui,
    app: &App,
    item: &md2pdf_domain::Attention,
) -> Option<Request> {
    let mut intent = None;
    for offer in &item.offers {
        if ui.button(offer.label).clicked() {
            intent = Some(app.allow_request(Override {
                id: item.id,
                permit: offer.permit,
            }));
        }
    }
    intent
}

/// The rendered page. **It *is* the output** — the same Compilation that writes the PDF.
pub fn preview(ui: &mut egui::Ui, app: &App, texture: Option<&egui::TextureHandle>) {
    ui.horizontal(|ui| {
        ui.heading("Preview");
        if app.pages > 0 {
            ui.label(format!("page {} of {}", app.showing + 1, app.pages));
        }
    });
    match texture {
        Some(handle) => {
            egui::ScrollArea::both().id_salt("preview").show(ui, |ui| {
                ui.add(
                    egui::Image::new(handle)
                        .max_width(ui.available_width())
                        .fit_to_original_size(1.0),
                );
            });
        }
        None if app.open.is_some() => {
            ui.spinner();
        }
        None => {
            ui.label("Open a document to see its pages.");
        }
    }
}

/// Progress and the summary sentence the whole design aims at.
pub fn batch_progress(ui: &mut egui::Ui, app: &App) {
    if app.running {
        ui.horizontal(|ui| {
            ui.spinner();
            ui.label("Converting…");
        });
    }
    if !app.sources.is_empty() {
        ui.label(app.summary());
    }
    if let Some(problem) = &app.problem {
        ui.colored_label(ui.visuals().error_fg_color, problem);
    }
}

/// A Compromise in the user's terms.
///
/// Duplicated from the CLI's `describe` deliberately? **No** — this is the one place the
/// wording could drift between the two adapters, and it is worth watching. It is here
/// rather than in `md2pdf-app` only because the CLI's copy predates that crate; if a third
/// adapter appears, this belongs in the domain.
fn describe(kind: &md2pdf_domain::CompromiseKind) -> String {
    use md2pdf_domain::CompromiseKind as K;
    match kind {
        K::ShrunkToFloor { size_pt } => format!("shrunk to {size_pt}pt to fit"),
        K::Scaled { factor } => format!("scaled to {:.0}% to fit", factor * 100.0),
        K::Rotated => "given a landscape page".into(),
        K::Reflowed => "wrapped its cells instead of shrinking".into(),
        K::Clipped => "CLIPPED — content was lost".into(),
        K::ImageMissing => "image not found on disk".into(),
        K::ImageSkipped => "remote image not fetched".into(),
        K::UnsupportedConstruct { construct } => construct.clone(),
    }
}
