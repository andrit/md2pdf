# Plan — the Conversion crate (`md2pdf-convert`)

**Status:** approved and in progress — T5, T6 complete · **Written:** 2026-08-18 · **Amended:** 2026-08-18
**Continues:** T1–T4 (the Typst anti-corruption layer), complete and verified.

The Conversion bounded context: **markdown in, Typst markup out. Pure and total, no I/O.**
It is the most testable code in the app and everything downstream is blocked on it.

---

## 1. The contract this crate must satisfy

These are not preferences. Each is a constraint the already-built `md2pdf-typeset` imposes,
verified by reading the code rather than assumed.

### 1.1 The body invariant — the one that matters most

`Element.body` is interpolated into Typst source in **two different syntactic positions**:

| Position | Where | Code |
|---|---|---|
| Raw, at top level | `Rung::None` | `render.rs:28` — `"{}\n#v(0.65em)\n"` |
| Inside a content block | Shrink / Rotate / Clip, and **every** probe | `render.rs:40,52,58`, `probe.rs:45,73` — `[{body}]` |

> **Invariant: every emitted body must compile, and render identically, in both positions.**

This is the crate's central correctness property. A body that is fine at top level but
unbalanced inside `[...]` breaks the *probe harness itself* — meaning the failure shows up as a
corrupted measurement of an unrelated element, not as an error pointing at the culprit.

**Test:** every element of the fixture corpus is pushed through both harness shapes via the real
`Typesetter`, and must compile clean and produce identical text. The working compile loop is the
oracle; nothing here is checked by eye.

### 1.2 `ElementId.order` must be unique across a document

`harvest.rs:27` resolves probe metadata back to elements with
`elements.iter().find(|e| e.id.order == order)`. A duplicate `order` silently binds a decision to
the wrong element. Contiguity is *not* required — uniqueness is. Emission assigns `order` from a
single monotonic counter, and a unit test asserts uniqueness over the corpus.

### 1.3 Bodies carry no trailing spacing

`render.rs` appends `#v(0.65em)` after each element. Bodies that end with their own vertical space
double it. Inter-element spacing belongs to the render pass; **intra**-element spacing is the body's.

### 1.4 Class drives the ladder, so classification is load-bearing

`ElementClass::is_atomic()` is `Table | Image` only. Misclassifying a table as prose means it
**never enters the escalation ladder** and silently overflows the page. Classification errors are
invisible in the output, which is why `classify.rs` gets exhaustive per-construct tests.

---

## 2. Module breakdown

Four stub modules exist. **This plan adds a fifth — `escape.rs`** — because escaping is the crate's
highest-risk logic and burying it inside `emit.rs` makes it untestable in isolation.

```
parse.rs     pulldown-cmark events  ->  internal block stream
classify.rs  block construct        ->  ElementClass
escape.rs    arbitrary text         ->  Typst-safe text        [NEW]
emit.rs      block stream           ->  Vec<Element>
images.rs    relative path          ->  resolved reference + policy
lib.rs       convert()              ->  Conversion
```

Dependencies flow one way: `escape` ← `emit` → `classify`/`images`, `parse` → `emit`. No cycles,
each unit testable without the others.

### 2.1 `escape.rs` — the highest-risk module

Markdown text is arbitrary; Typst markup is a programming language. Characters that are inert in
markdown are significant in Typst:

| Char | Typst meaning | Failure if unescaped |
|---|---|---|
| `#` | code expression | **arbitrary code injection** from document text |
| `[` `]` | content block delimiters | unbalanced `]` **terminates the probe harness early** |
| `*` `_` | strong / emphasis | silent wrong formatting |
| `$` | math mode | swallows text until the next `$` |
| `@` | label reference | broken reference, possible compile error |
| `<` `>` | label / syntax | context-dependent |
| `\` | escape character | escape-the-escape errors |

**This table is a starting point, not the answer.** The character set is derived **empirically**,
not from memory: a round-trip property test is the discriminator.

> **Round-trip property:** arbitrary input text → emit → compile via `Typesetter` → extracted
> rendered text **equals the input**.

Run over a corpus of adversarial strings (every ASCII punctuation char alone, doubled, and in
pairs; the `#let x = 1` injection case; unbalanced brackets; backslash runs). Anything that fails
round-trip goes in the escape set — the test *discovers* the set rather than confirming a guess.

