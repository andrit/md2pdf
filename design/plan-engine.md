# Plan — the engine, walking skeleton (phase 3c1)

**Written:** 2026-08-20 · **Follows:** 3b2. The pure core is complete; nothing above it exists.
**Goal:** `notes.md` on disk becomes `notes.pdf` on disk. One file, no batch, through every layer.

This is the project's first **vertical** slice. Everything so far was horizontal — complete layers
with nothing above them — which was right while the risk sat in one layer, and is now the thing to
correct: layers that pass their own tests can still fail to meet each other, and every integration
defect this project has hit was exactly that.

---

## Scope — deliberately one file

**In:** read a Source, convert it, register its images, probe, render, write a PDF, report what
happened.

**Out, and each is a *widening* of a path that already works:** directory walking, source-tree
mirroring, collision resolution and blanket answers, template discovery, the attention gate,
overrides, recompilation.

The point is to prove the seams, not to be useful yet.

---

## Decisions

### E1 · The `ImageProbe` adapter lives in the engine, not in `md2pdf-paths`

`plan-conversion-crate.md` §2.5 says *"the real implementation lives in `md2pdf-paths` and is
injected by the engine"*. **It cannot.** `ImageProbe` is defined in `md2pdf-convert`, and `md2pdf-paths` depends only
on `md2pdf-domain` — it cannot see the trait.

Two ways out, and the choice matters beyond this task:

- Move `ImageProbe` into `md2pdf-domain` so both can see it. Widens the shared vocabulary with a
  port that only one crate implements.
- **Implement the adapter in the engine**, wrapping `PathBroker`. The engine already depends on both,
  and it is the composition root — wiring ports to adapters is precisely its job.

*Chosen: the second.* `struct BrokerImages<'a>(&'a PathBroker)` in the engine. No dependency is
added, no crate boundary moves, and `md2pdf-paths` stays a pure filesystem crate that knows nothing
about conversion. The conversion plan is corrected in place.

### E2 · An unreadable image fails the Job — the TOCTOU gap, named

`convert` asks the probe *does this exist?*, then the engine reads the bytes a moment later. Between
those two the file can vanish, or be unreadable for permissions. That gap is unavoidable without
holding an open handle, and it has a sharp edge:

**The markup already references the virtual name.** If the read fails and the engine registers
nothing, Typst gets a reference it cannot satisfy — and that fails the *whole document*, which is
the failure the manifest-coverage guard exists to prevent.

Options: fail the Job loudly, or degrade the image to a placeholder and re-convert.

*Chosen: fail the Job*, with `JobError::ImageUnreadable { path }`. It is rare, it is honest, and the
message names the file. Degrading would mean converting twice for a case that essentially only
happens when something else is already wrong. Recorded so 3f can revisit it if real use disagrees.

### E3 · 3c1 refuses to overwrite; it does not "not have a policy"

Collision handling is 3c2. But "no policy yet" cannot mean *silently overwrite* — the stack decisions
are explicit that output is never silently overwritten, and a skeleton that quietly destroys a file
is worse than one that stops.

So 3c1 **errors** if the OutputPath exists (`JobError::OutputExists`). 3c2 replaces that error with
`Resolution` / `BlanketResolution`. The guarantee holds from the first commit rather than being
retrofitted.

### E4 · The contract starts minimal, and is expected to be wrong

One `Command`, a handful of `Event`s, all plain serializable data — no lifetimes, closures, or trait
objects, so the same shape works in-process on a channel or across a boundary as line-delimited JSON.

```rust
pub enum Command {
    ConvertSource { source: PathBuf, destination: PathBuf },
}

pub enum Event {
    SourceParsed { elements: usize },
    MarkupEmitted { images: usize },
    CompilationSucceeded { pages: usize },
    CompilationFailed { message: String },
    OutputWritten { path: PathBuf },
    Failed { message: String },
}
```

Event names come from the event storm, so the vocabulary is not invented here.

