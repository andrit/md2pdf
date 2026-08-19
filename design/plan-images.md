# Plan — Images (phase 3b, Stage 2)

**Written:** 2026-08-19 · **Follows:** phase 3a, `plan-conversion-crate.md` (T5–T8 complete).
**Goal:** a local image in a markdown file appears in the PDF. Missing and remote images degrade
visibly and are recorded, never fail the document.

Deliberately shorter than the conversion plan. Most of the shape was settled there (§2.5, §3, D2,
D4); this fixes the four things that were not, and two probes have already removed the guesswork.

---

## What the probes established

Both were prototyped and reverted; neither is in the tree.

**1. The `World` file map works.** `RefCell<HashMap<FileId, Bytes>>` on `TypstWorld`, served from
`World::file()`, compiled `#image("diagram.png")` against planted PNG bytes — ProbePass OK,
RenderPass OK, a 3121-byte PDF containing the image.

**2. `comemo` does *not* serve stale bytes.** The risk was real enough to test: `Typesetter` holds a
long-lived `World` precisely so memoisation survives between compilations, and swapping image bytes
behind that cache could plausibly have gone unnoticed. It does not. Replacing the bytes under the
**same** virtual name changed the measured natural width from 10pt to 200pt on the next probe —
`comemo` tracks the `World::file` access and invalidates correctly.

*Consequence:* a plain mutable file map is safe, virtual names may be reused across recompiles, and
no cache-busting scheme is needed. That removes the largest unknown in this phase.

---

## The four decisions this plan makes

### I1 · `convert()` needs to know where the Source lives

`convert(&str)` — shipped in T8 — cannot resolve `![](./img/x.png)`, because a relative path is
relative to the `.md` file and no directory is passed.

**Decision:** add a second parameter carrying the conversion's *context*, rather than widening the
signature again later:

```rust
pub fn convert(markdown: &str, ctx: &SourceContext) -> Conversion

pub struct SourceContext<'a> {
    /// Directory of the Source. Relative image paths resolve against it.
    pub source_dir: &'a Path,
    /// Existence check, injected — a pure crate cannot stat a file.
    pub images: &'a dyn ImageProbe,
}
```

- Keeps `md2pdf-convert` pure: it takes a *description* of the world, never touches it.
- One parameter to add now, not one per future need.
- `SourceContext::none()` for callers with no filesystem (tests, stdin): no directory, an
  always-absent probe, so every image degrades to a placeholder exactly as in Stage 1.

**`convert()` stays total.** No `Result` appears: an unresolvable path is a `Compromise`.

### I2 · Virtual names must be unique and stable

Two files named `diagram.png` in different folders collide in a flat `FileId` namespace, and an
unstable name would defeat memoisation on recompile — the hot path for Element overrides.

**Decision:** the virtual name is derived from the **resolved absolute path**:

```
img-<fnv1a(absolute_path)>.<ext>      e.g. img-9f3a1c77b2e40d51.png
```

- **Unique** — different paths differ.
- **Stable** — the same file yields the same name every compile, so `comemo` keeps its hit.
- **Deduplicating** — the same image used twice is stored once.
- The extension is preserved because Typst infers format from it.
- `fnv1a` already exists in `md2pdf-domain::element`; reuse it rather than adding a hash crate.

Names are *not* derived from the markdown text — the same document converted from a different
directory legitimately points at different files.

### I3 · The manifest is the seam between three crates

Convert cannot read bytes; typeset cannot touch the filesystem; only `md2pdf-paths` may. So:

```
convert  →  Conversion { elements, compromises, images: ImageManifest }
                                                 └─ virtual name → absolute path
engine   →  reads bytes via md2pdf-paths (the only crate permitted to)
typeset  →  Typesetter::add_file(virtual_name, bytes) before probe/render
```

`ImageManifest` is plain data and **no crate boundary moves**.

> **Superseded by T12(b), 2026-08-19:** this originally placed the type in `md2pdf-domain`. It lives
> in `md2pdf-convert` beside `Conversion` instead — nothing below convert needs it, so putting it in
> the shared vocabulary would widen the domain for a single consumer. Shape settled as a
> `BTreeMap<String, PathBuf>`, for dedup by construction and deterministic ordering.

### I4 · Image sizing and the ladder

