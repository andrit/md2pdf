//! The ladder census — golden hashes, but for **decisions** rather than bytes.
//!
//! `design/compromise-mechanism.md` §9 requires every ladder change to re-measure the
//! baseline. That was honour-system: nothing enforced it, nothing noticed when it was
//! skipped, and the baseline was a snapshot with no history — so a drift spread across
//! three changes would have been invisible. This is the tripwire.
//!
//! The existing golden hashes pin a PDF's bytes. They would catch a ladder change as an
//! opaque mismatch on whichever fixture happened to contain a wide table; they cannot
//! say *"rotations went from 84 to 12"*, which is the thing worth seeing.
//!
//! ## One source of truth
//!
//! The expected numbers are **not** in this file. The census is regenerated from the
//! fixtures and compared against `design/ladder-census.txt`, so the committed file is
//! the record and this is only the check. Updating the baseline is a reviewable edit to
//! a readable artefact, and its **git history is the log** of how the ladder has moved.
//!
//! Fixtures live in `tests/corpus/` — a directory of `.md` and `.png`, which cargo does
//! not turn into a test target, so no additional typst-linking binary is created.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use md2pdf_convert::{convert, ImageProbe, SourceContext};
use md2pdf_domain::{CompromiseKind, Diagnostic, Template};
use md2pdf_paths::PathBroker;
use md2pdf_typeset::Typesetter;

/// Real existence, so a missing image is missing for the same reason it would be in
/// production rather than because a stub said so.
struct CorpusImages;

impl ImageProbe for CorpusImages {
    fn exists(&self, path: &Path) -> bool {
        PathBroker::new().exists(path)
    }
}

fn corpus_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/corpus")
}

fn census_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../design/ladder-census.txt")
}

/// Short, stable names so a diff reads as prose rather than as symbols.
fn label(kind: &CompromiseKind) -> &'static str {
    match kind {
        CompromiseKind::ShrunkToFloor { .. } => "shrunk",
        CompromiseKind::Scaled { .. } => "scaled",
        CompromiseKind::Rotated => "rotated",
        CompromiseKind::Reflowed => "reflowed",
        CompromiseKind::Clipped => "CLIPPED",
        CompromiseKind::ImageMissing => "image-missing",
        CompromiseKind::ImageSkipped => "image-skipped",
        CompromiseKind::UnsupportedConstruct { .. } => "unsupported",
    }
}

/// Run every fixture and render the result as text.
///
/// Deliberately records *kinds and counts*, not sizes or factors: an 8.5pt shrink
/// becoming 8.0pt is a tuning change, while a shrink becoming a rotation is a change of
/// behaviour, and only the second should turn this red.
fn generate() -> String {
    let broker = PathBroker::new();
    let template = Template::default();
    let dir = corpus_dir();

    let mut names: Vec<PathBuf> = broker
        .walk(&dir)
        .expect("corpus directory")
        .sources
        .into_iter()
        .collect();
    names.sort();

    let mut out = String::from(
        "# Ladder census — decisions made for each fixture in md2pdf-engine/tests/corpus.\n\
         #\n\
         # Regenerate:\n\
         #   cargo test -p md2pdf-engine --test walking_skeleton regenerate_the_census \\\n\
         #        -- --ignored --nocapture\n\
         #\n\
         # A change here is a change in behaviour. If it was intended, commit this file\n\
         # in the same commit as the change and say why. If it was not, you have found a\n\
         # defect in something you were not touching.\n\
         #\n\
         # See design/compromise-mechanism.md\n\n",
    );

    for source in names {
        let markdown = broker.read_to_string(&source).expect("fixture");
        let images = CorpusImages;
        let ctx = SourceContext::new(&dir, &images);
        let conversion = convert(&markdown, &ctx);

        let typesetter = Typesetter::new();
        for (name, path) in &conversion.images {
            if let Ok(bytes) = broker.read_bytes(path) {
                typesetter.add_file(name, bytes);
            }
        }

        let (decisions, _) = typesetter
            .probe(&conversion.elements, &template)
            .expect("probe");
        let sealed = Diagnostic::seal(conversion.compromises.clone(), &decisions);

        let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
        for c in &sealed.compromises {
            *counts.entry(label(&c.kind)).or_default() += 1;
        }

        let summary = if counts.is_empty() {
            "clean".to_string()
        } else {
            counts
                .iter()
                .map(|(k, n)| format!("{n} {k}"))
                .collect::<Vec<_>>()
                .join(", ")
        };

        out.push_str(&format!(
            "{:<28} {:>2} elements   {summary}\n",
            source.file_name().unwrap().to_string_lossy(),
            conversion.elements.len(),
        ));
    }
    out
}

