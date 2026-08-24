# Commit log — the record of *why*

**Convention (from 2026-08-20):** every substantive commit records its **body** here — the
reasoning, not the diff. `git log` already holds the what; this holds the why in one readable
place, so a decision can be found without archaeology through twenty commit messages.

**Write the entry when staging, not after committing**, so it ships inside the commit it describes.
Missed on the first three commits after this file was created; those entries landed a commit late
and are marked as such.

Which means the hash is not knowable yet, and the entry goes in as `` `<pending>` ``. **Fill it in
at the next staging**, before writing the new entry:

```bash
./scripts/commit-log.sh          # stale? (a pending entry whose commit already exists)
./scripts/commit-log.sh --fix    # fill them in from git log
```

That second half was itself honour-system and failed silently seven times — T19, T20, T21, T23, T25,
T26a and T27 all sat `<pending>` after being committed. Backfilled 2026-08-21. The script matches on
the full subject line, because the task tag alone is ambiguous: "(T27)" named both the commit that
planned the census and the one that built it.

Deliberately **not** wired into `verify.sh`. A pending entry is *correct* between staging and
committing, and the gap between your commit and the next staging is normal working state — gating on
it would turn the build red for bookkeeping at the one moment it is expected.

Newest last. Docs-only and plan commits are listed by subject alone; code commits carry their body.

---

## Phase 2 — foundation

- `1f5ca8d` **feat: foundation — six-crate workspace, Typst anti-corruption layer, verify gate**
  (T1–T4). Phase 0 design plus the first build slice: `md2pdf-typeset` confines the typst crate
  behind domain types, 11 contract tests pin Typst's observable behaviour, typst pinned `=0.15.1`
  with an upgrade runbook, and `scripts/verify.sh` gates boundaries, fmt, clippy, test, build.

## Phase 3a — Conversion, Stage 1 (text)

- `497652c` docs: plan the Conversion crate (T5–T9)
- `da2b622` **feat(convert): compiler-verified escaping; Markup types Element.body** (T5, D5).
  The escape set was derived empirically rather than recalled — 51 of 113 candidates broke before
  it existed, and `#lorem(50)` in document text actually executed. Two failure classes: syntax
  errors, and *silent* lexer-level shorthands (`--`, `...`) that no `#set` can disable. Blanket
  ASCII-punctuation escaping kills both.
- `901d127` **feat(convert): parse markdown into top-level blocks; classify constructs** (T6).
  Parser options settled from the vendored source, closing open item 5. Front matter is recognised
  so it can be dropped — without the flag, `---` reads as a setext underline and the title renders
  as a heading.
- `5c87621` **feat(convert): emit Typst markup per block; absorb footnotes; D1** (T7).
  Function forms, not line-start markers, so the body invariant holds by construction. Footnote
  definitions are the exception to one-block-one-Element. Two bugs the tests caught: code blocks
  double-escaped, and compromises silently dropped when a block emitted an empty body.
- `105ca2e` **fix(typeset): render italics — vendor Source Sans 3 italic faces.**
  The variable font carries a `wght` axis and no italic, so `#emph` silently fell back to upright.
  Static faces were needed because the variable italic reports family `SourceSans3VF` and Typst
  groups faces by family name. Found by looking at a rendered page; no text-based test could see it.
  Added `Compilation::text_runs()` — the project's first assertion about appearance.
- `26987b7` docs: roadmap to v1; plan T8 and amend the conversion plan
- `2316aab` **feat(convert): convert() public API; Conversion type** (T8).
  Total by design — no `Result`, because an unrepresentable construct becomes a `Compromise`.
  `Conversion` was added to the glossary rather than bending the code to a name that did not exist.

## Phase 3b — Images, Stage 2

- `5a12b90` docs: plan phase 3b (images)
- `a9702d8` docs: add task ledger; raise forModeA 001 (custom phases, `task_list` bug)
- `2f59b6b` **feat(typeset): World file map so documents can reference images** (T10).
  `World::file()` served nothing, and an unresolvable file fails the *whole document*. Verified that
  `comemo` does not serve stale bytes when a file is replaced under the same name.
- `3e00720` docs: split 3c into a vertical slice first; plan T11
- `f6da45d` **feat(convert): resolve image paths and policy** (T11).
  Virtual names are `img-<fnv1a(abs path)>.<ext>` — unique, stable so memoisation survives
  recompiles, and deduplicating. Path traversal is *allowed*: this is a local tool on the user's own
  files, and refusing `../assets/logo.png` would break ordinary documents.
- `3613857` docs: plan T12
- `1ea25d4` docs: restate the image failure guard as a manifest-coverage property
- `ba34fec` **feat(convert): wire image resolution through convert and emit** (T12).
  One entry point, so "I have no filesystem" is explicit at the call site. `ImageManifest` lives in
  convert, not the domain — nothing below convert consumes it. Inline images wrap in a `box`;
  without it a sentence broke across three lines.
- `6aab1e0` docs: plan T13 against measured behaviour; split out T14
- `447182a` **fix(typeset): images enter the escalation ladder by scale** (T13).
  The probe stepped *font size* to shrink an atomic element, which cannot move an image — so the one
  class that shrinks losslessly was the only one that never did. Added `Rung::Scale { factor }`
  rather than laundering a ratio through a points field. `reflow: true` is load-bearing: Typst's
  default scale is visual only.
- `ebfd88d` docs: close phase 3b; plan T14

## Phase 3b2 — finish the escalation ladder

- `e7db9e2` **feat(typeset): split the ladder into orientation and reduction** (T14).
  `Reduction::Clip` was unreachable code and rotation never re-measured, so an element too wide even
  for landscape overflowed silently. Two independent axes, because a flat enum cannot express
  "rotated *and* shrunk to 8pt" — and the Element overrides the design specifies are three
  independent toggles.

## Phase 3c1 — the walking skeleton

- `3556221` docs: plan 3c1; correct the `ImageProbe` adapter location
- `16f8c0c` **feat(paths): PathBroker — the one door to the filesystem** (T15).
  `write_new` refuses to replace; replacing requires calling `overwrite` by name, so the guarantee
  that output is never silently overwritten is enforced here rather than remembered upstream.
  Invalid UTF-8 is refused, not lossily converted.
- `d6141eb` docs: add `invariants.md` as the fixed target model
- `6ca9a30` **feat(engine): Command/Event contract** (T16).
  One channel, not two (`INV-8`): `handle` returns nothing and every outcome, including every
  failure, travels as an Event — an adapter may run out-of-process, where a `Result` is invisible.
  One conversion event, not the two the event storm lists, because `convert()` parses and emits
  atomically and the engine never observes the moment between them.
