# Commit log — the record of *why*

**Convention (from 2026-08-20):** every substantive commit records its **body** here — the
reasoning, not the diff. `git log` already holds the what; this holds the why in one readable
place, so a decision can be found without archaeology through twenty commit messages.

**Write the entry when staging, not after committing**, so it ships inside the commit it describes.
Missed on the first three commits after this file was created; those entries landed a commit late
and are marked as such.

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

- `<pending>` **feat(paths): walk a SourceSet, recording its SourceRoot** (T19).

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

- `<pending>` **feat: pre-flight collision planning; seal the Diagnostic** (T20).

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
