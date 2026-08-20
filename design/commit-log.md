# Commit log — the record of *why*

**Convention (from 2026-08-20):** every substantive commit records its **body** here — the
reasoning, not the diff. `git log` already holds the what; this holds the why in one readable
place, so a decision can be found without archaeology through twenty commit messages.

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