- `8f468d1` docs: plan T17; merge T18 into it
- `70cad0f` **feat(engine): the walking skeleton — one file, disk to disk** (T17).

  > `job.rs` sequences read → convert → register images → probe → render → write, driven by a
  > Command and reporting only through events (INV-8).
  >
  > `BrokerImages` adapts `PathBroker` to convert's `ImageProbe` in the engine, the composition
  > root — `md2pdf-paths` cannot see the trait (E1). `mirror::output_path` computes the OutputPath
  > in the crate that owns paths (INV-9), already taking the SourceRoot 3c2 will pass.
  >
  > `handle` deliberately does not clear the typesetter's file map: that is per-Job, and only the
  > caller knows where a Job begins.
  >
  > `paths::testing::TempDir` is lent to other crates' tests so `std::fs` stays inside the one crate
  > allowed to call it, and dedupes the copy that was in broker's tests.
  >
  > Also fixes `check-boundaries.sh` to ignore comments.

## Housekeeping and planning

- `43c2194` docs: add `commit-log.md` — the record of why, per commit
- `944caa8` docs: plan 3c2 (batch); sequence the boundary guards as T22–T24.
  3c2's shape turns on INV-12: because output mirrors the source tree, Source→OutputPath is
  injective, so two Sources in a batch can never collide with each other and **every** Collision is
  knowable before any conversion begins. Pre-flight detection is therefore complete rather than
  merely convenient, and the event stream stays one-way.

## Guards track

- `ad90e38` **feat: guard determinism in the pure core; golden-hash tests** (T22, G3).
  *(entry written one commit late — the convention above was not yet being followed)*

  > `check-boundaries.sh` gains a third rule: no `SystemTime`, `Instant`, `std::env`, or `rand` in
  > domain/convert/typeset (INV-7). `md2pdf-paths` is deliberately exempt — `testing::TempDir` uses
  > the clock for unique directory names, which never reaches output.
  >
  > Cashes the property in immediately: six golden tests pin the `fnv1a` hash and length of whole
  > PDFs across prose, structure, code, escaping, degraded images and the escalation ladder. `fnv1a`
  > rather than a cryptographic hash — this detects change, it does not resist an adversary — so no
  > dependency is added.
  >
  > Every other test asserts something specific. These assert that *nothing* changed, which is the
  > only way to catch a change nobody thought to look for — how the italic bug survived five green
  > commits.

  **Verified before building:** determinism holds across **separate processes**, not just within
  one — three runs, identical hash. The earlier in-process check would not have caught per-process
  nondeterminism, which is exactly what a stored golden hash is exposed to.

## Phase 3c2 — batch

- `0d98965` **feat(paths): walk a SourceSet, recording its SourceRoot** (T19).

  > `walk` collects `.md` and `.markdown` recursively, case-insensitively, and records the
  > SourceRoot on the `SourceSet` so `mirror::output_path` has one source of truth for where
  > output lands.
  >
  > Four rules chosen to be predictable rather than clever: hidden entries are skipped so `.git`
  > is never descended; **symlinks are not followed**, because a link to an ancestor would hang the
  > walk and a hang reports nothing; order is sorted, because filesystem order would make batch
  > output vary between runs on identical input; and an unreadable directory **fails** the walk
  > rather than being skipped, since converting fewer files than the user has is the failure that
  > looks like success.
  >
  > `SourceSet` lives in `md2pdf-paths`, not the domain — same reasoning as `ImageManifest` in
  > convert: nothing below this crate needs the type. Exposed as `PathBroker::walk` so filesystem
  > access keeps going through the one door a future sandbox must be fitted to, while `walk.rs`
  > keeps the policy.
  >
  > `mirror::output_path` needed no work: T17 built it taking the SourceRoot already.

- `bcb410f` **feat: pre-flight collision planning; seal the Diagnostic** (T20).

  > Collisions are resolved **before any Source is converted**. That is possible because of
  > INV-12: output mirrors the source tree, so Source→OutputPath is injective and every Collision
  > is with something already on disk. The event stream stays one-way (INV-8) and nothing pauses
  > mid-batch waiting for an answer an out-of-process adapter could not give.
  >
  > **Found while building:** mirroring does not make *renamed* outputs unique. `a.md` colliding
  > with an existing `a.pdf` renames to `a-1.pdf` — exactly what a sibling `a-1.md` was about to
  > write. The planner therefore tracks the paths the batch has already claimed, not just what is
  > on disk. Without it two planned writes target one path and one silently destroys the other;
  > pinned by a test and confirmed by removing the check.
  >
  > `Collision`, `Resolution` and `BlanketResolution` live in the domain — unlike `SourceSet` and
  > `ImageManifest` — because they cross the Command boundary and an adapter must be able to
  > construct one.
  >
  > `Diagnostic::seal` closes the §4 seam: convert-time Compromises had no route into a Diagnostic
  > at all, so INV-4 held in the code and broke at the join. Sealed output is ordered by element,
  > because the user cares where in their document a concession happened, not which pass made it.
  >
  > Also serialises `verify.sh`'s test step (`-j 1`). Linking two typst-laden test binaries at once
  > exhausts memory on a 4 GB machine; it has bitten three times and gets likelier with every crate
  > that gains an integration test. Clippy has already compiled the graph, so the step is mostly
  > linking and serialising it costs little.

- `2f84795` **feat(engine): convert a directory, mirroring the tree** (T21) — closes 3c2.

  > `ConvertBatch` walks a SourceRoot, plans every write up front, and converts. `on_collision` is
  > required with no default: `OverwriteAll` would destroy files nobody was asked about, and a
  > silent `SkipAll` looks like success while producing nothing.
  >
  > Per-Source events gained a `source` — the contract reshape 3c1 predicted, done now while the
  > only consumer is a test rather than guessed at then. `SourceFailed` is separate from `Failed`:
  > one document failing inside a healthy Job is not the Job failing, and the batch continues.
  > `SkipReason` distinguishes "you asked me to skip" from "every name was taken", which feel the
  > same and are not.
  >
  > `clear_files` is finally called, and only here: one batch is one Job. Clearing between Sources
  > would re-register a shared logo for every document and discard the `comemo` hit the long-lived
  > World exists to keep — the deferral in 3c1 paying off exactly as intended.
  >
  > `Diagnostic::seal` is now on the live path, so `flagged` counts documents where either half of
  > the pipeline conceded (INV-4). Flagged is not failed: a document with a missing image converts,
  > and is counted separately (INV-5).
  >
  > 3c1's skeleton tests pass unchanged and are the regression suite for `ConvertSource`, which
  > keeps its own path rather than becoming a batch of one.

