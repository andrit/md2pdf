//! The compiling thread, and the only thread that touches a `Typesetter`.
//!
//! ## Why a thread at all
//!
//! **[measured]** a batch of 146 documents takes ~35s and opening one for review takes
//! ~900ms. Both are far past the point where a UI that did them inline would stop drawing.
//! So the app sends [`Request`]s and draws whatever [`Update`]s have arrived, and the
//! window keeps painting whatever the engine is doing.
//!
//! ## What it is *not* protecting against
//!
//! Until phase 4 began, `TypstWorld` carried `unsafe impl Send + Sync` justified by a
//! comment — *"compilation is driven from one thread at a time"* — which was true only
//! because the CLI is single-threaded. That has been fixed at the source: the `RefCell`s
//! became `Mutex`es and the `unsafe impl` pair is gone, so the compiler now enforces it
//! (`plan-app.md` D3). This channel exists to keep the UI responsive, **not** to hold a
//! safety invariant together.
//!
//! ## One worker, deliberately
//!
//! **[assumed]** a second compiling thread would contend on `comemo`'s process-global
//! cache — the one T31 measured — rather than halve the time. A batch is sequential
//! anyway. If that assumption is ever tested it should be measured, not argued.

use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, Sender};

use md2pdf_domain::{AttentionList, Override, Template};
use md2pdf_engine::{BrokerImages, Command, Deps, Event, Review};
use md2pdf_paths::PathBroker;
use md2pdf_typeset::Typesetter;

use crate::preview::Page;

/// What the UI asks the engine to do.
pub enum Request {
    /// Run a Job — one Source or a whole tree. Events stream back as it goes.
    ///
    /// **The Template travels with the Command**, exactly as it does for [`Self::Open`].
    /// It used to be `Run(Command)` and the worker supplied `Template::default()`, so
    /// picking a template changed the preview and not the PDF — a preview that can
    /// disagree with the output is not one.
    Run {
        command: Command,
        template: Box<Template>,
    },
    /// Open one document for review, without writing anything.
    Open {
        source: PathBuf,
        template: Box<Template>,
    },
    /// Allow something the ladder refused, and re-decide under it.
    ///
    /// **A list, because the attention list groups.** One row of it is one intention —
    /// "give all five of these a landscape page" — and sending five requests would
    /// re-probe five times and draw four decisions nobody asked to see.
    Allow(Vec<Override>),
    /// Take back a permission.
    Withdraw(Override),
    /// Raster one page of whatever is currently open.
    Page { index: usize, scale: f32 },
    /// Stop the thread. Sent on window close; the channel dropping also ends it.
    Stop,
}

/// What the engine tells the UI. Every variant is something a read model draws.
pub enum Update {
    /// Straight from the engine's contract — the CLI reads exactly the same stream.
    Engine(Box<Event>),
    /// A document was opened: what it conceded, and how many pages it has.
    Opened {
        source: PathBuf,
        attention: Box<AttentionList>,
        pages: usize,
    },
    /// The document was re-decided under an Override.
    Redecided {
        attention: Box<AttentionList>,
        pages: usize,
    },
    /// A rastered page, ready to become a texture.
    Page(Box<Page>),
    /// Something went wrong for one request. Not fatal to the session.
    Failed(String),
    /// A Job is over — **however it ended, and whatever it was**.
    ///
    /// Sent when `md2pdf_engine::handle` returns, which is the only signal that is true
    /// for both Commands. `BatchCompleted` is emitted by `convert_batch` alone, so a
    /// single-file conversion produced no completion event at all and the window span
    /// "Converting…" forever over a PDF that was already on disk. A synchronous caller
    /// like the CLI never noticed: for it, the Job ending *is* the call returning.
    Finished,
}

/// A handle to the compiling thread.
pub struct Worker {
    to_worker: Sender<Request>,
    from_worker: Receiver<Update>,
    handle: Option<std::thread::JoinHandle<()>>,
}

impl Worker {
    /// Spawn the thread. The `Typesetter` is **created inside it** and never crosses.
    pub fn spawn() -> Self {
        let (to_worker, requests) = channel::<Request>();
        let (updates, from_worker) = channel::<Update>();
        let handle = std::thread::Builder::new()
            .name("md2pdf-compile".into())
            .spawn(move || run(requests, updates))
            .expect("could not spawn the compile thread");
        Self {
            to_worker,
            from_worker,
            handle: Some(handle),
        }
    }

    /// Ask for something. False if the worker is gone.
    pub fn send(&self, request: Request) -> bool {
        self.to_worker.send(request).is_ok()
    }

    /// Everything that has arrived since last time. **Never blocks** — a UI that waited
    /// on the engine would be a UI that stops drawing, which is the whole reason the
    /// thread exists.
    pub fn drain(&self) -> Vec<Update> {
        self.from_worker.try_iter().collect()
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        let _ = self.to_worker.send(Request::Stop);
        if let Some(handle) = self.handle.take() {
            // Joined rather than detached: the thread owns a Typesetter and a partially
            // written output, and a process exiting mid-write is how a half-PDF gets left
            // on disk looking like a whole one.
            let _ = handle.join();
        }
    }
}

