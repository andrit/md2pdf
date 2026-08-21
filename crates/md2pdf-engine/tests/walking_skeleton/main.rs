//! The walking skeleton: `notes.md` on disk becomes `notes.pdf` on disk.
//!
//! The first tests in the project that touch a real filesystem, and the first that run
//! every layer together — paths, convert, typeset — rather than one layer against a
//! stub. Layers that pass their own tests can still fail to meet each other; this is
//! where that would show.
//!
//! `TempDir` is borrowed from `md2pdf-paths` so `std::fs` stays inside the one crate
//! allowed to call it (`INV-9`).
//!
//! Several files, one test binary. Each `tests/*.rs` is a separate binary statically
//! linking ~250 typst crates, and two linking at once exhausts a 4 GB machine — so the
//! census joins this target as a module rather than becoming a fourth. Same arrangement
//! as `md2pdf-convert`'s `tests/compiler/`, and for the same reason.
//! See `docs/development.md`.

mod census;

use std::path::Path;

use md2pdf_domain::{CompromiseKind, Template};
use md2pdf_engine::{handle, Command, Deps, Event};
use md2pdf_paths::{testing::TempDir, PathBroker};
use md2pdf_typeset::Typesetter;

const PNG: &[u8] = include_bytes!("../../../md2pdf-typeset/tests/fixtures/wide-200x20.png");

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
        Event::OutputWritten { path, .. } => Some(path.as_path()),
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
            } => Some((*images, *compromises)),
            _ => None,
        })
        .expect("no SourceConverted event");
    assert_eq!(converted.0, 1, "the image was not resolved");
    assert_eq!(
        converted.1, 0,
        "a resolvable image should not be a compromise"
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

    // The complete set now arrives with the seal, not with SourceConverted.
    let compromises = events
        .iter()
        .find_map(|e| match e {
            Event::DiagnosticSealed { compromises, .. } => Some(compromises.clone()),
            _ => None,
        })
        .expect("no DiagnosticSealed event");
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
        events
            .iter()
            .any(|e| matches!(e, Event::Failed { .. } | Event::SourceFailed { .. })),
        "expected a failure event: {events:?}"
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
        events
            .iter()
            .any(|e| matches!(e, Event::Failed { .. } | Event::SourceFailed { .. })),
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
            Event::SourceSkipped { .. } => "skipped",
            Event::SourceFailed { .. } => "source_failed",
            Event::BatchCompleted { .. } => "batch_done",
            Event::DiagnosticSealed { .. } => "flagged",
        })
        .collect();
    assert_eq!(shape, vec!["converted", "compiled", "written"]);
}

// ---- Batch (T21) ------------------------------------------------------------

use md2pdf_domain::BlanketResolution;

fn run_batch(root: &Path, destination: &Path, on_collision: BlanketResolution) -> Vec<Event> {
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
        Command::ConvertBatch {
            source_root: root.to_path_buf(),
            destination: destination.to_path_buf(),
            on_collision,
        },
        &deps,
        &mut emit,
    );
    events
}

fn completion(events: &[Event]) -> (usize, usize, usize, usize) {
    events
        .iter()
        .find_map(|e| match e {
            Event::BatchCompleted {
                converted,
                flagged,
                skipped,
                failed,
            } => Some((*converted, *flagged, *skipped, *failed)),
            _ => None,
        })
        .expect("no BatchCompleted event")
}

#[test]
fn a_directory_converts_with_the_tree_mirrored() {
    let tmp = TempDir::new("batch");
    tmp.write("docs/a.md", b"# A\n");
    tmp.write("docs/guide/b.md", b"# B\n");
    tmp.write("docs/guide/deep/c.md", b"# C\n");
    tmp.write("docs/notes.txt", b"not a source");

    let events = run_batch(
        &tmp.join("docs"),
        &tmp.join("out"),
        BlanketResolution::SkipAll,
    );

    assert_eq!(completion(&events), (3, 0, 0, 0));
    // INV-12: mirrored, never flattened.
    for expected in ["out/a.pdf", "out/guide/b.pdf", "out/guide/deep/c.pdf"] {
        assert!(
            tmp.join(expected).is_file(),
            "missing {expected}; got {:?}",
            events
        );
    }
}

#[test]
fn one_unconvertible_source_does_not_end_the_batch() {
    let tmp = TempDir::new("batch-bad");
    tmp.write("docs/good1.md", b"# Fine\n");
    // Invalid UTF-8: readable bytes, unreadable as text.
    tmp.write("docs/bad.md", &[b'#', b' ', 0xFF, b'\n']);
    tmp.write("docs/good2.md", b"# Also fine\n");

    let events = run_batch(
        &tmp.join("docs"),
        &tmp.join("out"),
        BlanketResolution::SkipAll,
    );

    let (converted, _, _, failed) = completion(&events);
    assert_eq!(converted, 2, "the good files should still convert");
    assert_eq!(failed, 1);
    assert!(tmp.join("out/good1.pdf").is_file());
    assert!(tmp.join("out/good2.pdf").is_file());
    assert!(events
        .iter()
        .any(|e| matches!(e, Event::SourceFailed { .. })));
}

