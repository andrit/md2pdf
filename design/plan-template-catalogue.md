# Plan — the template catalogue (3e)

**Written:** 2026-08-24 · **Phase:** 3e, the last engine phase before the review surface (3f) and
the app itself (4).
**Closes:** `INV-11` — *"Templates are swappable config, discovered from a directory."*

Claims marked **[measured]** or **[assumed]**, per *Planning discipline* in `design/invariants.md`.
Options laid out in full and scored against the goal, per *Cost is not a reason to decline an
option*.

---

## Where this actually stands

**[measured]** `templates/github-print/` exists and is **empty**. `Template::default()` is a Rust
constructor, and `INV-11` says templates must not be compiled in — so the invariant is currently
false, and has been since it was written.

**[measured]** what a Template controls today, in full: page width, page height, margin, base size,
three floors, body font, mono font. `render.rs` turns those into a seven-line Typst preamble; nothing
else about the page is adjustable.

**[measured]** the GLOSSARY defines a Template as a *folder of two files*:

```
templates/github-print/
  template.toml    — metadata + Tokens
  template.typ     — the layout
```

`template.typ` does not exist and nothing reads one. **That gap is the whole decision in this plan.**

## The goal

> **A person can copy the shipped template folder, edit values, and get a visibly different PDF —
> without rebuilding md2pdf.**

That is the roadmap's exit criterion for 3e, and it is the thing phase 4 needs: a desktop app whose
template picker has more than one entry.

---

## The decision: does 3e ship `template.typ`?

### Option A · Tokens only — `template.toml`, no `template.typ`

`Template` is deserialised from TOML instead of constructed in Rust. `catalogue.rs` discovers
folders, parses them, and reports rejects with reasons. `render.rs` is untouched — it still builds
the preamble from tokens.

- **An author can change:** page size and orientation, margins, base size, the floors, both fonts.
- **An author cannot change:** heading scale, colours, table borders, spacing, anything needing a
  Typst show-rule.
- **Cost:** ~1 day. One new module, a serde derive, a TOML file, CLI wiring, tests.
- **Risk:** low, and contained — a bad TOML rejects the template rather than breaking a document.
- **Against the goal:** meets it. A4→Letter, 12pt→11pt, a different font are all visibly different.
- **Against the GLOSSARY:** contradicts it. Either `template.typ` gets a named follow-on task or the
  glossary entry is corrected. **Silently shipping half the definition is the one thing not on offer.**

### Option B · Tokens plus a `template.typ` md2pdf imports

The template folder supplies Typst show-rules; generated source imports them and emits only
structure. Authors get real styling control.

- **Cost:** ~3–4 days, and **[assumed]** most of it is not the loading — it is the two failure modes
  below.
- **Risk 1 — the probe must see the same template.** The ProbePass measures against its own
  preamble. If `template.typ` changes text size or spacing through show-rules and the probe does not
  apply them, **every measurement is wrong and the whole escalation ladder decides against a page
  that does not exist.** T29–T31a were four tasks about exactly this class of error. The template
  would have to be applied identically in both passes, and that needs a test that can see it.
- **Risk 2 — a template can break the document.** A Typst error in author code fails the *whole*
  compilation, not one element. That needs a validation compile at load and a `TemplateRejected` with
  a real message, or a user's first edit produces "compilation failed" and no page.
- **Against the goal:** exceeds it.

### Option C · Option A now, `template.typ` as its own task

Ship tokens; scope `template.typ` as **T33** with the two risks above as its actual content.

- **Cost:** ~1 day now, ~3–4 later, slightly more in total than doing B once.
- **Against the goal:** meets it now, and keeps the phase-4 date.

**Recommendation: C.** Not because B is expensive — that reasoning is what `invariants.md` now
forbids — but because **B's cost is almost entirely in a correctness risk that deserves its own
measurement**, and bundling it into a phase whose stated exit is "edited values produce a different
PDF" would hide it. The probe/render divergence in Risk 1 is the same shape as the four estimator
defects, and it should be a task with an oracle rather than a paragraph inside another task.

**What C costs you honestly:** until T33, "template" means the ten tokens above. A user wanting a
different heading scale cannot have one, and the GLOSSARY entry stays half-true with a pointer to
the task that finishes it.

---

## The change (Option C)

### 1 · `Template` gains serde and a file format

Already `Serialize + Deserialize`. What is missing is a *file*:

```toml
name = "github-print"
description = "GitHub's rendering, sized for print"

[page]
width_pt = 595.0     # A4
height_pt = 842.0
margin_pt = 56.0

[text]
base_size_pt = 12.0
font_body = "Source Sans 3"
font_mono = "JetBrains Mono"

[floors]
table_pt = 7.0
table_comfort_pt = 10.0
image_scale = 0.25
```

Grouped rather than flat, because a flat list of ten numbers is what `Template` looks like in Rust
and a person editing a file wants the page separated from the type. **[assumed]** a nested serde
representation distinct from the in-memory struct is worth the mapping code; the alternative is
exposing field names chosen for Rust.

### 2 · `catalogue.rs` in a new `md2pdf-template` crate

**Not in `md2pdf-domain`** — domain is plain data with no I/O, and discovery is I/O. **Not in
`md2pdf-engine`** — the app (phase 4) needs the catalogue without running a Job.

```rust
pub struct TemplateCatalogue { pub found: Vec<Found>, pub rejected: Vec<Rejected> }
pub struct Rejected { pub path: PathBuf, pub reason: String }
```

