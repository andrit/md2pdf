# Plan — batch, paths and output (phase 3c2)

**Written:** 2026-08-20 · **Follows:** 3c1, the walking skeleton (`plan-engine.md`).
**Goal:** a directory of Sources converts and writes with the tree mirrored, collisions resolved,
no UI. The widening of a path that already works.

Decisions cite `design/invariants.md`. Where no invariant applies: **build the simple thing.**

---

## Scope

**In:** SourceSet discovery (`walk`), source-tree mirroring with a real SourceRoot, Collision
detection and Resolution, the Diagnostic seal, and the Command/Event surface growing to carry a
batch.

**Out:** template discovery (3e), the attention gate and Overrides (3f), recompilation, any UI.

The four one-line domain stubs — `job.rs`, `collision.rs`, `event.rs`, `escalation.rs` — get filled
here from the glossary, or deleted if nothing needs them.

---

## Decisions

### B1 · Collisions are detected **before any work starts** — the shape of the whole phase

The obvious design has the engine convert files one at a time and stop to ask whenever an
OutputPath is taken. That means a request/response *inside* a stream that is supposed to be one-way
(`INV-8`: commands in, events out), and an out-of-process adapter cannot be called back
synchronously mid-batch.

**It is also unnecessary, and the reason is INV-12.** Because batch output mirrors the source tree,
the map from Source to OutputPath is **injective** — `docs/a/x.md` becomes `out/a/x.pdf` and
`docs/b/x.md` becomes `out/b/x.pdf`. Two Sources in one batch can never collide *with each other*.
The glossary agrees: a Collision is "an OutputPath that **already exists on disk**".

So every Collision in a batch is knowable **up front**, from path arithmetic alone, before a single
document is converted. Pre-flight detection is therefore *complete*, not merely convenient:

```
walk  →  compute every OutputPath  →  which already exist?  →  apply the Resolution  →  convert
```

The user answers before any work happens, the stream stays one-way, and nothing pauses mid-batch.
This is the rare case where the honest simplification is also the more capable one.

> If flattening were ever allowed, this collapses — two Sources could target one OutputPath and
> collisions would become emergent rather than pre-computable. Another reason INV-12 is load-bearing.

### B2 · `BlanketResolution` is required on the command, with no default — `INV-3`

```rust
Command::ConvertBatch {
    source_root: PathBuf,
    destination: PathBuf,
    on_collision: BlanketResolution,   // Skip | Rename | Overwrite — not Option
}
```

No default, because every possible default is wrong: `Overwrite` destroys files without being
asked, and a silent `Skip` looks like success while producing nothing. Requiring the field forces
each adapter to state intent — the CLI as a flag, an interactive adapter as the answer to a prompt
it has already shown.

**`Overwrite` still satisfies INV-3**, because the invariant forbids *silent* overwriting, and a
value carried on the command is a recorded human decision. That is exactly the distinction
`PathBroker::overwrite` was named for.

**Not built now:** a two-phase `PlanBatch` → `CollisionDetected` → `ConvertBatch` handshake, which
is what an interactive adapter will want. Adding a command variant later is a mechanical addition,
so by the gate test it waits until an interactive adapter exists (phase 4).

### B3 · Rename appends `-1`, incrementing until free

`notes.pdf` taken → `notes-1.pdf` → `notes-2.pdf`. Chosen over ` (1)` because it survives shells,
URLs and filesystems without quoting. The search is bounded; an absurd number of collisions is a
`Failed` for that Source rather than an infinite loop.

### B4 · One bad Source does not end the batch

A Source that cannot be read, or whose compilation fails, produces its own failure event and the
batch **continues**. Forty-nine good conversions must not be lost to one malformed file.

`BatchCompleted { converted, flagged, failed }` closes the run — the counts behind
*"47 converted cleanly, 3 need your attention"* (`INV-5`).

### B5 · `clear_files` is now decidable, and belongs here

3c1 deliberately left it alone because *only the caller knows where a Job begins*. In a batch, the
Job boundary is finally visible to the engine: **one `ConvertBatch` is one Job.** So the handler
clears the Typesetter's file map **once, at the start of the batch** — and never between Sources,
because virtual names are keyed by resolved path and a logo shared by fifty documents should be
read and registered once.

The deferral in 3c1 was correct, and this is the moment it pays off.

### B6 · The Diagnostic is sealed in the domain — the §4 seam

