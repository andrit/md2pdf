# Spike findings — Typst `measure()` / `layout()` verification

**Date:** 2026-08-17 · **Verified against:** `typst 0.15.1 (9dfd3a08)`, released 2026-07-17, **Apache-2.0**
**Probes:** `/workspace/spikes/typst-measure/` · **Blocker addressed:** #1 from `design/event-storm.md`

**Verdict: the measured-fit thesis holds — proven twice, through the CLI (Stage 1) and through the
embedded crate API (Stage 2).** Typst can measure content against available space, branch on the
result, and the resulting decisions can be pulled back out into the host in-process. One rung of the
escalation ladder cannot be executed where the design put it, which forces a two-pass compile — and
that turns out to resolve two other open problems at the same time.

**Blockers #1, #2 and #3 are all cleared. Build.** The full two-pass runs in **12 ms** embedded;
preview and PDF export come from one compilation; the DecisionMap injects cleanly through `World`;
ElementId falls out of the pass boundary; and the FontBook is decided (Source Sans 3 + JetBrains
Mono). What remains is engineering, not risk: probe the clipping rung, run a non-Latin glyph coverage
pass, and pin the Typst version hard behind an anti-corruption layer.

Companion crates `typst-pdf`, `typst-render`, `typst-library`, `typst-kit` are all published at 0.15.1.
Apache-2.0 keeps the "don't commit to a licence yet" decision open.

---

## What was proven

### P1 — measurement works, and metadata escapes the callback ✅

```
measure(content, width: auto|length, height: auto|length) → dictionary {width, height}
layout(size => …)   // container dims, or page minus margins; supplies context
```

On a 220pt page with 12pt margins, `layout` reported `available: 196.0` — page minus margins, exactly
as documented. A deliberately over-wide table measured `natural-width: 328.26` against it.

**The more important result:** `metadata()` emitted from *inside* a `layout()` callback is queryable
from outside the compilation. This is the mechanism by which the Diagnostic crosses the Typst→host
boundary, and nothing in the design had verified it existed.

> ⚠️ `typst query` is **deprecated** in 0.15.1. The replacement is
> `typst eval 'query(<label>).map(it => it.value)' --in file.typ --format json`.

### P3 — the rotate rung is illegal where the design put it ✅ (confirmed failure)

```
error: pagebreaks are not allowed inside of containers
error: page configuration is not allowed inside of containers
```

Both `pagebreak()` and `page(flipped: true)` fail hard inside `layout()`, because `layout` forces its
contents into a block-level container. The ladder's rung 3 — *"rotate that element to landscape on its
own page"* — **cannot be taken by the code that discovers it is needed.**

### P5 — two-pass resolves it, end to end ✅

```
pass 1 (probe)   layout + measure every element, emit decision as metadata, discard rendering
host             typst eval → decisions.json                    ← this IS the Diagnostic
pass 2 (render)  read decisions, apply each at TOP LEVEL where page()/pagebreak() are legal
```

Verified on a five-element document: prose correctly left alone, a narrow table correctly left alone,
an over-wide table and an over-wide figure both escalated to `rotate`. Output was 5 pages, and the PNG
dimensions confirm pages 2 and 4 are genuinely landscape (533×367 vs 367×533). The table that collided
and overflowed in portrait renders cleanly across a full landscape page.

**Two other blockers collapse into this pass boundary:**

