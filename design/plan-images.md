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

## Work breakdown

| | Task | Deliverable | Tests |
|---|---|---|---|
| ✅ **T10** | **`World` file map** in `md2pdf-typeset` | `add_file`/`clear_files` on `TypstWorld` + `Typesetter`; `World::file()` serves it | Contract: image renders; **stale-bytes pin** (swap bytes, measurement changes); unknown name still errors cleanly |
| **T11** | **`images.rs`** in `md2pdf-convert` | path resolution, `ImageProbe` trait, virtual naming, remote/missing policy | Stub-probe: resolved / missing / remote; collision of same-basename files; traversal **allowed** (see T11(a)); same basename in two dirs; name stability across runs |
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
- **Format conversion.** Typst's native support decides what works; anything it rejects is an
  `UnsupportedConstruct`, not a converter feature. *(SVG is **in** scope — it is native. The earlier
  exclusion was wrong; see T11(b).)*
- **`data:` URIs** — solvable but needs a bytes-carrying manifest variant; see T11(e).
- **`Caption` class.** `ElementClass::Caption` exists and nothing produces it yet; figure captions
  are a Template question, deferred to 3e.

## Still open, carried forward

- `documents/` untracked · `/phase-audit` unavailable in this environment · SDLC tooling decision
- Glyph coverage beyond Latin (before phase 6) · nested-atomics `ponytail:`
