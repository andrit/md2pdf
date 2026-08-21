//! The CLI, run as a process.
//!
//! These are the only tests in the project that execute the binary. That matters for
//! one of them in particular: `--json` output parsed back into `Event` values is the
//! **out-of-process adapter contract exercised for real** (`INV-8`). Every other test
//! of that boundary only simulates it by round-tripping types in memory.

use std::path::Path;
use std::process::{Command, Output};

use md2pdf_engine::Event;
use md2pdf_paths::testing::TempDir;

/// Run the real binary. Cargo hands us its path.
fn md2pdf(args: &[&Path]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_md2pdf"))
        .args(args)
        .output()
        .expect("could not run md2pdf")
}

fn stdout(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).to_string()
}

fn code(out: &Output) -> i32 {
    out.status.code().expect("no exit code")
}

#[test]
fn a_single_file_converts() {
    let tmp = TempDir::new("cli-one");
    let source = tmp.write("notes.md", b"# Title\n\nSome prose.\n");
    let out = tmp.join("out");

    let result = md2pdf(&[&source, Path::new("-o"), &out]);

    assert_eq!(
        code(&result),
        0,
        "stderr: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(tmp.join("out/notes.pdf").is_file());
    assert!(stdout(&result).contains("1 converted cleanly"));
}

#[test]
fn a_directory_converts_with_the_tree_mirrored() {
    let tmp = TempDir::new("cli-batch");
    tmp.write("docs/a.md", b"# A\n");
    tmp.write("docs/deep/b.md", b"# B\n");
    let out = tmp.join("out");

    let result = md2pdf(&[&tmp.join("docs"), Path::new("-o"), &out]);

    assert_eq!(code(&result), 0);
    assert!(tmp.join("out/a.pdf").is_file());
    assert!(
        tmp.join("out/deep/b.pdf").is_file(),
        "the tree was flattened"
    );
}

/// `INV-8`, proven rather than asserted.
///
/// An adapter running out-of-process sees the event stream and nothing else. This takes
/// the real binary's stdout and rebuilds `Event` values from it — the first time that
/// boundary is crossed for real in this project.
#[test]
fn json_output_parses_back_into_events() {
    let tmp = TempDir::new("cli-json");
    tmp.write("docs/a.md", b"# A\n");
    tmp.write("docs/b.md", b"![missing](gone.png)\n");
    let out = tmp.join("out");

    let result = md2pdf(&[
        &tmp.join("docs"),
        Path::new("-o"),
        &out,
        Path::new("--json"),
    ]);
    assert_eq!(code(&result), 0);

    let events: Vec<Event> = stdout(&result)
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|line| {
            serde_json::from_str(line)
                .unwrap_or_else(|e| panic!("stdout line is not an Event: {e}\n  line: {line}"))
        })
        .collect();

    assert!(!events.is_empty(), "no events on stdout");
    assert!(
        events
            .iter()
            .any(|e| matches!(e, Event::BatchCompleted { .. })),
        "the stream should end with a completion: {events:?}"
    );
    // The Compromise survived serialisation, a process boundary, and deserialisation —
    // carrying what actually happened, not just that something did.
    assert!(
        events.iter().any(|e| matches!(
            e,
            Event::DiagnosticSealed { compromises, .. } if !compromises.is_empty()
        )),
        "a compromise did not survive the boundary"
    );
}

/// In `--json` mode stdout carries nothing but JSON — otherwise the mode is unusable by
/// the consumer it exists for.
#[test]
fn json_mode_puts_nothing_else_on_stdout() {
    let tmp = TempDir::new("cli-jsonpure");
    tmp.write("docs/a.md", b"# A\n");
    let out = tmp.join("out");

    let result = md2pdf(&[
        &tmp.join("docs"),
        Path::new("-o"),
        &out,
        Path::new("--json"),
    ]);

    for line in stdout(&result).lines().filter(|l| !l.trim().is_empty()) {
        serde_json::from_str::<serde_json::Value>(line)
            .unwrap_or_else(|_| panic!("non-JSON on stdout in --json mode: {line}"));
    }
}

#[test]
fn a_document_that_fails_exits_one() {
    let tmp = TempDir::new("cli-fail");
    tmp.write("docs/good.md", b"# Fine\n");
    tmp.write("docs/bad.md", &[b'#', b' ', 0xFF, b'\n']); // not UTF-8
    let out = tmp.join("out");

    let result = md2pdf(&[&tmp.join("docs"), Path::new("-o"), &out]);

    assert_eq!(code(&result), 1);
    assert!(stdout(&result).contains("1 failed"));
    assert!(
        tmp.join("out/good.pdf").is_file(),
        "the batch should continue"
    );
}

#[test]
fn a_job_that_cannot_start_exits_two() {
    let tmp = TempDir::new("cli-noroot");
    let result = md2pdf(&[&tmp.join("absent"), Path::new("-o"), &tmp.join("out")]);
    assert_eq!(code(&result), 2);
}

/// `INV-5`: flagged is not failed. A script author might expect otherwise, so it is
/// pinned.
#[test]
fn a_flagged_document_still_exits_zero() {
    let tmp = TempDir::new("cli-flagged");
    tmp.write("docs/a.md", b"![gone](missing.png)\n");
    let out = tmp.join("out");

    let result = md2pdf(&[&tmp.join("docs"), Path::new("-o"), &out]);

    assert_eq!(code(&result), 0, "a compromise is not a failure");
    assert!(stdout(&result).contains("need your attention"));
    assert!(tmp.join("out/a.pdf").is_file());
}

/// `INV-3` through the whole stack, from a command line.
#[test]
fn an_existing_output_is_not_destroyed_by_default() {
    let tmp = TempDir::new("cli-collide");
    tmp.write("docs/a.md", b"# New\n");
    tmp.write("out/a.pdf", b"ORIGINAL");

    let result = md2pdf(&[&tmp.join("docs"), Path::new("-o"), &tmp.join("out")]);

    assert_eq!(code(&result), 0);
    assert!(stdout(&result).contains("skipped"), "skips must be loud");
    let still = md2pdf_paths::PathBroker::new()
        .read_bytes(&tmp.join("out/a.pdf"))
        .expect("read");
    assert_eq!(still, b"ORIGINAL", "the default destroyed a file");
}

#[test]
fn rename_writes_alongside() {
    let tmp = TempDir::new("cli-rename");
    tmp.write("docs/a.md", b"# New\n");
    tmp.write("out/a.pdf", b"ORIGINAL");

    let result = md2pdf(&[
        &tmp.join("docs"),
        Path::new("-o"),
        &tmp.join("out"),
        Path::new("--on-collision"),
        Path::new("rename"),
    ]);

    assert_eq!(code(&result), 0);
    assert!(tmp.join("out/a-1.pdf").is_file());
}

#[test]
fn help_prints_and_succeeds() {
    let result = md2pdf(&[Path::new("--help")]);
    assert_eq!(code(&result), 0);
    assert!(stdout(&result).contains("USAGE"));
}

#[test]
fn a_bad_argument_explains_itself() {
    let tmp = TempDir::new("cli-badarg");
    let result = md2pdf(&[
        &tmp.join("a.md"),
        Path::new("-o"),
        &tmp.join("out"),
        Path::new("--on-collision"),
        Path::new("clobber"),
    ]);
    assert_eq!(code(&result), 2);
    assert!(
        String::from_utf8_lossy(&result.stderr).contains("clobber"),
        "the error should name the bad value"
    );
}
