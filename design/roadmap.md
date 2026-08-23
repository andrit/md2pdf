# Roadmap — md2pdf to v1

**Written:** 2026-08-18 · **Status:** living document, amend rather than replace.

> **The target model is `design/invariants.md`**; the record of *why* each commit happened is
> `design/commit-log.md`. Proposals not yet scheduled live in `design/feature-design/`.
> **`design/compromise-mechanism.md`** documents the escalation ladder, its baseline measurements,
> and its risk register — *every ladder change re-measures that baseline.*
>
> **`design/invariants.md`** This document is the *sequence*; that one is what
> must be true when we are done. Decisions cite an invariant, or say "no invariant — simple thing".
>
> **`design/flags.md`** is the typed register of things worth noticing — `DEFECT`, `DEBT`,
> `INSTRUMENT`, `CONSTRAINT`, `DECISION`. Anything worth flagging goes there with a type and its
> evidence, rather than into prose in a plan.

## Why this exists

Work has been planned one task at a time, which is fine for a task and bad for a project: it hides
how much is left and lets scope arrive by surprise. This is the whole route, so each gate is a step
along a known path rather than a fresh negotiation.

**It will change.** Phases past the current one are sketched at the resolution the evidence
supports; the Conversion crate is planned in detail in `plan-conversion-crate.md` because it is
being built. Detail arrives when a phase is entered, not before — but the *shape* is committed here.

## The SDLC problem, and what we do about it

md2pdf is registered as a **`custom`** project, which seeds **zero** SDLC phases
(`total_phases: 0`). That is why `session_start` reports "not started" despite four commits of real
work — no step was skipped, there is no phase machine for this project type.

Two consequences:

- `set_current_phase` validates its index against the project type's phase list, so it **cannot**
  hold the phases below. **This document is the phase machine** until that changes.
- Per CLAUDE.md, `/phase-audit` is the universal advance gate for every code phase. It applies to
  the code phases below; document-only phases are exempt, matching the workbench's own convention.

**Open for the operator:** either mirror these phases into workbench tasks (`task_create` carries a
`phase_index`), or raise a `forModeA/` request for `custom` projects to define their own phases. The
second is the real fix; the first is the workaround. Neither has been done — see §Decisions.

The `custom` roadmap in CLAUDE.md is steps 0–6. These phases map onto **step 3, "build
iteratively"**, which is where the project sits; steps 0–2 are complete.

---

## Task ledger

Every numbered task, so completed work has a status row rather than only a prose mention.
Detail lives in the phase plans; this is the index.