- `045f622` **feat: guard against network dependencies** (T23, G1).

  > `check-boundaries.sh` gains a fourth rule: no network-capable crate anywhere in the dependency
  > graph, and no `std::net` in source (INV-1).
  >
  > Greps **`Cargo.lock`, not the manifests** — and the negative control proved why that matters.
  > Adding `ureq` to one crate pulled in **`rustls` transitively**; `rustls` appears nowhere in any
  > manifest, so a manifest grep would have missed half the violation it was meant to catch. The
  > realistic accident is never someone adding HTTP deliberately.
  >
  > Verified clean at 308 crates, so the guard starts from a true state rather than grandfathering
  > an exception.
  >
  > **What it buys beyond protection:** "no network, ever" is the product's most marketable
  > property — privacy, zero running cost, works offline. It was a promise held by memory. It is now
  > an audit that runs on every build, which is a materially stronger thing to say on a download
  > page.
  >
  > Honest limit, stated in the script itself: a denylist is never complete, so this raises the cost
  > of adding a network stack without making it impossible. `cargo-deny` would do it properly by
  > understanding the graph, and is not used because a gate that needs a network fetch to install is
  > not a gate that runs everywhere.

## Phase 3d — the CLI adapter

- `9060cf8` **feat(cli): the md2pdf binary, with tracing that explains itself** (T25).

  > `md2pdf <path> -o <dir>` — a file converts, a directory converts recursively. `pico-args`
  > rather than `clap`: it is already in the graph via typst, so it costs no new crates, where
  > `clap` would add ~10 to spare a 15-line help string.
  >
  > `--json` emits line-delimited events and **nothing else on stdout**, which makes the CLI the
  > *proof* of INV-8 rather than a claim about it: a test runs the real binary and parses its stdout
  > back into `Event` values, crossing the out-of-process boundary for the first time.
  >
  > Exit 0 / 1 / 2. Flagged documents exit **0** — INV-5 says flagged is not failed, and a script
  > author could reasonably expect otherwise, so it is pinned by a test and stated in `--help`.
  >
  > **Found by running 146 real documents: the diagnostic was being thrown away.** `job.rs` sealed a
  > Diagnostic, used it to decide whether a document was flagged, and discarded the contents — so a
  > run reported "70 need your attention" and could explain two. Every escalation-ladder decision
  > died at the boundary. INV-4 held inside the engine and broke on the way out.
  >
  > Fixed with `Event::DiagnosticSealed` (a name the event storm already had), carrying the complete
  > set from both halves. `SourceConverted` now carries a *count* rather than half the truth, so
  > there is one complete answer instead of two partial ones.
  >
  > Also: the 146-document run printed nothing for 31 minutes. Per-document tracing is now on by
  > default (`-q` to suppress), and compromises are **described rather than counted** — "4 elements
  > shrunk, 2 rotated, 1 CLIPPED" instead of "7 compromises". A number is not something a user can
  > act on.
  >
  > `PathBroker::kind` was added so the CLI stops calling `Path::is_dir` itself (path access outside
  > `md2pdf-paths`, INV-9), and so a path that does not exist is a **job that could not start**
  > rather than a document that failed — a typo was reporting "1 document failed" and exiting 1.

- `e227eba` **feat: tables reflow instead of clipping** (T26a).

  > Adds `Reduction::Reflow`, one rung before `Clip`. An `Element` may now carry an alternate body
  > that cannot overflow, and `emit` gives every table one: the same cells with
  > `columns: (1fr, ..)` instead of `columns: N`.
  >
  > **Why, from the corpus:** 25 elements across 9 real documents were being clipped, and every
  > single one was a table — including one needing 71% of its natural width. `#table(columns: N)`
  > sizes columns to content, so such a table has exactly one possible width and the ladder's only
  > lever is shrinking the whole thing; past the floor that meant losing the right-hand columns.
  > GitHub wraps cell text instead, and so do we now.
  >
  > Measured on the corpus: **25 clipped → 0 clipped, 25 reflowed.** Everything else unchanged —
  > 163 shrunk, 84 rotated, 70 flagged — so the change touched only the elements that were losing
  > content. Confirmed by eye on a real document: full-size readable text, wrapped cells, every
  > column present.
  >
  > `Clip` stays reachable for Elements with no alternate — an image cannot reflow.
  >
  > Recorded as `CompromiseKind::Reflowed` even though the output is usually *better* than the
  > original: it is still a departure from the authored shape, and INV-4 says every judgment call
  > is reported.
  >
  > Not applied to a table nested inside a blockquote, which is emitted into the parent's body and
  > is not separable — the same nested-atomics ceiling already recorded in `classify.rs`.

- `ba6e60a` **test: a census of the ladder's decisions** (T27).

  > Makes `compromise-mechanism.md` §9 — *every ladder change re-measures the baseline* — a build
  > failure instead of a rule people must remember. It was honour-system, which is the failure mode
  > the document exists to warn about: nothing enforced it, nothing noticed when it was skipped, and
  > §6 was a snapshot with no history, so a drift spread over three changes was invisible.
  >
  > Golden hashes for **decisions** rather than bytes. They pin a PDF's bytes and would catch a
  > ladder change as an opaque mismatch on whichever fixture happened to hold a wide table; they
  > cannot say *"rotations went from 84 to 12"*, which is the thing worth seeing.
  >
  > Three parts. A committed corpus of 12 fixtures, one per rung and edge — unlike `documents/`,
  > which is untracked, borrowed, and may be deleted, and **a baseline that cannot be reproduced is
  > not a baseline**. A committed `design/ladder-census.txt`. And a test that regenerates it and
  > compares, so the file is the record and the test is only the tripwire — they cannot disagree,
  > and updating the baseline is a reviewable edit rather than a number changed inside a test.
  >
  > **The census file's git history is the log.** Every ladder change leaves a diff naming which
  > kinds moved, beside the reasoning here. No separate log format to invent or maintain.
  >
  > Records kinds, not sizes: 8.5pt → 8.0pt is tuning, a shrink becoming a rotation is behaviour,
  > and only the second should turn the build red.
  >
  > **The tripwire was proved to fire**, not assumed to — a guard that has never failed is not known
  > to be one. A row appended to `rotate.md` reported `was: 1 rotated, 1 shrunk / now: 1 rotated,
  > 2 shrunk`, then the regenerate command and the instruction to commit the new reading *with* the
  > change. That last part is the lesson the golden hashes already taught.
  >
  > **Two fixtures were not doing what their names claimed** — written from reasoning about widths,
  > and wrong. `shrink-slight.md` was clean (429pt against 483pt available) and `shrink-floor.md`
  > stopped at 7.5pt. Both re-measured: now 9.0pt and exactly 7.0pt. This also exposed something
  > reasoning would not have predicted — **a table's width is not linear in font size**, since
  > padding and gutters do not scale, so the portrait shrink band tops out near 649pt rather than
  > the 690pt that `483 / 0.7` implies, and a wider table rotates instead of reaching the floor.
  >
  > Finding that needed per-element sizes, which the census deliberately omits. Rather than the
  > throwaway id-matching probe R4 describes, `describe_the_fixtures` was added as the inspection
  > counterpart to the tripwire.
  >
  > `tests/walking_skeleton.rs` became `tests/walking_skeleton/main.rs` with `census.rs` beside it.
  > Every `tests/*.rs` is a binary statically linking ~250 typst crates and two linking at once
  > exhaust a 4 GB machine, so a new integration test joins an existing target as a module — the
  > arrangement `md2pdf-convert/tests/compiler/` already uses, and for the same reason. The target
  > name is unchanged, so the commands in `docs/development.md` still work.
  >
  > Built **before** T26b deliberately: the census is the instrument that measures whether the
  > reordering worked, and building it afterwards means judging the change with the thing it
  > altered.
