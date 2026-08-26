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

    assert!(
        worker.send(App::default().run_request(Command::ConvertSource {
            source: source.clone(),
            destination: tmp.join("out"),
        }))
    );

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
fn converting_one_file_stops_saying_converting() {
    // The defect: `BatchCompleted` is emitted by `convert_batch` alone, so a single
    // Source finished the Job with no completion event and the window span "Converting…"
    // forever over a PDF that was already on disk. Asserted for **both** Commands,
    // because the batch case worked and its passing is what hid the other one.
    for (name, batch) in [("one", false), ("many", true)] {
        let tmp = TempDir::new(&format!("app-finish-{name}"));
        let source = tmp.write("notes.md", b"# Title\n\nSome prose.\n");
        let worker = Worker::spawn();
        let mut app = App::default();

        let command = if batch {
            Command::ConvertBatch {
                source_root: source.parent().expect("parent").to_path_buf(),
                destination: tmp.join("out"),
                on_collision: md2pdf_domain::BlanketResolution::OverwriteAll,
            }
        } else {
            Command::ConvertSource {
                source: source.clone(),
                destination: tmp.join("out"),
            }
        };

        let request = app.run_request(command);
        app.sent(&request);
        assert!(app.running, "the spinner never started");
        worker.send(request);

        for update in collect(&worker, |all| {
            all.iter().any(|u| matches!(u, Update::Finished))
        }) {
            app.absorb(update);
        }
        assert!(
            !app.running,
            "{name}: still 'Converting…' after the Job ended"
        );
    }
}

/// The wide table that will not fit on A4. Reused: it is the cheapest thing that makes
/// the engine visibly concede.
const WIDE_TABLE: &[u8] = b"# Report\n\n| a | b | c | d | e |\n|---|---|---|---|---|\n\
     | xxxxxxxxxxxxxxxxxxxxxxxx | xxxxxxxxxxxxxxxxxxxxxxxx | xxxxxxxxxxxxxxxxxxxxxxxx \
     | xxxxxxxxxxxxxxxxxxxxxxxx | xxxxxxxxxxxxxxxxxxxxxxxx |\n";

#[test]
fn a_job_is_typeset_under_the_chosen_template_not_the_default_one() {
    // The regression: `Request::Run` carried no Template, so the worker built a
    // `Template::default()` and the chosen one reached the preview only. Asserted through
    // the engine's own event rather than the request, because the request being right and
    // the worker ignoring it is exactly the bug that was here.
    //
    // **[measured]** a 1400pt page fits the table and seals an empty Diagnostic; A4
    // reflows it and seals a `Reflowed`. So what was conceded *is* the answer to "which
    // template did the Job actually use". Verified by putting the bug back: this test
    // fails with the Reflowed in its message.
    let tmp = TempDir::new("app-template");
    let source = tmp.write("wide.md", WIDE_TABLE);
    let worker = Worker::spawn();

    let mut app = App::default();
    app.catalogue.found.push(md2pdf_template::Found {
        template: md2pdf_domain::Template {
            name: "wide".into(),
            page_width_pt: 1400.0,
            ..Default::default()
        },
        description: "wide enough for the table".into(),
        folder: tmp.join("templates/wide"),
    });
    app.chosen.template = Some("wide".into());

    worker.send(app.run_request(Command::ConvertSource {
        source: source.clone(),
        destination: tmp.join("out"),
    }));
    let updates = collect(&worker, |all| {
        all.iter().any(
            |u| matches!(u, Update::Engine(e) if matches!(**e, md2pdf_engine::Event::OutputWritten { .. })),
        )
    });

    // Asserted against the sealed Diagnostic rather than `SourceConverted.compromises`:
    // that field is *conversion-time counts only* by contract, and the complete set
    // arrives in `DiagnosticSealed`. `App::absorb_event` ignores the seal and flags from
    // the partial count, which is its own defect — see the commit log.
    let conceded: Vec<_> = updates
        .iter()
        .filter_map(|u| match u {
            Update::Engine(e) => match &**e {
                md2pdf_engine::Event::DiagnosticSealed { compromises, .. } => Some(compromises),
                _ => None,
            },
            _ => None,
        })
        .flatten()
        .collect();
    assert!(
        conceded.is_empty(),
        "the table conceded {conceded:?}, so the Job ran under A4 rather than the 1400pt \
         template it was given"
    );
    assert!(tmp.join("out/wide.pdf").is_file(), "no PDF was written");
}

