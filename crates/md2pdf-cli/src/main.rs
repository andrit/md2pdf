//! The composition root: arguments in, events out, exit code back.
//!
//! Holds no conversion logic, no policy, and no state beyond what it hands the engine.

use std::io::Write;
use std::path::Path;
use std::process::ExitCode;

use md2pdf_cli::args::{self, ArgsError, HELP};
use md2pdf_cli::report::Report;
use md2pdf_domain::Template;
use md2pdf_engine::{handle, Command, Deps, Event};
use md2pdf_paths::{PathBroker, PathKind};
use md2pdf_template::{roots, TemplateCatalogue};
use md2pdf_typeset::Typesetter;

fn main() -> ExitCode {
    let options = match args::parse(std::env::args_os().skip(1).collect()) {
        Ok(options) => options,
        Err(ArgsError::HelpRequested) => {
            print!("{HELP}");
            return ExitCode::SUCCESS;
        }
        Err(ArgsError::Invalid(message)) => {
            eprintln!("md2pdf: {message}\n\n{HELP}");
            return ExitCode::from(2);
        }
    };

    let broker = PathBroker::new();
    let typesetter = Typesetter::new();

    // Templates are discovered, never compiled in (`INV-11`). The font check is handed
    // to the catalogue rather than performed by it, because only this layer holds both
    // the FontBook and the catalogue.
    let catalogue = TemplateCatalogue::discover_with_fonts(
        &roots::roots(
            options.templates.clone(),
            roots::beside_binary(),
            &roots::Env::current(),
        ),
        &broker,
        &|font| typesetter.font_families().iter().any(|f| f == font),
    );

    // Reported before anything converts, and to stderr so `--json` stays parseable. A
    // template that will not load is the author's to fix and they cannot fix what they
    // cannot see (`GLOSSARY`, TemplateCatalogue).
    for rejected in &catalogue.rejected {
        eprintln!(
            "md2pdf: ignoring template {}: {}",
            rejected.folder.display(),
            rejected.reason
        );
    }

    let template = match catalogue.get(&options.template) {
        Some(found) => found.template.clone(),
        // The shipped template is missing from disk — a development tree, or an install
        // that lost its `templates/`. Falling back keeps md2pdf working; saying so keeps
        // it honest, because otherwise an edited template that failed to load looks like
        // an edit that did nothing.
        None if options.template == "github-print" => {
            eprintln!(
                "md2pdf: no template found on disk, using the built-in defaults{}",
                if catalogue.found.is_empty() {
                    String::new()
                } else {
                    format!(" (found: {})", catalogue.names().join(", "))
                }
            );
            Template::default()
        }
        None => {
            let known = if catalogue.found.is_empty() {
                "none were found".to_string()
            } else {
                catalogue.names().join(", ")
            };
            eprintln!(
                "md2pdf: no template named {:?} — available: {known}",
                options.template
            );
            return ExitCode::from(2);
        }
    };

    let deps = Deps {
        broker: &broker,
        typesetter: &typesetter,
        template: &template,
    };

    // A directory is a batch; a file is one conversion. Users do not think in commands,
    // and the engine already has both.
    //
    // A path that is not there at all is a **job that could not start**, not a document
    // that failed. Letting it fall through would report "1 document failed" for a typo,
    // which is both wrong and the wrong exit code.
    let command = match broker.kind(&options.path) {
        PathKind::Directory => Command::ConvertBatch {
            source_root: options.path.clone(),
            destination: options.output.clone(),
            on_collision: options.on_collision,
        },
        PathKind::File => Command::ConvertSource {
            source: options.path.clone(),
            destination: options.output.clone(),
        },
        PathKind::Missing => {
            let message = format!("{} does not exist", options.path.display());
            if options.json {
                match serde_json::to_string(&Event::Failed { message }) {
                    Ok(line) => println!("{line}"),
                    Err(e) => eprintln!("md2pdf: {e}"),
                }
            } else {
                eprintln!("md2pdf: {message}");
            }
            return ExitCode::from(2);
        }
    };

    let mut report = Report::default();
    let json = options.json;
    let quiet = options.quiet;
    let mut emit = |event: Event| {
        report.record(&event);

        // Live tracing. A batch can run for a long time, and a report that arrives only
        // at the end is not something you can watch for problems.
        if !json && !quiet {
            if let Some(line) = md2pdf_cli::report::trace(&event) {
                println!("{line}");
            }
        }

        if json {
            // In `--json` mode stdout carries **nothing but JSON**. A mode that is
            // unparseable by the consumer it exists for is worse than not having it,
            // so anything else goes to stderr.
            match serde_json::to_string(&event) {
                Ok(line) => println!("{line}"),
                Err(e) => eprintln!("md2pdf: could not serialise an event: {e}"),
            }
        }
    };

    handle(command, &deps, &mut emit);

    if !json {
        println!("{}", report.summary());
        report_written(&report.written);
    }

    let _ = std::io::stdout().flush();
    ExitCode::from(report.exit_code())
}

fn report_written(written: &[std::path::PathBuf]) {
    match written.len() {
        0 => {}
        1 => println!("Wrote {}", display(&written[0])),
        n => println!("Wrote {n} files"),
    }
}

fn display(path: &Path) -> String {
    path.to_string_lossy().to_string()
}
