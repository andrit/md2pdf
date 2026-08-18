# Event Storm — md2pdf

**Date:** 2026-08-17 · **Skill:** `event-storming` · **Input:** `.workbench/designer/current/stack-decisions.md`

A local, offline, single-user desktop converter. There are no accounts, no network, no billing, and
no server — so the storm has **no auth events, no tier/limit policies, and no webhook actors**. The
`event-storming` checklist item "free-tier limit enforcement appears as a policy + hotspot" is not
applicable here and is deliberately empty rather than invented.

The domain's substance *is* the pipeline. Parse and compile are therefore modelled as real domain
events, but per-element mechanics (`ElementMeasured`) are not — measurement is how a decision is
reached, and the **decision** is the event.

---

## Domain Events

**Startup & catalogue**
- TemplatesDiscovered
- TemplateLoaded
- TemplateRejected
- SettingsLoaded
- DefaultDestinationSet

**Selection & job definition**
- SourcesSelected
- SourceTreeWalked
- DestinationResolved
- DestinationUnresolved
- TemplateChosenForJob
- JobDefined

**Conversion**
- SourceParsed
- MarkupEmitted
- ImageResolved
- ImageMissing
- RemoteImageSkipped
- UnsupportedConstructEncountered

**Typesetting — probe pass**
- ProbePassStarted
- ElementMeasured
- ElementJudgedFitting
- ElementShrunkToFit
- ElementFloorReached
- ElementMarkedForRotation
- ElementMarkedForClipping
- CompromiseRecorded
- DiagnosticSealed
- ProbePassCompleted

**Typesetting — render pass**
- DecisionsInjected
- ElementRotatedToLandscape
- ElementRemeasuredAfterRotation
- ElementClipped
- RenderPassCompleted
- CompilationSucceeded
- CompilationFailed

**Review & adjustment**
- PreviewRastered
- AttentionRequested
- TemplateTokenChanged
- JobSettingOverridden
- ElementOverrideApplied
- RecompileTriggered

**Output**
- OutputPathComputed
- CollisionDetected
- CollisionResolved
- BlanketResolutionChosen
- OutputWritten
- OutputSkipped
- BatchCompleted

---

## Commands → Events

| Command | Actor | Event(s) |
|---|---|---|
| DiscoverTemplates | System (startup) | TemplatesDiscovered, TemplateLoaded / TemplateRejected |
| LoadSettings | System (startup) | SettingsLoaded |
| SetDefaultDestination | User | DefaultDestinationSet |
| SelectSources | User | SourcesSelected, SourceTreeWalked |
| ResolveDestination | System | DestinationResolved / DestinationUnresolved |
| ChooseTemplate | User | TemplateChosenForJob |
| DefineJob | System | JobDefined |
| ParseSource | System | SourceParsed |
| EmitMarkup | System | MarkupEmitted, ImageResolved / ImageMissing / RemoteImageSkipped, UnsupportedConstructEncountered |
| RunProbePass | System | ProbePassStarted, ElementMeasured, ProbePassCompleted |
| ShrinkElement | System (probe pass) | ElementShrunkToFit, ElementFloorReached |
| MarkForRotation | System (probe pass) | ElementMarkedForRotation |
| MarkForClipping | System (probe pass) | ElementMarkedForClipping |
| RecordCompromise | System (probe pass) | CompromiseRecorded |
| SealDiagnostic | System | DiagnosticSealed |
| InjectDecisions | System | DecisionsInjected |
| RunRenderPass | System | ElementRotatedToLandscape, ElementRemeasuredAfterRotation, ElementClipped, RenderPassCompleted |
| Compile | System | CompilationSucceeded / CompilationFailed |
| RasterPages | System | PreviewRastered |
| RequestAttention | System | AttentionRequested |
| ChangeTemplateToken | User | TemplateTokenChanged |
| OverrideJobSetting | User | JobSettingOverridden |
| OverrideElement | User | ElementOverrideApplied |
| ComputeOutputPath | System | OutputPathComputed |
| ResolveCollision | User | CollisionResolved |
| ApplyResolutionToAll | User | BlanketResolutionChosen |
| WriteOutput | User (commit) / System | OutputWritten / OutputSkipped |
| CompleteBatch | System | BatchCompleted |

