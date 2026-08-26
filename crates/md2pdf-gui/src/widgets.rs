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

use md2pdf_app::state::{App, Destination, SourceState};
use md2pdf_app::worker::Request;

/// The chosen Sources and what happened to each.
///
/// **Draws what has been chosen, not only what has been converted.** The first version
/// showed `app.sources`, which stays empty until a Job runs — so dropping a file set the
/// state and the window went on saying "drop a file", and the operator reported the drop
/// as doing nothing. It was working; nothing rendered it. A UI that accepts input without
/// acknowledging it is indistinguishable from one that is broken.
pub fn source_list(ui: &mut egui::Ui, app: &App, hovering: usize) -> Option<Request> {
    let mut intent = None;
    ui.heading("Documents");

    // Acknowledge the *attempt*, before the drop even lands.
    if hovering > 0 {
        ui.colored_label(
            ui.visuals().selection.stroke.color,
            format!("release to add {hovering} file(s)"),
        );
    }

    match &app.chosen.path {
        Some(path) => {
            // **The name, truncated — never the whole path.** A `SidePanel` takes its
            // width from its content (`panel.rs`: the returned rect is the frame's, and
            // `PanelState` persists it), so one unwrappable label of an absolute path
            // inflates the panel every frame and no drag can shrink it again. A dropped
            // file from a deep directory ate the window. The path is still one hover away.
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| path.display().to_string());
            ui.horizontal(|ui| {
                ui.strong(if path.is_dir() { "folder:" } else { "file:" });
                ui.add(egui::Label::new(name).truncate())
                    .on_hover_text(path.display().to_string());
            });
        }
        None => {
            ui.label("Drop a markdown file or a folder onto this window.");
        }
    }

    if app.sources.is_empty() {
        return None;
    }
    ui.separator();
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
pub fn job_settings(ui: &mut egui::Ui, app: &mut App, destination: &mut String) -> Option<Request> {
    use md2pdf_domain::BlanketResolution as R;
    ui.heading("Output");

    // Backed by a real String rather than rebuilt from the PathBuf each frame: an
    // egui text field edits the buffer it is given, and handing it a fresh temporary
    // every frame loses the caret and fights the typist.
    ui.horizontal(|ui| {
        ui.label("Save to:");
        ui.text_edit_singleline(destination);
        // **A button beside the field, not a panel on click-into-the-field.** Opening a
        // modal when a text box takes focus would fight anyone trying to type, and typing
        // still has to work — it is the only way in on a platform with no panel. Left out
        // entirely where there is nothing to open, rather than drawn dead.
        if md2pdf_mac::can_choose() && ui.button("Choose…").clicked() {
            // Start where they already are, so the panel opens somewhere meaningful
            // rather than at the default.
            if let Some(picked) = md2pdf_mac::choose_folder(app.chosen.destination.as_deref()) {
                // Written back into the field, not just into `chosen`: the field is the
                // record of what was asked for, and the two disagreeing is the bug the
                // dropped-folder path already had to fix.
                *destination = picked.display().to_string();
            }
        }
    });
    // Re-read every frame rather than on `changed()`: a rejection has to stay on screen
    // for as long as the text that caused it does, and `changed()` fires once.
    match app.type_destination(destination) {
        Destination::Empty => {
            ui.small("drop a folder onto the window, or type a full path");
        }
        // Refused, and it says why. Typing `~/Documents/out` used to create a folder
        // *named* `~`, and a bare `out` wrote to the root of the disk, because a
        // double-clicked .app has `/` for a working directory.
        Destination::Rejected(why) => {
            ui.colored_label(ui.visuals().warn_fg_color, why);
        }
        // Only when it is not what was typed — showing `/Users/you/out` under a field
        // already reading `/Users/you/out` is noise, but showing what a `~` became is
        // the difference between trusting the expansion and checking it.
        Destination::Folder(path) => {
            let shown = path.display().to_string();
            if shown != destination.trim() {
                ui.small(format!("→ {shown}"));
            }
        }
    }

    // **Radios, not selectable labels.** `selectable_value` draws nothing at all when it
    // is unselected — so three *required* choices rendered as plain text continuing the
    // sentence above them, and a folder drop looked like the app doing nothing while it
    // waited for an answer nobody could see was being asked. A radio with an empty circle
    // says both that this is a control and that it has not been used.
    //
    // Still no default. The engine refuses one because every possible answer is wrong
    // (`plan-app.md` D6); the fix for an unanswered question is to make it look like a
    // question, not to answer it on the user's behalf.
    let unanswered = app.chosen.on_collision.is_none();
    ui.horizontal(|ui| {
        // Tinted while it is the thing standing between you and a conversion, so the eye
        // lands on the control rather than on the greyed-out button it is disabling.
        if unanswered {
            ui.colored_label(ui.visuals().warn_fg_color, "If a PDF already exists:");
        } else {
            ui.label("If a PDF already exists:");
        }
        for (label, value) in [
            ("skip it", R::SkipAll),
            ("write alongside", R::RenameAll),
            ("replace it", R::OverwriteAll),
        ] {
            ui.radio_value(&mut app.chosen.on_collision, Some(value), label);
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
            go.clicked().then(|| app.run_request(command))
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
    let mut intent = None;
    // **The heading is drawn before the early return, not after it.** An entire panel
    // with nothing in it — no title, no text — is indistinguishable from one that failed
    // to draw, which is exactly how it read in the batch screenshot.
    ui.heading("Needs your attention");
    let Some(list) = &app.attention else {
        ui.label("Open a document and anything md2pdf had to decide will appear here.");
        return None;
    };
    if list.is_empty() {
        ui.label("Nothing — this document converted cleanly.");
        return None;
    }
    egui::ScrollArea::vertical()
        .id_salt("attention")
        .show(ui, |ui| {
            // **Grouped and ranked by `md2pdf-app`'s domain, drawn in the order given.**
            // Twenty rows saying "wrapped its cells" used to bury the one saying content
            // was clipped; now the worst kind is first and each kind is one row.
            for group in list.grouped() {
                ui.group(|ui| {
                    let count = group.count();
                    ui.label(describe(&group.what));
                    if count > 1 {
                        ui.small(format!("{count} elements"));
                    }
                    if let Some(reference) = pages_line(&group.pages) {
                        ui.small(reference);
                    }
                    if let Some(request) = adjustment_panel(ui, app, &group) {
                        intent = Some(request);
                    }
                });
            }
        });
    intent
}

/// "page 4" / "pages 2, 5 and 9" — or nothing at all.
///
/// **Nothing rather than "page unknown"**: the page is absent only when the layout could
/// not place the Element, and a row that explains its own missing metadata is worse than
/// a row that simply does not offer one.
fn pages_line(pages: &[u32]) -> Option<String> {
    match pages {
        [] => None,
        [one] => Some(format!("page {one}")),
        [rest @ .., last] => Some(format!(
            "pages {} and {last}",
            rest.iter()
                .map(u32::to_string)
                .collect::<Vec<_>>()
                .join(", ")
        )),
    }
}

/// The offers for one Element.
///
/// **Only where the Diagnostic named an Element** — md2pdf is not a layout editor, and it
/// offers a fix exactly where the engine already admitted it compromised. An entry with
/// no offers (a missing image file) draws nothing here rather than an inert button.
pub fn adjustment_panel(
    ui: &mut egui::Ui,
    app: &App,
    group: &md2pdf_domain::AttentionGroup,
) -> Option<Request> {
    let mut intent = None;
    for offer in &group.offers {
        // One button for the whole row, because a group is one `CompromiseKind` and
        // offers are made per kind — the same permission is the right answer for every
        // member. It is also one re-decide instead of five.
        if ui.button(offer.label).clicked() {
            intent = Some(app.allow_group_request(group, offer.permit));
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
        // `opening` as well as `open`: the gap between asking for a document and its
        // first page is ~900ms, and the panel must not spend it claiming nothing was
        // chosen.
        None if app.open.is_some() || app.opening => {
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
