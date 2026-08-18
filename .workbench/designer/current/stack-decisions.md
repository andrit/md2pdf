<!-- COPY for md2pdf's Knowledgebase. Canonical source: the WORKBENCH repo at design/md2pdf/stack-decisions.md — edit there, then re-copy. Copied 2026-08-16, after md2pdf was scaffolded (the 'not yet registered' line below was true when written and is now superseded). -->

# md2pdf — Stack & Scope Decisions

**Written:** 2026-08-16 · **Status:** pre-project record. md2pdf is **not yet registered**; it will live in `/projects`, not `factory/apps/`.
**Origin:** began as `mdpub` (`mdpub-markdown-hosted-publisher`, a hosted markdown publisher in `factory/apps/mdpub`). Scope and thesis changed enough that the name changed with it.

---

## What it is

**A local desktop utility that converts markdown files to PDF for printing and sharing.**

Not a site generator. Not an editor. Not hosted.

| | |
|---|---|
| **Primary output** | PDF |
| **HTML output** | **OUT OF SCOPE — that is a different product** |
| **Editing markdown** | **OUT OF SCOPE — permanently** (see "why this matters") |
| **Backend / accounts / network** | none |
| **Data** | the user's own local folder |
| **Running cost** | ~zero |
| **Trajectory** | personal tool first → cheap product or freeware |

---

## The stack

| Layer | Choice | Why |
|---|---|---|
| Language | **Rust** | one binary, native performance |
| UI | **egui** (via `eframe`) | no webview, no frontend build, ~half the memory of a webview app |
| Markdown parse | **`pulldown-cmark`** | fast CommonMark + GFM extensions, native Rust |
| Typeset → PDF | **Typst**, embedded as a library | excellent print typography, compiles in ms, no sidecar |
| Preview | **Typst page raster** | preview *is* the output — fidelity guaranteed by construction |
| Settings | `directories` + serde | platform config dir |

Every layer is Rust. One executable. No network.

---

## Decision trail — what was rejected and why

Recorded because the rejections carry more information than the picks.

### Hosted publisher → local desktop tool
The hosted framing carried a **per-user marginal cost forever** (storage, bandwidth). That is survivable for paid SaaS and fatal for freeware. A desktop tool publishing to the user's own disk has ~zero marginal cost — which is what makes "cheap product or freeware" a viable ending rather than a slow bleed. **The packaging choice is what makes the business model possible.**

### Dropping md→HTML — the decision that resolved three others
Cutting HTML output killed the only serious objection to Typst (maintaining CSS for web *and* Typst templates for print, forever). It also made "converter-first" true, which is what then made egui right. **One scope cut resolved the engine, the framework, and the product thesis.**

### Electron — rejected
Chosen at one point and then reversed. Its advantage was one bundled Chromium rendering *both* preview and PDF (`webContents.printToPDF` is native — puppeteer never needed). That advantage evaporated once Typst became the PDF engine: with Typst rendering the document, the UI layer no longer touches output fidelity. Left behind ~100MB of bundle for nothing.

### Tauri — rejected (and my earlier objection to it was wrong)
Originally argued against on the grounds that its *system* webview differs per platform (WKWebView / WebView2 / WebKitGTK), which would make **PDF output differ by platform**. That was a correctness argument and it was correct — until Typst took over rendering. Then the webview only drew buttons and lists, which all three engines do identically, and the objection was dead.

Tauri was then re-recommended on the grounds that "polish is cheaper in CSS" and it looks product-ready. **Both were withdrawn when the designer said the utilitarian look was actively wanted, not merely tolerated.** With aesthetics no longer an argument for it, egui wins on memory, startup, and having fewer moving parts.

### Slint / iced — rejected
**Slint:** GPL / royalty-free-with-conditions / paid commercial. A licensing decision, not a technical one, and unresolved — avoid it while a price tag is on the table. **iced:** meaningful API churn across versions plus a thin widget set (tables, rich text, tree views built by hand).

