//! Arguments in, `Options` out. No I/O, so it is testable without a filesystem.

use std::ffi::OsString;
use std::path::PathBuf;

use md2pdf_domain::BlanketResolution;

pub const HELP: &str = "\
md2pdf — convert markdown to PDF, locally.

USAGE:
    md2pdf <PATH> -o <DIR> [OPTIONS]

    <PATH>  a .md file, or a directory to convert recursively

OPTIONS:
    -o, --output <DIR>        where PDFs are written (required)
        --on-collision <HOW>  skip | rename | overwrite   [default: skip]
        --template <NAME>     which template to render with  [default: github-print]
        --templates <DIR>     an extra directory to look for templates in
        --attention           list what needed a judgment call; write nothing
        --json                emit line-delimited events on stdout
    -q, --quiet               only the final summary, no per-document lines
    -h, --help                print this help

TEMPLATES:
    A template is a folder holding template.toml. md2pdf looks in --templates,
    then your config directory, then beside the binary; the first to supply a
    name wins, so copying the shipped folder into your config directory and
    editing it replaces the original.

        Linux    ~/.config/md2pdf/templates
        macOS    ~/Library/Application Support/md2pdf/templates
        Windows  %APPDATA%\\md2pdf\\templates

    A folder that will not load is reported with the reason, never dropped.

COLLISIONS:
    An output that already exists is never silently replaced. The default is
    to skip it; `rename` writes alongside as name-1.pdf; `overwrite` replaces
    it, and only because you said so.

ATTENTION:
    --attention converts nothing. It reports, per document, every place md2pdf
    made a judgment call — what it did, and what you could allow instead. It is
    the same list the app will offer to act on.

EXIT CODES:
    0  everything converted (documents needing attention still exit 0)
    1  one or more documents failed
    2  the job could not start
";

/// What the CLI was asked to do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Options {
    pub path: PathBuf,
    pub output: PathBuf,
    pub on_collision: BlanketResolution,
    pub json: bool,
    pub quiet: bool,
    /// Report judgment calls instead of converting. Writes nothing.
    pub attention: bool,
    /// Which template to render with. Resolved against the catalogue, not here.
    pub template: String,
    /// An extra directory to search first. Mostly for tests and one-off runs.
    pub templates: Option<PathBuf>,
}

#[derive(Debug)]
pub enum ArgsError {
    /// `--help` was asked for. Not a failure.
    HelpRequested,
    Invalid(String),
}

/// Parse an argument vector, excluding the program name.
///
/// `--on-collision` defaults to `skip`. B2 forbade a default on the *Command* so that
/// every adapter must state its intent — the CLI stating `skip` **is** that intent, and
/// `skip` is the only answer that never destroys anything (`INV-3`).
pub fn parse(argv: Vec<OsString>) -> Result<Options, ArgsError> {
    let mut args = pico_args::Arguments::from_vec(argv);

    if args.contains(["-h", "--help"]) {
        return Err(ArgsError::HelpRequested);
    }

    let attention = args.contains("--attention");
    let json = args.contains("--json");
    let quiet = args.contains(["-q", "--quiet"]);

    let on_collision = match args
        .opt_value_from_str::<_, String>("--on-collision")
        .map_err(|e| ArgsError::Invalid(e.to_string()))?
    {
        None => BlanketResolution::SkipAll,
        Some(v) => match v.as_str() {
            "skip" => BlanketResolution::SkipAll,
            "rename" => BlanketResolution::RenameAll,
            "overwrite" => BlanketResolution::OverwriteAll,
            other => {
                return Err(ArgsError::Invalid(format!(
                    "--on-collision must be skip, rename or overwrite (got {other:?})"
                )))
            }
        },
    };

    let template: String = args
        .opt_value_from_str("--template")
        .map_err(|e| ArgsError::Invalid(e.to_string()))?
        .unwrap_or_else(|| "github-print".to_string());

    let templates: Option<PathBuf> = args
        .opt_value_from_str("--templates")
        .map_err(|e| ArgsError::Invalid(e.to_string()))?;

    let output: PathBuf = args
        .value_from_str(["-o", "--output"])
        .map_err(|_| ArgsError::Invalid("-o/--output is required".into()))?;

    let path: PathBuf = args
        .free_from_str()
        .map_err(|_| ArgsError::Invalid("a source file or directory is required".into()))?;

    let leftover = args.finish();
    if !leftover.is_empty() {
        return Err(ArgsError::Invalid(format!(
            "unexpected arguments: {leftover:?}"
        )));
    }

    Ok(Options {
        path,
        output,
        on_collision,
        json,
        quiet,
        attention,
        template,
        templates,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_str(args: &[&str]) -> Result<Options, ArgsError> {
        parse(args.iter().map(OsString::from).collect())
    }

    #[test]
    fn a_source_and_an_output_are_enough() {
        let opts = parse_str(&["notes.md", "-o", "out"]).expect("parses");
        assert_eq!(opts.path, PathBuf::from("notes.md"));
        assert_eq!(opts.output, PathBuf::from("out"));
        assert!(!opts.json);
        assert!(!opts.quiet, "per-document tracing is on by default");
    }

    #[test]
    fn quiet_suppresses_per_document_lines() {
        let opts = parse_str(&["notes.md", "-o", "out", "-q"]).expect("parses");
        assert!(opts.quiet);
    }

    #[test]
    fn collisions_default_to_skip_because_it_destroys_nothing() {
        let opts = parse_str(&["notes.md", "-o", "out"]).expect("parses");
        assert_eq!(opts.on_collision, BlanketResolution::SkipAll);
    }

    #[test]
    fn each_collision_answer_maps_across() {
        for (flag, expected) in [
            ("skip", BlanketResolution::SkipAll),
            ("rename", BlanketResolution::RenameAll),
            ("overwrite", BlanketResolution::OverwriteAll),
        ] {
            let opts =
                parse_str(&["notes.md", "-o", "out", "--on-collision", flag]).expect("parses");
            assert_eq!(opts.on_collision, expected, "for {flag}");
        }
    }

    #[test]
    fn an_unknown_collision_answer_is_refused_by_name() {
        let err = parse_str(&["notes.md", "-o", "out", "--on-collision", "clobber"])
            .expect_err("must refuse");
        match err {
            ArgsError::Invalid(m) => assert!(m.contains("clobber"), "{m}"),
            other => panic!("expected Invalid, got {other:?}"),
        }
    }

    #[test]
    fn the_output_directory_is_required() {
        assert!(matches!(
            parse_str(&["notes.md"]),
            Err(ArgsError::Invalid(_))
        ));
    }

    #[test]
    fn a_source_is_required() {
        assert!(matches!(
            parse_str(&["-o", "out"]),
            Err(ArgsError::Invalid(_))
        ));
    }

    #[test]
    fn help_is_not_a_failure() {
        assert!(matches!(
            parse_str(&["--help"]),
            Err(ArgsError::HelpRequested)
        ));
        assert!(matches!(parse_str(&["-h"]), Err(ArgsError::HelpRequested)));
    }

    #[test]
    fn json_is_a_flag() {
        let opts = parse_str(&["notes.md", "-o", "out", "--json"]).expect("parses");
        assert!(opts.json);
    }

    #[test]
    fn stray_arguments_are_reported_rather_than_ignored() {
        // Silently ignoring an argument is how a user ends up believing a flag worked.
        assert!(matches!(
            parse_str(&["a.md", "b.md", "-o", "out"]),
            Err(ArgsError::Invalid(_))
        ));
    }
}
