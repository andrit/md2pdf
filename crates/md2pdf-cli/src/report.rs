//! Events in, a summary and an exit code out.
//!
//! Separate from `main` so the mapping from what happened to what the user is told can
//! be tested without running a binary or touching a disk.

use std::path::PathBuf;

use md2pdf_domain::{Compromise, CompromiseKind};
use md2pdf_engine::Event;

/// What the run amounted to.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Report {
    pub converted: usize,
    /// Converted, but md2pdf made a judgment call the user should see (`INV-5`).
    /// **Flagged is not failed.**
    pub flagged: usize,
    pub skipped: usize,
    pub failed: usize,
    /// The Job never started — an unwalkable root. Distinct from documents failing.
    pub job_failed: bool,
    pub written: Vec<PathBuf>,
    /// Documents needing attention, so the summary can name them rather than only
    /// counting them (`INV-5`: the gate fires where a decision was made).
    pub attention: Vec<String>,
}

impl Report {
    pub fn record(&mut self, event: &Event) {
        match event {
            Event::SourceConverted { .. } => self.converted += 1,
            // The complete set, from both halves of the pipeline. Described rather than
            // counted: "3 compromises" tells the user nothing they can act on.
            Event::DiagnosticSealed {
                source,
                compromises,
            } => {
                self.flagged += 1;
                self.attention
                    .push(format!("{} — {}", display(source), describe(compromises)));
            }
            Event::OutputWritten { path, .. } => self.written.push(path.clone()),
            Event::SourceSkipped { source, .. } => {
                self.skipped += 1;
                self.attention.push(format!(
                    "{} — skipped, output already exists",
                    display(source)
                ));
            }
            Event::SourceFailed { source, message }
            | Event::CompilationFailed { source, message } => {
                self.failed += 1;
                self.attention
                    .push(format!("{} — failed: {message}", display(source)));
            }
            Event::Failed { message } => {
                self.job_failed = true;
                self.attention.push(format!("could not start: {message}"));
            }
            // Authoritative for a batch: the engine counted, so trust it over the
            // per-Source tally accumulated above.
            Event::BatchCompleted {
                converted,
                flagged,
                skipped,
                failed,
            } => {
                self.converted = *converted;
                self.flagged = *flagged;
                self.skipped = *skipped;
                self.failed = *failed;
            }
            Event::CompilationSucceeded { .. } => {}
        }
    }

    /// 0 everything converted · 1 something failed · 2 the job could not start.
    ///
    /// **Flagged documents exit 0.** That is a decision, not an oversight: `INV-5` says
    /// flagged is not failed — the document converted and merely needs attention. A
    /// script author could reasonably expect otherwise, which is why it is written down
    /// here and in `--help`.
    pub fn exit_code(&self) -> u8 {
        if self.job_failed {
            2
        } else if self.failed > 0 {
            1
        } else {
            0
        }
    }

    /// The human summary. Shaped after the design's own example: "47 converted
    /// cleanly, 3 need your attention."
    pub fn summary(&self) -> String {
        if self.job_failed {
            return self
                .attention
                .first()
                .cloned()
                .unwrap_or_else(|| "could not start".into());
        }

        let clean = self.converted.saturating_sub(self.flagged);
        let mut out = format!("{clean} converted cleanly");

        let mut notes = Vec::new();
        if self.flagged > 0 {
            notes.push(format!("{} need your attention", self.flagged));
        }
        if self.skipped > 0 {
            notes.push(format!("{} skipped", self.skipped));
        }
        if self.failed > 0 {
            notes.push(format!("{} failed", self.failed));
        }
        if !notes.is_empty() {
            out.push_str(&format!(", {}", notes.join(", ")));
        }
        out.push('.');

        for line in &self.attention {
            out.push_str(&format!("\n  {line}"));
        }
        out
    }
}

fn display(path: &std::path::Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| path.to_string_lossy().to_string())
}

/// Say what actually happened, grouped, so a line reads like a sentence.
///
/// "3 compromises" is a number; "2 tables rotated, 1 image missing" is something the
/// user can act on — which is the whole point of the diagnostic (`INV-4`).
fn describe(compromises: &[Compromise]) -> String {
    let mut shrunk = 0;
    let mut scaled = 0;
    let mut rotated = 0;
    let mut clipped = 0;
    let mut missing = 0;
    let mut skipped = 0;
    let mut unsupported: Vec<&str> = Vec::new();

    for c in compromises {
        match &c.kind {
            CompromiseKind::ShrunkToFloor { .. } => shrunk += 1,
            CompromiseKind::Scaled { .. } => scaled += 1,
            CompromiseKind::Rotated => rotated += 1,
            CompromiseKind::Clipped => clipped += 1,
            CompromiseKind::ImageMissing => missing += 1,
            CompromiseKind::ImageSkipped => skipped += 1,
            CompromiseKind::UnsupportedConstruct { construct } => {
                unsupported.push(construct.split(':').next().unwrap_or("construct"))
            }
        }
    }

    let mut parts = Vec::new();
    let mut push = |n: usize, one: &str, many: &str| {
        if n == 1 {
            parts.push(format!("1 {one}"));
        } else if n > 1 {
            parts.push(format!("{n} {many}"));
        }
    };
    push(shrunk, "element shrunk", "elements shrunk");
    push(scaled, "image scaled", "images scaled");
    push(rotated, "element rotated", "elements rotated");
    push(clipped, "element CLIPPED", "elements CLIPPED");
    push(missing, "image missing", "images missing");
    push(skipped, "remote image skipped", "remote images skipped");
    if !unsupported.is_empty() {
        let n = unsupported.len();
        unsupported.sort_unstable();
        unsupported.dedup();
        parts.push(format!("{n} unsupported ({})", unsupported.join(", ")));
    }

    if parts.is_empty() {
        "flagged".into()
    } else {
        parts.join(", ")
    }
}

