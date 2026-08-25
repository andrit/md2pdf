//! The compile thread, exercised for real.
//!
//! The state machine is pure and tested inline. **This is the part that could be wrong in
//! a way unit tests cannot see**: a thread that never starts, a channel that deadlocks, a
//! `Typesetter` that will not cross the boundary. It is checkable in the container even
//! though the GUI is not, which is the whole argument for splitting the crate.

use std::path::PathBuf;

use md2pdf_app::state::App;
use md2pdf_app::worker::{Request, Update};
use md2pdf_app::Worker;
use md2pdf_domain::Permit;
use md2pdf_engine::Command;
use md2pdf_paths::testing::TempDir;

/// Collect updates until `done` says we have what we came for, or time runs out.
///
/// Polling rather than blocking: it is what the UI does every frame, so a deadlock here
/// is a deadlock there. The timeout means a broken worker fails the test instead of
/// hanging the suite.
fn collect(worker: &Worker, done: impl Fn(&[Update]) -> bool) -> Vec<Update> {
    let mut all = Vec::new();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
    while std::time::Instant::now() < deadline {
        all.extend(worker.drain());
        if done(&all) {
            return all;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    panic!(
        "the worker never sent what was expected; got {} updates",
        all.len()
    );
}

#[test]
fn a_job_runs_on_the_worker_and_streams_events_back() {
    let tmp = TempDir::new("app-worker");
    let source = tmp.write("notes.md", b"# Title\n\nSome prose.\n");
    let worker = Worker::spawn();

    assert!(worker.send(Request::Run(Command::ConvertSource {
        source: source.clone(),
        destination: tmp.join("out"),
    })));

    let updates = collect(&worker, |all| {
        all.iter()
            .any(|u| matches!(u, Update::Engine(e) if matches!(**e, md2pdf_engine::Event::OutputWritten { .. })))
    });

    // The events fold into the state exactly as they would in the app.
    let mut app = App::default();
    for update in updates {
        app.absorb(update);
    }
    assert_eq!(app.summary(), "1 converted cleanly.");
    assert!(tmp.join("out/notes.pdf").is_file(), "no PDF was written");
}

#[test]
fn a_document_opens_for_review_and_an_override_re_decides_it() {
    // The 3f loop, driven the way the app will drive it: open, look at what was
    // conceded, allow something, and get a fresh decision back.
    let tmp = TempDir::new("app-review");
    let source = tmp.write(
        "wide.md",
        b"# Report\n\n| a | b | c | d | e |\n|---|---|---|---|---|\n\
          | xxxxxxxxxxxxxxxxxxxxxxxx | xxxxxxxxxxxxxxxxxxxxxxxx | xxxxxxxxxxxxxxxxxxxxxxxx \
          | xxxxxxxxxxxxxxxxxxxxxxxx | xxxxxxxxxxxxxxxxxxxxxxxx |\n",
    );
    let worker = Worker::spawn();
    let mut app = App::default();

    worker.send(app.open_request(&source));
    for update in collect(&worker, |all| {
        all.iter().any(|u| matches!(u, Update::Opened { .. }))
    }) {
        app.absorb(update);
    }

    let attention = app
        .attention
        .as_ref()
        .expect("opened without an attention list");
    assert!(
        !attention.is_empty(),
        "a table that cannot fit conceded nothing"
    );
    let offer = attention
        .actionable()
        .next()
        .expect("nothing actionable to click");
    let over = md2pdf_domain::Override {
        id: offer.id,
        permit: offer.offers[0].permit,
    };

    worker.send(app.allow_request(over));
    for update in collect(&worker, |all| {
        all.iter().any(|u| matches!(u, Update::Redecided { .. }))
    }) {
        app.absorb(update);
    }
    assert!(app.pages > 0, "re-decided into a document with no pages");
}

#[test]
fn a_page_comes_back_as_pixels() {
    // The Preview read model *is* the output: the same Compilation that writes the PDF
    // is the one rastered. A preview that could disagree with the PDF would not be one.
    let tmp = TempDir::new("app-preview");
    let source = tmp.write("notes.md", b"# Title\n\nSome prose.\n");
    let worker = Worker::spawn();
    let mut app = App::default();

    worker.send(app.open_request(&source));
    for update in collect(&worker, |all| {
        all.iter().any(|u| matches!(u, Update::Opened { .. }))
    }) {
        app.absorb(update);
    }

    let request = app.page_request(2.0).expect("a page should be wanted");
    worker.send(request);
    for update in collect(&worker, |all| {
        all.iter().any(|u| matches!(u, Update::Page(_)))
    }) {
        app.absorb(update);
    }

    let page = app.page.as_ref().expect("no page arrived");
    assert!(page.width > 0 && page.height > 0);
    assert_eq!(
        page.rgba.len(),
        page.width as usize * page.height as usize * 4
    );
    // And nothing more is asked for once it has arrived.
    assert!(app.page_request(2.0).is_none());
}

#[test]
fn a_stale_override_is_reported_rather_than_silently_ignored() {
    // The user clicked an offer for an element that is no longer there. Saying nothing
    // would look like the click did something.
    let tmp = TempDir::new("app-stale");
    let source = tmp.write("notes.md", b"# Title\n\nSome prose.\n");
    let worker = Worker::spawn();
    let mut app = App::default();

    worker.send(app.open_request(&source));
    for update in collect(&worker, |all| {
        all.iter().any(|u| matches!(u, Update::Opened { .. }))
    }) {
        app.absorb(update);
    }

    worker.send(Request::Allow(md2pdf_domain::Override {
        id: md2pdf_domain::ElementId::new(99, "not in this document"),
        permit: Permit::Landscape,
    }));
    for update in collect(&worker, |all| {
        all.iter().any(|u| matches!(u, Update::Failed(_)))
    }) {
        app.absorb(update);
    }
    assert!(app.problem.is_some(), "the stale click said nothing");
}

#[test]
fn dropping_the_worker_stops_the_thread() {
    // Otherwise closing the window leaves a thread compiling into a channel nobody reads.
    let worker = Worker::spawn();
    assert!(worker.send(Request::Page {
        index: 0,
        scale: 1.0
    }));
    drop(worker);
    // `Drop` joins; reaching here at all means the thread ended rather than hanging.
}

#[test]
fn nothing_here_needs_a_path_that_exists() {
    // A guard on the split: this crate must stay buildable and testable with no display
    // and no GUI toolkit. If that stops being true the whole option-C argument goes with
    // it — so it is asserted rather than assumed.
    let _ = PathBuf::from("/nonexistent");
    let worker = Worker::spawn();
    worker.send(Request::Open {
        source: PathBuf::from("/definitely/not/here.md"),
        template: Box::new(md2pdf_domain::Template::default()),
    });
    let updates = collect(&worker, |all| !all.is_empty());
    assert!(
        matches!(updates[0], Update::Failed(_)),
        "a missing file should fail cleanly"
    );
}
