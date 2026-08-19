# Roadmap — md2pdf to v1

**Written:** 2026-08-18 · **Status:** living document, amend rather than replace.

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

## Phase status

| Phase | Name | State | Gate |
|---|---|---|---|
| 0 | Foundation — event storm, glossary | ✅ done 2026-08-17 | exempt (documents) |
| 1 | Define — stack & scope decisions | ✅ done 2026-08-16 | exempt (documents) |
| 2 | Structure — workspace, toolchain, verify gate | ✅ done 2026-08-17 | `verify.sh` |
| **3a** | **Conversion, Stage 1 — text** | **✅ code-complete 2026-08-18 (T5–T8); `/phase-audit` not yet run** | `/phase-audit` + `verify.sh` |
| 3b | Images, Stage 2 | planned | `/phase-audit` |
| 3c | Engine, Paths, Output | planned | `/phase-audit` |
| 3d | CLI adapter, end to end | planned | `/phase-audit` |
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

Work: the `World` file map in typeset (with contract tests); T9 (`images.rs` — resolution,
`ImageProbe` injected, remote/missing policy per D4); the virtual-name → path manifest; engine reads
bytes via `md2pdf-paths`.

**Exit:** a document with local, missing, and remote images converts — real image embedded,
placeholders elsewhere, one `Compromise` each.

## 3c · Engine, Paths, Output

The imperative shell. Four domain stubs still hold one line each — `job.rs`, `collision.rs`,
`escalation.rs`, `event.rs` — and they get filled here, from the glossary.

- **`contract.rs`** — `Command`/`Event`, plain serializable data. No lifetimes, closures, or trait
  objects: in-process a channel, across a boundary line-delimited JSON, same shape either way.
- **`job.rs`** — orchestration: convert → probe → harvest → render → write.
- **`md2pdf-paths`** — `PathBroker` (the only crate touching the filesystem), `walk`, `mirror`,
  and `settings`. Batch output **mirrors the source tree, never flattens**.
- **`output.rs`** — collision detection, `Resolution`, `BlanketResolution` (the apply-to-all
  affordance, without which batch is unusable at 50 files).
- **The §4 seam** — merging convert-time compromises with the probe's `DecisionMap` into one sealed
  `Diagnostic`. `Diagnostic::from_decisions` only maps Rungs today.

**Exit:** a batch of sources converts and writes to disk with mirrored structure, collisions
resolved, no UI.

## 3d · CLI adapter, end to end

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
| **3c is lumpy** — Engine + Paths + Output is three bounded contexts in one phase, plausibly as large as 3a–3f combined. Expect it to split once entered. | Sizing needs the contract designed first |
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
| Glyph coverage beyond Latin | before 6 |
| Bare-URL autolinks unsupported by pulldown-cmark | accepted ceiling |
| Clipping rung never probed | 3f |
| One integration-test binary per crate — new `tests/*.rs` files that link typst will OOM the linker | permanent build constraint |

## Decisions open for the operator

1. **SDLC tooling** — mirror these phases into workbench tasks, or raise a `forModeA/` request for
   `custom` projects to carry their own phases? *Recommend the outbox request*; this document works
   in the meantime.
2. **`documents/`** — gitignore, delete, or commit.
3. **Glossary entry for `Conversion`** — see `plan-conversion-crate.md` §2.5a.