Only six commands have a human actor at the keyboard: **SelectSources, ChooseTemplate,
SetDefaultDestination, the three override commands, ResolveCollision, and the final commit.**
Everything else is the machine. That ratio is the product thesis — the app makes the judgment calls
and asks only where it actually compromised.

---

## Policies

**Destination**
- Whenever SourcesSelected and the job carries no destination → resolve to the default destination
- Whenever no default destination is set → DestinationUnresolved → prompt the user
  *(the fallback chain: job → default → prompt)*

**The escalation ladder** — the core policy chain, and the reason Typst was chosen.
Verified against Typst 0.15.1; see `design/spike-typst-measure-findings.md`.

The ladder runs **entirely in the probe pass**, which decides but does not act. Actions that need a
page break — rotation above all — are illegal inside `layout()` and are executed by the render pass.

- Whenever an element is of a **wrappable** class → it cannot overflow horizontally; skip the ladder
- Whenever an **atomic** element measures wider than the available width → ShrinkElement toward its
  class floor *(shrink = font size for text-bearing classes, scale factor for images)*
- Whenever an atomic element is at its class floor and still over → MarkForRotation
- Whenever a rotated element is re-measured in landscape → recompute its size from the wider
  available width; **do not inherit the portrait floor size**
- Whenever a rotated, re-measured element is still over → MarkForClipping, with a visible marker
- Whenever any of the above fires → RecordCompromise

**Pass boundary**
- Whenever the probe pass completes → SealDiagnostic → the Diagnostic leaves Typst as queryable
  metadata and becomes host-side data
- Whenever decisions are injected → the render pass applies them at top level, where `pagebreak()`
  and `page(flipped: true)` are legal
- The render pass **never measures and never decides.** It only executes.

**Diagnostic & review**
- Whenever CompilationSucceeded → SealDiagnostic
- Whenever a single-source job compiles → RasterPages **always** (preview is free; nothing commits to
  file without a preview opportunity)
- Whenever a batch job compiles → RequestAttention **only** for sources whose diagnostic is non-empty
- Whenever DiagnosticSealed → the elements named in it, and only those, become adjustable

**Adjustment**
- Whenever TemplateTokenChanged, JobSettingOverridden, or ElementOverrideApplied → RecompileTriggered
- Whenever RecompileTriggered → the previous diagnostic is discarded, not merged

**Collision**
- Whenever OutputPathComputed and the path exists → CollisionDetected → prompt showing the conflict
- Whenever BlanketResolutionChosen → apply it to every remaining collision in the batch without
  prompting again
- Never overwrite silently — there is no policy that writes over an existing file without a
  recorded human decision

**Batch shape**
- Whenever a batch destination is computed → mirror the source subfolder structure beneath it, never flatten

---

## Read Models

| Read model | Shows | Needed when |
|---|---|---|
| **SourceList** | selected sources, per-source state (pending / compiled / flagged / written / skipped) | always, the main surface |
| **TemplateCatalogue** | templates discovered on disk, with the rejected ones and why | job setup; template authoring |
| **Preview** | rastered pages of the current compilation — *is* the output, by construction | single-source jobs always; batch on demand |
| **AttentionList** | the "47 converted cleanly, 3 need your attention" summary: source, compromise, page | batch completion gate |
| **DiagnosticView** | every compromise for one source — what shrank, what rotated, what clipped, on which page | when a flagged source is opened |
| **AdjustmentPanel** | only the elements the diagnostic named, with their available overrides | when a flagged element is selected |
| **JobSettings** | destination, template, page size, orientation, base font for this job | job setup |
| **CollisionPrompt** | the conflicting path, existing file, and the four choices incl. apply-to-all | on CollisionDetected |
| **BatchProgress** | count converted / flagged / failed while running | during batch |