`Diagnostic::from_decisions` maps Rungs only; convert-time Compromises (`ImageMissing`,
`ImageSkipped`, `UnsupportedConstruct`) have never had a route in. Sealing them together is pure
logic over values, so it belongs in the domain rather than the engine (`INV-13`):

```rust
impl Diagnostic {
    pub fn seal(convert_compromises: Vec<Compromise>, map: &DecisionMap) -> Self;
}
```

Ordering is by `ElementId.order`, so a Diagnostic reads in document order rather than by which
pass produced it — the user does not care which half of the pipeline conceded.

Then `from_decisions` becomes an internal detail or is removed; there should be **one** way to
build a Diagnostic, or the guarantee that it is complete is only a convention.

### B7 · Walk rules, stated so they are testable

- **Extensions:** `.md` and `.markdown`, case-insensitive. Nothing else is a Source.
- **Hidden entries are skipped** — anything whose name starts with `.`. Walking `.git` would be
  slow and absurd.
- **Symlinks are not followed.** A cycle would hang the walk, and the failure mode is silent.
- **Order is deterministic** — sorted by path. Non-deterministic order would make batch output
  order vary run to run, which undermines `INV-7` in spirit and makes tests flaky.
- **The SourceRoot is recorded on the SourceSet**, because `mirror::output_path` needs it and
  recomputing it elsewhere would be a second source of truth.

### B8 · Events grow a `source`, as E4 predicted

Per-Source events in a batch are meaningless without saying which Source. This is the contract
reshape 3c1 said to expect — and doing it now, while the only consumer is a test, is exactly why
it was deferred rather than guessed at.

```rust
Event::SourceConverted { source: PathBuf, elements, images, compromises }
Event::OutputWritten   { source: PathBuf, path: PathBuf }
Event::SourceSkipped   { source: PathBuf, reason: SkipReason }   // collision, by Resolution
Event::SourceFailed    { source: PathBuf, message: String }
Event::BatchCompleted  { converted: usize, flagged: usize, failed: usize }
```

`Command::ConvertSource` stays: converting one file is not a special case of a batch, it is the
thing a user does most often, and 3c1's tests are the regression suite for it.

---

## Work breakdown

| | Task | Deliverable | Tests |
|---|---|---|---|
| **T19** | `walk` + `mirror` with a real SourceRoot | SourceSet discovery; OutputPath mirroring | extensions, hidden dirs, symlink refusal, deterministic order, nested mirroring |
| **T20** | `collision.rs` + `output.rs` + `Diagnostic::seal` | pre-flight detection, `Resolution`, `BlanketResolution`, rename search | each Resolution; rename increments; existing bytes survive Skip and Rename; sealed Diagnostic carries both halves in document order |
| **T21** | batch `Command`/`Event` + orchestration | `ConvertBatch`, per-Source events, `BatchCompleted`, `clear_files` once | a directory converts with the tree mirrored; one bad Source does not end the run; counts are right |

Each closes with `verify.sh` green.

## Exit criteria

1. A directory of Sources converts; output mirrors the tree and is never flattened (`INV-12`).
2. Every Collision is detected before any conversion begins, and resolved by a stated
   `BlanketResolution` — nothing is silently overwritten (`INV-3`).
3. One unreadable or uncompilable Source does not end the batch; the counts report it.
4. A sealed Diagnostic carries convert-time *and* probe-time Compromises, in document order
   (`INV-4`).
5. `verify.sh` green; `/phase-audit` run or explicitly waived, as in every phase so far.

## Doubts

**D1 · B1 assumes the walk is the only source of Sources.** If a future adapter lets a user
hand-pick files from several directories, the SourceRoot is ambiguous and two picks *could* map to
one OutputPath. Pre-flight detection still works — it would simply need to check the batch against
itself as well as against disk. Recorded so the assumption is visible rather than load-bearing and
forgotten.

**D2 · `Rename` in a batch is the least defensible of the three.** "Rename all" produces
`notes-1.pdf`, `report-1.pdf`, … which is tidy for one file and clutter for fifty. It may be that
`Skip` and `Overwrite` are the only sensible blanket answers and `Rename` is a per-Collision choice
only. Not decided; the CLI in 3d will make it obvious.

**D3 · The batch is sequential, and nothing here says it must be.** Typst compilation is the slow
part and is CPU-bound, so parallelism is the obvious later win — but the Typesetter holds a
long-lived `World` that is explicitly single-threaded (`unsafe impl Send/Sync`, "driven from one
thread at a time"). Parallel batch is therefore **not** a free win, and belongs to a later
performance pass with its own measurements, not to this phase.