- `2acd04e` docs: backfill commit-log hashes; script the half that was honour-system
- `bc1aee7` docs: render the T26b comparison sheets; withdraw the reordering recommendation
- `b59efac` docs: record class C — when the rendered page overrules the plan

- `15d0a43` **feat(convert): a reflow alternate worth choosing** (T26a2).

  > The alternate emitted `columns: (1fr, ...)` — equal shares — so a column holding "P1" was given
  > the same width as a column holding a paragraph. That is why the T26b comparison sheets found deep
  > shrinking beating reflow on every real table: not the ladder's order, the alternate itself.
  >
  > Now a column shares the leftover width when it is at least half as wide as the widest column in
  > the same table, and sizes to its content otherwise. **Relative, not absolute** — the corpus offers
  > no natural cut to pick: per-column max cell length runs p10 = 8, p50 = 29, p90 = 84 characters
  > across 1215 columns in 434 tables, a smooth spread. The widest column always qualifies, so at
  > least one always absorbs and the alternate never collapses into the content-sized body it exists
  > to replace.
  >
  > Vetted before building, because "does a mixed `auto`/`1fr` table still fit" is a fact about Typst:
  > three table shapes × four specs, rendered, with ink past the right margin detected in the raster.
  > The rule fits everywhere the old one fits. Two things fell out of that experiment. **`1fr`
  > measures as 0.0pt in infinite space** — fractional columns collapse when unconstrained, so the
  > probe *cannot* measure the alternate, which is why nothing guards this rung. And **the "always
  > fits" guarantee was already false**: with unbreakable content, all-`1fr` overflows the page and
  > the cells overprint into mush while the ladder reports `Reflowed`. 261 real table cells hold a
  > 30+ character token — URLs and absolute paths. Unchanged by this commit, raised as **T29**, and
  > the risk register no longer implies the rung is sound.
  >
  > **Looked at, which is the point of the task.** Today's 7pt shrink beside the new reflow of the
  > same table: 10pt, one line per row but for one. And the rotation pair that killed O2a now reads
  > well in portrait with no page turn. **Reflow now beats deep shrinking** — R2's original claim,
  > right conclusion for the wrong reason. T26b is unparked.
  >
  > **A prediction that was wrong, and worth more than the ones that were right.** The plan said
  > `the_escalation_ladder_is_unchanged` would move. No golden moved: that fixture shrinks to 7.5pt
  > and never reaches Reflow, so **no golden covered the reflowed rendering at all** — this change
  > could have landed silently while being entirely about how a reflowed table looks. Closed by
  > `a_reflowed_table_is_unchanged` and by `golden_at`, which asserts which rung was chosen before
  > hashing. A golden cannot otherwise say what it covers.
  >
  > The census is unchanged, as predicted — no rung moved, so the improvement stayed inside the rung.
  >
  > Widths are measured from the emitted markup rather than the rendered text; a `ponytail:` records
  > the ceiling. A link's URL inflates its column, which at worst grants a narrow column `1fr`.
- `b87e587` docs: re-vet the ladder order against measured overflow exposure (T26b)

- `f4388ee` **feat(typeset): reflow before shrinking small, and before rotating** (T26b).

  > Closes **R2**. A table that would shrink below the comfort floor now wraps at full size instead,
  > and wraps rather than taking a landscape page of its own (option O2a). One new `Floors` field, one
  > branch in the probe's text-atomic path; the image branch is untouched.
  >
  > **Measured on 146 real documents: 163 shrunk → 46, 25 reflowed → 152, 84 rotated → 0**, 70 flagged
  > either way, 146 valid PDFs. The 41 elements rendering at 7.0–7.5pt that started R2 are gone. The
  > flagged count not moving is the correct result, not a disappointment — the same elements are
  > compromised, just better — and it means **T26b does nothing for R1**.
  >
  > The simulation predicted 46 / 152 / 0 / 70 before any code was written, and the census diff was
  > predicted line for line. Both matched exactly, including `shrink-slight.md` at exactly 9.0pt
  > staying shrunk, which pins `>=` rather than `>`.
  >
  > **Sequenced deliberately after T26a2**, and that order was the whole difficulty. Reordering first
  > — on R2's original reasoning, which was written before anything was rendered — would have moved
  > 117 elements onto a rung whose alternate gave a "P1" column the same width as a paragraph. The
  > census would have recorded that as progress.
  >
  > **Known cost, measured rather than hoped:** every table in the corpus was rendered reflowed and
  > checked for ink past the margin. 3 elements newly overflow, joining 4 that already did — **T29**,
  > next. A textual proxy would have put the exposure at 85 and shelved this change; only rendering
  > gives 7.
  >
  > Two fixtures renamed because the change made their names untrue: `rotate.md` →
  > `reflow-instead-of-rotate.md`, `shrink-floor.md` → `below-comfort-reflows.md`.
  >
  > Both goldens moved with reasons recorded: the ladder fixture shrank at 7.5pt and now reflows, and
  > the reflow fixture now reflows in portrait rather than landscape. `golden_at` confirmed the rung
  > held, which is the check that was missing when T26a2 landed silently.
  >
  > **R5 gets sharper, not milder.** Rotation and clipping are both zero, and 152 of 198 compromised
  > elements now sit on one rung — so a defect in reflow is a defect nearly everywhere, and
  > `Reflowed` reads the same whether the table wrapped beautifully or ran off the page.
- `7ea6bd1` docs: add the flags register; plan T29 against measured token locations