#[test]
fn a_table_the_ladder_reflowed_is_reported_as_needing_attention() {
    // Through the real engine, because the defect lived in the gap between what the
    // engine emits and what this crate listens to. Conversion concedes *nothing* for this
    // document — `SourceConverted.compromises` is 0 — and the ladder reflows the table at
    // typeset time. The app used to fold that into "1 converted cleanly", which is the
    // exact failure `DiagnosticSealed` was added to prevent.
    let tmp = TempDir::new("app-flagged");
    let source = tmp.write("wide.md", WIDE_TABLE);
    let worker = Worker::spawn();
    let mut app = App::default();

    worker.send(app.run_request(Command::ConvertSource {
        source: source.clone(),
        destination: tmp.join("out"),
    }));
    for update in collect(&worker, |all| {
        all.iter().any(
            |u| matches!(u, Update::Engine(e) if matches!(**e, md2pdf_engine::Event::OutputWritten { .. })),
        )
    }) {
        app.absorb(update);
    }

    assert_eq!(
        app.sources.get(&source),
        Some(&md2pdf_app::state::SourceState::Flagged),
        "the ladder reflowed the table and the app called it clean"
    );
    assert_eq!(app.summary(), "0 converted cleanly, 1 need your attention.");
    // And it was still written — flagged is not failed.
    assert!(tmp.join("out/wide.pdf").is_file(), "no PDF was written");
}

#[test]
fn a_document_opens_for_review_and_an_override_re_decides_it() {
    // The 3f loop, driven the way the app will drive it: open, look at what was
    // conceded, allow something, and get a fresh decision back.
    let tmp = TempDir::new("app-review");
    let source = tmp.write("wide.md", WIDE_TABLE);
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
fn an_attention_entry_says_which_page_to_turn_to() {
    // The whole chain, because every link is in a different crate: the RenderPass leaves
    // a marker, `Compilation::element_pages` reads it back through the introspector, the
    // worker composes it into the list, and the window draws it.
    //
    // Two identical wide tables with a page of prose between them. Asserted *relatively*
    // — the second must land later than the first — so the test pins the mechanism
    // rather than the font metrics that decide exactly where a page breaks.
    let table = String::from_utf8(WIDE_TABLE.to_vec()).expect("utf8");
    let filler = "Prose that exists only to fill a page, at length. ".repeat(40);
    let markdown = format!("{table}\n\n{filler}\n\n{filler}\n\n{table}\n");

    let tmp = TempDir::new("app-pages");
    let source = tmp.write("two.md", markdown.as_bytes());
    let worker = Worker::spawn();
    let mut app = App::default();

    worker.send(app.open_request(&source));
    for update in collect(&worker, |all| {
        all.iter().any(|u| matches!(u, Update::Opened { .. }))
    }) {
        app.absorb(update);
    }

    let list = app.attention.as_ref().expect("opened without a list");
    let pages: Vec<u32> = list
        .items
        .iter()
        .map(|i| i.page.expect("an entry could not say what page it is on"))
        .collect();
    assert_eq!(
        pages.len(),
        2,
        "expected one entry per table, got {pages:?}"
    );
    assert_eq!(pages[0], 1, "the first table is not on page 1");
    assert!(
        pages[1] > pages[0],
        "both tables report page {:?} — the marker is not tracking the layout",
        pages
    );
    assert!(
        pages[1] as usize <= app.pages,
        "page {} is past the end of a {}-page document",
        pages[1],
        app.pages
    );
}

#[test]
fn one_click_on_a_grouped_row_re_decides_every_element_in_it() {
    // What the window actually sends now: the attention list groups three reflowed
    // tables into one row, and its single button must move all three — in one re-decide,
    // not three.
    let table = String::from_utf8(WIDE_TABLE.to_vec()).expect("utf8");
    let tmp = TempDir::new("app-group");
    let source = tmp.write(
        "three.md",
        format!("{table}\n{table}\n{table}\n").as_bytes(),
    );
    let worker = Worker::spawn();
    let mut app = App::default();

    worker.send(app.open_request(&source));
    for update in collect(&worker, |all| {
        all.iter().any(|u| matches!(u, Update::Opened { .. }))
    }) {
        app.absorb(update);
    }

    let groups = app.attention.as_ref().expect("no list").grouped();
    assert_eq!(groups.len(), 1, "three identical tables drew {groups:?}");
    let group = &groups[0];
    assert_eq!(group.count(), 3);
    let offer = group.offers.first().expect("nothing to click").permit;

    worker.send(app.allow_group_request(group, offer));
    let updates = collect(&worker, |all| {
        all.iter().any(|u| matches!(u, Update::Redecided { .. }))
    });
    assert_eq!(
        updates
            .iter()
            .filter(|u| matches!(u, Update::Redecided { .. }))
            .count(),
        1,
        "one row, one re-decide — three applies would redraw the list three times"
    );
    for update in updates {
        app.absorb(update);
    }

    // Every one of them moved: none is still reporting the concession that was allowed
    // away. The permission was Landscape, so nothing should still be reflowing.
    let after = app.attention.as_ref().expect("no list after");
    assert!(
        !after
            .items
            .iter()
            .any(|i| i.what == md2pdf_domain::CompromiseKind::Reflowed),
        "a table was left behind by the group's button: {:?}",
        after.items
    );
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

    // A one-element list is still the stale case: `apply_all` reports how many named a
    // live Element, and none did.
    worker.send(Request::Allow(vec![md2pdf_domain::Override {
        id: md2pdf_domain::ElementId::new(99, "not in this document"),
        permit: Permit::Landscape,
    }]));
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