`ElementClass::Image` is Atomic and `shrinks_by_scale()`, but **no real image has been through the
probe** — every image test so far used a placeholder box.

**Decision:** emit `#image("<virtual>")` with no explicit width, letting intrinsic size drive
layout, and let the existing ladder do its job. Verified, not assumed.

> **Corrected by T13, 2026-08-19:** verification found the ladder does **not** do its job for
> images — the shrink rung steps font size, which cannot move an image, so every oversized image
> skips straight to Rotate; and `#scale` without `reflow: true` does not change the space the
> content occupies. T13 is therefore a fix task, not a test task. The emission decision itself
> stands, except that images are wrapped in a `box` so they are inline-level (T12).

---

## T11 in detail — `images.rs` *(planned 2026-08-19)*

Pure path arithmetic and policy. No I/O: existence arrives through the injected probe.

```rust
pub trait ImageProbe {
    fn exists(&self, path: &Path) -> bool;
}

/// What a markdown image destination resolved to.
pub enum ImageRef {
    /// Embed it. `virtual_name` goes in the markup, `absolute` in the manifest.
    Resolved { virtual_name: String, absolute: PathBuf },
    /// Named so the placeholder and the Compromise can say which file.
    Missing { shown: String },
    Remote { shown: String },
    Unsupported { shown: String, why: &'static str },
}

pub fn resolve(dest: &str, source_dir: Option<&Path>, probe: &dyn ImageProbe) -> ImageRef;
```

Classification runs in this order, first match wins:

| Destination | Outcome |
|---|---|
| `http://…`, `https://…`, any `scheme://`, or protocol-relative `//host/…` | `Remote` |
| `data:…` | `Unsupported` — see (e) |
| extension not one Typst reads | `Unsupported` |
| absolute path | resolve as-is |
| anything else | join to `source_dir` |

Then: probe for existence → `Resolved` or `Missing`. No `source_dir` (stdin, tests) → every relative
path is `Missing`, which is exactly Stage 1 behaviour.

### Decisions

**(a) Path traversal is ALLOWED — this corrects the work breakdown below.**
The table originally called for "path-escape refusal (`../../etc/passwd`)". That is wrong for this
product. md2pdf is a **local desktop tool converting the user's own files**; `![](../assets/logo.png)`
is legitimate and common, and refusing it would break ordinary documents to defend a boundary that
does not exist. There is no sandbox here and no untrusted author — the user already has read access
to everything the app can reach.

If a sandboxed build ever needs a policy (Mac App Store, security-scoped bookmarks), it belongs in
`md2pdf-paths` — the crate that actually opens files — not in a pure resolver that only does string
arithmetic. Recorded rather than silently dropped.

**(b) Supported extensions, from the source** (`typst-library/src/visualize/image/mod.rs:352-357`):
`png`, `jpg`, `jpeg`, `gif`, `webp`, `svg`, `svgz`, `pdf`. Matched case-insensitively. Anything else
— including no extension at all — is `Unsupported`, because Typst infers format from the extension
and a mystery file would fail the whole compilation.

**This also corrects "Deliberately not in this phase":** SVG needs no work. It is native, and
excluding it was a mistake.

**(c) Percent-decoding.** `![](my%20image.png)` is what an editor writes for a filename with a
space, and GitHub decodes it. Decode `%XX` in local destinations. Hand-rolled — it is ten lines and
does not justify a dependency.

**(d) Strip query and fragment** from local destinations (`img.png?v=2#frag`). Meaningful on the
web, never part of a filename.

**(e) `data:` URIs are `Unsupported` for now.** GitHub renders them, so this is a real fidelity gap,
and it is *solvable* — decode to bytes, key the virtual name off a hash of the bytes. But the
manifest is path-based (I3), so supporting it needs a bytes-carrying variant plus a base64 decoder.
Deferred deliberately, not overlooked.

**(f) `fnv1a` must become public.** It is private in `md2pdf-domain::element`. I2 reuses it for
virtual naming, so it gets promoted to a documented public function — still explicitly
not-cryptographic, since it detects edits rather than resisting attacks.

**(g) Dedup is best-effort.** `./a.png` and `a.png` normalise to the same name; a symlink or a
case-different spelling of the same file will not, because collapsing those needs the filesystem
and this crate has none. Two entries for one file wastes a read, and is not wrong.

### Tests

