//! Job orchestration: convert -> probe -> harvest -> render -> write.
//!
//! The imperative shell for one conversion. The decisions it applies are made by
//! the pure core; this module sequences the passes and owns the I/O.
//!
//! This is the walking skeleton — one Source, no batch, no collision policy, no
//! template discovery. Every one of those is a *widening* of a path that works.

use std::path::{Path, PathBuf};

use md2pdf_convert::{convert, ImageProbe, SourceContext};
use md2pdf_domain::{BlanketResolution, Compromise, CompromiseKind, Diagnostic, Element, Template};
use md2pdf_paths::{output_path, PathBroker, PathError};
use md2pdf_typeset::Typesetter;

use crate::contract::{Command, Emit, Event, SkipReason};
use crate::output::{plan, WriteMode};

/// Everything the engine needs from the outside world.
///
/// The `Typesetter` is borrowed rather than constructed per Job because it holds a
/// long-lived `World`, and that is what keeps `comemo` memoisation alive between
/// compilations — the difference between an Override costing ~8ms and a cold compile.
pub struct Deps<'a> {
    pub broker: &'a PathBroker,
    pub typesetter: &'a Typesetter,
    /// 3c1 uses `Template::default()`; the catalogue arrives in 3e.
    pub template: &'a Template,
}