| Task | What | Phase | State |
|---|---|---|---|
| T1 | Typst anti-corruption layer (`md2pdf-typeset`) | 2 | ✅ `1f5ca8d` |
| T2 | 11 behavioural contract tests pinning Typst | 2 | ✅ `1f5ca8d` |
| T3 | Version pinning `=0.15.1`; boundary + verify scripts | 2 | ✅ `1f5ca8d` |
| T4 | Typst upgrade runbook (`docs/typst-upgrade.md`) | 2 | ✅ `1f5ca8d` |
| T5 | `escape.rs` + compiler round-trip harness | 3a | ✅ `da2b622` |
| T6 | `parse.rs` + `classify.rs` | 3a | ✅ `901d127` |
| T7 | `emit.rs` + D1 (`UnsupportedConstruct`) | 3a | ✅ `5c87621` |
| T8 | `convert()` public API + `Conversion` type | 3a | ✅ `2316aab` |
| — | Italic faces + `text_runs()` styling oracle | 3a | ✅ `105ca2e` |
| T9 | *(superseded — split into T10–T13)* | 3b | — |
| T10 | `World` file map in `md2pdf-typeset` | 3b | ✅ `2f59b6b` |
| T11 | `images.rs` — resolution, probe, virtual naming | 3b | ✅ `f6da45d` |
| T12 | Wire through: `SourceContext`, `ImageManifest` | 3b | ✅ `ba34fec` |
| T13 | Ladder on real images — `Rung::Scale`, image floor, `reflow` | 3b | ✅ `447182a` |
| T14 | Finish the ladder — two axes, landscape re-measure, `clip` reachable | 3b2 | ✅ `e7db9e2` |
| T15 | `PathBroker` — the first real `std::fs` in the tree | 3c1 | ✅ staged |
| T16 | `contract.rs` — minimal `Command`/`Event` | 3c1 | ✅ staged |
| T17 | `job.rs` + `BrokerImages` + `output_path`; tests included | 3c1 | ✅ staged |
| ~~T18~~ | *merged into T17 — tests ship with the code* | 3c1 | — |
| T19 | `walk` + `mirror` with a real SourceRoot | 3c2 | ✅ staged |
| T20 | Collisions: detection, `Resolution`, `Diagnostic::seal` | 3c2 | ✅ staged |
| T21 | Batch `Command`/`Event` + orchestration | 3c2 | ✅ staged |
| T22 | Guard G3 — determinism + golden-hash tests | guards | ✅ staged |
| T23 | Guard G1 — no network (greps `Cargo.lock`) | guards | ✅ staged |
| T24 | Guard G2 — no UI toolkit in the five core crates (INV-8) | guards | ✅ staged |
| T25 | CLI binary — `pico-args`, `--json`, tracing, exit codes | 3d | ✅ staged |
| T26a | Tables reflow instead of clipping (option C) | 3d | ✅ staged |
| T27 | **Ladder census** — committed fixture corpus + decision baseline *(the tripwire)* | 3d | ✅ staged |
| T26a2 | **Proportional reflow alternate** — `auto` for narrow columns, `1fr` for the widest | 3d | ✅ staged |
| T26b | **Ladder order** — 9pt comfort floor, reflow before rotation (O2a) *(closes R2)* | 3d | ✅ staged |
| T29 | Break long runs in the alternate; **build the overflow oracle** (F1) | 3d | ◐ **staged — partial, 7 → 4** |
| T29b | Weighted `fr` columns — diagnosed F8 as `auto` refusing to shrink | 3d | ◐ **staged — severity 6× down, count still 4** |
| T29c | Per-column break limits, from the weights that size them *(F8: 4 → 1)* | 3d | ✅ staged |
| T28 | **Glyph coverage** — ✅/❌ render as tofu in ~20% of real documents | 3d | ☐ |
| T30 | **Base size 12pt** — the readable target, not the assumed one *(F10)* | 3d | ☐ **next, blocks T26c** |
| T31 | **Inspect `comemo` memory growth** *(F3, three sightings)* — 3f's recompile loop rests on it | 3f | ☐ **before 3f** |
| T26c | Floors by eye — planned in `plan-floors.md`; **one live floor, four dead** *(F4)* | 3d | ☐ blocked on T30 — the pairs must be re-rendered at the right base |

**The guards track interleaves rather than follows.** T22 is built *before* T19: determinism already
holds, so golden-hash tests go green immediately and act as a regression net **during** 3c2 — a
phase that changes orchestration and output paths but must not change rendering at all. A golden
hash going red during 3c2 is an unambiguous signal that something untouched broke. Built afterwards
it would only protect work already finished.

Tasks beyond 3b are not numbered yet — they are named when their phase is planned, so the
numbering reflects decisions actually made rather than a guess at future shape.

---

## Phase status