### Pandoc for md→typst — rejected
~150MB dependency doing a thousand things this needs none of. `pulldown-cmark` → emit Typst markup is bounded work because **Typst's markup is already markdown-shaped** (`= Heading`, `*bold*`, `_italic_`, `- list`). Most of the converter is mechanical.

### PDF-viewer dependency — avoided
Preview by rasterising Typst's own pages rather than embedding a PDF viewer. One compile serves preview and export; drops pdf.js entirely.

### Knowledgebase inheritance from the factory — rejected
Considered so md2pdf could inherit the factory's 77 KB documents. Rejected on inspection: of those 77, ~45 are reusable build knowledge, ~6 are factory pipeline state, and **~8 are *other apps'* design artifacts** (propflow's bounded contexts, glossary, event storm). Inheriting all of them would import documents engineered to mislead. Moot for md2pdf anyway — the reusable ~45 are SaaS-stack skills (Clerk, Stripe, Supabase, R2, OWASP) and **a local Rust desktop app needs approximately none of them.**

---

## Settled scope

### Conversion targets — "GitHub fidelity"
**The bar: however markdown renders on GitHub or a top-tier web viewer is how the PDF should look.** Testable, which is why it's a good spec.

- GFM: tables, strikethrough, task lists, footnotes, autolinks
- **Code highlighting: let Typst do it.** Typst's `raw` blocks highlight by language natively — pass the language through, no `syntect` needed
- Tables and footnotes: native in Typst
- Images: resolve relative to the source `.md` and embed

### Files and output
- **Both batch and individual** conversion
- A **persisted default output folder**, overridable per job
- Fallback chain: job folder → default folder → prompt if unset
- **Collision policy: prompt to rename**, showing the conflict itself as the reason. No silent overwrite.
  - *Design consequence:* in batch, a naive prompt-per-conflict is unusable at 50 files. The prompt needs **"apply to all"** (rename all / skip all / overwrite all) or it becomes the worst part of the app.
- **Batch output shape: mirror the source subfolder structure** inside the target directory. Not flat.

### Layout — measured fit, not a fixed overflow policy

**This is the capability that most justifies choosing Typst, and a browser could not have provided it.**

Typst offers grid layout (`grid()`, `stack()`, with `fr` / `auto` / fixed track sizing) *and* — the important part — **`measure()` and `layout()`**, which let the document inspect the actual rendered size of content and the space available **during compilation**, then branch on it. Typst is a programming language, so layout can be conditional.

That converts the overflow problem from a single global policy into a **per-element decision made after measuring**:

| Measured against available width | Response |
|---|---|
| fits | render normally |
| slightly over | step the font down within a floor |
| substantially over | rotate that element to landscape on its own page |
| extreme | clip, with a visible marker |

So the "self-inspection step before print" is native, not something to build around. Content is placed in page grid sections and resized to fit based on what it actually measures.

**Honest limits:** Typst's grid is not CSS Grid — different mental model, no subgrid. `measure()` costs compile time (fine at document scale). And the exact `measure`/`layout` API surface should be verified against the current Typst version before building on it.

#### Floors — plural, and they drive the escalation

Auto-shrink without a floor silently produces unreadable output. But the floor does more than guard readability: **it is what triggers the next strategy.**

```
measure → over? → shrink toward floor
        → at floor and still over? → rotate to landscape
        → still over? → clip + visible marker + diagnostic entry
```

Element classes tolerate shrink differently, so floors are **per class, not global** — body prose ~9pt (read linearly, fatigue matters), tables and code ~7pt (scanned, not read). **Floors live in `template.toml`** with every other design token: a dense technical template legitimately wants lower floors than a letter template.

#### The layout pass emits a diagnostic, not just output

The measure step already knows every compromise it made — what shrank, what rotated, what clipped, on which page. **Capture that rather than discarding it.** The diagnostic is what makes preview and adjustment tractable.

### Preview — mandatory, but exception-driven in batch

**No conversion commits to file without a preview opportunity.** Single-file always previews (it's free — Typst compiles in ms).

Batch cannot preview 50 documents, and doesn't need to. The diagnostic says which ones had a judgment call made on their behalf:

> **47 converted cleanly. 3 need your attention.**
> `api-reference.md` — table on p.4 shrunk to floor, still overflowing
> `setup.md` — code block rotated to landscape (p.2)
> `changelog.md` — 2 long lines clipped (p.7)

**The gate fires where a decision was actually made**, not on every artifact.

### Adjustment affordances — three tiers

Mirrors the default-with-override pattern already established for output folders, keeping one mental model across the app:

| Tier | Scope | Controls |
|---|---|---|
| **Template** (`template.toml`) | persistent default | page size, margins, type scale, floors |
| **Job** | this conversion | page size, orientation, base font, template choice |
| **Element** | *only where the diagnostic flagged it* | force landscape, allow clip, permit below-floor for this element |

Tier 3 exists **only because the diagnostic identifies the elements.** This is not a general layout editor — it offers a fix precisely where the engine already admitted it compromised.

**Why this matters strategically:** CSS has no "measure this, then choose a different layout" primitive without JavaScript, and print CSS especially cannot do it. The Chromium path would have forced one blunt global overflow rule. This is a capability *gain* from the Typst decision, not just a typography one.

### Templates
**v1 ships exactly one**, and "GitHub, printed nicely" defines it.

**Templates are swappable config, not code.** A template is a folder that can be authored, traded, and dropped in without touching the app:

```
templates/github-print/
  template.toml    — metadata + design tokens (page size, margins, fonts,
                     type scale, colours, code-block styling, furniture)
  template.typ     — the layout itself
```

The `.toml` tokens cover the common case, so someone can produce a new template by copying the folder and editing values. The `.typ` is there when real layout control is needed. **The shipped template doubles as the reference example** — which is the cheapest possible documentation.

Consequence to design for now: templates are **discovered from a directory**, not compiled in. That keeps user-authored templates first-class from day one and leaves room for sharing/distribution later.

A template is the **print stylesheet** — page size, margins, body and heading type scale, line height, page furniture (numbers, running headers), and element styling (code blocks, table rules, blockquotes, image captions). It does not compete with the document's structure; markdown supplies semantics, the template decides what those semantics look like on a page.

Multiple templates matter later, when the same markdown needs *different presentations* (technical doc vs. letter vs. handout). A differentiator to earn, not v1 scope.

### Project type — `custom` for now
No existing type fits. `cli` is closest in spirit but this has a GUI. A **`desktop` type is genuinely warranted eventually** — desktop has SDLC phases no web type has: multi-platform builds, code signing, notarization, auto-update, release channels.

But the workbench's own convention says wait: `subject-seed` moved to the shared home when a **second** caller appeared, not on speculation. **Register `custom`; let md2pdf be the evidence that defines a `desktop` type later** — the same pre-project→graduate model the designer proposed for factory apps, applied to types.

---

## Open — decide before or during v1

1. **Collision policy.** `notes.md` → `notes.pdf` when it already exists: overwrite, skip, or auto-suffix? Batch makes this frequent.
2. **Batch output shape.** Flat into one folder, or mirror the source subfolder structure?
3. ~~Overflow policy~~ — **RESOLVED** by measured fit + per-class floors + escalation ladder + exception-driven preview (see Layout above). Remaining: the **actual floor values** per element class (starting points ~9pt body / ~7pt table+code) and the rotate-to-landscape threshold — both tunable template tokens, so they can be set by eye during the first real conversions rather than decided up front.
4. **Remote images.** Fetching breaks "no network"; skipping breaks GitHub fidelity. Leaning: skip with a visible placeholder, and say so.
5. **`pulldown-cmark` GFM extensions** — confirm which are on by default vs. opt-in.
6. **Batch conflict prompt** — "apply to all" affordance (see Files and output).

---

## Distribution reality (v1.0, not day 1)

Modern OSes distrust downloaded software that isn't signed by an identified developer.

- **macOS / Gatekeeper** — unsigned downloads read as *"unidentified developer"* or *"damaged and can't be opened"* (the quarantine attribute makes it look corrupt). Proper path: Apple Developer Program **~$99/yr** → Developer ID cert → sign → **notarize** (automated scan, minutes) → staple.
- **Windows / SmartScreen** — *"Windows protected your PC"* with **Don't run** as the default. Authenticode cert **~$100–400/yr**; standard certs must accumulate download reputation before warnings stop, EV certs (**~$300–600/yr**, hardware token) get it immediately. Since ~2023 CAs require keys on hardware tokens, complicating CI.
- **Linux** — no equivalent gate.

**~$200–500/year before a single sale.** That is the live tension with "freeware." Alternatives all have costs: ship unsigned with instructions (kills non-technical adoption), technical-audience-only distribution, or the app stores — where **Mac App Store sandboxing would restrict filesystem access, which is hostile to a folder-based tool.**

This is the argument that may force a price rather than freeware. Worth settling before v1.0, not at it.

### Position: defer the cost, don't get locked out

**Decision (2026-08-16): freeware is impractical at ~$200–500/yr for zero revenue.** Do not buy certificates now. Handle distribution when we get there — but take the cheap steps that keep every door open. All four cost nothing today:

1. **Set a proper reverse-DNS bundle identifier from day one** (e.g. `com.<org>.md2pdf`). On macOS the config and data directories derive from it, so changing it later strands user settings and breaks any future update path.
2. **Put all folder access behind one module.** If the Mac App Store is ever a target, sandboxing forbids raw persisted paths and requires **security-scoped bookmarks**. Isolated, that's a swap; spread through the codebase, it's a rewrite. This is the single most plausible lock-out and the cheapest to prevent.
3. **Keep signing as an optional post-build step** in the build script, not something the pipeline is shaped around. Adding it later should be config, not restructuring.
4. **Don't commit to a license yet.** Freeware vs. paid is undecided; adopting a viral licence now would foreclose the paid option. Leave it proprietary/unspecified until the call is made.

Explicitly **not** doing now: buying certificates, building auto-update, targeting app stores.

---

## Emerging `desktop` type shape *(feeds roadmap C26)*

Captured **as md2pdf is built**, so the type is authored from evidence rather than invented. Roadmap C26 defers creation until a second desktop app exists.

What a desktop type needs that no current type provides:

- **Packaging/installer phase** instead of a deploy phase — `.app`/`.dmg`, `.msi`/`.exe`, AppImage/`.deb`
- **Multi-platform build matrix** as a first-class SDLC concern, not an afterthought
- **Code signing + notarization** as an explicit gated phase with real cost and lead time
- **Update strategy** — channels, or an explicit decision not to auto-update
- **No `/health`, no deploy target, no env-var config** — the software-class must-haves assume a server and simply do not apply
- **Local data locations** — platform config/data dirs, and their migration story across versions
- **Distribution surface** as a deliverable: download page, checksums, install instructions per OS

Skills that would belong to it: platform packaging, signing/notarization runbook, local-settings persistence, native file-dialog patterns, release/versioning for shipped binaries.

---

## Why "no editing" matters more than it looks

egui's weakest area is text input — basic editing, weaker IME, no system spell-check, non-native selection. **That weakness is the entire reason egui is safe here:** md2pdf consumes markdown, it never edits it, so the weak surface is never exercised.

If md2pdf ever grows an editor — and *"let me just fix that typo before exporting"* is an extremely natural request — **egui becomes the wrong framework in the app's most-used surface.** Editing being permanently out of scope is not a product preference; it is a load-bearing constraint of the stack choice.

Treat any future editing request as a **framework decision**, not a feature request.
