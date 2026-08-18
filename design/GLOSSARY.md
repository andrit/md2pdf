# GLOSSARY — md2pdf

**Date:** 2026-08-17 · **Skill:** `ubiquitous-language` · **Derived from:** `design/event-storm.md`

One term per concept, used identically in code, docs, UI copy, and conversation. This file governs
every naming decision from module names to `template.toml` keys, and it exists *before* the first
line of Rust — which is the only time a glossary prevents drift rather than documenting it.

The `ubiquitous-language` skill is written for a TypeScript/Postgres SaaS. md2pdf has no database, no
API routes, and no React, so §"Naming conventions" below is restated for Rust. The `tier` convention
from that skill refers to *billing* tiers; md2pdf has no billing, and the word is repurposed here —
see **Scope ⚠️ DISPUTED**.

---

## The pipeline nouns

These five name the same content at five stages. Each has a distinct lifecycle, so each gets its own
term; none of them is called "document" or "file".

### Source
A markdown file selected for conversion. Always the `.md` on disk and its text.
- **Type:** `Source` · **Module:** `source`
- ⚠️ NOT "input", "doc", "document", or "file"

### SourceSet
The collection of Sources selected for one Job — one file, or a directory walked recursively.
- **Type:** `SourceSet` · Carries the **SourceRoot** so the batch can mirror the tree

### Markup
Typst markup emitted from a parsed Source. Text, not a Typst object.
- **Type:** `Markup` · Produced by the Conversion context
- ⚠️ NOT "typst source", "intermediate", or "IR"

### Compilation
The result of running Typst over Markup with a Template: paged output **plus its Diagnostic**.
Always **two passes** — see ProbePass and RenderPass.
- **Type:** `Compilation` · A Compilation without a Diagnostic is not a valid Compilation
- ⚠️ NOT "render", "build", or "result"

### ProbePass
The first Typst compilation. Measures every Element, runs the Escalation ladder, emits a Decision per
Element as queryable metadata, and **discards its own rendering.** It decides; it never acts.
- Cannot rotate or page-break: those are illegal inside `layout()` (verified, Typst 0.15.1)
- ⚠️ NOT "first pass", "measure pass", or "dry run"

### RenderPass
The second Typst compilation. Reads the DecisionMap and executes each Decision at top level, where
`pagebreak()` and `page(flipped: true)` are legal. It acts; it never measures or decides.

### DecisionMap
Serialized `ElementId → Decision` produced by the ProbePass and consumed by the RenderPass. Also the
channel for user Overrides — an Override is an entry written into this map, indistinguishable in kind
from an engine Decision.
- Being plain serializable data is deliberate: it keeps Templates static config rather than generated
  code, and it is the same discipline that keeps the engine UI-agnostic

### Output
A written PDF on disk. Only exists after a successful write.
- **Type:** `Output` · Before it is written it is an **OutputPath**, not an Output
- ⚠️ NOT "artifact", "export", "file", or "pdf" as a noun for the thing

---

## Job and destination

### Job
One conversion run: a SourceSet, a resolved Destination, a chosen Template, and Job-scope settings.
- **Type:** `Job` · A Job over one Source is still a Job — there is no separate "single" type
- **Lifecycle:** defined → compiling → reviewable → committed / abandoned

### Destination
The folder Outputs are written into.
- **Type:** `Destination`
- **Resolution chain:** Job Destination → **DefaultDestination** → prompt the user
- ⚠️ NOT "output dir", "target", or "out folder"

### DefaultDestination
The persisted Destination in Settings, used when a Job does not carry one.

### SourceRoot
The directory a batch SourceSet was walked from. Every OutputPath mirrors the Source's path relative
to the SourceRoot, beneath the Destination. Batch output is **never flattened**.

---

## Layout — the escalation ladder

The most important vocabulary in the app, because these terms name the decisions it makes on the
user's behalf.

### Element
One measurable unit of content: a paragraph, table, code block, image, heading, or blockquote. The
unit the ladder acts on, and the unit a Compromise refers to.
- **Type:** `Element` · **ElementId** addresses one Element across both passes and across recompilations
- **ElementId is assigned by md2pdf when it emits the Markup** — source order within the emission.
  Stable by construction, because md2pdf generates the Markup rather than inferring structure from
  Typst's tree. The ProbePass and the RenderPass agree on this key; so do user Overrides.

### ElementClass
The category an Element belongs to. It carries **three** jobs, not one:
1. it selects the Floor,
2. it selects the **overflow predicate** (via Atomic vs Wrappable),
3. it defines what "shrink" *means* — font size for text-bearing classes, **scale factor for images**.
- **Type:** `ElementClass` (enum) · `Prose | Table | Code | Heading | Caption | Image | Quote`
- Classes tolerate shrinking differently: prose is read linearly and fatigues; tables are scanned

