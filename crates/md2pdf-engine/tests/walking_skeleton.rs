//! The walking skeleton: `notes.md` on disk becomes `notes.pdf` on disk.
//!
//! The first tests in the project that touch a real filesystem, and the first that run
//! every layer together — paths, convert, typeset — rather than one layer against a
//! stub. Layers that pass their own tests can still fail to meet each other; this is
//! where that would show.
//!
//! `TempDir` is borrowed from `md2pdf-paths` so `std::fs` stays inside the one crate
//! allowed to call it (`INV-9`).

use std::path::Path;

use md2pdf_domain::{CompromiseKind, Template};
use md2pdf_engine::{handle, Command, Deps, Event};
use md2pdf_paths::{testing::TempDir, PathBroker};
use md2pdf_typeset::Typesetter;

const PNG: &[u8] = include_bytes!("../../md2pdf-typeset/tests/fixtures/wide-200x20.png");

/// Run one Command and collect what the engine said.
fn run(source: &Path, destination: &Path) -> Vec<Event> {
    let broker = PathBroker::new();
    let typesetter = Typesetter::new();
    let template = Template::default();
    let deps = Deps {
        broker: &broker,
        typesetter: &typesetter,
        template: &template,
    };

    let mut events = Vec::new();
    let mut emit = |e: Event| events.push(e);
    handle(
        Command::ConvertSource {
            source: source.to_path_buf(),
            destination: destination.to_path_buf(),
        },
        &deps,
        &mut emit,
    );
    events
}

fn written_path(events: &[Event]) -> Option<&Path> {
    events.iter().find_map(|e| match e {
        Event::OutputWritten { path } => Some(path.as_path()),
        _ => None,
    })
}

#[test]
fn a_markdown_file_becomes_a_pdf_on_disk() {
    let tmp = TempDir::new("skeleton");
    let source = tmp.write("notes.md", b"# Title\n\nSome *prose* with `code`.\n");
    let out = tmp.join("out");

    let events = run(&source, &out);

    let written = written_path(&events).unwrap_or_else(|| panic!("no output: {events:?}"));
    assert_eq!(written, tmp.join("out/notes.pdf"));

    let pdf = PathBroker::new().read_bytes(written).expect("read back");
    assert!(pdf.starts_with(b"%PDF"), "not a PDF");
    assert!(pdf.len() > 1000, "suspiciously small: {} bytes", pdf.len());
}

/// The seam this whole phase exists to prove: a real image on disk, resolved by the
/// broker, carried through the manifest, registered with the typesetter, embedded.
/// Every part of that has been tested against a stub; none of it together.
#[test]
fn a_real_image_on_disk_reaches_the_pdf() {
    let tmp = TempDir::new("image");
    tmp.write("img/chart.png", PNG);
    let source = tmp.write("report.md", b"# Report\n\n![a chart](img/chart.png)\n");
    let out = tmp.join("out");

    let events = run(&source, &out);

    let converted = events
        .iter()
        .find_map(|e| match e {
            Event::SourceConverted {
                images,
                compromises,
                ..
            } => Some((*images, compromises.clone())),
            _ => None,
        })
        .expect("no SourceConverted event");
    assert_eq!(converted.0, 1, "the image was not resolved");
    assert!(
        converted.1.is_empty(),
        "a resolvable image should not be a compromise: {:?}",
        converted.1
    );

    let written = written_path(&events).unwrap_or_else(|| panic!("no output: {events:?}"));
    let pdf = PathBroker::new().read_bytes(written).expect("read back");
    assert!(pdf.starts_with(b"%PDF"));
}

/// `INV-4`: a concession must reach the adapter, or the attention gate has nothing to
/// gate on.
#[test]
fn a_missing_image_degrades_and_is_reported() {
    let tmp = TempDir::new("missing");
    let source = tmp.write("notes.md", b"![gone](nope.png)\n");
    let out = tmp.join("out");

    let events = run(&source, &out);

    let compromises = events
        .iter()
        .find_map(|e| match e {
            Event::SourceConverted { compromises, .. } => Some(compromises.clone()),
            _ => None,
        })
        .expect("no SourceConverted event");
    assert!(
        compromises
            .iter()
            .any(|c| c.kind == CompromiseKind::ImageMissing),
        "ImageMissing never reached the event stream: {compromises:?}"
    );

    // And the Job still succeeded — a missing picture is not a failed document.
    assert!(
        written_path(&events).is_some(),
        "the job should still write"
    );
}

#[test]
fn a_source_that_does_not_exist_fails_and_writes_nothing() {
    let tmp = TempDir::new("nosource");
    let source = tmp.join("absent.md");
    let out = tmp.join("out");

    let events = run(&source, &out);

    assert!(
        events.iter().any(|e| matches!(e, Event::Failed { .. })),
        "expected a Failed event: {events:?}"
    );
    assert!(written_path(&events).is_none());
    assert!(
        !out.exists(),
        "the destination was created for a failed job"
    );
}

/// `INV-3`: output is never silently overwritten. The skeleton has no collision
/// policy yet, and "no policy" must not mean "destroy the file".
#[test]
fn an_existing_output_is_refused_not_overwritten() {
    let tmp = TempDir::new("collide");
    let source = tmp.write("notes.md", b"# Second\n");
    let out = tmp.join("out");
    tmp.write("out/notes.pdf", b"ORIGINAL");

    let events = run(&source, &out);

    assert!(
        events.iter().any(|e| matches!(e, Event::Failed { .. })),
        "expected a refusal: {events:?}"
    );
    let existing = PathBroker::new()
        .read_bytes(&tmp.join("out/notes.pdf"))
        .expect("read back");
    assert_eq!(existing, b"ORIGINAL", "the existing file was overwritten");
}

#[test]
fn events_arrive_in_the_order_the_work_happened() {
    let tmp = TempDir::new("order");
    let source = tmp.write("notes.md", b"# Title\n\nBody.\n");
    let out = tmp.join("out");

    let events = run(&source, &out);

    let shape: Vec<&str> = events
        .iter()
        .map(|e| match e {
            Event::SourceConverted { .. } => "converted",
            Event::CompilationSucceeded { .. } => "compiled",
            Event::CompilationFailed { .. } => "compile_failed",
            Event::OutputWritten { .. } => "written",
            Event::Failed { .. } => "failed",
        })
        .collect();
    assert_eq!(shape, vec!["converted", "compiled", "written"]);
}
