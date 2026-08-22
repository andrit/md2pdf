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

- `<pending>` **feat(convert): a reflow alternate worth choosing** (T26a2).

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

- `<pending>` **feat(typeset): reflow before shrinking small, and before rotating** (T26b).

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

- `<pending>` **feat(convert): break long runs in the reflow alternate; build the overflow oracle** (T29).

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