| Phase | Name | State | Gate |
|---|---|---|---|
| 0 | Foundation — event storm, glossary | ✅ done 2026-08-17 | exempt (documents) |
| 1 | Define — stack & scope decisions | ✅ done 2026-08-16 | exempt (documents) |
| 2 | Structure — workspace, toolchain, verify gate | ✅ done 2026-08-17 | `verify.sh` |
| **3a** | **Conversion, Stage 1 — text** | **✅ code-complete 2026-08-18 (T5–T8); `/phase-audit` not yet run** | `/phase-audit` + `verify.sh` |
| 3b | Images, Stage 2 | ✅ code-complete 2026-08-19 (T10–T13); `/phase-audit` not run | `/phase-audit` |
| 3b2 | Finish the escalation ladder — all atomic classes | ✅ code-complete 2026-08-20 (T14); `/phase-audit` not run | `/phase-audit` |
| 3c1 | Engine — walking skeleton: one file, disk to disk | ✅ code-complete 2026-08-20 (T15–T17); `/phase-audit` not run | `/phase-audit` |
| 3c2 | Paths + Output — widen to batch, collisions | ✅ code-complete 2026-08-20 (T19–T21); `/phase-audit` not run | `/phase-audit` |
| **3d** | **CLI adapter, end to end** | **◐ next — planned in `plan-cli.md` (T25–T26)** | `/phase-audit` |


| 3e | Template catalogue + shipped template | planned | `/phase-audit` |
| 3f | Review — attention gate, overrides | planned | `/phase-audit` |
| 4 | egui adapter — **host work** | planned | `/phase-audit`, on host |
| 5 | Settings & identity | planned | `/phase-audit` |
| 6 | Packaging & distribution | planned | operator-owned |

---

## 3a · Conversion, Stage 1 — text *(code-complete)*

Detail in `plan-conversion-crate.md`. **T5–T8 all delivered**, `verify.sh` green, 73 tests.
`convert()` is the crate's whole public surface and is **total** — no `Result`, because an
unrepresentable construct becomes a `Compromise` rather than an error.

**Exit criteria:**

- ✅ a markdown corpus covering every construct converts to a real PDF through library tests
- ✅ `verify.sh` green (boundaries, fmt, clippy `-D warnings`, test, build)
- ⚠️ **`/phase-audit` NOT run** — the skill is not available in this session's toolset. The phase is
  code-complete but not formally signed off; run it on the host, or accept the gate as waived.

**Carried out of this phase:** the nested-atomics `ponytail:` in `classify.rs`; styling assertions
remain thin (one italic pin).

## 3b · Images, Stage 2

`world.rs:77` serves no files, and Typst treats an unresolvable file as a **compile error for the
whole document** — which is why Stage 1 emits placeholders instead.

> **De-risked 2026-08-18.** A throwaway prototype added `RefCell<HashMap<FileId, Bytes>>` to
> `TypstWorld`, served it from `World::file()`, and compiled `#image("diagram.png")` against planted
> PNG bytes: **ProbePass OK, RenderPass OK, 3121-byte PDF**. The approach is proven; the prototype
> was reverted so the real thing lands with tests. Entry risk here is now low.

**Planned in detail: `design/plan-images.md`** (T10–T13). Second probe, 2026-08-19: `comemo` does
**not** serve stale bytes when a file is replaced under the same virtual name — measurement changed
10pt → 200pt on recompile. So a plain mutable file map is safe and no cache-busting is needed; the
phase's largest unknown is gone.

Four decisions the sketch had not made, now settled there: `convert()` gains a `SourceContext`
(it cannot resolve a relative path without knowing where the Source lives); virtual names are
`img-<fnv1a(abs path)>.<ext>` for uniqueness, stability and dedup; the `ImageManifest` is the
three-crate seam; and real images go through the escalation ladder for the first time.

**Exit criteria — all met 2026-08-19:**

- ✅ a local image reaches the PDF, **confirmed by eye**
- ✅ missing and remote images degrade to visible placeholders, one `Compromise` each
- ✅ an oversized image escalates through the ladder
- ✅ `verify.sh` green
- ⚠️ `/phase-audit` not run — unavailable in this environment, as in 3a

**Found while building, and fixed here:** inline images were block-level and broke a sentence
across three lines (now wrapped in a `box`); `clear_files()` carried the wrong rationale, which
would have made a batch re-read every shared image.

## 3b2 · Finish the escalation ladder *(current)*

Not an images task, which is why it was lifted out of 3b. **Planned in `design/plan-ladder.md`.**