Every one of these is derived from the diagnostic or the source list. There is no state a read model
needs that the pipeline does not already produce — which is the payoff of treating the layout pass as
something that *emits* rather than *discards*.

---

## Hotspots

✅ **RESOLVED — `measure()` / `layout()` verified against Typst 0.15.1.** Both exist in the assumed
shape and compose as needed. Metadata emitted from inside a `layout()` callback is queryable from
outside the compilation, which is how the Diagnostic escapes Typst. **New constraint discovered:**
`pagebreak()` and `page(flipped: true)` are illegal inside `layout()`, so the ladder must decide in a
probe pass and act in a render pass. See `design/spike-typst-measure-findings.md`.

✅ **RESOLVED — ElementId.** It is the key the probe pass and the render pass agree on, assigned by
md2pdf when it emits the Typst markup. Stable by construction, because md2pdf generates the markup
rather than inferring structure from Typst's tree. Element-scope Overrides are entries injected into
the same decision map, so a user's override and an engine decision travel one identical channel.

✅ **RESOLVED — recompile latency.** 97 pages / 120 elements measured at ~425 ms for the full two-pass
via CLI, including process startup. Measurement roughly doubles compile time on a document larger and
more pathological than a realistic target. Preview-on-every-Override stays viable; the embedded
library will be faster.

✅ **RESOLVED — what an "element" is,** and the answer changed the model. Elements divide into
**atomic** (table, image, fixed-width block) and **wrappable** (prose, list, quote, code). Only atomic
elements can overflow horizontally. Neither obvious predicate worked: `natural > available` is true for
everything including content that wraps perfectly, and `measure(el, width: available)` is **clamped to
the available width in every case**, so it never detects overflow at all. The correct predicate is
`class is atomic AND measure(el).width > available` — decidable at emission, because md2pdf emits a
table precisely because the markdown had one.

✅ **RESOLVED — fonts.** Typst 0.15.1 embeds exactly four faces (Libertinus Serif, New Computer
Modern, New Computer Modern Math, DejaVu Sans Mono) and **no sans-serif at all**, so md2pdf ships its
own FontBook. Decided by rendering the same specimen through four candidates:

- **Body: Source Sans 3** (OFL) — most compact of the four, so directly fewer pages; Adobe designed
  the Source family for print *and* screen; humanist and neutral, which is closer to GitHub's actual
  system-font stack than any geometric face.
- **Mono: JetBrains Mono** (OFL).
- **Rejected: Jost**, the honest Avenir substitute — its U+2022 renders as a midpoint-sized dot, and
  bulleted lists are among the most common constructs in the corpus. Geometric faces also degrade
  faster at the 7–9pt Floors the ladder drives text toward, so the geometric instinct fights the
  ladder.
- **Avenir itself is not available** — Monotype-licensed; a desktop licence does not grant application
  bundling. Templates may *name* a system font, and a user who does so knowingly trades away
  cross-platform determinism.

Bundling an own FontBook is what makes identical PDF output across macOS/Windows/Linux achievable —
and testable, since the project has both a Linux container and a macOS host.

❗ **Glyph coverage beyond Latin is unverified.** Only Latin was tested. "GitHub fidelity" implies at
least Latin-1 plus common symbols, arrows, and box-drawing characters; needs a coverage pass before
the FontBook is fixed.

❗ **The embedded library path is unverified, and all residual risk now sits there.** Every probe used
the `typst` **CLI**; md2pdf embeds the crate and cannot shell out. `typst eval` is a CLI feature, and
the library-side introspection equivalent has not been proven. The crate API is explicitly unstable
across releases — pin 0.15.1.