Stub `ImageProbe` backed by a `HashMap`, so every case runs with no filesystem: resolved, missing,
remote, unsupported extension, no extension, `data:` URI, percent-encoded name, query/fragment
stripping, absolute path, traversal **succeeding**, same basename in two directories producing
**different** virtual names, the same path producing an **identical** name across runs (stability
matters — an unstable name defeats memoisation on recompile).

---

## T12 in detail — wiring it through *(planned 2026-08-19)*

T11 built the resolver; nothing calls it. T12 is the plumbing, and it is the task that
**changes a shipped public API**, so the decisions are about blast radius as much as design.

### The shape

```rust
pub struct SourceContext<'a> {
    /// Directory of the Source. `None` when no file backs the markdown.
    pub source_dir: Option<&'a Path>,
    pub images: &'a dyn ImageProbe,
}

impl SourceContext<'_> {
    /// No filesystem: every image degrades to a placeholder. Stage 1 behaviour.
    pub fn none() -> Self { … }
}

pub fn convert(markdown: &str, ctx: &SourceContext) -> Conversion

pub struct Conversion {
    pub elements: Vec<Element>,
    pub compromises: Vec<Compromise>,
    pub images: ImageManifest,   // NEW — virtual name → absolute path
}
```

### Decisions

**(a) One entry point, not two.** The alternative is keeping `convert(&str)` as a convenience
wrapper over `convert_in(&str, &SourceContext)`. Rejected: T8 declared `convert()` "the crate's
whole public surface", and two ways to start a conversion invites the filesystem-less one to be
chosen by accident — silently degrading every image in a document that had real ones.

`SourceContext::none()` makes "I have no filesystem" **explicit at the call site**, which is the
property worth having. Cost is 9 call sites and the doctest, all mechanical.

**(b) `ImageManifest` lives in `md2pdf-convert`, beside `Conversion` — not in the domain.**
I3 originally said domain. Revisited: nothing *below* convert needs the type. `md2pdf-paths`
receives a `&Path`, not a manifest; `md2pdf-typeset` receives a name and bytes. Only the engine
consumes it, and the engine already depends on convert. Widening the shared vocabulary for a single
consumer is the wrong trade.

Shape: a `BTreeMap<String, PathBuf>`. The map deduplicates by construction (one file referenced
twice is stored once), and `BTree` ordering makes the manifest **deterministic**, which matters
because tests compare it and batch conversions should not vary run to run.

**(c) Compromises become specific — a behaviour change.** Today every image emits
`UnsupportedConstruct { construct: "image: d.png" }`, because Stage 1 had no better information.
Now:

| `ImageRef` | Compromise | Emitted |
|---|---|---|
| `Resolved` | none | `#image("img-<hash>.<ext>")` |
| `Missing` | `ImageMissing` | placeholder naming the file |
| `Remote` | `ImageSkipped` | placeholder naming the URL |
| `Unsupported` | `UnsupportedConstruct { construct }` carrying the `why` | placeholder |

`ImageMissing` and `ImageSkipped` have existed in `CompromiseKind` since the domain was written and
have never been constructed. This is where they start being true.

**(d) Alt text is dropped once an image resolves.** It is a fallback, and GitHub shows the picture
rather than the text. It stays in the placeholder, where it is the only thing left to show. A PDF
has no alt-text channel to put it in, so nothing is lost that could have been kept.

**(e) `emit` takes the context and returns the manifest.** `emit(&[Block], &SourceContext) ->
Emitted { elements, compromises, images }`. The `Emitter` already accumulates `unsupported`; it
gains a manifest accumulator alongside.

### The failure guard, restated at the level it actually operates

`emit::tests::images_become_placeholders_never_image_calls` asserts **"Stage 1 must not emit
`#image`"**, and that becomes false in T12 — a resolved image must emit exactly that.

But it was never a test of Stage 1 behaviour. It is a **failure guard**, and the failure it guards
against is total: a file reference Typst cannot satisfy fails the **whole document**, so the output
is not a PDF with a missing picture, it is no PDF at all.

So the invariant it protects was never "do not emit `#image`". That was a *proxy*, exact in Stage 1
only because the satisfiable set was empty. The real invariant does not change in T12:

> **never emit a file reference md2pdf cannot satisfy**

State it that way and the test gets **stronger**, not narrower:

> every `#image("name")` in the emitted markup has a matching entry in the `ImageManifest`