**Rejects are carried, not dropped** (GLOSSARY): template authoring is a supported activity, and a
silent disappearance is the worst possible feedback. Reads through `PathBroker` — `INV-9`.

### 3 · Validation, with named reasons

A template is rejected when: the folder has no `template.toml`; the TOML does not parse; a required
token is missing; a number is out of range (a negative margin, a floor above the base size); or a
**named font is not in the FontBook**. That last one matters — `INV-1` means we never fetch a font,
so a template naming "Helvetica" must say so at load rather than silently rendering in a fallback.

### 4 · The shipped template becomes the reference example

`templates/github-print/template.toml`, with the values `Template::default()` currently hardcodes and
comments explaining the non-obvious ones — the 12pt base and the 10pt comfort floor both have
reasoning worth carrying to a template author.

### 5 · `Template::default()` stays, and is renamed in spirit

It becomes the **fallback when no catalogue is available**, not the definition. The CLI loads from
disk; if the directory is missing it falls back and says so. **[assumed]** removing it entirely would
make every test construct a template from a file, which is a large change to tests for no gain.

### 6 · CLI: `--template <name>` and `--templates <dir>`

Searching the three roots in D2. An unknown name lists what *was* found, because "template not
found" without the list is the least useful error available.

## Exit criteria — **all met 2026-08-24**

1. ✅ A copied folder with `base_size_pt = 16` and US Letter page **rendered and looked at**: visibly
   larger type on a 612×792 page, no rebuild.
2. ✅ A malformed template is rejected by name with the TOML complaint — *"expected `.`, `=` (TOML
   parse error at line 2, column 6)"* — and the run converts everything else.
3. ✅ A template naming Helvetica is refused at load: *"font not available: Helvetica — md2pdf never
   fetches a font"*. Exit 2, nothing written.
4. ✅ **The shipped TOML is `Template::default()`**, pinned by `the_file_that_ships_is_the_rust_
   default` reading the real file through `include_str!`. Goldens did not move.
5. ✅ Census unchanged; `verify.sh` green; 22 new tests.

## What was learned building it

**The font check could not live where the plan put it.** Knowing which fonts exist means holding the
FontBook, which lives in the one crate allowed to link typst. So `discover_with_fonts` takes the
predicate as a parameter and the *rejection* still lands in the catalogue, where an author will look
for it. Dependencies passed rather than reached for — the project's own rule, arriving as a
constraint rather than a preference.

**Two tests were wrong in ways worth recording**, both found by running them:

- `PathBuf::join` uses the *running* platform's separator, so the Windows test asserting
  `…Roaming\md2pdf` failed on Linux with a forward slash. Correct behaviour, wrong assertion. It now
  checks structure — APPDATA is the parent, `md2pdf` is the leaf — which is what actually has to hold.
- `toml`'s error Display is a span diagram whose *first* line is `TOML parse error at line 6,
  column 1`. Taking the first line gave an author the location and never the complaint; the sentence
  that names their mistyped key is the last line.

## Doubts — audited

### D1 · Does loading from disk break determinism? — **[assumed] no, but it must be checked**

`INV-7` says the same markdown produces byte-identical PDFs, and the goldens rest on it. The template
becomes an *input* rather than a constant, which is fine — but only if the shipped TOML reproduces
`Template::default()` exactly. Exit criterion 4 is that check, and a drift of 0.5pt anywhere would
turn the goldens red for a reason that looks like a rendering change and is not.

### D2 · Where do templates live at runtime? — **decided 2026-08-24: the user config directory, now**

I proposed deferring this to phase 5. **The operator overruled it, and the reasoning is better than
mine:** md2pdf is a personal tool, the config directory is the right answer for one, and deferring it
means shipping a location we already know is wrong and then migrating anyone who used it.

**Three roots, merged, in precedence order:**

| | Where | Why |
|---|---|---|
| 1 | `--templates <dir>` | explicit beats everything; how a test or a one-off run pins a directory |
| 2 | user config — `<config>/md2pdf/templates/` | where a person's own templates live and survive an upgrade |
| 3 | beside the binary — `./templates/` | the shipped `github-print`, which is also the reference example |

**Merged rather than first-wins**, so `github-print` is always available even when a user has their
own. A name collision resolves to the *earlier* root, so a user can shadow the shipped template by
copying it and editing — which is exactly the workflow the exit criterion describes.

**The config directory is resolved by hand**, because **[measured]** neither `dirs` nor `directories`
is in the offline registry and `INV-1` forbids fetching one. It is a pure function of environment
variables, which makes all three platforms testable from this one:

```
Linux    $XDG_CONFIG_HOME/md2pdf   else  $HOME/.config/md2pdf
macOS    $HOME/Library/Application Support/md2pdf
Windows  %APPDATA%\md2pdf
```

**[assumed]** hand-rolling is the smaller risk here. A crate would be better-tested against real
platforms, but the rules above are short, stable, and the failure mode of getting one wrong is a
directory that is simply not found — visible immediately, not silently corrupting anything.

### D3 · Is a new crate justified for one module? — **[assumed] yes**

Six crates become seven. The alternative is `catalogue.rs` in `md2pdf-engine`, which makes the phase-4
app depend on the batch engine to list templates. Narrow crates are the existing pattern and the
boundary script already polices them.

### D4 · Should `template.typ` be scoped now? — **named, not planned**

**T33**, with the probe/render divergence as its first doubt. Naming it now is the difference between
a deferral and an omission.
