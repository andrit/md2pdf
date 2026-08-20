//! Job orchestration: convert -> probe -> harvest -> render -> write.
//!
//! The imperative shell for one conversion. The decisions it applies are made by
//! the pure core; this module sequences the passes and owns the I/O.
//!
//! This is the walking skeleton — one Source, no batch, no collision policy, no
//! template discovery. Every one of those is a *widening* of a path that works.

use std::path::{Path, PathBuf};

use md2pdf_convert::{convert, ImageProbe, SourceContext};
use md2pdf_domain::{BlanketResolution, Diagnostic, Template};
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
    }

    emit(Event::BatchCompleted {
        converted,
        flagged,
        skipped: plan.skipped.len(),
        failed,
    });
}

/// Existence, answered by the one crate allowed to ask.
///
/// The adapter lives here rather than in `md2pdf-paths` because `ImageProbe` is
/// defined in `md2pdf-convert`, and paths cannot see it. The engine already depends on
/// both, and wiring ports to adapters is what a composition root is for.
struct BrokerImages<'a>(&'a PathBroker);

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
        compromises: conversion.compromises.clone(),
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
    let diagnostic = Diagnostic::seal(conversion.compromises.clone(), &decisions);

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