- `3612437` **feat(convert): break long runs in the reflow alternate; build the overflow oracle** (T29).

  > **Partial. 7 elements would have overflowed; 4 still do.** Committed as a real improvement and an
  > unfinished one rather than reported as success — the residual is flag **F8**, scheduled as T29b.
  >
  > `Reflow` sits immediately before `Clip` because the alternate is supposed to always fit, and it
  > does not: with unbreakable content it runs off the page and the cells overprint into mush, while
  > the ladder records `Reflowed` and every consumer reads that as handled.
  >
  > Long runs now get a zero-width break opportunity — **in the alternate only**. The body keeps its
  > runs unbroken, which is load-bearing: the probe measures the body to choose a rung, so a body that
  > could wrap mid-token would measure narrow and be given the wrong decision. Both halves are pinned
  > by test.
  >
  > Built by emitting the table's cells a **second time** with breaking on, rather than rewriting the
  > first pass's markup. Both forms then come from one code path and cannot drift, and nothing has to
  > parse Typst to find where a break may safely go. The second pass's compromises and images are
  > discarded, or one missing image in a table would be reported twice.
  >
  > **Two measurements decided the design, and both contradicted what I had written.** Long runs come
  > from *inline code* (196 of 256), not links (15) — the earlier claim that markup surgery inside
  > `#link` made this hard was wrong. And a zero-width space breaks inside `#raw`, which is the
  > load-bearing fact and not obvious, since raw text is otherwise rendered verbatim.
  >
  > **The threshold had to become relative to column count.** A flat 24 characters let
  > `below-comfort-reflows.md` spill 21pt — caught by the new oracle on its first run, in a fixture
  > that had been committed and green for two commits. A flat 16 would have newly broken ~930 ordinary
  > words in the corpus, so the limit is `96 / columns`: a 20-character word has room in a two-column
  > table and none in a nine-column one.
  >
  > **The oracle is the deliverable that outlives the fix** (flag **F1**). Typst does not report
  > overflow — it lays out content and anything too wide simply extends past the margin. The only way
  > to ask is to render and look at the pixels, written as throwaway code four times before this.
  > It checks every page, and ships with a negative control that strips the breaks back out and
  > asserts it still fires; a guard that never fails is not a guard.
  >
  > **Without it this would have shipped as done.** Rungs unchanged, census unchanged, every test
  > green, 146 valid PDFs — and four tables still running off the page. That is exactly the failure
  > this project keeps finding: the record says handled, the output is wrong, and nothing fails.
  >
  > Also flagged: **F7** `DEBT`, zero-width spaces reach the PDF text layer, so a path copied out of a
  > reflowed table pastes with invisible characters in it.

- `e949671` **fix(convert): weighted fractional columns in the reflow alternate** (T29b).

  > **Diagnosed before fixing**, and the diagnosis was the work. All four tables still running off
  > the page shared one shape: several `auto` columns and a single `1fr`. **`auto` in Typst means
  > size to content and do not shrink**, so the autos summed past the page and the lone `1fr` was
  > left with negative space. Neither candidate mechanism written down in F8 was quite right — it was
  > not that runs fell under the break threshold, and not fragmented text events.
  >
  > Fractional columns **divide** the width available, so a table of them cannot exceed it — the
  > guarantee `Reflow` needs to sit one rung before `Clip`. Weighting them keeps the proportionality
  > `auto` was there to provide: `(1fr, 1fr, 1fr, 6fr)` rather than `(auto, auto, auto, 1fr)`.
  > Verified on all four failing tables, at two weight scales, before any of it was written.
  >
  > **Third design for this spec, and each failure explains the next** — recorded in the doc comment
  > so the code does not read as arbitrary. Equal `1fr` gave "P1" a paragraph's width; `auto` plus
  > `1fr` fixed the proportions and broke the fitting guarantee; weighted `fr` is proportional *and*
  > bounded.
  >
  > **Improvement, stated honestly: severity fell about 6× — 3/47/53/55pt down to 9pt and 5pt — and
  > the count did not move. Still 4 of 152.** One document is new to the list, so something crossed
  > the threshold the wrong way while others improved, and that is not yet explained. F8 stays open.
  >
  > The residual has a different cause, now identified: **`fr` bounds the table, not the cell.** A
  > 20-character unbreakable run in a column with a ~54pt share still spills, because the break
  > threshold is uniform while the shares are deliberately unequal. T29c derives each column's limit
  > from its weight.
  >
  > **No golden moved, and that was the third instance of the same gap.** Both ladder goldens hold
  > *uniform* tables, where every equal-share spec renders identically — `(1fr × 6)` and `(6fr × 6)`
  > are the same layout. So neither noticed three successive changes to the thing they were assumed
  > to cover. Added `a_proportional_reflow_is_unchanged`, lopsided by construction, which moves when
  > the weighting does.
  >
  > Five tests were rewritten rather than renumbered: they pinned the `auto`/`1fr` design. One is now
  > `every_column_is_fractional_so_the_table_cannot_exceed_the_page`, which asserts the invariant
  > instead of the arrangement, and fails if an `auto` column ever creeps back.

- `6844894` **fix(convert): break limits per column, from the weights that size them** (T29c).

  > **1 of 152 tables now overflows, down from 4 — and from 7 before this line of work started.**
  > The last is a 2pt overhang in one document, recorded in **F8** rather than chased.
  >
  > A single break threshold per table assumed every column had equal room. That stopped being true
  > the moment T29b made the spec deliberately lopsided: in `(1fr, 1fr, 1fr, 6fr)` a weight-1 column
  > holds a ninth of the width, while the old threshold — `96 / 4` — gave it room for twenty-four
  > characters. Measured, that is what was still running off the page.
  >
  > Each column's limit now comes from its own share, computed by `column_weights`, which also
  > produces the column spec. One function feeding both, so the two cannot disagree about how wide a
  > column is meant to be. **`fr` bounds the table; this bounds the cell.**
  >
  > One test changed shape, and the reason is a sign the rule works rather than a patch to keep it
  > green: its long identifier sat beside four one-character columns, so that column now takes weight
  > 6 of 10 — a 290pt share — and correctly needs no break at all. Breaking is only exercised when the
  > run sits in a *narrow* column, so the fixture now has a prose column crowding it. The old test
  > would have kept passing for the wrong reason.
  >
  > Three fixes, three diagnoses, and each one contradicted the previous plan's expectation:
  > breaks were not enough, `auto` refused to shrink, and a uniform threshold could not survive a
  > lopsided spec. None of it would have been visible without the overflow oracle (**F1**) — rungs,
  > census, goldens and valid-PDF counts were unchanged at every step.