`probe.rs` can only emit `none`, `shrink`, `rotate` — **`Rung::Clip` is unreachable code**, though
`harvest` parses it and `render` implements it. So an element still too wide after rotation
overflows silently, with no marker and no Compromise. That is true for **tables as much as images**,
and it has been open since the spike first flagged it.

Doing it now, before 3d tunes floors by eye and before 3f offers overrides, because both of those
assume the ladder means what the design says it means.

**Exit criteria — all met 2026-08-20:**

- ✅ `Reduction::Clip` is reachable; a 40-column table clips rather than overflowing silently
- ✅ rotation re-measures against the landscape width and does not inherit the portrait floor
- ✅ a clipped element carries a visible red marker, confirmed by eye
- ✅ both axes compose: `Landscape + Clip` yields two Compromises, `[Rotated, Clipped]`
- ✅ `verify.sh` green
- ⚠️ `/phase-audit` not run — unavailable in this environment, as in 3a and 3b

## 3c · Engine, Paths, Output — **vertical slice first** *(revised 2026-08-19)*

The imperative shell. Four domain stubs still hold one line each — `job.rs`, `collision.rs`,
`escalation.rs`, `event.rs` — and they get filled here, from the glossary.

### Why this phase is shaped differently

Everything so far has been built **horizontally** — complete layers, nothing above them. That was
deliberate: the project's risk sat almost entirely in one layer (does Typst's `measure()`/`layout()`
actually support the thesis?), and slicing vertically through an unproven engine would have meant
rebuilding the slice once the engine turned out different. The bet paid: the ladder, escaping, and
images are all proven.

But horizontal layers pass their own tests and can still fail to meet each other. **Every
integration defect this project has hit was exactly that** — the body invariant across
convert→typeset, a missing image failing the whole compilation rather than one element, compromises
with no Element to attach to. Horizontal building finds those late, and 3c is the phase where the
cost of finding them late is highest, because three bounded contexts land at once.

So 3c is **explicitly split**, narrow end first. This is the plan, not a prediction that it might
split.

### 3c1 — the walking skeleton *(planned in `design/plan-engine.md`)*

**One file, in and out, through every layer.** No batch, no collisions, no template discovery, no
overrides.

`PathBroker::read` → `convert()` → `probe` → `render` → `PathBroker::write`, driven by a minimal
`Command`/`Event` pair.

Deliberately excluded: directory walking, mirroring, collision policy, blanket resolutions,
attention list. Each is a real requirement and each is a *widening* of a path that already works.

**Exit:** `notes.md` on disk becomes `notes.pdf` on disk, with a real image in it, exercised by an
engine-level test. The first time the whole stack runs end to end.

### 3c2 — widen to batch *(planned in `design/plan-batch.md`)*

- **`md2pdf-paths`** — `walk` (SourceSet discovery, records the SourceRoot), `mirror`
  (OutputPath = Destination + Source relative to SourceRoot). Batch output **mirrors the source
  tree, never flattens**.
- **`output.rs`** — collision detection, `Resolution`, `BlanketResolution` (the apply-to-all
  affordance, without which batch is unusable at 50 files).
- **The §4 seam** — merging convert-time compromises with the probe's `DecisionMap` into one sealed
  `Diagnostic`. `Diagnostic::from_decisions` only maps Rungs today.
- **`contract.rs`** grows to the full `Command`/`Event` surface — plain serializable data, no
  lifetimes, closures, or trait objects: in-process a channel, across a boundary line-delimited
  JSON, same shape either way.

**Exit:** a directory of sources converts and writes with mirrored structure, collisions resolved,
no UI.

> **Carried risk:** the Command/Event contract is designed in 3c1 when only one command exists, and
> a contract designed against a single case tends to fit that case only. Expect 3c2 to reshape it —
> and prefer reshaping it there, while the only consumer is a test, over discovering the problem in
> phase 4 when an adapter depends on it.

## 3d · CLI adapter, end to end *(planned in `design/plan-cli.md`)*

