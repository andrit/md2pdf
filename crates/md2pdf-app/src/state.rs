//! The app's state, and every decision it makes.
//!
//! **This is where the GUI's thinking lives.** `md2pdf-gui` draws what is here and sends
//! back what was clicked; it decides nothing, which is what makes the part I cannot
//! compile the part that cannot be subtly wrong.
//!
//! Everything is a plain value derived from what the engine emitted — the event storm's
//! claim that *"there is no state a read model needs that the pipeline does not already
//! produce"* is enforced by there being nowhere else for it to come from.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use md2pdf_domain::{AttentionGroup, AttentionList, BlanketResolution, Override, Permit, Template};
use md2pdf_engine::{Command, Event};
use md2pdf_template::TemplateCatalogue;

use crate::preview::Page;
use crate::worker::{Request, Update};

/// What has happened to one Source, as far as the UI knows.
///
/// Drawn by `source_list()`. **Flagged is not failed** — a flagged Source converted
/// successfully and a judgment call was made inside it (`GLOSSARY`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceState {
    Pending,
    Converted,
    Flagged,
    Written,
    Skipped(String),
    Failed(String),
}

impl SourceState {
    /// Whether this Source is worth a person's attention. Used by the summary line.
    pub fn needs_attention(&self) -> bool {
        matches!(self, Self::Flagged | Self::Failed(_) | Self::Skipped(_))
    }
}

/// What a dropped path turned into.
///
/// Returned by [`Chosen::accept`] so the adapter can do the two things that need a window
/// — re-sync the destination field's buffer, and ask for a preview — **without deciding
/// which of them applies**. That decision is here, where it can be tested.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Accepted {
    /// It became the Source. `previewable` is true for a single file, the only thing
    /// `Request::Open` can render — a folder is a batch, and a batch has no one page.
    Source { previewable: bool },
    /// It became the destination.
    Destination,
}

/// What the user has chosen to do. `JobSettings` in the event storm.
#[derive(Debug, Clone, Default)]
pub struct Chosen {
    pub path: Option<PathBuf>,
    pub destination: Option<PathBuf>,
    pub template: Option<String>,
    pub on_collision: Option<BlanketResolution>,
}

impl Chosen {
    /// Route one dropped path, and say what it became.
    ///
    /// A file is **always** the Source — dropping a second one replaces the first rather
    /// than being silently ignored. A folder is the Source when nothing has been chosen
    /// and the destination otherwise, so file-then-folder does the common case in two
    /// gestures.
    ///
    /// `is_dir` is passed in rather than read off the path: this is the app's thinking,
    /// and thinking that touches the disk cannot be tested without one.
    pub fn accept(&mut self, path: PathBuf, is_dir: bool) -> Accepted {
        if !is_dir {
            self.path = Some(path);
            Accepted::Source { previewable: true }
        } else if self.path.is_none() {
            self.path = Some(path);
            Accepted::Source { previewable: false }
        } else {
            self.destination = Some(path);
            Accepted::Destination
        }
    }

    /// The Command this describes, or the reason it is not one yet.
    ///
    /// **The engine's `on_collision` has no default by design** — every possible one is
    /// wrong, so each adapter must state intent. Here that means the button is disabled
    /// until the user has answered, and this returns why rather than guessing for them.
    pub fn command(&self, is_directory: bool) -> Result<Command, &'static str> {
        let path = self.path.clone().ok_or("choose a file or folder")?;
        let destination = self.destination.clone().ok_or("choose where to save")?;
        if is_directory {
            Ok(Command::ConvertBatch {
                source_root: path,
                destination,
                on_collision: self
                    .on_collision
                    .ok_or("say what to do about files that already exist")?,
            })
        } else {
            Ok(Command::ConvertSource {
                source: path,
                destination,
            })
        }
    }
}