- `3578b8d` **feat: guard the engine against learning what a window is** (T24, G2).

  > `INV-8` — the whole out-of-process-adapter design rests on the engine never depending on a UI
  > toolkit, and until now that was protected by nothing but the absence of a UI. The moment phase 4
  > starts, *"just import egui here for a second to get the preview working"* is a one-line change
  > with no alarm attached, and it is exactly the kind of shortcut that survives because it works.
  >
  > Two greps over the five core crates: dependency keys in their manifests, and references in their
  > source. Scoped to those five rather than the workspace, because the adapter is an in-workspace
  > crate and is *supposed* to link a toolkit — which resolves the open question the guards document
  > raised before this could be built.
  >
  > **All three cases exercised, not just the failing ones.** A toolkit in `md2pdf-engine/Cargo.toml`
  > fails; `egui::Context` in `job.rs` fails; the same dependency added to `md2pdf-cli` passes. The
  > third is the one that keeps the guard alive — one that also blocked the legitimate case would be
  > switched off the first time it got in the way.
  >
  > **Honest limit, recorded in the script:** it sees direct dependencies, not a toolkit arriving
  > transitively. That is a deliberate difference from the network rule, which greps `Cargo.lock`
  > because *there* the realistic accident is transitive. Here it is a direct import.
  >
  > Cheap now, expensive later, which is the gate test in `invariants.md`: one grep before the
  > adapter exists, versus cleaning up whatever leaked against a working UI nobody wants to break.
- `5c5088d` docs: plan the floors (T26c) — four of five have no observable effect

- `f5ef1fc` **fix(convert): break long runs at separators, not by counting** (F9).

  > `user_organiz | ation_roles` and `submissions. | content_flag | s` — visible in the T26c pairs,
  > and the main reason the reflowed rendering lost the comparison on sight rather than on substance.
  >
  > Breaks now go after `_ . / - : \ ,` first, falling back to counting only where a run has none,
  > which is how a reader would break the name anyway: `user_ / organization_ / roles`.
  >
  > The counting fallback also moved from `limit / 2` to `limit`. Half a column's width chopped words
  > that would have fitted whole — the first attempt still produced `user_organi|zation_roles`
  > *because* the fallback fired between separators. `limit` is the column's actual character
  > capacity, so counting now breaks exactly when the line is full.
  >
  > **Overflow held at 1 of 152**, checked with the oracle rather than assumed — looser breaking is
  > the kind of change that could have quietly let a table back off the page.
  >
  > Anticipated as doubt D2 of `plan-t29.md` (*"a break inserted mid-identifier is ugly"*), observed
  > in evidence, then fixed. Both goldens moved and were regenerated deliberately.
- `3506772` docs: plan T30 (12pt base); F3 is a missing comemo::evict, not a constraint

- `78915ee` **feat: the base size is 12pt, and break limits come from the template** (T30, closes F10).

  > **`base_size_pt: 10.0` was never chosen** — no doc comment, nothing in the repository recording a
  > reason, carried forward from the font spike's *"10pt body"* specimen. The template is called
  > `github-print`; GitHub's body text is 16px ≈ 12pt. It also put the measure at 96 characters per
  > line, far outside the 45–75 band that reads comfortably; 12pt puts it at 80.
  >
  > **Flagged documents go 48% → 64%, and that is the point rather than a regression.** Shrinks go
  > 46 → 122, reflow is unchanged at 152, corpus overflow is unchanged at 1. Every one of the 76 new
  > shrinks renders **at least as large as it did** — a table that fitted at 10pt shrinks to at least
  > 10pt — and over half land at 10.0–11.5pt, at or above the base we used to ship. What changed is
  > that the shortfall is now reported. R1's watch instruction is re-scoped with it: the goal is
  > distance from optimal, not the flagged count, which was only ever a proxy while the baseline was
  > one nobody had to choose.
  >
  > **`CHARS_ACROSS = 96` is gone.** Its ponytail said *"upgrade: pass the Template into conversion"*
  > and the ceiling had been reached — 96 is A4 at 10pt, so every break limit was 20% too generous
  > the moment the base moved. `SourceContext` now carries the `Template`, and `Template::chars_in`
  > answers for a given width rather than only the full line.
  >
  > **The oracle then caught a defect that had been green for eight commits.** `reflow-hostile.md`
  > spilled 4pt at 12pt and not at 10pt, and the base size was not the cause: `break_limits` never
  > charged a cell for its **inset**. It divided the characters that fit the full text width among
  > the columns, while each cell also pays 5pt of padding per side — measured by rendering a
  > one-column table and finding where the glyph ink starts. Twelve columns spend 120 of 483 points
  > on inset, so the limit was a quarter too generous, and 10pt had the slack to hide it.
  >
  > The deduction happens **after** each column takes its share, not before. Pooling it first spreads
  > the cost in proportion to weight and under-charges narrow columns — the F8 shape exactly: for
  > `(1fr, 6fr)` the narrow column really holds 59pt (9 characters), and pooling would say 66pt (10).
  >
  > **The census did not move, and that is the finding.** No fixture changed rung. The ladder's
  > chosen size is *absolute* — a table fits at the size it fits at, whichever base it started from —
  > so the plan's prediction that fixtures would need re-tuning was wrong, and is superseded in place.
  > What moved is what the names mean: 12 → 9 is a quarter off where 10 → 9 was a tenth, so
  > `shrink-slight.md` becomes `shrink-to-comfort-floor.md`, the role it actually plays.
  >
  > All seven goldens regenerated — every rendering changes size, which is expected and deliberate.
  > One unit test stopped pinning an exact run of broken text and now asserts the property it was
  > written for (no separator is passed over), because the exact string encoded the old base.
  >
  > **Fourth defect of one shape in four tasks** (T29, T29b, T29c, T30): `convert` estimating layout
  > arithmetic that `typeset` could measure exactly, each one found by rendering a page and counting
  > pixels rather than by a test. Recorded as `plan-base-size.md` D5, with the operator's scoping
  > decision — a 12-column table is not a case worth engineering for in a markdown converter.