`md2pdf-cli` already documents itself as "adapter #1, not a throwaway": it runs in CI with no
display, and a contract with one implementation is not a contract.

**This is where the floors and the rotate threshold get set by eye**, on real documents — deferred
to exactly this point by the stack decisions, because it is the first phase with documents flowing
end to end.

**Exit:** `md2pdf <dir> -o <dir>` converts a real corpus; floors and threshold chosen and written
into the template.

## 3e · Template catalogue + the shipped template

**`templates/` is empty and `Template::default()` is hardcoded in Rust.** The catalogue has nothing
to discover until the shipped `github-print` template exists on disk as
`template.toml` + `template.typ`. Templates are swappable config, not code — and the shipped one
doubles as the reference example, which is the cheapest documentation available.

Work: `catalogue.rs` (discovery, token parsing, `TemplateRejected` with reasons); the template
itself; `Template` loaded from tokens rather than constructed in Rust.

**Exit:** the app discovers templates from a directory; a hand-copied folder with edited values
produces a visibly different PDF.

## 3f · Review — the attention gate

The payoff for treating the layout pass as something that *emits*.

Work: `review.rs` — `Diagnostic` → `AttentionList`; the three adjustment scopes (template / job /
element); `Override` application and the recompile loop. **Incremental recompile performance is
measured here** — it is the hot path and the spike never exercised it (`comemo` memoisation across
compilations is why `Typesetter` holds a long-lived `World`).

**Exit:** "47 converted cleanly, 3 need your attention" is real; an element override changes the
output and recompiles fast enough to feel immediate.

## 4 · egui adapter — **host work, not container work**

⚠️ **This phase cannot be executed here.** The container has no display, and `eframe` is not in the
workspace dependencies. `md2pdf-cli`'s own doc comment says the egui adapter is "added on the macOS
host, where a window actually exists."

The engine's Command/Event contract is what makes this a new adapter rather than a rewrite, and
phases 3c–3f are what make it thin. Widget functions are named for the read models they draw
(`source_list()`, `attention_list()`, `collision_prompt()`) — never invented names.

**Editing markdown remains permanently out of scope.** It is a load-bearing stack constraint, not a
product preference: egui's weakest surface is text input, and md2pdf never exercises it. Treat any
future editing request as a framework decision.

## 5 · Settings & identity

Persisted defaults in the platform config directory; **the reverse-DNS bundle identifier set here at
the latest** — macOS derives config and data directories from it, so changing it later strands user
settings. One of the four cheap day-one steps from the stack decisions.

## 6 · Packaging & distribution — operator-owned

Lifted from the "Emerging desktop type shape" section of the stack decisions, and mostly not
engineering:

- Platform packaging (`.app`/`.dmg`, `.msi`, AppImage/`.deb`), multi-platform build matrix
- **Signing and notarization deferred by decision (2026-08-16)** — ~$200–500/yr before a single
  sale. Not bought now; the cheap door-opening steps are taken instead.
- **Glyph coverage beyond Latin must be checked before the FontBook is frozen** — still open from
  the spike, and the rejected-Jost bullet failure is exactly the defect class it catches.
- Licence still deliberately unset: adopting a viral licence now would foreclose the paid option.

---

---

## Confidence review *(2026-08-18)*

This roadmap was reviewed for what it was least sure of, and the two cheapest unknowns were probed
rather than assumed. Both paid.

### Probed — and one found a live bug

**1. Can the output be inspected visually, or is "set the floors by eye" (3d, 3e) impossible here?**
→ **It can.** `Compilation::raster()` yields RGBA, which encodes to PNG without any new dependency
and can be read directly. Note the constraint that shaped the method: `check-boundaries.sh` forbids
`std::fs` outside `md2pdf-paths` — **including tests** — so the probe printed hex to stdout rather
than writing a file. Visual checking of layout, floors, and templates is therefore in scope, not
host-only work.