**A contract designed against a single case fits that case only.** Expect 3c2 to reshape it once
batch, collisions and progress exist — and prefer reshaping it there, while the only consumer is a
test, over discovering the problem in phase 4 when an adapter depends on it.

### E5 · `md2pdf-paths` is where `std::fs` finally appears

`check-boundaries.sh` has been enforcing "no `std::fs` outside `md2pdf-paths`" over a crate that
contains no filesystem code at all. This is the task where the guard starts guarding something.

`PathBroker` is a struct, not a trait — there is exactly one implementation and the seam that
matters (sandboxing, security-scoped bookmarks) is *inside* it, not above it.

```rust
impl PathBroker {
    pub fn read_to_string(&self, path: &Path) -> Result<String, PathError>;
    pub fn read_bytes(&self, path: &Path) -> Result<Vec<u8>, PathError>;
    pub fn write(&self, path: &Path, bytes: &[u8]) -> Result<(), PathError>;
    pub fn exists(&self, path: &Path) -> bool;
}
```

### E6 · Template comes from `Template::default()`

The catalogue is 3e and `templates/` is still empty. Wiring a real template loader here would build
3e badly rather than early. Noted so the skeleton's use of a hardcoded default is a deliberate
placeholder, not an oversight.

---

## The sequence

```
Command::ConvertSource
  → PathBroker::read_to_string(source)
  → convert(&markdown, &SourceContext { source_dir, images: &BrokerImages(&broker) })
  → for (name, path) in conversion.images:  broker.read_bytes(path) → typesetter.add_file(name, ..)
  → typesetter.probe(&elements, &template)      → DecisionMap + Diagnostic
  → typesetter.render(&elements, &template, &map) → Compilation
  → compilation.pdf()
  → PathBroker::write(output_path, &pdf)
  → Event::OutputWritten
```

`Typesetter::clear_files()` is called **per Job**, not per Source — the file map is keyed by resolved
path, so sharing across a batch is correct and desirable, and clearing per Source would discard the
`comemo` hit the long-lived `World` exists to keep.

## T16 in detail — the contract *(planned 2026-08-20, revised same day)*

Decisions cite `design/invariants.md` rather than re-deriving the finished product. Where no
invariant applies, the rule is: **build the simple thing.**

### C1 · Events go to a plain closure sink — *revised*

> **Superseded 2026-08-20.** The first version specified an `EventSink` trait, justified as
> "keeps closures out of the contract types". That reason does not hold — the sink is not a
> contract type either way — and the decision fails the gate test: swapping a sink type later is a
> mechanical refactor the compiler finds for you. Cheap now *and* cheap later means **build the
> simple thing.** Recorded rather than quietly deleted, because it is a clean example of the
> failure mode `invariants.md` exists to prevent.

```rust
pub fn handle(command: Command, deps: &Deps, emit: &mut dyn FnMut(Event));
```

An adapter passes `|e| tx.send(e)`; a test passes `|e| collected.push(e)`. Nothing to implement.

### C2 · Failure is an **Event** — `INV-8`

An adapter may run out-of-process, so it sees the event stream and nothing else. A failure reported
only through a `Result` is invisible across the exact boundary the contract exists to support.

`handle` therefore returns **nothing**. Every outcome — success, compilation failure, unreadable
source, refused overwrite — is emitted. `job.rs` uses `Result` internally, and `handle` converts at
the edge.

> The earlier draft called the return value "a convenience for in-process callers", which would
> have re-created the two-channel ambiguity this decision exists to remove. One channel.

### C3 · Events carry the Compromises — `INV-4`

Information not emitted cannot be recovered later. `Compromise` derives `Serialize`, and the whole
domain was **verified to round-trip through JSON** before this was committed to — all seven
`CompromiseKind` variants and all four `Reduction` variants, including the internally tagged ones.