// **There is no `CollisionPrompt` here, and that is a finding rather than an omission.**
//
// The event storm lists one — "the conflicting path, existing file, and the four choices
// incl. apply-to-all" — but **[measured]** the engine's contract has no `CollisionDetected`
// event to hang it on. `output::plan` resolves every collision *before any conversion
// begins*, from the `BlanketResolution` the caller supplied, and what comes back is a
// `SourceSkipped` after the fact.
//
// So the app asks the question **before** the run, exactly as `--on-collision` does, and
// reports what was skipped afterwards. An interactive prompt-per-collision would need a
// new event and a way for an adapter to answer mid-Job — a contract change, and a real
// one, since the engine currently emits and never asks. Recorded in `plan-app.md` D6.
/// The whole app.
#[derive(Default)]
pub struct App {
    pub chosen: Chosen,
    pub catalogue: TemplateCatalogue,
    pub sources: BTreeMap<PathBuf, SourceState>,
    /// The document open for review, if any, and what it conceded.
    pub open: Option<PathBuf>,
    pub attention: Option<AttentionList>,
    pub pages: usize,
    pub page: Option<Page>,
    pub showing: usize,
    /// Most recent failure, for a status line. Not a log: one message, replaced.
    pub problem: Option<String>,
    pub running: bool,
    /// A document has been asked for and has not arrived. **Purely so the wait is
    /// visible** — opening one takes ~900ms, and a window that says "open a document" for
    /// a second after you dropped one is telling you it ignored you.
    pub opening: bool,
}

impl App {
    /// The Template the user picked, or the built-in default.
    ///
    /// Falling back rather than refusing keeps the app usable with no templates on disk,
    /// which is what a fresh install looks like before 3e's catalogue finds anything.
    pub fn template(&self) -> Template {
        self.chosen
            .template
            .as_deref()
            .and_then(|name| self.catalogue.get(name))
            .map(|found| found.template.clone())
            .unwrap_or_default()
    }

    /// Record that a Request has gone to the worker.
    ///
    /// The mirror of [`absorb`](Self::absorb): everything the window must show *while*
    /// the engine is busy begins here, so the adapter carries the request rather than
    /// interpreting it.
    pub fn sent(&mut self, request: &Request) {
        match request {
            Request::Run { .. } => self.running = true,
            Request::Open { .. } => self.opening = true,
            _ => {}
        }
    }

    /// Fold one Update into the state. **The only way state changes.**
    pub fn absorb(&mut self, update: Update) {
        match update {
            Update::Engine(event) => self.absorb_event(*event),
            Update::Opened {
                source,
                attention,
                pages,
            } => {
                self.open = Some(source);
                self.attention = Some(*attention);
                self.pages = pages;
                self.showing = 0;
                self.page = None;
                self.problem = None;
                self.opening = false;
            }
            Update::Redecided { attention, pages } => {
                self.attention = Some(*attention);
                self.pages = pages;
                // The page on screen is now stale — it was rendered under the old
                // decision. Dropping it is what makes the preview honest while the new
                // one is on its way.
                self.page = None;
            }
            Update::Page(page) => {
                self.showing = page.index;
                self.page = Some(*page);
            }
            // Whatever was being waited for is not coming. Leaving the spinner up would
            // be the same lie in the other direction.
            Update::Failed(message) => {
                self.problem = Some(message);
                self.opening = false;
            }
        }
    }

    fn absorb_event(&mut self, event: Event) {
        match event {
            // **Converted, never Flagged.** `SourceConverted.compromises` counts the
            // *conversion* half only — `contract.rs` says so, and says the complete
            // answer arrives in `DiagnosticSealed`. Flagging from it made every
            // escalation-ladder concession (shrunk, rotated, reflowed, clipped) invisible
            // to this crate: a batch of reflowed tables reported "converted cleanly",
            // which is the exact failure the seal was added to prevent.
            Event::SourceConverted { source, .. } => {
                self.sources.insert(source, SourceState::Converted);
            }
            // The one place a Source becomes Flagged. The engine seals both halves of the
            // pipeline into this (`INV-4`) and emits it only when there is something to
            // say, so the payload is the whole truth about what needed a judgment call.
            Event::DiagnosticSealed {
                source,
                compromises,
            } => {
                if !compromises.is_empty() {
                    self.sources.insert(source, SourceState::Flagged);
                }
            }
            Event::OutputWritten { source, .. } => {
                // Written beats Converted, but never overwrites Flagged: a flagged
                // document is still written, and losing that is losing the whole point
                // of the attention gate.
                if self.sources.get(&source) != Some(&SourceState::Flagged) {
                    self.sources.insert(source, SourceState::Written);
                }
            }
            Event::SourceFailed { source, message } => {
                self.sources.insert(source, SourceState::Failed(message));
            }
            Event::SourceSkipped { source, reason, .. } => {
                self.sources
                    .insert(source, SourceState::Skipped(format!("{reason:?}")));
            }
            Event::BatchCompleted { .. } => self.running = false,
            Event::Failed { message } => {
                self.problem = Some(message);
                self.running = false;
            }
            _ => {}
        }
    }