/// The thread body. Owns the `Typesetter` and whatever document is open.
fn run(requests: Receiver<Request>, updates: Sender<Update>) {
    let broker = PathBroker::new();
    let typesetter = Typesetter::new();
    let mut open: Option<(PathBuf, Review)> = None;

    while let Ok(request) = requests.recv() {
        match request {
            Request::Stop => break,

            Request::Run { command, template } => {
                let deps = Deps {
                    broker: &broker,
                    typesetter: &typesetter,
                    template: &template,
                };
                // The engine emits as it goes, so the UI fills in during a long batch
                // rather than after it.
                md2pdf_engine::handle(command, &deps, &mut |event| {
                    let _ = updates.send(Update::Engine(Box::new(event)));
                });
                // **After the stream, always.** `handle` returning is the one fact that
                // means "the Job is over" for every Command and every outcome.
                let _ = updates.send(Update::Finished);
            }

            Request::Open { source, template } => {
                match open_document(&source, *template, &broker, &typesetter) {
                    Ok((review, attention, pages)) => {
                        let _ = updates.send(Update::Opened {
                            source: source.clone(),
                            attention: Box::new(attention),
                            pages,
                        });
                        open = Some((source, review));
                    }
                    Err(message) => {
                        let _ = updates.send(Update::Failed(message));
                    }
                }
            }

            Request::Allow(overs) => {
                if let Some((_, review)) = open.as_mut() {
                    match review.apply_all(&overs, &typesetter) {
                        // None of them named an Element this document still has — a
                        // stale click after the file changed underneath.
                        Ok(0) => {
                            let _ = updates.send(Update::Failed(
                                "that element is no longer in this document".into(),
                            ));
                        }
                        Ok(_) => send_redecided(review, &typesetter, &updates),
                        Err(e) => {
                            let _ = updates.send(Update::Failed(e.to_string()));
                        }
                    }
                }
            }

            Request::Withdraw(over) => {
                if let Some((_, review)) = open.as_mut() {
                    match review.withdraw(&over, &typesetter) {
                        Ok(()) => send_redecided(review, &typesetter, &updates),
                        Err(e) => {
                            let _ = updates.send(Update::Failed(e.to_string()));
                        }
                    }
                }
            }

            Request::Page { index, scale } => {
                if let Some((_, review)) = open.as_ref() {
                    match review.render(&typesetter) {
                        Ok(compilation) => {
                            if let Some(page) = Page::raster(&compilation, index, scale) {
                                let _ = updates.send(Update::Page(Box::new(page)));
                            }
                        }
                        Err(e) => {
                            let _ = updates.send(Update::Failed(e.to_string()));
                        }
                    }
                }
            }
        }
    }
}

fn open_document(
    source: &std::path::Path,
    template: Template,
    broker: &PathBroker,
    typesetter: &Typesetter,
) -> Result<(Review, AttentionList, usize), String> {
    let markdown = broker.read_to_string(source).map_err(|e| e.to_string())?;
    let parent = source
        .parent()
        .unwrap_or(std::path::Path::new("."))
        .to_path_buf();
    let images = BrokerImages(broker);
    let context = md2pdf_convert::SourceContext::with_template(&parent, &images, template.clone());
    let review =
        Review::open(&markdown, &context, template, typesetter).map_err(|e| e.to_string())?;
    let (attention, pages) = looked_at(&review, typesetter);
    Ok((review, attention, pages))
}

/// The attention list and the page count, from **one** render.
///
/// Both answers come from the laid-out document, and it is the same `Compilation` the PDF
/// and the preview are made from — so a page reference in the list is the page the reader
/// will actually turn to. Rendering twice to get them separately would also be rendering
/// twice to get two answers that must agree.
fn looked_at(review: &Review, typesetter: &Typesetter) -> (AttentionList, usize) {
    match review.render(typesetter) {
        Ok(compilation) => (
            review.attention().with_pages(&compilation.element_pages()),
            compilation.page_count(),
        ),
        // A document that will not render still has concessions worth showing; they just
        // cannot say where. Reporting none would hide the reason it failed.
        Err(_) => (review.attention(), 0),
    }
}

fn send_redecided(review: &Review, typesetter: &Typesetter, updates: &Sender<Update>) {
    // Re-derived, not carried over: an Override changes the layout, so the page an
    // Element sits on can move. A stale page reference would send the reader to the page
    // it *used* to be on, which is the failure mode a reference exists to prevent.
    let (attention, pages) = looked_at(review, typesetter);
    let _ = updates.send(Update::Redecided {
        attention: Box::new(attention),
        pages,
    });
}