Vacuously true in Stage 1 (no `#image` at all), meaningfully true in Stage 2, and it catches what
neither phrasing would — a resolved image whose manifest entry is dropped, or a name that disagrees
between emission and manifest. Those are precisely the mistakes this task's plumbing can introduce,
and each one fails the whole document.

**This becomes a property assertion over the whole corpus**, not a single case: extract every
`#image("…")` name from every emitted element and require it in the manifest. Cheap, and it holds
for free in every future test that converts anything.

### Tests

- Resolved image: markup contains `#image("img-…")`, manifest has one entry, **no** compromise.
- Missing / remote / unsupported: placeholder, no `#image`, exactly one compromise of the right kind.
- One image referenced twice: **one** manifest entry, two `#image` calls.
- Two different images: two entries.
- `SourceContext::none()`: unchanged Stage 1 behaviour across the whole corpus.
- **Manifest coverage, as a property over the whole corpus:** every `#image("…")` name emitted
  appears in the manifest. This is the failure guard above, and the one assertion that must never be
  weakened.
- End to end with `md2pdf-typeset`: feed the manifest through `add_file`, compile, confirm the PDF
  contains the image — and **look at the page**, since scale and placement pass assertions blind.

---

## T13 in detail — the ladder on real images *(planned 2026-08-19)*

**T13 was scoped as "tests only". That was wrong.** I4 said "let the existing ladder do its job,
verified not assumed" — verification says it does not. Three defects, found by putting a real
2000×300 image through the probe:

```
NATURAL 2000   AVAILABLE 196   RUNG Rotate     ← shrink skipped entirely
SCALE natural=2000 | SCALE+REFLOW natural=400
```

**1. Images can never shrink.** The probe's ladder steps *font size* down and re-measures:

```typst
while s >= floor and chosen == none {
  if measure(text(size: s, body)).width <= avail { chosen = s }
  s = s - 0.5pt
}
```

An image's width does not depend on font size, so every measurement returns the same number, no `s`
ever fits, and the element falls straight through to **Rotate**. The one class that can shrink
*losslessly* is the one class that never does — a slightly-too-wide diagram gets a landscape page of
its own instead of being scaled 10%.

**2. `#scale` does not affect layout.** Typst's docs: *"Scales content without affecting layout"*,
`reflow` defaults to `false`. `render.rs` emits `#scale({factor}%)[{body}]`, so a shrunk image is
drawn smaller while the layout still reserves its **full original width** — measured above: 2000pt
scaled to 20% still occupies 2000pt, and only `reflow: true` gives the expected 400pt. The shrink
rung would not have fixed the overflow even if it were reachable.

**3. The probe never emits `clip`.** `Rung::Clip` exists, `harvest` parses it, `render` implements
it — and `probe.rs` can only produce `none`, `shrink`, or `rotate`. The last rung is unreachable
code. This matches the spike's own open item ("clipping, the last rung, still unprobed") and it
affects **tables as much as images**.

Also inaccurate: `Rung::Rotate`'s doc comment says *"The RenderPass re-measures there"*. It does
not — the RenderPass never measures, by design. It renders at natural size on a landscape page and
hopes. Fixing the comment is part of this task; fixing the behaviour is not.

### Decisions

**(a) Image shrink is arithmetic, not a search.** For an atomic image the probe needs no loop:

```
required = available / natural
required >= 1            -> None
required >= floor_scale  -> Shrink by `required`
otherwise                -> Rotate
```

One measurement instead of ~20, and it is exact rather than quantised to 0.5pt steps.

**(b) A scale is not a font size — add `Rung::Scale { factor }`.**
The tempting zero-churn option is to reuse `Shrink { size_pt }` and emit `base_size_pt × required`,
because `render.rs` already divides by `base_size_pt` to recover a factor. Rejected on the
full-picture check: in the finished app the attention gate must say *"shrunk to 8pt"* for text and
*"scaled to 40%"* for an image, and an Element-scope Override must let the user **set** one or the
other. Laundering a ratio through a points field means every consumer reverses the arithmetic to
find out which fact it is holding.

Blast radius is real but contained: `Rung`, `render.rs`, `diagnostic.rs`
(`CompromiseKind::ShrunkToFloor` needs a scale sibling), and the tests that name rungs.