    /// "47 converted cleanly, 3 need your attention."
    ///
    /// The sentence the whole design aims at, computed in one place so the GUI cannot
    /// word it differently from the CLI.
    pub fn summary(&self) -> String {
        let (clean, attention) = self
            .sources
            .values()
            .fold((0, 0), |(clean, attention), state| {
                if state.needs_attention() {
                    (clean, attention + 1)
                } else {
                    (clean + 1, attention)
                }
            });
        if attention == 0 {
            format!("{clean} converted cleanly.")
        } else {
            format!("{clean} converted cleanly, {attention} need your attention.")
        }
    }

    /// What to ask for when the user opens a Source for review.
    pub fn open_request(&self, source: &Path) -> Request {
        Request::Open {
            source: source.to_path_buf(),
            template: Box::new(self.template()),
        }
    }

    /// What to ask for when the user starts a Job.
    ///
    /// **Built here rather than in the widget, and from the same `template()` the preview
    /// uses**, which is the only thing that makes "the preview *is* the output" true.
    pub fn run_request(&self, command: Command) -> Request {
        Request::Run {
            command,
            template: Box::new(self.template()),
        }
    }

    /// What to ask for when the user accepts an offer for one Element.
    pub fn allow_request(&self, over: Override) -> Request {
        Request::Allow(vec![over])
    }

    /// What to ask for when the user accepts an offer for a whole grouped row.
    ///
    /// The same permission for every Element in the group — which is coherent precisely
    /// because a group is one `CompromiseKind`, and offers are made per kind.
    pub fn allow_group_request(&self, group: &AttentionGroup, permit: Permit) -> Request {
        Request::Allow(
            group
                .ids
                .iter()
                .map(|&id| Override { id, permit })
                .collect(),
        )
    }