/// A one-line trace of a Source as it finishes, for watching a long batch.
///
/// A 146-document run took 31 minutes and printed nothing until the end. Silence for
/// half an hour is not a report.
pub fn trace(event: &Event) -> Option<String> {
    match event {
        Event::OutputWritten { source, .. } => Some(format!("  ok   {}", display(source))),
        Event::DiagnosticSealed {
            source,
            compromises,
        } => Some(format!(
            "  flag {} — {}",
            display(source),
            describe(compromises)
        )),
        Event::SourceSkipped { source, .. } => {
            Some(format!("  skip {} — output exists", display(source)))
        }
        Event::SourceFailed { source, message } | Event::CompilationFailed { source, message } => {
            Some(format!("  FAIL {} — {message}", display(source)))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use md2pdf_domain::{Compromise, CompromiseKind, ElementId};
    use md2pdf_engine::contract::SkipReason;

    fn source(name: &str) -> PathBuf {
        PathBuf::from(format!("/docs/{name}"))
    }

    fn converted(name: &str, _compromises: usize) -> Event {
        Event::SourceConverted {
            source: source(name),
            elements: 5,
            images: 0,
            compromises: 0,
        }
    }

    fn sealed(name: &str, kinds: &[CompromiseKind]) -> Event {
        Event::DiagnosticSealed {
            source: source(name),
            compromises: kinds
                .iter()
                .enumerate()
                .map(|(i, kind)| Compromise {
                    id: ElementId::new(i as u32, "body"),
                    kind: kind.clone(),
                    page: None,
                })
                .collect(),
        }
    }

    #[test]
    fn a_clean_run_exits_zero_and_says_so() {
        let mut r = Report::default();
        r.record(&converted("a.md", 0));
        r.record(&converted("b.md", 0));
        assert_eq!(r.exit_code(), 0);
        assert_eq!(r.summary(), "2 converted cleanly.");
    }

    #[test]
    fn flagged_documents_still_exit_zero() {
        // INV-5: flagged is not failed. The document converted.
        let mut r = Report::default();
        r.record(&converted("a.md", 0));
        r.record(&converted("b.md", 0));
        r.record(&sealed(
            "b.md",
            &[CompromiseKind::Rotated, CompromiseKind::ImageMissing],
        ));
        assert_eq!(r.exit_code(), 0, "a compromise is not a failure");
        assert!(r
            .summary()
            .starts_with("1 converted cleanly, 1 need your attention."));
        assert!(
            r.summary()
                .contains("b.md — 1 element rotated, 1 image missing"),
            "the summary must say what happened, not count it: {}",
            r.summary()
        );
    }

    #[test]
    fn a_failed_document_exits_one() {
        let mut r = Report::default();
        r.record(&converted("a.md", 0));
        r.record(&Event::SourceFailed {
            source: source("b.md"),
            message: "not valid UTF-8".into(),
        });
        assert_eq!(r.exit_code(), 1);
        assert!(r.summary().contains("1 failed"));
        assert!(r.summary().contains("not valid UTF-8"));
    }

    #[test]
    fn a_job_that_never_started_exits_two() {
        let mut r = Report::default();
        r.record(&Event::Failed {
            message: "/nope could not be read".into(),
        });
        assert_eq!(r.exit_code(), 2);
        assert!(r.summary().contains("could not start"));
    }

    #[test]
    fn skips_are_reported_loudly_rather_than_looking_like_success() {
        // The reason `skip` is a safe default: it is never silent.
        let mut r = Report::default();
        r.record(&Event::SourceSkipped {
            source: source("a.md"),
            output: PathBuf::from("/out/a.pdf"),
            reason: SkipReason::Collision,
        });
        assert_eq!(r.exit_code(), 0);
        assert!(r.summary().contains("1 skipped"));
        assert!(r.summary().contains("a.md — skipped"));
    }

    #[test]
    fn compromises_are_described_not_counted() {
        // "3 compromises" is a number. The user needs something actionable.
        let mut r = Report::default();
        r.record(&sealed(
            "a.md",
            &[
                CompromiseKind::Rotated,
                CompromiseKind::Rotated,
                CompromiseKind::Clipped,
            ],
        ));
        assert!(
            r.summary()
                .contains("2 elements rotated, 1 element CLIPPED"),
            "{}",
            r.summary()
        );
    }

    #[test]
    fn a_batch_total_overrides_the_running_tally() {
        // Both are emitted; counting both would double.
        let mut r = Report::default();
        r.record(&converted("a.md", 0));
        r.record(&converted("b.md", 0));
        r.record(&sealed("b.md", &[CompromiseKind::Rotated]));
        r.record(&Event::BatchCompleted {
            converted: 2,
            flagged: 1,
            skipped: 0,
            failed: 0,
        });
        assert_eq!(r.converted, 2);
        assert_eq!(r.flagged, 1);
    }
}