### Atomic / Wrappable
The division that determines whether an Element can overflow at all.
- **Atomic** — `Table`, `Image`, fixed-width blocks. Natural width *is* required width. Can overflow.
- **Wrappable** — `Prose`, `Quote`, lists, and (verified in 0.15.1) `Code`. Reflows to any width, so it
  **cannot overflow horizontally** and the ladder skips it entirely.
- **The predicate:** `class is Atomic AND measure(el).width > available`
- ⚠️ Two predicates were tried and both are wrong. `natural > available` is true for *everything*,
  including prose that wraps perfectly. And `measure(el, width: available).width` is **clamped to the
  available width in every case** — it returns "fits" even for content that visibly draws off the
  page. Do not reach for either. See `design/spike-typst-measure-findings.md`.

### Floor
The minimum font size for an ElementClass. **Per class, never global.** Starting points: ~9pt Prose,
~7pt Table and Code — tunable, and settled by eye during the first real conversions.
- A Floor does two jobs: it guards readability, **and it is what triggers the next rung of the ladder**
- **Lives in:** `template.toml`, alongside every other Token — a dense technical template legitimately
  wants lower Floors than a letter template
- ⚠️ `Code` is Wrappable in Typst 0.15.1, so a code Floor guards a *typographic* preference (wrapped
  code misleads the reader) rather than a layout failure. Decide deliberately or drop it.

### Fit
The measured comparison of an Element against its available space during compilation. The verb is
**measure**; the noun for the outcome is **Fit**.

### Escalation
The ordered response when an Atomic Element does not fit. Fixed order, no policy switch:

```
Wrappable?  → skip the ladder entirely
Atomic, over? → shrink toward Floor        (font size, or scale for images)
  at Floor and still over? → rotate to landscape on its own page
    → RE-MEASURE in landscape; do not inherit the portrait size
    → still over? → clip, with a visible marker
```

- Decided entirely in the **ProbePass**; rotation and clipping are executed in the **RenderPass**
- The re-measure step matters: landscape offers far more width, so an Element driven to its Floor in
  portrait will usually fit at a larger size once rotated. Carrying the Floor size over is a bug.
- ⚠️ NOT "overflow policy" — the design deliberately replaced a single global policy with a per-Element
  measured decision, and the old term implies the thing that was rejected

### Compromise
**One recorded concession** — a shrink to Floor, a rotation, a clip, a skipped remote image — naming
the Element, the page, and what was done.
- **Type:** `Compromise` (enum) · `ShrunkToFloor | Rotated | Clipped | ImageSkipped | ImageMissing`
- The central noun of the design: it is what makes preview exception-driven, what makes the attention
  gate possible, and what makes Element-scope adjustment offerable at all

### Diagnostic
Every Compromise recorded during one Compilation. **Sealed** when compilation finishes; discarded
whole and rebuilt on recompile, never merged.
- **Type:** `Diagnostic` · An empty Diagnostic means the document converted cleanly
- ⚠️ NOT "warnings", "log", "report", or "issues"

### Flagged
A Source whose Diagnostic is non-empty. The **AttentionList** is exactly the set of Flagged Sources.
- ⚠️ NOT "failed" — a Flagged Source converted successfully; a judgment call was simply made on the
  user's behalf and is being surfaced

---

## Adjustment

### Scope ⚠️ DISPUTED
The stack decisions call the three adjustment levels "tiers". **Resolved: they are Scopes.** "Tier"
carries a pricing meaning across every other project in this workbench, and md2pdf may yet become a
paid product; reusing the word for layout controls guarantees a collision later.

| Scope | Persistence | Controls |
|---|---|---|
| **Template scope** | persistent, in `template.toml` | page size, margins, type scale, Floors, fonts |
| **Job scope** | this Job only | page size, orientation, base font, Template choice |
| **Element scope** | this Element, this Job | force landscape, allow clip, permit below Floor |

Element scope is offered **only where the Diagnostic named an Element.** md2pdf is not a layout
editor; it offers a fix precisely where the engine already admitted it compromised.

### Override
A user decision at Job or Element scope. Triggers a full recompile.
- **Type:** `Override` · ⚠️ NOT "setting", "adjustment", or "tweak"

### Token
A named design value in `template.toml` — page size, margins, colours, type scale, Floors, and font
family names.
- A font Token naming a face from the shipped **FontBook** keeps determinism; one naming a system font
  knowingly trades it away. Both are legal; only the second is a gamble, and the Template author is
  the one taking it.
- ⚠️ NOT "variable", "option", or "config value"

---

## Templates

### Template
A **folder**, discovered from a directory at startup, never compiled into the binary:

```
templates/github-print/
  template.toml    — metadata + Tokens
  template.typ     — the layout
```

- **Type:** `Template` · The shipped `github-print` Template doubles as the reference example
- A Template is the **print stylesheet**: page furniture and element styling. It decides how the
  markdown's semantics *look*; it never supplies structure or content.