**2. 🐞 Italic does not render.** Rendering `*italic emphasis*` at 28pt produced glyphs identical to
regular text; bold renders correctly. `assets/fonts/` holds exactly two files, and
`SourceSans3.ttf` carries a **weight** axis but no italic face — so `#emph` silently falls back to
upright.

This is a GitHub-fidelity defect in one of the most common constructs, and **no existing test could
catch it**: `Compilation::text()` sees characters, not styling. It was invisible until the page was
looked at.

- **✅ FIXED 2026-08-18, same session.** `SourceSans3-It.ttf` and `SourceSans3-BoldIt.ttf` vendored
  and added in `fonts.rs`. The **static** faces were chosen deliberately: Adobe's *variable* italic
  reports its family as `SourceSans3VF` while the roman reports `Source Sans 3`, and Typst groups
  faces by family name — so the variable file would have looked applied and changed nothing.
- **Root cause was a documented assumption nobody checked.** `assets/fonts/README.md` asserted that
  Typst "resolves weight and italic from the axes". It resolves weight; there is no italic axis.
  The README is corrected and now records why.
- **Follow-on, done:** `Compilation::text_runs()` exposes each glyph run as
  `(text, family, style)`, and `emphasis_renders_in_an_italic_face` pins it. Verified by negative
  control: with the italic faces removed, the run carrying "slanted" reports `normal` and the test
  fails. **This is the project's first assertion about appearance rather than characters** — extend
  it as styling grows.

### Not probed — ranked by remaining risk

| Risk | Why it stays open |
|---|---|
| ~~3c is lumpy~~ — **resolved 2026-08-19**: split into 3c1 (vertical walking skeleton) and 3c2 (widen to batch), so the layers are proven to meet before three contexts land at once | done |
| **Preview across a process boundary.** The contract is "line-delimited JSON either way", but an A4 page at 2× is ~5.6 MB of RGBA — roughly 7.5 MB base64 **per page**. In-process it is a pointer; across a boundary it is not obviously viable. May force a shared-memory or file-handle path for rasters. | Only bites if an out-of-process adapter is built; in-process egui is unaffected |
| **Incremental recompile performance** never measured, though `comemo` memoisation is why `Typesetter` holds a long-lived `World`. | Named in the spike's own "Still open"; belongs to 3f |
| **egui adapter** cannot be built or seen here. | Environmental, not resolvable by planning |

## Cross-cutting, carried until closed

| Item | Where it lands |
|---|---|
| `documents/` — 152 files of other projects' artifacts, untracked, appears in every `git status` | housekeeping, any time — gitignore, delete, or commit |
| Nested atomics `ponytail:` (table inside a blockquote never escalates) | 3f or later; surfaced by `/phase-audit` |
| ~~Italic face missing~~ — ✅ fixed 2026-08-18, pinned by a styling test | done |
| **Styling assertions are new and thin** — one test covers italic; nothing pins colour, size, or spacing | extend as 3e templates land |
| **Glyph coverage** — ✅ U+2705 and ❌ U+274C render as tofu; measured 2026-08-22 in 28 and 8 of 146 documents. ⚠ ✓ ✗ → ▸ and code-fence box drawing are fine. | **T28** — was "before 6", now scheduled with evidence |
| Bare-URL autolinks unsupported by pulldown-cmark | accepted ceiling |
| ~~Clipping rung never probed~~ — **confirmed dead code 2026-08-19**: `probe.rs` can only emit none/shrink/rotate, so `Rung::Clip` is unreachable. Scheduled as T14 | T14 |
| One integration-test binary per crate — new `tests/*.rs` files that link typst will OOM the linker | permanent build constraint |

## Decisions open for the operator

1. **SDLC tooling** — mirror these phases into workbench tasks, or raise a `forModeA/` request for
   `custom` projects to carry their own phases? *Recommend the outbox request*; this document works
   in the meantime.
2. **`documents/`** — gitignore, delete, or commit.
3. **Glossary entry for `Conversion`** — see `plan-conversion-crate.md` §2.5a.