/// What can go wrong. Internal — every one of these reaches an adapter as an `Event`,
/// because an adapter may be out-of-process and cannot see a `Result` (`INV-8`).
#[derive(Debug, thiserror::Error)]
pub enum JobError {
    #[error("could not read the source: {0}")]
    SourceUnreadable(#[source] PathError),
    /// The gap between "the probe said this file exists" and "read its bytes".
    ///
    /// It cannot be closed without holding an open handle, and it has a sharp edge:
    /// the markup already references the virtual name, so registering nothing would
    /// hand Typst a reference it cannot satisfy — failing the **whole document**
    /// rather than one element. Failing the Job here is louder and more honest.
    #[error("could not read the image {path}: {source}")]
    ImageUnreadable {
        path: PathBuf,
        #[source]
        source: PathError,
    },
    #[error("compilation failed: {0}")]
    Compile(String),
    #[error("could not write the output: {0}")]
    OutputUnwritable(#[source] PathError),
}

/// Answer one Command.
///
/// Returns nothing: **every** outcome travels through `emit` (`INV-8`). One channel,
/// so an in-process adapter and an out-of-process one learn exactly the same things.
///
/// Deliberately does **not** call `Typesetter::clear_files`. The file map must be
/// cleared per *Job*, not per *Source* — names are keyed by resolved path, so sharing
/// across a batch is correct and clearing per Source would throw away the `comemo` hit
/// the long-lived `World` exists to keep. Only the caller knows where a Job begins,
/// because the caller owns the Typesetter.
pub fn handle(command: Command, deps: &Deps, emit: Emit) {
    match command {
        Command::ConvertSource {
            source,
            destination,
        } => {
            // One Source is still a Job, so the file map is cleared here too.
            deps.typesetter.clear_files();
            let output = output_path(&destination, &source, None);
            report(
                &source,
                convert_one(&source, &output, WriteMode::New, deps, emit),
                emit,
            );
        }
        Command::ConvertBatch {
            source_root,
            destination,
            on_collision,
        } => convert_batch(&source_root, &destination, on_collision, deps, emit),
    }
}

/// Turn one Source's outcome into events. The only place a `Result` becomes an `Event`.
fn report(source: &Path, outcome: Result<bool, JobError>, emit: Emit) {
    match outcome {
        // The flagged flag matters to a batch's counts, not to reporting.
        Ok(_) => {}
        Err(JobError::Compile(message)) => emit(Event::CompilationFailed {
            source: source.to_path_buf(),
            message,
        }),
        Err(other) => emit(Event::SourceFailed {
            source: source.to_path_buf(),
            message: other.to_string(),
        }),
    }
}

/// How many compilations a memoised result survives without being used.
///
/// Not seconds — `comemo` ages entries by calls to `evict`, so this is "five documents
/// ago" in the batch. **[measured]** 2026-08-24: see `design/plan-comemo.md` for the
/// curve this bounds and the trade it does not cost.
const EVICT_AGE: usize = 5;

/// Convert every Source under a root, mirroring the tree.
///
/// Collisions are resolved **before any conversion begins** — see `output::plan`. One
/// bad Source does not end the run: it is reported and the batch continues, because
/// forty-nine good conversions must not be lost to one malformed file.
fn convert_batch(
    source_root: &Path,
    destination: &Path,
    on_collision: BlanketResolution,
    deps: &Deps,
    emit: Emit,
) {
    // One batch is one Job, so the file map is cleared once — here, and never between
    // Sources. Virtual names are keyed by resolved path, so a logo shared by fifty
    // documents is read and registered once; clearing per Source would throw away the
    // `comemo` hit the long-lived World exists to keep.
    deps.typesetter.clear_files();

    let set = match deps.broker.walk(source_root) {
        Ok(set) => set,
        // The Job itself cannot proceed — distinct from one Source failing inside it.
        Err(e) => {
            emit(Event::Failed {
                message: e.to_string(),
            });
            return;
        }
    };

    let plan = plan(&set, destination, on_collision, deps.broker);

    let mut converted = 0usize;
    let mut flagged = 0usize;
    let mut failed = 0usize;

    for collision in &plan.skipped {
        emit(Event::SourceSkipped {
            source: collision.source.clone(),
            output: collision.output.clone(),
            reason: match on_collision {
                BlanketResolution::RenameAll => SkipReason::RenameExhausted,
                _ => SkipReason::Collision,
            },
        });
    }

    for write in &plan.writes {
        match convert_one(&write.source, &write.output, write.mode, deps, emit) {
            Ok(was_flagged) => {
                converted += 1;
                if was_flagged {
                    flagged += 1;
                }
            }
            Err(e) => {
                failed += 1;
                report(&write.source, Err(e), emit);
            }
        }
        // **Between documents, not within one.** `comemo`'s cache is process-global and
        // nothing evicted it until T31: the batch grew ~24 MiB per document, unbounded,
        // and was killed by the OOM killer at ~141 of the 146-document corpus. With this
        // call it plateaus around 690 MiB and the corpus completes.
        //
        // Age 5, **[measured]**: recompiling a document steadily costs 4ms at age 5
        // against 5ms with no eviction at all, so the memoisation the long-lived World
        // exists for is fully preserved. Only `evict(0)` destroys it — 470ms, a hundred
        // times slower. The number is a memory/speed dial and this end of it is free.
        //
        // Here rather than inside `Typesetter::render` because the right cadence belongs
        // to the caller: a batch wants it per document, 3f's recompile loop will want it
        // far less often. See `design/plan-comemo.md`.
        md2pdf_typeset::evict(EVICT_AGE);
    }

    emit(Event::BatchCompleted {
        converted,
        flagged,
        skipped: plan.skipped.len(),
        failed,
    });
}

/// Report characters that will render as an empty box.
///
/// **The engine asks, because only the engine can.** `convert` chose the text and has no
/// fonts by design; `typeset` has the FontBook and never sees a Diagnostic being built.
/// This is the composition layer, so the question is asked here.
///
/// `convert` already substitutes the two characters with a sensible plain equivalent
/// (`glyphs::SUBSTITUTIONS`). This covers everything else — **[measured]** an exhaustive
/// scan of the corpus found **21** characters with no glyph, not the 2 that were noticed
/// by eye: emoji faces, coloured circles, a clipboard. There is no editorial substitute
/// for 😊, so the honest thing is to say so rather than draw a box and call the document
/// clean. See `design/plan-glyphs.md`.
///
/// One Compromise per character per document, not per occurrence — the Diagnostic is
/// read by a person deciding where to look.
fn tofu(elements: &[Element], deps: &Deps) -> Vec<Compromise> {
    let mut seen: Vec<char> = Vec::new();
    for el in elements {
        for c in el.body.as_str().chars() {
            // ASCII is covered by construction, and asking about every one of it would
            // dominate the cost of a check that runs on every document.
            if (c as u32) > 0x7F && !seen.contains(&c) {
                seen.push(c);
            }
        }
    }
    if seen.is_empty() {
        return Vec::new();
    }

    let missing = deps.typesetter.uncovered(seen);
    elements
        .first()
        .map(|first| {
            missing
                .into_iter()
                .map(|c| Compromise {
                    id: first.id,
                    kind: CompromiseKind::UnsupportedConstruct {
                        construct: format!("no glyph for U+{:04X} {c}", c as u32),
                    },
                    page: None,
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Existence, answered by the one crate allowed to ask.
///
/// The adapter lives here rather than in `md2pdf-paths` because `ImageProbe` is
/// defined in `md2pdf-convert`, and paths cannot see it. The engine already depends on
/// both, and wiring ports to adapters is what a composition root is for.
pub struct BrokerImages<'a>(pub &'a PathBroker);

impl ImageProbe for BrokerImages<'_> {
    fn exists(&self, path: &Path) -> bool {
        self.0.exists(path)
    }
}

/// Read, convert, compile, write — one Source. Returns whether it was flagged.
fn convert_one(
    source: &Path,
    output: &Path,
    mode: WriteMode,
    deps: &Deps,
    emit: Emit,
) -> Result<bool, JobError> {
    let markdown = deps
        .broker
        .read_to_string(source)
        .map_err(JobError::SourceUnreadable)?;

    let images = BrokerImages(deps.broker);
    let context = match source.parent() {
        Some(dir) => SourceContext::new(dir, &images),
        None => SourceContext::none(),
    };
    let conversion = convert(&markdown, &context);

    emit(Event::SourceConverted {
        source: source.to_path_buf(),
        elements: conversion.elements.len(),
        images: conversion.images.len(),
        compromises: conversion.compromises.len(),
    });

    // Typst cannot load a file the World will not serve, and an unresolvable file
    // fails the whole document — so every manifest entry must be registered before
    // anything is compiled.
    for (virtual_name, path) in &conversion.images {
        let bytes = deps
            .broker
            .read_bytes(path)
            .map_err(|source| JobError::ImageUnreadable {
                path: path.clone(),
                source,
            })?;
        deps.typesetter.add_file(virtual_name, bytes);
    }

    let (decisions, _) = deps
        .typesetter
        .probe(&conversion.elements, deps.template)
        .map_err(|e| JobError::Compile(e.to_string()))?;

    // Both halves of the pipeline concede, and only a sealed Diagnostic carries both
    // (`INV-4`). Flagged is not failed: the document converted, and something in it
    // needed a judgment call.
    let mut convert_compromises = conversion.compromises.clone();
    convert_compromises.extend(tofu(&conversion.elements, deps));
    let diagnostic = Diagnostic::seal(convert_compromises, &decisions);

    // Emit the whole thing, not just whether it is empty. Sealing it and reporting only
    // `is_flagged()` is how a run can say "70 need your attention" and explain two.
    if diagnostic.is_flagged() {
        emit(Event::DiagnosticSealed {
            source: source.to_path_buf(),
            compromises: diagnostic.compromises.clone(),
        });
    }

    let compilation = deps
        .typesetter
        .render(&conversion.elements, deps.template, &decisions)
        .map_err(|e| JobError::Compile(e.to_string()))?;

    let pdf = compilation
        .pdf()
        .map_err(|e| JobError::Compile(e.to_string()))?;

    emit(Event::CompilationSucceeded {
        source: source.to_path_buf(),
        pages: compilation.page_count(),
    });

    // `overwrite` only where the plan says a Collision was resolved that way — a
    // recorded human decision, never a default (`INV-3`).
    match mode {
        WriteMode::New => deps.broker.write_new(output, &pdf),
        WriteMode::Replace => deps.broker.overwrite(output, &pdf),
    }
    .map_err(JobError::OutputUnwritable)?;

    emit(Event::OutputWritten {
        source: source.to_path_buf(),
        path: output.to_path_buf(),
    });
    Ok(diagnostic.is_flagged())
}