### TemplateCatalogue
Every Template discovered on disk, including ones **Rejected** during load with the reason. A
rejected Template is shown, not silently dropped — template authoring is a supported activity.

---

## Output and collisions

### Collision
An OutputPath that already exists on disk.
- ⚠️ NOT "conflict", "duplicate", or "overwrite"

### Resolution
The user's answer to one Collision: `Rename | Skip | Overwrite`.
- **There is no silent overwrite anywhere in the app.** Overwrite requires a recorded human decision.

### BlanketResolution
A Resolution applied to every remaining Collision in a batch without prompting again — "rename all",
"skip all", "overwrite all". Without this, a batch of 50 with a naive prompt-per-Collision is the
worst part of the app.

---

## Infrastructure

### PathBroker
The **single module every filesystem path passes through.** Exists so that a future Mac App Store
requirement (security-scoped bookmarks instead of raw persisted paths) is a swap inside one module
rather than a rewrite across the codebase. No other module opens a file by path.
- ⚠️ Nothing else may call `std::fs` directly. This is enforceable by lint and should be.

### FontBook
The set of typefaces md2pdf **ships with the binary** and supplies to Typst. Typst embeds no
sans-serif of its own, and bundling our own is what makes PDF output identical across platforms.
- **Body: Source Sans 3** · **Mono: JetBrains Mono** — both SIL OFL, which permits bundling *and*
  PDF embedding. The licence text ships alongside the fonts.
- Vendored into the repo, not fetched at build time, so builds are reproducible
- A Template may **name** a system font instead. That is a supported Override and a knowing trade:
  the document gains the user's typeface and loses cross-platform determinism.
- ⚠️ Distinct from the **UI font**, which is a property of the egui adapter and never reaches a PDF.
  Different job, different face — the UI may use a screen-optimised font the documents do not.

### Settings
Persisted user state in the platform config directory: DefaultDestination, last Template, window
state. Located via `directories`, keyed off the **BundleId**.

### BundleId
The reverse-DNS identifier (`com.<org>.md2pdf`), fixed **before the first release** — macOS derives
config and data directory paths from it, so changing it later strands user settings.

---

## Naming conventions — Rust

| Layer | Convention | Example |
|---|---|---|
| Modules | `snake_case`, one domain concept per file | `source`, `markup`, `layout`, `diagnostic`, `collision` |
| Types | `PascalCase`, matching the glossary term exactly | `Source`, `Job`, `Compromise`, `Diagnostic`, `Template` |
| Enum variants | `PascalCase`, past tense for Compromises | `Compromise::ShrunkToFloor`, `Compromise::Rotated` |
| Functions | `snake_case`, verb + noun | `resolve_destination()`, `emit_markup()`, `seal_diagnostic()` |
| Predicates | `is_` / `has_` | `is_flagged()`, `has_compromises()` |
| Errors | `PascalCase` + `Error`, one per context | `ConversionError`, `CompileError`, `TemplateLoadError` |
| `template.toml` keys | `snake_case` | `page_size`, `margin_top`, `floor_prose_pt`, `floor_code_pt`, `font_body`, `font_mono` |
| egui widget fns | `snake_case`, named for the read model they draw | `source_list()`, `attention_list()`, `collision_prompt()` |
| Test names | `snake_case`, states the behaviour | `shrinks_to_floor_before_rotating()` |

The read models in the event storm are the widget names. `AttentionList` in the domain is
`attention_list()` in the UI — never `WarningsPanel`, never `IssuesView`.

---

## Synonym drift to watch

| Do not use | Use |
|---|---|
| document, file, doc, input | **Source** (the `.md`) or **Output** (the `.pdf`) |
| export, generate, produce, render | **compile** (Typst runs), **write** (bytes hit disk), **convert** (the whole user-facing action) |
| warning, issue, problem, error | **Compromise** (one concession) or **Diagnostic** (all of them) — errors are for failures, and a Compromise is not a failure |
| tier, level | **Scope** (Template / Job / Element) |
| overflow policy, fallback | **Escalation** |
| conflict, duplicate | **Collision** |
| output dir, target folder | **Destination** |
| setting, option, tweak | **Token** (in a Template) or **Override** (by the user) |

---

## Checklist

- [x] Every aggregate, entity, and value object in the event storm has an entry
- [x] Every Compromise variant is named and defined
- [x] Per-layer naming conventions restated for Rust, not inherited from the TS/SaaS skill
- [x] "tier" collision identified and resolved → **Scope**
- [x] Read-model names bound to widget names so UI cannot drift from the domain
- [x] **ElementId scheme defined** — assigned at Markup emission, shared by both passes and Overrides
- [x] Two-pass vocabulary added (ProbePass / RenderPass / DecisionMap) after the 0.15.1 spike
- [x] Atomic / Wrappable added — the overflow predicate is per-class, not global
- [ ] Glossary committed alongside the first module skeleton