### 2.2 `parse.rs` — events to blocks

`pulldown-cmark` emits a flat event stream; this crate needs **top-level blocks**. `parse.rs` folds
the stream into a block sequence, tracking nesting depth so a code block *inside* a list stays
inside that list's block rather than becoming a sibling.

**Options — settled from the vendored source** (`pulldown-cmark-0.12.2/src/lib.rs:520-600`),
which closes **open item 5**. Every non-CommonMark feature is opt-in; nothing is on by default.

**Enable:**

| Flag | Why |
|---|---|
| `ENABLE_TABLES` | GFM fidelity; tables are the primary Atomic class |
| `ENABLE_FOOTNOTES` | GFM fidelity; Typst has native footnotes |
| `ENABLE_STRIKETHROUGH` | GFM fidelity |
| `ENABLE_TASKLISTS` | GFM fidelity; extremely common in the corpus |
| `ENABLE_GFM` | GitHub alert blockquotes (`[!NOTE]`, `[!WARNING]`, …) |
| `ENABLE_YAML_STYLE_METADATA_BLOCKS` | **see D3** — off, front matter renders as visible garbage |

**Deliberately off:** `SMART_PUNCTUATION` (rewrites the user's text; a typographic choice that
belongs to the Template, and it breaks the round-trip property), `HEADING_ATTRIBUTES` (HTML-targeted,
no PDF meaning), `MATH` (emits TeX; TeX→Typst math translation is its own project),
`OLD_FOOTNOTES` (superseded), `DEFINITION_LIST` (not GFM), `PLUSES_DELIMITED_METADATA_BLOCKS`.

**Fidelity gap, recorded:** pulldown-cmark implements CommonMark autolinks (`<https://x>`) but
**not** GFM bare-URL autolinking (`https://x` as running text). Listed as a GFM target in the stack
decisions; not available from the parser. Ceiling, not a defect — needs a `ponytail:` comment.

### 2.3 `classify.rs` — construct to `ElementClass`

Pure lookup, one arm per construct, exhaustively tested. The whole module is a `match`.

```
Heading            -> Heading      Table            -> Table    (ATOMIC)
Paragraph          -> Prose        Image (alone)    -> Image    (ATOMIC)
List / TaskList    -> List         Image caption    -> Caption
BlockQuote / Alert -> Quote        CodeBlock        -> Code
```

`Code` is **Wrappable** — verified in the spike: `raw` blocks wrap in Typst 0.15.1. Do not
"fix" this to Atomic; the finding is recorded in `spike-typst-measure-findings.md`.

### 2.4 `emit.rs` — blocks to `Vec<Element>`

One top-level block becomes one `Element`. Nested content is emitted **into the parent's body**.
Class comes from the **outermost** construct.

**Known ceiling** (gets a `ponytail:` comment, not a v1 fix — now live in `classify.rs`):

```rust
// ponytail: an atomic table nested inside a wrappable blockquote inherits Quote,
// so it never enters the escalation ladder and can overflow.
// ceiling: nested atomics. upgrade: promote nested atomics to their own Elements.
```

#### Exception to one-block-one-Element: footnote definitions *(added 2026-08-18)*

**GFM footnote definitions must NOT become their own `Element`.** They are the one construct
where the rule above is wrong, and following it would print them as stray paragraphs.

GFM puts definitions at the *bottom* of the document as separate top-level blocks
(`Tag::FootnoteDefinition`), referenced inline by `Event::FootnoteReference`. Typst is the other
way round: `#footnote[...]` is **inline at the reference site**, and Typst places the note itself.

So emit runs in two steps:

1. Collect every `FootnoteDefinition` block into a map, keyed by label. These produce **no**
   `Element`.
2. When emitting a `FootnoteReference`, inline the matching definition's markup as
   `#footnote[...]`.