- **The Diagnostic** no longer needs a separate mechanism. Pass 1's metadata *is* it.
- **`ElementId` (blocker #2) is solved.** It is simply the key pass 1 and pass 2 agree on. Because
  md2pdf generates the Typst markup itself, it assigns the id at emission — stable by construction, no
  inference from Typst's tree required. Element-scope **Overrides** are entries injected into the same
  decision map, so the user's override and the engine's decision travel one identical channel.

### P6 — cost is not a problem ✅

97-page document, 120 elements, CLI invocations including process startup and font loading:

| | |
|---|---|
| baseline plain render, no measurement | 173 ms |
| pass 1 (measure every element) | 304 ms |
| pass 2 (render with decisions) | 121 ms |
| **two-pass total** | **~425 ms** |

Measurement roughly doubles compile time, on a document far larger and more pathological than a
typical target. Preview-on-every-Override stays viable. The embedded library will be faster still —
no process startup, no font re-load — so treat this as an upper bound.

---

## What the spike got wrong in the design

### The overflow predicate was wrong, and it is per-class ⚠️

The design implies one measured test for every element. Neither obvious predicate works:

| element | `natural > avail` | `constrained > avail` | **actually overflows?** |
|---|---|---|---|
| prose, plain | true | false | **no** — it wraps |
| prose w/ long URL | true | false | **no** — Typst broke the token |
| code block (`raw`) | true | false | **no** — `raw` wraps in 0.15.1 |
| wide table | true | false | **YES** |
| 400pt figure | true | false | **YES** |

Ground truth came from rendering with the text block outlined in red (`p2c-visual.typ`) and looking at
it. `natural > avail` is true for everything, including content that wraps perfectly — all false
positives. And `measure(el, width: avail).width` is **clamped to the available width in every case**,
including a 400pt rect that visibly draws past the page edge — all false negatives.

**The correct rule:** natural width is the required width **only for atomic content**. For wrappable
content it is meaningless — it just means "all on one line."

```
overflows = class is atomic AND measure(el).width > available
```

md2pdf knows the class at emission time — it emits a table because the markdown *had* a table — so
this is decidable without asking Typst. That is a genuine advantage of the pulldown-cmark→Typst design
over handing Typst a document and interrogating it.

This validates the design's instinct that Floors are per-class, and extends it: **the predicate is
per-class too, not just the floor.**

### `raw` blocks wrap — so code may not need the ladder at all ⚠️

Verified visually: a long Rust line in `raw(block: true)` broke onto a second line rather than
overflowing. So a code Floor of ~7pt may be solving a problem that does not exist. The real question
is a **typographic** one, not a layout one: wrapped code is misleading to read, so md2pdf may *prefer*
shrink-then-rotate over Typst's default wrap. That is now a template decision made with evidence
rather than an assumption.

### "Shrink" means different things per class ⚠️

`measure(text(size: s, el))` steps text down, which is right for tables and prose but does nothing to
a figure — a rect's width is fixed regardless of font size. In P5 the figure reached `rotate` only
because shrinking could never help it. Correct outcome, accidental mechanism. **For images the shrink
rung must be a scale factor, not a font size.**

### Rotation should re-measure, not carry the floor size over ⚠️

P5 rotates the table onto a landscape page *and* applies the 7pt floor size it had reached in
portrait. But landscape has far more width available — the element likely fits at base size once
rotated. The ladder should **re-run measurement after rotating** rather than inheriting the shrink.

---

## Consequences for the design

1. **The escalation ladder becomes a two-pass model.** Amend `design/event-storm.md`: the
   Typesetting context runs probe → decide → render, and `DiagnosticSealed` happens at the pass
   boundary, not at the end of a single compile.
2. **`ElementId` moves from open hotspot to decided:** assigned by md2pdf at markup emission, carried
   through pass-1 metadata and the pass-2 decision map.
3. **`ElementClass` gains a second job** — it selects the overflow predicate and the meaning of
   "shrink", not just the Floor value.
4. **Decisions are data, injected into a static template.** P5 passes them as JSON the template reads.
   This fits "templates are swappable config, not code" better than generating per-document markup.

---

---

# Stage 2 — the embedded crate API ✅

**Spike:** `/workspace/spikes/typst-embed/` · `rustc 1.97.1` · deps pinned `=0.15.1`

Stage 1 proved the design through the **CLI**. md2pdf embeds the crate and cannot shell out, and
`typst eval` is a CLI-only feature — so the open question was whether the *library* offers an
equivalent way to recover ProbePass decisions. **It does.**

### P1′ — metadata recovered in-process ✅

```rust
doc.introspector()                              // PagedDocument::introspector()
   .query_labelled()                            // trait Introspector
   .iter()
   .filter_map(|c| c.to_packed::<MetadataElem>())
   .map(|m| serde_json::to_value(&m.value))     // Value implements Serialize
```

Identical decisions to the CLI run, to the same fractional widths (`328.26`, `400.0`, `485.81`):
prose → `none`, wide table → `rotate`, narrow table → `none`, oversized figure → `rotate`. **The
Diagnostic crosses out of Typst with no subprocess.**

### P5′ — full loop, and the DecisionMap goes through `World` ✅

`json.decode` no longer exists in 0.15.1, which forced a better answer than the CLI's `--input`:
the DecisionMap is served as a **virtual file through `World::file()`**, and the template reads it
with `json("decisions.json")`. That is exactly how md2pdf will serve templates, images and decisions
— **one injection point, and it sits behind PathBroker.** The RenderPass template stays static config.

Result: 4 pages, pages 2 and 4 confirmed landscape by frame geometry (`320×220pt` vs `220×320pt`).
`typst_pdf::pdf` produced 11,484 bytes and `typst_render::render` a 440×640px preview **from the same
compilation** — preview and export share one compile, as the design assumed.

### Timings — embedded, no process startup

| | debug | **release** |
|---|---|---|
| ProbePass | 137 ms | ~10 ms |
| RenderPass | 31 ms | **2 ms** |
| **two-pass total** | 168 ms | **12 ms** |

**12 ms for the full two-pass**, against 425 ms for the same work through the CLI — most of that
figure was process startup and font loading, neither of which an embedded engine pays. Measurement
costing "2× compile" is arithmetically true and practically irrelevant at this scale.

Latency is not a design constraint. Preview-on-every-Override is comfortably interactive, and the
earlier worry that `measure()` at document scale might push a real document into hundreds of
milliseconds is settled.

### The API churns badly — pin the version ⚠️

Eight breaking differences surfaced while writing ~200 lines against 0.15.1:

| Expected | Actual at 0.15.1 |
|---|---|
| `FileId::new(None, vpath)` | `RootedPath::new(VirtualRoot::Project, vpath).intern()` |
| `VirtualPath::new(..) -> VirtualPath` | returns `Result<_, PathError>` |
| `Library::default()` | `Library::builder().build()`, needs `LibraryExt` in scope |
| `Introspector` inherent methods | it is a **trait**; must be imported |
| `typst_render::render(page, 2.0)` | `render(page, &RenderOptions)` |
| `VirtualPath::as_rootless_path` | deprecated → `get_without_slash` |
| `json.decode(bytes)` | removed |
| `today(Option<i64>)` | `today(Option<Duration>)` |

`typst = "=0.15.1"` exactly, and treat a Typst upgrade as a scheduled task with its own spike — never
a routine `cargo update`.

### Build weight ⚠️

297 crates. The **cold release build exceeded 10 minutes**; once the dependency graph was cached the
crate itself rebuilt in **9.7 s**. Binaries: 542 MB debug, **61 MB release**.

Not a blocker, but it shapes CI — cache the registry and target dir, and budget a multi-minute cold
build on every platform in the matrix. 61 MB release is also a data point for the distribution
argument: still well under the ~100 MB Electron bundle the stack decisions rejected, before any
stripping or LTO.

---

---

# Blocker #3 — the FontBook ✅ decided

Typst's embedded book holds four families — `DejaVu Sans Mono`, `Libertinus Serif`, `New Computer
Modern`, `New Computer Modern Math` — and **no sans-serif**, confirmed both via CLI and in-process.
`FontStore::extend` accepts any `(impl FontSource, FontInfo)` iterator, so supplying our own is
mechanically trivial. The question was *which*.

Four candidates were rendered through an identical specimen — heading hierarchy, 10pt body, inline
code, bulleted list, table, highlighted code block, callout, numerals. **Typst 0.15.1 handles variable
fonts correctly**; bold and italic resolved from the weight axis with no static instances needed.

### Density — a print tool pays for space

| | page height | vs. best |
|---|---|---|
| **Source Sans 3** | 1119 px | **1.000×** |
| IBM Plex Sans | 1169 px | 1.045× |
| Jost | 1170 px | 1.046× |
| Inter | 1185 px | 1.059× |

Inter costs ~6% more paper than Source Sans 3 — six extra pages per hundred, every time.

### Decision: Source Sans 3 body, JetBrains Mono code

Both SIL OFL, which permits bundling *and* PDF embedding. Source Sans 3 is the most compact, Adobe
designed the Source family for print as well as screen, and it is humanist and neutral — closer to
GitHub's real system-font stack (`-apple-system, "Segoe UI", "Noto Sans", Helvetica`) than any
geometric face.

**IBM Plex Sans** is the alternative if md2pdf should have a visible identity — its distinctive `a`
and `g` read as deliberately engineered, at 4.5% more pages.

**Inter is the wrong body face but likely the right UI face.** It is screen-optimised: largest
x-height, loosest fit, most expensive on paper. Using Inter for the egui chrome and Source Sans 3 for
documents costs nothing, since the UI font never reaches a PDF.

### Jost rejected — and it decides the Avenir question

Jost was the honest Avenir substitute; both are humanised Futura. Two findings killed it:

1. **Its U+2022 renders as a midpoint-sized dot**, a fraction of the other three. Bulleted lists are
   among the most common constructs in a markdown corpus — that is a defect in a default face, not a
   nitpick.
2. **Geometric faces degrade faster at small sizes** — `o c e a` converge — and the escalation ladder
   deliberately drives text toward 7–9pt Floors. The geometric instinct fights the ladder.

**Avenir itself is unavailable regardless:** Monotype-licensed, and a desktop licence does not grant
application bundling. The workable compromise is already in the design — a Template may *name* a
system font, and a user who does so knowingly trades cross-platform determinism for their typeface.

---

# Stage 2b — three questions settled by benchmark

**Spike:** `spikes/typst-embed/src/bin/bench.rs` · 120 elements, 96 pages, release build.

> ⚠️ **comemo's memo cache is global, not per-`World`.** A first version of this benchmark ran the
> variants in sequence and reported a 100× difference that was entirely cache warming. Every "cold"
> number below is taken after `comemo::evict(0)`. Any future benchmark in this codebase must do the
> same or it measures ordering, not code.

### Q1 — the ProbePass must NOT compile the full document ✅

| | |
|---|---|
| A — full document, `layout()` per element, rendering discarded | 353.2 ms |
| B — measure-only, available width computed from the Template, never paginated | **97.4 ms** |
| decisions identical (120 vs 120) | **YES** |

**3.6× cheaper for the same answers.** md2pdf already knows page width and margins from
`template.toml`, so `layout()` was never needed — it was inherited from how the CLI spike happened to
be written, not from a requirement. **The ProbePass measures in a non-paginated harness and never
calls `layout()`.**

### Q2 — an Override costs 7.6 ms ✅

| | |
|---|---|
| render, cold | 34.5 ms (96 pages) |
| render, warm, nothing changed | ~0 ms |
| **render after ONE Override flipped** | **7.6 ms** |

`comemo` memoisation works and does **not** fully invalidate on a decision change. Adjustment is
comfortably interactive on a 96-page document. **Latency is closed as a design question.**

### Q3 — the two-pass split is a CHOICE, not a constraint ⚠️ *corrects an earlier claim*

Q1 removed `layout()` from the design — and `layout()` was the only reason the split was mandatory.
`page(flipped: true)` **is legal inside a plain `context` block**; `context` does not force a
block-level container the way `layout()` does. Verified: a single pass that measures *and* rotates
compiles cleanly, produces the same 120 decisions, the same 96 pages, and **zero warnings**.

| | |
|---|---|
| two-pass (probe → harvest → render) | **105.0 ms** |
| single-pass (measure and act in one compile) | 215.2 ms |

**Two-pass is kept for cost, not legality** — measuring inside a paginating document is roughly twice
as expensive as measuring outside one, which is the same effect Q1 isolated. The earlier statement
that Typst *forces* the split was wrong; what Typst forbids is acting inside `layout()`, and the
design no longer uses `layout()` at all.

Two-pass retains an independent advantage: the Diagnostic exists **before** anything is rendered, so
the batch attention gate can fire without committing to output.

**Optimisation left on the table:** the shrink loop is a linear scan in 0.5pt steps (≈7 measurements
per atomic element) and dominates probe cost. Binary search over candidate sizes would cut it to ~3.

---

## Still open

- **Glyph coverage beyond Latin.** Only Latin was tested. "GitHub fidelity" implies at least Latin-1
  plus common symbols, arrows and box-drawing characters — needs a coverage pass before the FontBook
  is fixed. The Jost bullet failure is precisely the class of defect this would catch.
- **Clipping (the last rung)** still unprobed. Lower risk than rotation.
- **Floor values and the rotate threshold** remain to be set by eye on real documents, as intended.
- **Incremental recompilation** was never exercised. `comemo` memoises across compilations, and
  Override-driven recompiles are the hot path — worth measuring before the UI is built.
