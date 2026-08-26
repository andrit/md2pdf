//! md2pdf, as a window.
//!
//! The thinnest thing that could work: it owns an [`App`] (all the state and every
//! decision), a [`Worker`] (the compile thread), and one texture. Everything else is in
//! `md2pdf-app`, which is buildable and tested in the container — this file is the only
//! one that is not, so it holds as little as possible.
//!
//! See `design/plan-app.md`.

mod widgets;

use md2pdf_app::state::{Accepted, App};
use md2pdf_app::worker::{Request, Update};
use md2pdf_app::Worker;
use md2pdf_paths::PathBroker;
use md2pdf_template::{roots, TemplateCatalogue};

/// How much bigger than a point a preview pixel is.
///
/// 2.0 is a retina page. It matters more than it looks: a rastered A4 page at this scale
/// is ~8 MB, and `egui::Context::load_texture` `debug_assert`s that neither side exceeds
/// the GPU's maximum — so a much larger value would panic in a debug build rather than
/// degrade.
const PREVIEW_SCALE: f32 = 2.0;

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("md2pdf")
            .with_inner_size([1100.0, 760.0]),
        ..Default::default()
    };
    eframe::run_native(
        "md2pdf",
        options,
        Box::new(|_cc| Ok(Box::new(Gui::new()) as Box<dyn eframe::App>)),
    )
}

struct Gui {
    app: App,
    worker: Worker,
    /// The page on screen. Rebuilt whenever a new one arrives, dropped when the decision
    /// it was rendered under changes.
    texture: Option<egui::TextureHandle>,
    /// The destination field's buffer. Owned here because an egui text field edits the
    /// String it is handed, and a temporary rebuilt each frame loses the caret.
    destination: String,
    /// How many files are being dragged over the window right now, so the drop can be
    /// acknowledged before it lands.
    hovering: usize,
}

impl Gui {
    fn new() -> Self {
        let broker = PathBroker::new();
        // Discovered at startup, exactly as the CLI does it. The font check the CLI
        // performs is skipped here: it needs a Typesetter, which lives on the worker
        // thread by design, and a template naming an unavailable font will be caught when
        // it is used rather than when it is listed.
        let catalogue = TemplateCatalogue::discover(
            &roots::roots(None, roots::beside_binary(), &roots::Env::current()),
            &broker,
        );
        Self {
            app: App {
                catalogue,
                ..Default::default()
            },
            worker: Worker::spawn(),
            texture: None,
            destination: String::new(),
            hovering: 0,
        }
    }

    /// Files dropped on the window. The only way to choose a Source — there is no file
    /// dialog, because that needs a dependency and this is a window with two jobs.
    fn absorb_drops(&mut self, ctx: &egui::Context) {
        let (dropped, hovering) = ctx.input(|i| {
            (
                i.raw
                    .dropped_files
                    .iter()
                    .filter_map(|f| f.path.clone())
                    .collect::<Vec<_>>(),
                i.raw.hovered_files.len(),
            )
        });
        // Repaint while something is being dragged over the window. egui draws lazily,
        // and without this the acknowledgement could fail to appear for the same reason
        // the drop looked like it did nothing.
        if hovering > 0 {
            ctx.request_repaint();
        }
        self.hovering = hovering;
        for path in dropped {
            // Where the path goes is `Chosen::accept`'s decision, tested in `md2pdf-app`.
            // What is left here is the two things that need a window.
            let is_dir = path.is_dir();
            match self.app.chosen.accept(path, is_dir) {
                // Open it straight away. Without this the preview panel is unreachable
                // until a Job has run — which is what made a good drop look like nothing
                // happening at all.
                Accepted::Source { previewable: true } => {
                    if let Some(source) = self.app.chosen.path.clone() {
                        let request = self.app.open_request(&source);
                        self.app.sent(&request);
                        self.worker.send(request);
                    }
                }
                Accepted::Source { previewable: false } => {}
                // Re-sync the text field's buffer: it is the edited String, so setting
                // the PathBuf alone would leave the box showing the old text.
                Accepted::Destination => {
                    self.destination = self
                        .app
                        .chosen
                        .destination
                        .as_ref()
                        .map(|d| d.display().to_string())
                        .unwrap_or_default();
                }
            }
        }
    }

    /// Take everything the worker has said, and fold it in.
    ///
    /// **Returns whether anything arrived**, which is what decides a repaint: egui draws
    /// lazily, so a worker finishing while the window is idle would leave stale state on
    /// screen until the mouse moved.
    fn absorb_worker(&mut self) -> bool {
        let updates = self.worker.drain();
        if updates.is_empty() {
            return false;
        }
        for update in updates {
            // A new page needs a new texture; a re-decision invalidates the old one.
            match &update {
                Update::Page(_) | Update::Opened { .. } | Update::Redecided { .. } => {
                    self.texture = None;
                }
                _ => {}
            }
            self.app.absorb(update);
        }
        true
    }

    /// Upload the current page, if there is one and it has not been uploaded yet.
    ///
    /// **Must happen inside `update()`** — texture allocation goes through the egui
    /// context, which belongs to the UI thread. The worker sends plain RGBA precisely so
    /// that it never touches this.
    fn upload_page(&mut self, ctx: &egui::Context) {
        if self.texture.is_some() {
            return;
        }
        let Some(page) = &self.app.page else { return };
        let image = egui::ColorImage::from_rgba_unmultiplied(
            [page.width as usize, page.height as usize],
            &page.rgba,
        );
        self.texture = Some(ctx.load_texture("preview", image, egui::TextureOptions::LINEAR));
    }
}

impl eframe::App for Gui {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.absorb_drops(ctx);

        if self.absorb_worker() {
            // The classic egui-plus-worker bug, avoided: without this the window shows
            // stale state until something else provokes a frame.
            ctx.request_repaint();
        }
        self.upload_page(ctx);

        // Ask for the page being looked at, and only that one. `md2pdf-app` decides
        // whether one is wanted; this just carries the answer.
        if let Some(request) = self.app.page_request(PREVIEW_SCALE) {
            self.worker.send(request);
        }

        let mut intent: Option<Request> = None;

        egui::SidePanel::left("controls")
            .resizable(true)
            .default_width(340.0)
            .show(ctx, |ui| {
                if let Some(r) = widgets::source_list(ui, &self.app, self.hovering) {
                    intent = Some(r);
                }
                ui.separator();
                widgets::template_catalogue(ui, &mut self.app);
                ui.separator();
                if let Some(r) = widgets::job_settings(ui, &mut self.app, &mut self.destination) {
                    intent = Some(r);
                }
                ui.separator();
                widgets::batch_progress(ui, &self.app);
            });

        egui::SidePanel::right("attention")
            .resizable(true)
            .default_width(320.0)
            .show(ctx, |ui| {
                if let Some(r) = widgets::attention_list(ui, &self.app) {
                    intent = Some(r);
                }
            });

        egui::CentralPanel::default().show(ctx, |ui| {
            widgets::preview(ui, &self.app, self.texture.as_ref());
        });

        if let Some(request) = intent {
            self.app.sent(&request);
            self.worker.send(request);
        }
    }
}