**(c) `Floors` gains an image scale floor.** Floors are per class and live with the design tokens.
Without one, shrink always succeeds and rotate never fires for images. Starting point **0.25** — a
diagram below a quarter size is unreadable, and rotation buys roughly 1.5× width, which is the
better trade. Tunable by eye in 3d, like the other floors.

**(d) `reflow: true` on the scale in `render.rs`.** Without it, (a) and (c) are decorative.

### Out of scope — new task T14, "finish the ladder"

Defect 3 is **not** image-specific, so fixing it inside an images task would bury it. After
rotation an element may still overflow, and nothing clips or marks it. Closing that means the probe
deciding rotate-and-scale together (the RenderPass cannot re-measure), plus emitting `clip` when
even landscape is not enough.

That is the escalation ladder finally being complete for **every** atomic class, and it deserves its
own task rather than riding along here. Recorded in the roadmap; the spike flagged it first.

### Tests

- A small image: `Rung::None`, unchanged.
- A slightly oversized image: **`Scale`**, not `Rotate` — the regression this task exists to prevent.
- A hugely oversized image (below the floor): `Rotate`.
- The scaled render actually **occupies less width** — the `reflow` pin, measured rather than eyeballed.
- A wide *table* still behaves exactly as before: this task must not disturb the text path.
- Visual: the scaled image looks right on the page and does not overlap the text beneath it.

---

## Work breakdown

| | Task | Deliverable | Tests |
|---|---|---|---|
| ✅ **T10** | **`World` file map** in `md2pdf-typeset` | `add_file`/`clear_files` on `TypstWorld` + `Typesetter`; `World::file()` serves it | Contract: image renders; **stale-bytes pin** (swap bytes, measurement changes); unknown name still errors cleanly |
| ✅ **T11** | **`images.rs`** in `md2pdf-convert` | path resolution, `ImageProbe` trait, virtual naming, remote/missing policy | Stub-probe: resolved / missing / remote; collision of same-basename files; traversal **allowed** (see T11(a)); same basename in two dirs; name stability across runs |
| ✅ **T12** | **Wire it through** | `SourceContext` (I1), `ImageManifest` (I3), `emit` uses real `#image` when resolved | End-to-end: a real PNG appears in the PDF; a missing one gives a placeholder + `ImageMissing`; a remote one gives `ImageSkipped` |
| ✅ **T13** | **Ladder on real images** | `Rung::Scale`, image floor, `reflow: true`, arithmetic shrink in the probe — **not tests-only** | Slightly-oversized image **scales** (not rotates); hugely oversized rotates; scaled render occupies less width; tables unaffected |
| **T14** | **Finish the ladder** *(new — all atomic classes, not just images)* | probe decides rotate-and-scale together; `clip` finally emitted | Element still over after rotation clips and shows a marker |

Each closes with `verify.sh` green. **Add test modules under `tests/compiler/`** — a new top-level
file in `crates/*/tests/` that links typst will OOM the linker.

**Visual check before the phase closes.** An embedded image is exactly the kind of thing that can
pass every assertion and still look wrong (wrong scale, wrong position, black box). Raster a page
and look at it — that is how the italic bug was caught.

---

## Exit criteria

1. A markdown file with a local image converts to a PDF **containing that image**, confirmed by eye.
2. Missing and remote images produce visible placeholders and one `Compromise` each; the document
   still converts.
3. An oversized image escalates through the ladder.
4. `verify.sh` green; `/phase-audit` run, or explicitly waived as in 3a.

## Deliberately not in this phase

- **Image caching across a batch.** Re-reading a shared logo 50 times is wasteful, not wrong.
  `ponytail:` it if it shows up.
- **Remote fetching.** Breaks "no network" (D4).
- **Format conversion.** Typst's native support decides what works; anything it rejects is an
  `UnsupportedConstruct`, not a converter feature. *(SVG is **in** scope — it is native. The earlier
  exclusion was wrong; see T11(b).)*
- **`data:` URIs** — solvable but needs a bytes-carrying manifest variant; see T11(e).
- **`Caption` class.** `ElementClass::Caption` exists and nothing produces it yet; figure captions
  are a Template question, deferred to 3e.

## Still open, carried forward

- `documents/` untracked · `/phase-audit` unavailable in this environment · SDLC tooling decision
- Glyph coverage beyond Latin (before phase 6) · nested-atomics `ponytail:`