/// `INV-3`, at batch scale: an existing output survives untouched.
#[test]
fn skip_all_leaves_existing_output_untouched() {
    let tmp = TempDir::new("batch-skip");
    tmp.write("docs/a.md", b"# New content\n");
    tmp.write("docs/b.md", b"# Fresh\n");
    tmp.write("out/a.pdf", b"ORIGINAL");

    let events = run_batch(
        &tmp.join("docs"),
        &tmp.join("out"),
        BlanketResolution::SkipAll,
    );

    let (converted, _, skipped, _) = completion(&events);
    assert_eq!((converted, skipped), (1, 1));
    assert_eq!(
        PathBroker::new()
            .read_bytes(&tmp.join("out/a.pdf"))
            .expect("read"),
        b"ORIGINAL",
        "a skipped Source overwrote its output"
    );
}

#[test]
fn rename_all_writes_beside_the_existing_file() {
    let tmp = TempDir::new("batch-rename");
    tmp.write("docs/a.md", b"# New\n");
    tmp.write("out/a.pdf", b"ORIGINAL");

    let events = run_batch(
        &tmp.join("docs"),
        &tmp.join("out"),
        BlanketResolution::RenameAll,
    );

    assert_eq!(completion(&events).0, 1);
    assert!(tmp.join("out/a-1.pdf").is_file(), "no renamed output");
    assert_eq!(
        PathBroker::new()
            .read_bytes(&tmp.join("out/a.pdf"))
            .expect("read"),
        b"ORIGINAL"
    );
}

#[test]
fn overwrite_all_replaces_only_because_it_was_asked_to() {
    let tmp = TempDir::new("batch-over");
    tmp.write("docs/a.md", b"# New\n");
    tmp.write("out/a.pdf", b"ORIGINAL");

    run_batch(
        &tmp.join("docs"),
        &tmp.join("out"),
        BlanketResolution::OverwriteAll,
    );

    let now = PathBroker::new()
        .read_bytes(&tmp.join("out/a.pdf"))
        .expect("read");
    assert!(now.starts_with(b"%PDF"), "the file was not replaced");
}

/// `INV-4` and `INV-5`: the counts behind "47 converted cleanly, 3 need your attention".
#[test]
fn flagged_sources_are_counted_separately_from_clean_ones() {
    let tmp = TempDir::new("batch-flag");
    tmp.write("docs/clean.md", b"# Nothing wrong here\n");
    tmp.write("docs/flagged.md", b"![missing](gone.png)\n");

    let events = run_batch(
        &tmp.join("docs"),
        &tmp.join("out"),
        BlanketResolution::SkipAll,
    );

    let (converted, flagged, _, _) = completion(&events);
    assert_eq!(converted, 2, "both should convert — flagged is not failed");
    assert_eq!(flagged, 1, "the missing image should flag exactly one");
}

#[test]
fn an_unwalkable_root_fails_the_job_not_a_source() {
    let tmp = TempDir::new("batch-noroot");
    let events = run_batch(
        &tmp.join("does-not-exist"),
        &tmp.join("out"),
        BlanketResolution::SkipAll,
    );

    assert!(
        events.iter().any(|e| matches!(e, Event::Failed { .. })),
        "expected a Job-level failure: {events:?}"
    );
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, Event::BatchCompleted { .. })),
        "a Job that never started should not report completion"
    );
}

#[test]
fn an_empty_directory_completes_with_zeroes() {
    let tmp = TempDir::new("batch-empty");
    std::hint::black_box(tmp.write("docs/ignored.txt", b"x"));

    let events = run_batch(
        &tmp.join("docs"),
        &tmp.join("out"),
        BlanketResolution::SkipAll,
    );

    assert_eq!(completion(&events), (0, 0, 0, 0));
}

/// Where the time goes, per phase. Kept as a tool rather than deleted: the next time
/// someone asks "what is slow", this answers it in one command.
///
///     cargo test -p md2pdf-engine --test walking_skeleton profile_the_phases -- --ignored --nocapture
///
/// Reads from `documents/`, which is untracked — it skips cleanly when absent.
#[test]
#[ignore = "profiling tool, not a gate"]
fn profile_the_phases() {
    use md2pdf_convert::{convert, SourceContext};
    use std::time::Instant;

    let broker = PathBroker::new();
    let template = Template::default();

    // Tests run with CWD = crate dir, so anchor on the workspace root.
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    for name in [
        "documents/design-docs/design__event-storm.md",
        "documents/factory/micro-saas-ideas.md",
        "documents/design-docs/design__kano.md",
    ] {
        let owned = root.join(name);
        let path = owned.as_path();
        let Ok(markdown) = broker.read_to_string(path) else {
            println!("SKIP {name}");
            continue;
        };

        let t0 = Instant::now();
        let conversion = convert(&markdown, &SourceContext::none());
        let convert_ms = t0.elapsed().as_millis();

        let ts = Typesetter::new();
        let t1 = Instant::now();
        let (map, _) = ts.probe(&conversion.elements, &template).expect("probe");
        let probe_ms = t1.elapsed().as_millis();

        let t2 = Instant::now();
        let compilation = ts
            .render(&conversion.elements, &template, &map)
            .expect("render");
        let render_ms = t2.elapsed().as_millis();

        let t3 = Instant::now();
        let pdf = compilation.pdf().expect("pdf");
        let pdf_ms = t3.elapsed().as_millis();

        let atomic = conversion
            .elements
            .iter()
            .filter(|e| e.class.is_atomic())
            .count();
        println!(
            "PROF {:<42} elements={:<4} atomic={:<3} convert={:>5}ms probe={:>6}ms render={:>5}ms pdf={:>4}ms  bytes={}",
            path.file_name().unwrap().to_string_lossy(),
            conversion.elements.len(),
            atomic,
            convert_ms, probe_ms, render_ms, pdf_ms, pdf.len()
        );
    }
}