Honest limit: an adapter currently receives `{order, content_hash}` and `page: null`, which it
cannot yet render as "table on p.4". Carrying them is about not destroying information, not about
present usefulness.

### C4 · The engine names the types; the adapter picks the wire format

`md2pdf-engine` depends on `serde` but **not** `serde_json` — the CLI holds that. The contract is
"plain serializable data", not "JSON". `serde_json` is a dev-dependency here, for the round-trip
test only.

### C5 · One conversion event, not two — *new*

The draft had `SourceParsed { elements }` and `MarkupEmitted { images }`, taken from the event
storm. **The API cannot honestly emit both.** `convert()` parses and emits atomically; the engine
only ever sees a finished `Conversion`, so two events would claim a granularity that does not exist
and carry timestamps that are fiction.

One event, reporting what actually happened at the only moment the engine can observe:

```rust
Event::SourceConverted { elements: usize, images: usize, compromises: Vec<Compromise> }
```

The event storm describes the **domain**, not the API surface. Where they disagree, the API tells
the truth about what it can observe.

### The contract

```rust
pub enum Command {
    ConvertSource { source: PathBuf, destination: PathBuf },
}

pub enum Event {
    SourceConverted { elements: usize, images: usize, compromises: Vec<Compromise> },
    CompilationSucceeded { pages: usize },
    CompilationFailed { message: String },
    OutputWritten { path: PathBuf },
    Failed { message: String },
}
```

Internally tagged, snake_case: `{"event":"output_written","path":"…"}`.

### Not decided — no invariant applies

`#[non_exhaustive]`, template selection in `Command`, correlation/job ids. All three are listed in
the gate test's "build the simple thing" column.

### Tests

- Every variant survives a JSON round-trip (`INV-8` — the property that makes an out-of-process
  adapter possible, and the one that rots silently when a field is added carelessly).
- Serialised tag names asserted explicitly, so a rename is a deliberate wire-format change.
- A closure sink collects events in order.

## T17 in detail — the Job *(planned 2026-08-20)*

The sequence is settled above; E1/E2/E3 decided the hard parts. Four things remain, and one of them
would have stopped the build.

### J1 · The engine's tests cannot use `std::fs` — `INV-9`

`scripts/check-boundaries.sh` greps **every** `.rs` under `crates/` except `md2pdf-paths`, test
files included. The engine's tests need temp directories, so the obvious `std::fs::create_dir_all`
helper would fail the boundary gate immediately.

Three bad answers and one good one:

- Relax the script to exempt `tests/` — weakens a guard that has already caught a real leak.
- Add `remove_dir_all` to `PathBroker` — product API invented to serve tests; the app never deletes.
- Leave temp directories behind — no.
- **Move the helper into `md2pdf-paths` as `pub mod testing`.** Every `std::fs` call stays inside the
  one crate allowed to make them, and the engine's tests borrow it.

*Chosen: the last.* `md2pdf_paths::testing::TempDir` — creates a uniquely-named directory, removes
it on drop even if the test panics. The copy currently living in `broker.rs`'s tests moves there, so
there is one implementation rather than two.

This **strengthens** INV-9: the invariant survives contact with the first crate that genuinely needs
files, instead of being the first thing bent.

### J2 · `OutputPath` is computed in `md2pdf-paths`, not the engine — `INV-9`, `INV-12`

Even in the degenerate single-file case, deciding *where output goes* is path arithmetic and belongs
with the crate that owns paths — `mirror.rs` exists for exactly this and currently holds one line of
doc comment.

```rust
pub fn output_path(destination: &Path, source: &Path, source_root: Option<&Path>) -> PathBuf
```

3c1 passes `None`: the output is `destination/<source stem>.pdf`. 3c2 passes the SourceRoot and the
same function mirrors the tree (`INV-12`). Writing it in the engine now would mean moving it later
*and* putting path logic outside `md2pdf-paths` in the meantime.

### J3 · `handle` does not call `clear_files` — deliberately

