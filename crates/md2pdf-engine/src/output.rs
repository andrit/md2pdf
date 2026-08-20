//! Collision detection, `Resolution`, `BlanketResolution`, and writing.
//!
//! Collision policy is prompt-to-rename, never silent overwrite; batch needs the
//! blanket "apply to all" answer or it becomes unusable at 50 files.
//!
//! ## Everything is decided before anything is converted
//!
//! The obvious design converts one Source at a time and stops to ask whenever an
//! OutputPath is taken. That would put a request/response *inside* a one-way event
//! stream (`INV-8`), and an out-of-process adapter cannot be called back mid-batch.
//!
//! It is also unnecessary, because of `INV-12`. Output mirrors the source tree, so the
//! map from Source to OutputPath is **injective** — two Sources can never produce the
//! same OutputPath. Every Collision is therefore a collision with something *already on
//! disk*, and all of them are knowable from path arithmetic before a single document is
//! converted.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use md2pdf_domain::{BlanketResolution, Collision, Resolution};
use md2pdf_paths::{output_path, PathBroker, SourceSet};

/// How a planned output should be written.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WriteMode {
    /// Nothing is there. `PathBroker::write_new`.
    New,
    /// Something is there and the user said to replace it. `PathBroker::overwrite`.
    Replace,
}

/// One Source, and where its PDF will go.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannedWrite {
    pub source: PathBuf,
    pub output: PathBuf,
    pub mode: WriteMode,
    /// The Collision this write resolved, if it resolved one. Carried so the batch can
    /// report *"renamed because notes.pdf existed"* rather than an unexplained name.
    pub resolved: Option<Collision>,
}

/// What the batch will do, decided before it does any of it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OutputPlan {
    pub writes: Vec<PlannedWrite>,
    /// Sources that will not be converted, and the Collision that stopped them.
    pub skipped: Vec<Collision>,
}

/// Give up looking for a free name. A directory with this many `notes-N.pdf` files is
/// a runaway, not a workflow, and an unbounded search would hang instead of reporting.
const MAX_RENAME_ATTEMPTS: u32 = 1000;

/// Decide every write for a batch, resolving Collisions up front.
pub fn plan(
    set: &SourceSet,
    destination: &Path,
    on_collision: BlanketResolution,
    broker: &PathBroker,
) -> OutputPlan {
    let mut plan = OutputPlan::default();

    // Paths this batch has already claimed. Needed because a *rename* can land on an
    // output another Source in the same batch is going to write — `notes.md` colliding
    // with an existing `notes.pdf` renames to `notes-1.pdf`, which is exactly what a
    // sibling `notes-1.md` was going to produce. Mirroring makes the *original* outputs
    // unique; it says nothing about the suffixed ones.
    let mut claimed: HashSet<PathBuf> = HashSet::new();

    for source in &set.sources {
        let output = output_path(destination, source, Some(&set.root));

        if !taken(&output, &claimed, broker) {
            claimed.insert(output.clone());
            plan.writes.push(PlannedWrite {
                source: source.clone(),
                output,
                mode: WriteMode::New,
                resolved: None,
            });
            continue;
        }

        let collision = Collision {
            source: source.clone(),
            output: output.clone(),
        };

        match on_collision.resolution() {
            Resolution::Skip => plan.skipped.push(collision),
            Resolution::Overwrite => {
                claimed.insert(output.clone());
                plan.writes.push(PlannedWrite {
                    source: source.clone(),
                    output,
                    mode: WriteMode::Replace,
                    resolved: Some(collision),
                });
            }
            Resolution::Rename => match free_name(&output, &claimed, broker) {
                Some(renamed) => {
                    claimed.insert(renamed.clone());
                    plan.writes.push(PlannedWrite {
                        source: source.clone(),
                        output: renamed,
                        mode: WriteMode::New,
                        resolved: Some(collision),
                    });
                }
                // Nowhere to put it. Skipping is the only answer that does not
                // overwrite, and INV-3 outranks converting this one file.
                None => plan.skipped.push(collision),
            },
        }
    }

    plan
}

fn taken(path: &Path, claimed: &HashSet<PathBuf>, broker: &PathBroker) -> bool {
    claimed.contains(path) || broker.exists(path)
}