- `9b7fa27` docs: the cost rule, the typeset-move options, and two blind instruments

  > Three things, one theme: a decision that was mine to make was not.
  >
  > **`design/invariants.md` gains "Cost is not a reason to decline an option"** — the operator's
  > rule, after `plan-base-size.md` declined *move break insertion into typeset* in four words
  > ("most correct, largest change") and T29, T29b, T29c and T30 then each fixed the defect it would
  > have closed once. Options get a real description and a score against the goal; cost is the
  > operator's to weigh, correctness is mine to argue.
  >
  > **`plan-typeset-move.md`** is the first plan written to it — three options, described in full and
  > scored, with the recommendation stated as a disagreement with the request rather than hidden as a
  > silent substitution.
  >
  > **[measured] a 5-minute spike moved corpus overflow 1 → 0**: offer a break after every character
  > of an already-over-long token instead of counting to an estimated limit. F8's residual 2pt
  > overhang, which four tasks could not close, closes — because the spike stops predicting where the
  > line ends and lets Typst choose. `U+200B` is an opportunity, not a break, so extra ones inside a
  > token that fits change nothing; the affected token set does not grow, only the granularity within
  > it; and it applies to the reflow alternate alone. Reverted, not shipped — it is Option 1, and the
  > choice is the operator's.
  >
  > **`scripts/commit-log.sh` was blind to every code entry.** Its regex required the line to end
  > after the bold subject, but code entries carry a task suffix — `**feat: …** (T29c).` — so eight
  > committed entries sat `<pending>` while it reported "hashes are current". Second time it has been
  > blind to a whole form, so it is now a `verify.sh` step: safe, because it reports only entries
  > whose commit already exists.
  >
  > **`design/evidence/` is now gitignored** (operator's call): the rasters are proof for questions
  > already answered in prose, and the ladder generates them faster than anything else in the repo.
  >
  > **Then the operator asked to move past assumption on Option 2, and both its numbers were wrong.**
  > `estimate.rs` measures them. Cost: 1636ms to measure every cell of all 375 reflow-capable tables
  > in release — **+5.5% on the 29.7s batch, an upper bound**, against the "noticeable slowdown" I
  > had warned of. Benefit: the estimated column share sits **3.6 points** from the measured one at
  > the median, 9.1 at p90, 31.4 at worst, with 7% of columns off by more than a tenth of the
  > table's width. So Option 2 is cheaper than I priced it *and* buys less than "correct sizing"
  > implies — usually-right to right, not broken to working. Both corrections came from measuring,
  > and both were mine to have measured before writing the option.
  >
  > A fourth possibility surfaced while measuring: Typst can size its own columns via `#context` and
  > `measure()`. It renders — and the probe then reports natural width `0.0`, because `#context`
  > defers to layout and the ladder picks rungs from that number. Disqualifying for a body, possibly
  > fine for the alternate, which is never measured. Recorded in the spike and left unchased.

  > ---
  >
  > **The Option 4 spike, same commit.**

  > **Asked to see whether the fourth option changes anything. It does, and not the way I expected.**
  >
  > **The mechanism is sound.** A `#context` alternate that measures its own cells and sets its own
  > column widths: census unchanged (so the alternate really is never measured for rung choice, the
  > open question), corpus overflow 1 → 0, and **free** — min-of-5 on a 56-document subset with 54
  > reflowed tables, `base 6636ms · opt4 6744ms`, inside the noise.
  >
  > **Then I rendered the page and it was unreadable.** `measure()` returns a cell's *unbroken*
  > natural width, so a prose cell measures as one long line and raw proportionality gave it ~95% of
  > the table; three numeric columns collapsed and **overprinted each other**.
  >
  > **The oracle called that page clean** — overprinting inside a table puts no ink past the margin.
  > Raised as **F11**, and it demotes every "corpus overflow is 0" in `plan-typeset-move.md`,
  > Option 1's included, from proof to necessary-but-not-sufficient. `look.rs` is the answer for now:
  > raster a page to a hand-rolled PNG so a human can look. `check-boundaries.sh` caught it using
  > `std::fs` on the first try, which is INV-9 doing its job on a throwaway.
  >
  > **With the existing policy applied to measured widths, the output is identical to today's.**
  > `WIDEST_WEIGHT` quantises weights into six buckets, so a column must be off by ~17 points before
  > the spec moves; the measured error is 3.6 at the median. Corpus-wide the spec would change for
  > **148 of 375 tables (39%)** — not a no-op — but on the table holding the corpus's worst error
  > (31.4 points) it changed nothing visible.
  >
  > **And the page named the real defect, which no option in the plan addresses.** Column 1 breaks
  > mid-word — `Micros/ervices`, `Capabi/lity`, `Regist/ry` — while three columns holding `79`, `145`
  > and `14` sit at the same width. That is `column_weights`' `.max(1)` floor: a numeric column gets
  > the same minimum share as a text column needing far more, and six buckets cannot tell them apart.
  > **The lever is the weighting policy, not the width source** — which is what Options 2, 3 and 4 all
  > replace. Recommendation revised in the plan accordingly.

- `3ecf400` docs: rewrite T31a — let Typst break the lines, stop starving the columns

  > **The option numbering is gone, and that is the point of the rewrite.** Four numbered options
  > invited *"so it's 1 and 4?"* when the answer is one task. Options 2 and 3 were measured out;
  > Option 1 was contradicted by the operator's direction — `Execution` already counts as over-long
  > in a narrow column, so break-anywhere would shred ordinary words at *every character*, the
  > opposite of what was asked. Option 4's mechanism survives with a different allocation rule. The
  > comparison is kept at the foot of the plan because it was acted on.
  >
  > **[measured] 45% of every break we insert lands inside a normal English word** — 579 ordinary
  > words against 712 identifiers, paths and hashes: `Executio|n`, `Conformi|st`, `Analytic|s`. Not
  > Typst's doing. Typst wraps at spaces, which is why body prose is never shredded; we break these
  > words before Typst ever sees them, because `offer_breaks` shreds anything longer than a
  > per-column character limit and a narrow column's limit is 5–9.
  >
  > **The two symptoms are one defect.** A column narrower than its longest word must either break
  > the word or overflow. We chose to break, and then blamed the breaking. The change is min-content
  > columns — no column narrower than its widest unbreakable token, remainder shared by demand —
  > computed in-document with `#context`, so Typst breaks the lines and no width is estimated.
  >
  > **The lesson.** All four options replaced the width *source*; the rendered page said the defect
  > was the allocation *policy*, a floor that cannot tell `79` from `Microservices`. Four options,
  > none addressing it, because all four were designed from the estimate's history rather than from
  > the output. The picture cost an hour and moved the plan further than three spikes did.

- `8f98450` **feat(convert): let Typst break the lines; stop starving the columns** (T31a, closes F8).

  > **579 ordinary words broken → 0. Corpus overflow 1 → 0. The estimator is deleted.**
  >
  > We were shredding ordinary English words — `Executio|n`, `Conformi|st`, `Analytic|s` — and
  > **[measured]** 45% of every break we inserted landed inside one. Not Typst's doing: Typst tracks
  > the line, measures the next word and wraps at the space, which is why body prose was never
  > shredded. We broke those words *before Typst ever saw them*, because `offer_breaks` split
  > anything longer than a per-column character limit and a narrow column's limit was five.
  >
  > **The two symptoms were one defect.** A column narrower than its longest word must either break
  > the word or overflow the page. We chose to break, and then blamed the breaking.
  >
  > The alternate now sizes itself, in the document, from measured content: min-content first — no
  > column narrower than its widest unbreakable run — with the remainder shared by how much more each
  > column wants. `convert` supplies the words, Typst supplies the widths, and the columns stay `fr`
  > so the total still cannot exceed the page whatever the arithmetic does (T26a2's guarantee).
  >
  > **`column_weights`, `column_spec`, `WIDEST_WEIGHT` and `cell_widths` are gone.** The character-
  > count estimator that T29, T29b, T29c and T30 each patched is deleted rather than corrected.
  >
  > **The fallback took two attempts, and the first was measured wrong.** A table whose minimums will
  > not fit side by side must break something — `below-comfort-reflows.md`, six columns of a
  > 22-character run wanting 798pt of a 423pt table. Sharing the shortfall in proportion to demand
  > still broke 103 ordinary words, because one column's hash drags every other column's share down.
  > Capping instead — the largest ceiling under which the table fits, leaving columns already below
  > it untouched — takes it to zero.
  >
  > **Looked at the page, not just the counts** (F11 says the oracle cannot see a table overprinting
  > itself). `project-state.md`: every word whole, numeric columns narrowed to what they hold, the
  > prose column given the slack, 24 pages down to 22.
  >
  > One golden moved — the only one that renders an alternate; the other six are byte-identical.
  > Census unchanged: this changes how the alternate is laid out, not which rung is chosen. Five
  > tests that pinned the old spec strings were rewritten to assert the invariants instead, and three
  > break fixtures were re-sized because a 41-character identifier now legitimately fits.
  >
  > **Not re-measured:** §6's corpus baseline. The release batch is OOM-killed at ~141 of 146 on this
  > machine (**F3**) — it killed the unmodified binary too, so it is the box, not this change.

- `103671f` **feat: the comfort floor is 10pt, chosen by eye; the dead floors are resolved** (T26c, closes F4).

  > **Five pairs of real corpus tables rendered both ways and looked at.** The turnover is between
  > 9.0 and 10.0: at 8.0 and 9.0 a wrapped table at full size beats a shrunk one; at 10.0 the shrink
  > is clean and keeps one line per row; at 11.0 the reduction is imperceptible and reflow wraps a
  > one-row table for nothing. So `table_comfort_pt` is **10.0**.
  >
  > **It moved because the base moved.** The floor is absolute but comfort is relative: 9.0 was a
  > tenth off a 10pt base and is a quarter off 12pt. `plan-base-size.md` D2 predicted exactly this
  > and left it for this task. **[assumed]** 9.5 is defensible on the same evidence — one number,
  > one step wide, and the pairs are in `plan-floors.md`.
  >
  > **[measured]** shrinks 122 → 76, reflows 152 → **198**, flagged unchanged at 93 of 146 — the
  > same documents, on different rungs, which is exactly what the count cannot see. **0 of the 198
  > overflow.** Moving 46 more elements onto the reflow rung would have been a gamble before T31a
  > made that rung safe; D1 checked it with the oracle rather than assuming.
  >
  > **The plan's own candidates were stale and the task had to re-derive them.** It proposed pairs at
  > 8.0/8.5/9.0, chosen when the base was 10pt — all four of those now reflow, so the question had
  > moved up with the base and two more pairs were rendered above it.
  >
  > **F4 closed by resolving all five floors, not one.** `prose_pt` and `code_pt` are **deleted** —
  > **[measured]** never read, since `for_class` only ever receives `Table` — and deleted now, while
  > `Floors` is still a Rust type rather than a `template.toml` migration for fields that never did
  > anything. `table_pt` is kept and re-documented as what it is: a bound on the probe's scan and the
  > clip marker's size, not a policy. `image_scale` is marked untuned, because no image in the corpus
  > reaches the ladder and inventing fixtures to tune it would be calling a guess evidence.
  >
  > **Looking found a defect in T31a.** The 9.0 reflow put `integration.destination.verified` over
  > its column border: the min-content token was measured as plain text and drawn in mono, which is
  > wider. Fixed by carrying the token's *markup* into the measurement. The oracle saw nothing
  > (**F11**), the census was unchanged, every test was green — second defect in two tasks that only
  > a rendered page could show.
  >
  > `shrink-to-comfort-floor.md` lost two characters per cell to land back on the boundary at 10.0.
  > It is the only fixture that shrinks, so `the_census_covers_every_compromise_kind` went red when
  > it reflowed — the census noticing it had lost sight of a rung, which is what it is for. Goldens
  > unmoved: no golden fixture's table sits in the 9.0–10.0 band.
  >
  > §6 re-measured with `recount_the_baseline` rather than the batch, which is OOM-killed at ~141 of
  > 146 on this machine (**F3**, and it kills an unmodified binary too).

- `<pending>` **feat(typeset): evict the `comemo` cache; the batch was never bounded** (T31, closes F3).

  > **Five sightings, and this is the first time anyone plotted the curve.** One long-lived
  > `Typesetter`, the corpus a document at a time, RSS from the kernel:
  >
  > ```
  >              no eviction     evict(5)
  >   1 doc         111 MiB       111 MiB
  >  41 docs       1629 MiB       563 MiB
  > 131 docs       3162 MiB       634 MiB
  > 146 docs        killed        690 MiB
  > ```
  >
  > **~24 MiB per document, unbounded — and never the `Typesetter`'s to free.** `comemo`'s cache is
  > process-global, keyed by memoised call rather than held by the `World`, so both earlier "fixes"
  > (a fresh `Typesetter` per element, then per probe) were placebo: they changed how the work was
  > scoped, not what was retained. Nothing in this project had ever called `comemo::evict`; typst's
  > own CLI does, and we inherited the cache without inheriting the eviction.
  >
  > **The batch that has been dying now completes: 146 documents, exit 0, 29.7s** — which is also the
  > timing §6 last recorded, so the documented baseline recipe works again.
  >
  > **It costs nothing.** A document recompiled steadily takes **4ms at age 5 against 5ms with no
  > eviction at all**, so the memoisation the long-lived `World` exists for is fully preserved. Only
  > `evict(0)` destroys it, and completely — 470ms, a hundred times slower. The dial has a wide flat
  > region and 5 sits in the middle of it. The call lives in `job.rs` rather than inside `render`
  > because the cadence is the caller's: a batch wants it per document, 3f's recompile loop will not.
  >
  > **`comemo` is pinned `=0.5.1`**, exactly as the typst crates are. A skew between the crate typst
  > memoises through and the crate we evict through would silently evict nothing.
  >
  > **The gate asserts eviction *works*, not that memory is bounded, and two failed attempts are why.**
  > `VmRSS` is process-wide and cargo runs a binary's tests concurrently: an absolute 60 MiB bound
  > read 81 MiB of other tests' allocations, and a paired evicting-vs-not comparison collapsed to
  > 82-vs-92 because the first half left the allocator warm for the second. The curve is real and
  > measured; it is not measurable from inside a concurrent test binary. So `eviction_still_evicts`
  > checks the ~100x recompile difference a cleared cache produces — robust to any machine noise, and
  > it would have caught both placebo fixes and any future version skew. Proved to fire.