The Typesetter is long-lived so `comemo` memoisation survives between compilations, and the file map
must be cleared **per Job, not per Source** (T12's finding — clearing per Source makes a batch
re-read every shared image).

In 3c1 one command *is* one Job, so clearing inside `handle` would look right and would bake in the
wrong granularity the moment a command covers many Sources. **Only the caller knows where a Job
begins**, because the caller owns the Typesetter's lifetime.

So `handle` leaves it alone and the doc comment says why. This is the rare full-picture check that
removes work rather than adding it.

### J4 · `JobError` is internal; the edge converts it — `INV-8`

`job.rs` uses a `Result` internally, because that is what Rust is for. `handle` converts at the
boundary:

| Internal failure | Emitted |
|---|---|
| Source unreadable, not UTF-8 | `Event::Failed` |
| Image unreadable (E2, the TOCTOU gap) | `Event::Failed` |
| Output already exists (E3) | `Event::Failed` |
| Typst compilation failed | `Event::CompilationFailed` |
| Output unwritable | `Event::Failed` |

Nothing important is reported only by returning it.

### Merge T18 into T17

T18 was "end-to-end tests over a temp directory" as a separate task. **That is the wrong split**:
CLAUDE.md requires tests alongside the code, and a walking skeleton whose tests arrive as a later
task is a skeleton nobody has walked. T17 lands with its tests; T18 is dropped rather than left as
an empty box to tick.

### Tests

- A markdown file with no images converts; the PDF lands at the expected path.
- **A real PNG on disk ends up in the PDF** — resolution, manifest, broker read and `add_file`
  meeting outside a stub for the first time.
- A missing image degrades to a placeholder, the Job still succeeds, and the Compromise reaches the
  event stream (`INV-4`).
- A source that does not exist emits `Failed` and writes nothing.
- An existing output path emits `Failed` and **leaves the original bytes untouched** (`INV-3`).
- Events arrive in order, ending with `OutputWritten`.

## Work breakdown

| | Task | Deliverable |
|---|---|---|
| ✅ **T15** | `md2pdf-paths`: `PathBroker` + `PathError` | read/write/exists; the first real `std::fs` in the tree |
| ✅ **T16** | `contract.rs`: `Command`/`Event` | plain data, serde round-trippable; failure travels as an Event (`INV-8`) |
| ✅ **T17** | `job.rs` + `BrokerImages` + `mirror::output_path` + `paths::testing` | one Source, disk to disk, with its tests |
| ~~T18~~ | *merged into T17 — tests ship with the code, not after it* | — |

## Tests

The engine's tests are the first in the project allowed to touch a real filesystem, and they must
clean up after themselves. A temp directory per test, created and removed by the test.

- A markdown file with no images converts and the PDF lands at the expected path.
- **A markdown file with a real image on disk produces a PDF containing it** — the whole point:
  resolution, manifest, broker read, and `add_file` all meeting for the first time outside a stub.
- A missing image degrades to a placeholder and the Job still succeeds, with the Compromise reported.
- A source that does not exist fails with a clear error, and writes nothing.
- An existing output path errors rather than overwriting (E3).
- The events emitted match the sequence, in order.
- Round-trip `Command` and `Event` through JSON — the property that makes an out-of-process adapter
  possible later.

## Exit criteria — all met 2026-08-20

1. ✅ `notes.md` on disk becomes `notes.pdf` on disk, with an embedded image, driven by a `Command`
2. ✅ nothing is ever silently overwritten — pinned by a test that checks the original bytes survive
3. ✅ `verify.sh` green, including the boundary check now that `std::fs` genuinely exists
4. ⚠️ `/phase-audit` not run — unavailable in this environment, as in 3a/3b/3b2

**No visual check this time, deliberately.** Rendering did not change: the same convert→typeset path
was confirmed by eye in T12 and T13, and what 3c1 added is file reading and path arithmetic, which
the tests assert exactly. Looking again would be ritual rather than evidence.