❗ **Clipping (the last rung) was never probed.** Lower risk than rotation, but unverified.

❗ **Code blocks wrap in 0.15.1**, so a code Floor may be solving a problem that does not exist. The
open question is typographic rather than mechanical: wrapped code misleads the reader, so md2pdf may
*prefer* shrink-then-rotate over Typst's default wrap. A template decision, now evidence-based.

❗ **Remote images** *(open item 4)* — fetching breaks "no network"; skipping breaks GitHub fidelity.
Leaning is skip with a visible placeholder and a diagnostic entry, which fits the compromise model
exactly: it is a concession the app made and should therefore be recorded like any other.

❗ **`pulldown-cmark` GFM extensions** *(open item 5)* — confirm which are on by default vs. opt-in
before assuming tables, footnotes, task lists, strikethrough, and autolinks are available.

❗ **Floor values and the rotate threshold** *(open item 3)* — starting points ~9pt body, ~7pt
table/code. Tunable template tokens, so settle by eye during the first real conversions, not now.

❗ **All filesystem access must sit behind one module from the first commit.** Not a question — a
decision from the distribution section — but it is a hotspot because it is cheap now and a rewrite
later if the Mac App Store ever requires security-scoped bookmarks.

❗ **Bundle identifier must be set before the first release**, since macOS derives config and data
directories from it and changing it later strands user settings.

**Resolved, recorded here so they are not re-litigated:** collision policy (prompt, never silently
overwrite) and batch output shape (mirror the source tree). Both are settled in the stack decisions
under "Files and output"; the doc's own open-items list still shows them as open — that list is stale.

---

## Bounded Context Candidates

**Core — where the product's value lives**

1. **Conversion** — markdown in, Typst markup out. Owns: SourceParsed, MarkupEmitted, image
   resolution, unsupported constructs. Pure and total: text → text, no I/O. The single most testable
   part of the app.

2. **Typesetting** — Typst markup + template in, compilation + diagnostic out. **Two passes, chosen
   for cost rather than forced:** the *probe pass* measures and decides; the *render pass* acts but
   never measures. A single pass **is** possible — `page(flipped: true)` is legal inside a plain
   `context` block, and only `layout()` forbids it — but it benchmarks at roughly **2× the cost**,
   because measuring inside a paginating document is far more expensive than measuring outside one.
   The probe therefore computes available width from the Template (page minus margins) and **never
   calls `layout()`**, which is 3.6× cheaper than probing through a full document layout and yields
   identical decisions. The DecisionMap between the passes is plain data, which makes the boundary a
   natural place to inject user Overrides. The diagnostic is this context's second first-class output,
   not a side effect — and since it must cross out of Typst as queryable metadata anyway, it is
   host-side data by construction.

3. **Review** — owns the diagnostic once sealed: preview, the attention gate, and the three
   adjustment scopes. Consumes the diagnostic, produces overrides that trigger recompilation.

**Supporting**

4. **Job** — selection, destination resolution, template choice, and the batch run. The orchestrator
   over 1–3; the imperative shell.

5. **Template Catalogue** — discovery from a directory, token parsing, validation, rejection reasons.
   Deliberately not compiled in, so user-authored templates are first-class from day one.

6. **Output** — path computation, source-tree mirroring, collision detection and resolution, writing.

**Generic**

7. **Path Access** — the single module every filesystem path passes through. Exists as its own
   context purely so that a future sandboxing requirement is a swap rather than a rewrite.

8. **Settings** — persisted defaults in the platform config directory (default destination, chosen
   template, window state).

The seam that matters most: **Conversion and Typesetting are both pure functions** — text and values
in, text and values out. Compilation, rastering, file reads, and file writes live in Job, Output, and
Path Access. That is functional core / imperative shell falling out of the domain rather than being
imposed on it, and it means the escalation ladder can be tested without a filesystem or a window.

Hand off to `define-bounded-contexts` for interface definitions between 1 → 2 → 3.
