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

`ImageManifest` is plain data (`Vec<(String, PathBuf)>` or a map) and lives in `md2pdf-domain`, so
convert can produce it without depending on paths or typeset. **No crate boundary moves.**

### I4 · Image sizing and the ladder

`ElementClass::Image` is Atomic and `shrinks_by_scale()`, but **no real image has been through the
probe** — every image test so far used a placeholder box.

**Decision:** emit `#image("<virtual>")` with no explicit width, letting intrinsic size drive
layout, and let the existing ladder do its job. Verified, not assumed: a deliberately oversized
image must escalate — shrink by scale, then rotate — exactly as `oversized_figure_escalates_to_rotate`
already pins for a rectangle.

---

## Work breakdown

| | Task | Deliverable | Tests |
|---|---|---|---|
| **T10** | **`World` file map** in `md2pdf-typeset` | `add_file`/`clear_files` on `TypstWorld` + `Typesetter`; `World::file()` serves it | Contract: image renders; **stale-bytes pin** (swap bytes, measurement changes); unknown name still errors cleanly |
| **T11** | **`images.rs`** in `md2pdf-convert` | path resolution, `ImageProbe` trait, virtual naming, remote/missing policy | Stub-probe: resolved / missing / remote; collision of same-basename files; **path-escape refusal** (`../../etc/passwd`); name stability across runs |
| **T12** | **Wire it through** | `SourceContext` (I1), `ImageManifest` (I3), `emit` uses real `#image` when resolved | End-to-end: a real PNG appears in the PDF; a missing one gives a placeholder + `ImageMissing`; a remote one gives `ImageSkipped` |
| **T13** | **Ladder on real images** | — | Oversized image escalates (shrink → rotate); a small image is left alone |

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
- **SVG, and format conversion.** Typst's native support decides what works; anything it rejects is
  an `UnsupportedConstruct`, not a converter feature.
- **`Caption` class.** `ElementClass::Caption` exists and nothing produces it yet; figure captions
  are a Template question, deferred to 3e.

## Still open, carried forward

- `documents/` untracked · `/phase-audit` unavailable in this environment · SDLC tooling decision
- Glyph coverage beyond Latin (before phase 6) · nested-atomics `ponytail:`