/// `notes.pdf` → `notes-1.pdf` → `notes-2.pdf`, until one is free.
///
/// `-N` rather than ` (N)` because it survives shells, URLs and filesystems without
/// quoting.
fn free_name(output: &Path, claimed: &HashSet<PathBuf>, broker: &PathBroker) -> Option<PathBuf> {
    let stem = output.file_stem()?.to_string_lossy().to_string();
    let extension = output.extension()?.to_string_lossy().to_string();
    let parent = output.parent()?;

    (1..=MAX_RENAME_ATTEMPTS).find_map(|n| {
        let candidate = parent.join(format!("{stem}-{n}.{extension}"));
        (!taken(&candidate, claimed, broker)).then_some(candidate)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use md2pdf_paths::testing::TempDir;

    /// Build a SourceSet without walking, so these tests are about planning only.
    fn set(root: &Path, names: &[&str]) -> SourceSet {
        SourceSet {
            root: root.to_path_buf(),
            sources: names.iter().map(|n| root.join(n)).collect(),
        }
    }

    fn outputs(plan: &OutputPlan) -> Vec<String> {
        plan.writes
            .iter()
            .map(|w| w.output.file_name().unwrap().to_string_lossy().to_string())
            .collect()
    }

    #[test]
    fn with_nothing_in_the_way_everything_is_a_new_write() {
        let tmp = TempDir::new("plan-clear");
        let broker = PathBroker::new();
        let sources = set(tmp.path(), &["a.md", "b.md"]);

        let plan = plan(
            &sources,
            &tmp.join("out"),
            BlanketResolution::SkipAll,
            &broker,
        );

        assert_eq!(outputs(&plan), vec!["a.pdf", "b.pdf"]);
        assert!(plan.writes.iter().all(|w| w.mode == WriteMode::New));
        assert!(plan.skipped.is_empty());
    }

    #[test]
    fn the_tree_is_mirrored_in_the_plan() {
        let tmp = TempDir::new("plan-mirror");
        let broker = PathBroker::new();
        let sources = set(tmp.path(), &["api/ref.md", "guide/ref.md"]);

        let plan = plan(
            &sources,
            &tmp.join("out"),
            BlanketResolution::SkipAll,
            &broker,
        );

        // Two Sources with the same basename must not collide — INV-12 is what makes
        // pre-flight detection complete.
        assert_eq!(plan.writes.len(), 2);
        assert_eq!(plan.writes[0].output, tmp.join("out/api/ref.pdf"));
        assert_eq!(plan.writes[1].output, tmp.join("out/guide/ref.pdf"));
        assert!(plan.skipped.is_empty());
    }

    #[test]
    fn skip_all_leaves_the_existing_file_alone() {
        let tmp = TempDir::new("plan-skip");
        let broker = PathBroker::new();
        tmp.write("out/a.pdf", b"ORIGINAL");
        let sources = set(tmp.path(), &["a.md", "b.md"]);

        let plan = plan(
            &sources,
            &tmp.join("out"),
            BlanketResolution::SkipAll,
            &broker,
        );

        assert_eq!(outputs(&plan), vec!["b.pdf"]);
        assert_eq!(plan.skipped.len(), 1);
        assert_eq!(plan.skipped[0].output, tmp.join("out/a.pdf"));
    }

    #[test]
    fn overwrite_all_plans_a_replace_and_says_what_it_resolved() {
        let tmp = TempDir::new("plan-over");
        let broker = PathBroker::new();
        tmp.write("out/a.pdf", b"ORIGINAL");
        let sources = set(tmp.path(), &["a.md"]);

        let plan = plan(
            &sources,
            &tmp.join("out"),
            BlanketResolution::OverwriteAll,
            &broker,
        );

        assert_eq!(plan.writes[0].mode, WriteMode::Replace);
        assert!(
            plan.writes[0].resolved.is_some(),
            "an overwrite must record the Collision it answered"
        );
    }

    #[test]
    fn rename_all_suffixes_and_keeps_counting() {
        let tmp = TempDir::new("plan-rename");
        let broker = PathBroker::new();
        tmp.write("out/a.pdf", b"ORIGINAL");
        tmp.write("out/a-1.pdf", b"ALSO ORIGINAL");
        let sources = set(tmp.path(), &["a.md"]);

        let plan = plan(
            &sources,
            &tmp.join("out"),
            BlanketResolution::RenameAll,
            &broker,
        );

        assert_eq!(outputs(&plan), vec!["a-2.pdf"]);
        assert_eq!(plan.writes[0].mode, WriteMode::New);
    }

    /// The case mirroring does **not** protect against, found while building this.
    ///
    /// `a.md` collides with an existing `a.pdf` and renames to `a-1.pdf` — which is
    /// precisely what the sibling `a-1.md` is about to write. Original outputs are
    /// unique by construction; *suffixed* ones are not.
    #[test]
    fn a_rename_never_lands_on_another_planned_output() {
        let tmp = TempDir::new("plan-selfclash");
        let broker = PathBroker::new();
        tmp.write("out/a.pdf", b"ORIGINAL");
        let sources = set(tmp.path(), &["a.md", "a-1.md"]);

        let plan = plan(
            &sources,
            &tmp.join("out"),
            BlanketResolution::RenameAll,
            &broker,
        );

        let names = outputs(&plan);
        let unique: HashSet<&String> = names.iter().collect();
        assert_eq!(
            names.len(),
            unique.len(),
            "two planned writes target the same path: {names:?}"
        );
    }

    #[test]
    fn a_runaway_rename_search_skips_rather_than_hangs() {
        let tmp = TempDir::new("plan-runaway");
        let broker = PathBroker::new();
        tmp.write("out/a.pdf", b"x");
        for n in 1..=MAX_RENAME_ATTEMPTS {
            tmp.write(&format!("out/a-{n}.pdf"), b"x");
        }
        let sources = set(tmp.path(), &["a.md"]);

        let plan = plan(
            &sources,
            &tmp.join("out"),
            BlanketResolution::RenameAll,
            &broker,
        );

        assert!(plan.writes.is_empty());
        assert_eq!(plan.skipped.len(), 1, "should skip rather than overwrite");
    }

    #[test]
    fn planning_writes_nothing() {
        // Pre-flight means pre-flight: deciding must not touch the disk.
        let tmp = TempDir::new("plan-pure");
        let broker = PathBroker::new();
        let sources = set(tmp.path(), &["a.md"]);

        let _ = plan(
            &sources,
            &tmp.join("out"),
            BlanketResolution::RenameAll,
            &broker,
        );
        assert!(
            !tmp.join("out").exists(),
            "planning created the destination"
        );
    }
}