    /// Which page to raster next, if any is missing.
    ///
    /// **[assumed]** rastering only what is on screen is necessary and sufficient: an A4
    /// page at 2x is ~8 MB, so keeping a 24-page document resident is 200 MB of textures
    /// nobody is looking at. `plan-app.md` D2.
    pub fn page_request(&self, scale: f32) -> Option<Request> {
        if self.open.is_none() || self.pages == 0 {
            return None;
        }
        match &self.page {
            Some(page) if page.index == self.showing => None,
            _ => Some(Request::Page {
                index: self.showing.min(self.pages.saturating_sub(1)),
                scale,
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use md2pdf_domain::Diagnostic;
    use md2pdf_engine::contract::SkipReason;

    fn app() -> App {
        App::default()
    }

    /// A sealed Diagnostic for `source`, the way the engine emits one: only when the
    /// document conceded something, carrying what it conceded.
    fn sealed(source: &str) -> Event {
        Event::DiagnosticSealed {
            source: PathBuf::from(source),
            compromises: vec![md2pdf_domain::Compromise {
                id: md2pdf_domain::ElementId::new(1, "#table(columns: 5)"),
                kind: md2pdf_domain::CompromiseKind::Reflowed,
                page: None,
            }],
        }
    }

    fn converted(source: &str) -> Event {
        Event::SourceConverted {
            source: PathBuf::from(source),
            elements: 1,
            images: 0,
            compromises: 0,
        }
    }

    #[test]
    fn a_flagged_source_stays_flagged_when_its_output_is_written() {
        // Flagged is not failed: the document converted *and* was written, and something
        // in it needed a judgment call. Letting Written overwrite Flagged would empty the
        // attention gate exactly when it matters.
        let mut a = app();
        let source = PathBuf::from("notes.md");
        a.absorb_event(converted("notes.md"));
        a.absorb_event(sealed("notes.md"));
        a.absorb_event(Event::OutputWritten {
            source: source.clone(),
            path: PathBuf::from("out/notes.pdf"),
        });
        assert_eq!(a.sources.get(&source), Some(&SourceState::Flagged));
    }

    #[test]
    fn a_ladder_concession_flags_the_source_even_though_conversion_was_clean() {
        // The defect this closes. A reflowed table concedes nothing at *conversion* time
        // — `SourceConverted.compromises` is 0 — and everything at typeset time. Flagging
        // from the first number meant the document that most needed attention got none.
        let mut a = app();
        a.absorb_event(converted("wide.md"));
        assert_eq!(
            a.sources.get(Path::new("wide.md")),
            Some(&SourceState::Converted),
            "flagged before anything had been sealed"
        );
        a.absorb_event(sealed("wide.md"));
        assert_eq!(
            a.sources.get(Path::new("wide.md")),
            Some(&SourceState::Flagged)
        );
        assert_eq!(a.summary(), "0 converted cleanly, 1 need your attention.");
    }

    #[test]
    fn the_summary_is_the_sentence_the_design_aims_at() {
        let mut a = app();
        for name in ["a.md", "b.md", "c.md"] {
            a.absorb_event(converted(name));
        }
        a.absorb_event(sealed("c.md"));
        assert_eq!(a.summary(), "2 converted cleanly, 1 need your attention.");
    }

    #[test]
    fn a_clean_run_does_not_mention_attention_at_all() {
        // No seal arrives for a clean document — the engine only emits one when there is
        // something to say.
        let mut a = app();
        a.absorb_event(converted("a.md"));
        assert_eq!(a.summary(), "1 converted cleanly.");
    }

    #[test]
    fn a_batch_needs_an_answer_about_collisions_before_it_can_start() {
        // The engine refuses a default because every one is wrong. The adapter's job is
        // to make the user state intent — here, by saying what is missing rather than
        // choosing for them.
        let mut chosen = Chosen {
            path: Some(PathBuf::from("docs")),
            destination: Some(PathBuf::from("out")),
            ..Default::default()
        };
        assert!(chosen.command(true).is_err(), "started without an answer");
        chosen.on_collision = Some(BlanketResolution::SkipAll);
        assert!(chosen.command(true).is_ok());
    }

    #[test]
    fn one_file_needs_no_collision_answer() {
        // ConvertSource is not a special case of ConvertBatch, and asking about blanket
        // collision handling for a single file would be asking a question with no
        // meaning.
        let chosen = Chosen {
            path: Some(PathBuf::from("notes.md")),
            destination: Some(PathBuf::from("out")),
            ..Default::default()
        };
        assert!(chosen.command(false).is_ok());
    }

    #[test]
    fn a_redecided_document_drops_the_page_on_screen() {
        // It was rendered under the decision that just changed. Keeping it would show the
        // user a page that no longer corresponds to anything.
        let mut a = app();
        a.page = Some(Page {
            index: 0,
            width: 1,
            height: 1,
            rgba: vec![0, 0, 0, 0],
        });
        a.absorb(Update::Redecided {
            attention: Box::new(AttentionList::from_diagnostic(&Diagnostic::default())),
            pages: 2,
        });
        assert!(a.page.is_none(), "a stale page survived a re-decision");
    }

    #[test]
    fn only_the_visible_page_is_asked_for() {
        let mut a = app();
        a.open = Some(PathBuf::from("notes.md"));
        a.pages = 24;
        a.showing = 3;
        assert!(matches!(
            a.page_request(2.0),
            Some(Request::Page { index: 3, .. })
        ));
        // Once it has arrived, nothing more is asked for — the obvious way to make this
        // feel slow is to re-raster every frame.
        a.page = Some(Page {
            index: 3,
            width: 1,
            height: 1,
            rgba: vec![0; 4],
        });
        assert!(a.page_request(2.0).is_none());
    }

    #[test]
    fn the_job_is_run_under_the_template_the_preview_was_drawn_under() {
        // The defect this closes: `Request::Run` carried no Template and the worker
        // supplied `Template::default()`, so choosing a template changed the page on
        // screen and not the PDF on disk. Both requests now come from `template()`.
        let mut a = app();
        a.catalogue.found.push(md2pdf_template::Found {
            template: Template {
                name: "wide".into(),
                page_width_pt: 1200.0,
                ..Template::default()
            },
            description: "a wide one".into(),
            folder: PathBuf::from("/templates/wide"),
        });
        a.chosen.template = Some("wide".into());

        let command = Command::ConvertSource {
            source: PathBuf::from("notes.md"),
            destination: PathBuf::from("out"),
        };
        let Request::Run { template, .. } = a.run_request(command) else {
            panic!("run_request built something other than a Run");
        };
        assert_eq!(template.page_width_pt, 1200.0);

        let Request::Open { template, .. } = a.open_request(Path::new("notes.md")) else {
            panic!("open_request built something other than an Open");
        };
        assert_eq!(
            template.page_width_pt, 1200.0,
            "the preview and the Job disagree about the template"
        );
    }

    #[test]
    fn asking_for_a_document_is_visible_until_it_arrives() {
        // The whole point of the flag: the wait has to look like a wait.
        let mut a = app();
        let request = a.open_request(Path::new("notes.md"));
        a.sent(&request);
        assert!(a.opening);
        a.absorb(Update::Opened {
            source: PathBuf::from("notes.md"),
            attention: Box::new(AttentionList::from_diagnostic(&Diagnostic::default())),
            pages: 3,
        });
        assert!(!a.opening, "the spinner outlived the document");
    }

    #[test]
    fn a_document_that_fails_to_open_stops_the_wait_too() {
        // Otherwise the failure is reported *and* the spinner keeps turning, which reads
        // as still working.
        let mut a = app();
        let request = a.open_request(Path::new("broken.md"));
        a.sent(&request);
        a.absorb(Update::Failed("no such file".into()));
        assert!(!a.opening);
        assert_eq!(a.problem.as_deref(), Some("no such file"));
    }

    #[test]
    fn nothing_is_rastered_when_nothing_is_open() {
        assert!(app().page_request(2.0).is_none());
    }

    #[test]
    fn a_dropped_file_is_the_source_and_is_worth_previewing() {
        // The bug this exists for: the drop was absorbed and *nothing observable
        // happened*, because the preview was only reachable from a list that stays empty
        // until a Job has run. A file lands, and there is immediately a page to draw.
        let mut chosen = Chosen::default();
        assert_eq!(
            chosen.accept(PathBuf::from("notes.md"), false),
            Accepted::Source { previewable: true }
        );
        assert_eq!(chosen.path, Some(PathBuf::from("notes.md")));
    }

    #[test]
    fn a_second_file_replaces_the_first_rather_than_being_ignored() {
        let mut chosen = Chosen::default();
        chosen.accept(PathBuf::from("first.md"), false);
        chosen.accept(PathBuf::from("second.md"), false);
        assert_eq!(chosen.path, Some(PathBuf::from("second.md")));
        assert!(chosen.destination.is_none(), "a file became a destination");
    }

    #[test]
    fn a_folder_is_the_source_first_and_the_destination_after() {
        // One gesture each, in the order a person does them: what to convert, then where
        // to put it.
        let mut chosen = Chosen::default();
        assert_eq!(
            chosen.accept(PathBuf::from("docs"), true),
            // A batch has no single page, so nothing is opened for review.
            Accepted::Source { previewable: false }
        );
        assert_eq!(
            chosen.accept(PathBuf::from("out"), true),
            Accepted::Destination
        );
        assert_eq!(chosen.path, Some(PathBuf::from("docs")));
        assert_eq!(chosen.destination, Some(PathBuf::from("out")));
    }

    #[test]
    fn a_folder_dropped_after_a_file_is_where_the_pdf_goes() {
        let mut chosen = Chosen::default();
        chosen.accept(PathBuf::from("notes.md"), false);
        assert_eq!(
            chosen.accept(PathBuf::from("out"), true),
            Accepted::Destination
        );
        // And that is the whole gesture: the pair is now a runnable Command.
        assert!(chosen.command(false).is_ok());
    }

    #[test]
    fn a_skip_is_reported_rather_than_looking_like_success() {
        let mut a = app();
        let source = PathBuf::from("notes.md");
        a.absorb_event(Event::SourceSkipped {
            source: source.clone(),
            output: PathBuf::from("out/notes.pdf"),
            reason: SkipReason::Collision,
        });
        assert!(a.sources[&source].needs_attention());
    }
}