/// The tripwire. Fails when the ladder decides anything differently.
#[test]
fn the_ladder_still_decides_what_it_decided() {
    let actual = generate();
    let expected = PathBroker::new()
        .read_to_string(&census_path())
        .unwrap_or_else(|_| String::new());

    if actual == expected {
        return;
    }

    // A readable diff: which fixtures moved, and how.
    let mut report = String::from("the ladder's decisions changed\n\n");
    let expected_lines: Vec<&str> = expected.lines().filter(|l| !l.starts_with('#')).collect();
    let actual_lines: Vec<&str> = actual.lines().filter(|l| !l.starts_with('#')).collect();
    for (was, now) in expected_lines.iter().zip(actual_lines.iter()) {
        if was != now {
            report.push_str(&format!("  was: {was}\n  now: {now}\n\n"));
        }
    }
    if expected_lines.len() != actual_lines.len() {
        report.push_str(&format!(
            "  fixture count changed: {} -> {}\n\n",
            expected_lines.len(),
            actual_lines.len()
        ));
    }
    report.push_str(
        "If this change was intended, regenerate the census and commit it in the same\n\
         commit as the change, saying what moved and why:\n\
         \n    cargo test -p md2pdf-engine --test walking_skeleton \\\n\
         \x20       regenerate_the_census -- --ignored --nocapture\n\
         \n\
         If it was not intended, you have found a defect in something you were not\n\
         touching. See design/compromise-mechanism.md.\n",
    );
    panic!("{report}");
}

/// Regenerate the committed census. Prints it; the operator writes it.
///
/// Printing rather than writing keeps `std::fs` out of this crate (`INV-9`) and keeps
/// updating the baseline a deliberate act rather than something a test does behind you.
#[test]
#[ignore = "regenerates the census; run deliberately"]
fn regenerate_the_census() {
    println!("---8<--- design/ladder-census.txt ---8<---");
    print!("{}", generate());
    println!("---8<--- end ---8<---");
}

/// The census is only meaningful if it covers the rungs it is watching.
#[test]
fn the_census_covers_every_compromise_kind() {
    let census = generate();

    // Only the summary half of each line. Searching the whole census would let a
    // *filename* satisfy the assertion — `image-scaled.md` contains "scaled",
    // `unsupported.md` contains "unsupported" — so three of these could never fail no
    // matter what the ladder did. A guard that asserts something it does not check is
    // the exact defect this file exists to catch.
    let summaries: String = census
        .lines()
        .filter_map(|line| line.split_once("elements"))
        .map(|(_, summary)| summary)
        .collect();

    for kind in [
        "shrunk",
        "scaled",
        "rotated",
        "reflowed",
        "CLIPPED",
        "image-missing",
        "image-skipped",
        "unsupported",
    ] {
        assert!(
            summaries.contains(kind),
            "no fixture produces `{kind}` — the census cannot see that rung.\n{census}"
        );
    }
}

/// Print the full decision for every element of every fixture — sizes and factors
/// included, which the census deliberately omits.
///
/// Exists because checking that `shrink-floor.md` really reaches the floor otherwise
/// means writing throwaway code that re-converts and matches ids by hand — the same
/// awkwardness recorded as **R4**. Keep the fixtures honest with this.
#[test]
#[ignore = "inspection; run deliberately"]
fn describe_the_fixtures() {
    let broker = PathBroker::new();
    let template = Template::default();
    let dir = corpus_dir();

    let mut sources = broker.walk(&dir).expect("corpus directory").sources;
    sources.sort();

    for source in sources {
        println!("\n{}", source.file_name().unwrap().to_string_lossy());
        let markdown = broker.read_to_string(&source).expect("fixture");
        let images = CorpusImages;
        let conversion = convert(&markdown, &SourceContext::new(&dir, &images));

        let typesetter = Typesetter::new();
        for (name, path) in &conversion.images {
            if let Ok(bytes) = broker.read_bytes(path) {
                typesetter.add_file(name, bytes);
            }
        }
        let (decisions, _) = typesetter
            .probe(&conversion.elements, &template)
            .expect("probe");

        for el in &conversion.elements {
            let Some(d) = decisions.get(&el.id) else {
                continue;
            };
            println!(
                "  {:>2} {:<10} {:<10} {:<24} natural {:>7.1}pt / available {:>6.1}pt",
                el.id.order,
                format!("{:?}", el.class),
                format!("{:?}", d.orientation),
                format!("{:?}", d.reduction),
                d.natural_pt,
                d.available_pt,
            );
        }
    }
}

/// Decisions must not flicker, or the tripwire is noise.
#[test]
fn the_census_is_stable_across_runs() {
    assert_eq!(generate(), generate());
}
