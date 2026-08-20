//! Job orchestration: convert -> probe -> harvest -> render -> write.
//!
//! The imperative shell for one conversion. The decisions it applies are made by
//! the pure core; this module sequences the passes and owns the I/O.
//!
//! This is the walking skeleton — one Source, no batch, no collision policy, no
//! template discovery. Every one of those is a *widening* of a path that works.

use std::path::{Path, PathBuf};

use md2pdf_convert::{convert, ImageProbe, SourceContext};
use md2pdf_domain::Template;
use md2pdf_paths::{output_path, PathBroker, PathError};
use md2pdf_typeset::Typesetter;

use crate::contract::{Command, Emit, Event};

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
        } => match convert_source(&source, &destination, deps, emit) {
            Ok(()) => {}
            // The one place a Result becomes an Event.
            Err(JobError::Compile(message)) => emit(Event::CompilationFailed { message }),
            Err(other) => emit(Event::Failed {
                message: other.to_string(),
            }),
        },
    }
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

fn convert_source(
    source: &Path,
    destination: &Path,
    deps: &Deps,
    emit: Emit,
) -> Result<(), JobError> {
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

    let (decisions, _diagnostic) = deps
        .typesetter
        .probe(&conversion.elements, deps.template)
        .map_err(|e| JobError::Compile(e.to_string()))?;

    let compilation = deps
        .typesetter
        .render(&conversion.elements, deps.template, &decisions)
        .map_err(|e| JobError::Compile(e.to_string()))?;

    let pdf = compilation
        .pdf()
        .map_err(|e| JobError::Compile(e.to_string()))?;

    emit(Event::CompilationSucceeded {
        pages: compilation.page_count(),
    });

    // `write_new`, not `overwrite` (`INV-3`). Collision resolution is 3c2; until the
    // user has actually been asked, refusing is the only honest answer.
    let out = output_path(destination, source, None);
    deps.broker
        .write_new(&out, &pdf)
        .map_err(JobError::OutputUnwritable)?;

    emit(Event::OutputWritten { path: out });
    Ok(())
}