A reference with no matching definition renders as literal text (GFM's own behaviour). A
definition never referenced is dropped, and — since that is a concession — recorded via
`UnsupportedConstruct` rather than silently discarded.

**Verified:** `#footnote[...]` compiles and measures in **both** the ProbePass and the RenderPass,
including inside a table cell. It is safe in the measure-only harness despite being a page-level
construct.

#### Verified since this plan was written *(2026-08-18)*

Two assumptions were tested with a throwaway harness rather than trusted:

- **Block markup is *not* position-sensitive inside `[...]`.** `= Heading`, `- list`, `+ enum`,
  and `/ term` all render identically raw at top level and inside a content block — the markers
  are consumed as markup in both. The §1.1 invariant holds for structural markup, not just prose.
- **Links, task-list checkbox glyphs (`☐`/`☑`), and escaped brackets inside table cells** all
  compile and render correctly in both passes.

### 2.5 `images.rs` — resolution only, never reading

Path resolution is pure math. **Existence is not** — and a pure crate cannot stat a file. So the
existence check arrives **as a parameter**, per CLAUDE.md's dependency-as-parameter rule:

```rust
pub trait ImageProbe { fn exists(&self, path: &Path) -> bool; }
```

A `HashMap`-backed stub makes every image case testable with no filesystem. The real implementation
lives in `md2pdf-paths` and is injected by the engine.

Three outcomes, mirroring the three domain events (`ImageResolved`, `ImageMissing`,
`RemoteImageSkipped`):

| Case | Emitted | Compromise |
|---|---|---|
| Local, exists | image reference + manifest entry | — |
| Local, missing | visible placeholder | `ImageMissing` |
| Remote (`http(s)://`) | visible placeholder | `ImageSkipped` |

---

## 3. Cross-crate gap: images cannot currently be embedded

**`world.rs:77-79` — `World::file()` returns `NotFound` for every path.** Typst cannot load an image
that the `World` will not serve, so *no image can render today*, regardless of what this crate emits.

This is **typeset work, not convert work**, and this plan does not absorb it. The seam is designed
here so the two halves meet:

- Convert emits an image reference against a **virtual name** and returns a **manifest**
  (`virtual name → resolved absolute path`).
- The engine reads the bytes (via `md2pdf-paths`, the only crate permitted to touch the filesystem).
- Typeset gains a virtual-file map in `TypstWorld` that `World::file()` serves from.

Recommended split: **Stage 1 — every text construct, end to end. Stage 2 — images**, once the World
file map exists.

#### Correction: a missing file is a hard compile failure, not a skipped element *(2026-08-18)*

The paragraph above originally claimed Stage 1 "delivers a working pipeline for the majority of
documents." **That was wrong, and testing it is what showed why:**

```
image local   probe    ERR typst compilation failed: file not found (searched at diagram.png)
image local   render   ERR typst compilation failed: file not found (searched at diagram.png)
```

Typst treats an unresolvable file as a **compilation error for the whole document**, not as a
degraded element. So with no World file map, one `![](diagram.png)` anywhere means **no PDF at
all** — not a document with a missing picture. Images are common enough in real markdown that this
would have made Stage 1 useless on exactly the documents most likely to be tried first.

**Therefore `emit` must never produce `#image(...)` in Stage 1.** Every image becomes a **visible
placeholder** — a bordered box carrying the alt text — plus a `Compromise`, which is precisely the
shape D4 already approved for remote images. The placeholder path is a permanent capability
(it is what missing and remote images use forever), so this is not throwaway work.

`#image(...)` appears only in **T9**, once `World::file()` can serve bytes. Until then the
placeholder is the only image output, and Stage 1 genuinely does convert every document.

---

## 4. Cross-crate gap: convert-time compromises cannot reach the Diagnostic

`Diagnostic::from_decisions()` (`diagnostic.rs:45`) builds a Diagnostic **solely from the probe's
`DecisionMap`** — it maps Rungs, and nothing else. But `ImageMissing` and `ImageSkipped` are
`CompromiseKind` variants produced **at convert time**, before any Typst compilation exists.

`convert()` therefore returns its own `Vec<Compromise>` (with `page: None`), and **merging the two
sources is the engine's job** — the Diagnostic is sealed after both halves have reported.
`from_decisions` stays as-is; a `Diagnostic::merge` or engine-side assembly is the seam. Recorded
here so the engine plan inherits it.

`UnsupportedConstructEncountered` has **no** `CompromiseKind` variant at all — see **D1**.

---

## 5. Work breakdown

Sequenced by risk: the escaping property test is built first, because it is the oracle everything
else is validated against. TDD throughout, per CLAUDE.md — tests alongside or before the code.

| | Task | Deliverable | Tests |
|---|---|---|---|
| **T5** | **`escape.rs` + round-trip harness** | Typst-safe text emission; the escape set derived empirically | Round-trip property over adversarial corpus; the `#let` injection case |
| **T6** | **`parse.rs` + `classify.rs`** | events → top-level block stream; construct → class | Nesting fold (code-in-list, table-in-quote); exhaustive per-construct class table; `order` uniqueness |
| **T7** | **`emit.rs`** + **D1** | block stream → `Vec<Element>` with valid bodies; image placeholders; footnote absorption; `CompromiseKind::UnsupportedConstruct` added to the domain | Golden markup per construct; **both-positions compile test** (§1.1); no-trailing-spacing; `order` uniqueness |
| **T8** | **`lib.rs` public API** | `convert()` → `Conversion { elements, compromises }` | Full-document fixtures end-to-end through the real `Typesetter` |
| **T9** | **`images.rs`** *(Stage 2, gated)* | resolution + policy + manifest, `ImageProbe` injected | Stub-probe cases: resolved / missing / remote; path-escape refusal |

`verify.sh` must stay green at every step — boundaries, fmt, clippy `-D warnings`, test, build.

**Test location.** The round-trip and both-positions tests need a real compiler, so
`md2pdf-convert` takes a **dev-dependency** on `md2pdf-typeset`. This is acyclic (typeset does not
depend on convert) and does **not** trip `check-boundaries.sh`, which greps for `typst::` /
`typst_*::` — not for `md2pdf_typeset::`. The alternative, parking them in typeset's `tests/`, puts
convert's correctness tests in the wrong crate. Recommend the dev-dependency.

---

## 6. Decisions for the gate

These change what gets built. They are yours, not mine.

> **All five decisions were approved on 2026-08-18.** They are kept here as the record of what was
> decided and why. One ordering correction: **D1 lands in T7, not T8** — `emit` is the only place
> that knows a construct was unsupported, and once events are flattened into markup that
> information is gone.

**D1 · How is an unsupported construct recorded?**
No `CompromiseKind` variant exists for it, though `UnsupportedConstructEncountered` is a domain event.
→ *Recommend:* add `CompromiseKind::UnsupportedConstruct { construct: String }`. It is a concession the
app made on the user's behalf, which is exactly what a Compromise is, and it reaches the attention
gate for free. Small domain edit; schemas are the single source of truth, so it belongs there.

**D2 · Stage images separately?**
→ *Recommend:* yes. Stage 1 = T5–T8 (all text constructs, end to end), Stage 2 = T9 + the typeset
World file map. Images are blocked on typeset work regardless (§3); bundling them stalls a
deliverable pipeline behind an unrelated crate.

**D3 · YAML front matter.**
Very common in real `.md` files. With the flag **off**, `---\ntitle: x\n---` parses as a thematic
break plus a paragraph — i.e. **visible garbage at the top of every such PDF**.
→ *Recommend:* enable `ENABLE_YAML_STYLE_METADATA_BLOCKS` and skip the block. Cheapest correct
behaviour. (Rendering front matter as a title block is a Template question for later.)

**D4 · Remote images — confirming the standing assumption.**
`stack-decisions.md` leans "skip with a visible placeholder + diagnostic entry." This plan assumes
that. Confirm, or switch to skip-silently.

**D5 · Should `Markup` type the element body?**
`Markup` exists in the domain but **nothing constructs or consumes it** — `Element.body` is a bare
`String`.
→ *Recommend:* make it `Element { body: Markup }`. The glossary defines Markup as exactly this, and
typing it gives escaping a home the compiler enforces: a `Markup` can only be built through
escape-aware constructors, so "raw string reached the body" becomes a type error rather than a test
failure. Blast radius is small — `probe.rs`/`render.rs` interpolate via `format!`, and `Markup`
already implements `Display`.

---

## 7. Out of scope

Editing markdown (permanent — it is a stack constraint, see CLAUDE.md). HTML output. Math
translation. Bare-URL autolinks (parser limitation, §2.2). Floor values and the rotate threshold —
those get set by eye on real documents, which this crate is the prerequisite for.
