# Plan — the desktop app (phase 4)

**Written:** 2026-08-25 · **Phase:** 4, the first phase whose exit criterion cannot be checked in
this container.
**Rests on:** 3c–3f. The engine's Command/Event contract is what makes this an adapter rather than a
rewrite.

Claims marked **[measured]** or **[assumed]**, per *Planning discipline* in `design/invariants.md`.
Options laid out in full and scored against the goal, per *Cost is not a reason to decline an
option*.

---

## Two constraints found before planning, and both change the shape

### 1 · `eframe` is not in the offline registry — **I cannot compile this phase at all**

**[measured]** nothing matching `eframe`, `egui`, `winit`, `wgpu` or `glow` is in
`~/.cargo/registry`, and `INV-1` forbids fetching. This is stronger than "the container has no
display": with no display I could still typecheck. **I cannot typecheck.**

So every line of egui code I write here is unverified against its own API — and egui's API moves
between minor versions. Writing a thousand lines blind and handing them over is a bad trade whoever
pays for it.

### 2 · `TypstWorld` was `unsafe impl Send + Sync`, and a GUI is what breaks it — **fixed, see D3**

```rust
// Compilation is driven from one thread at a time; the RefCell never escapes.
unsafe impl Send for TypstWorld {}
unsafe impl Sync for TypstWorld {}
```

**[measured]** it holds `RefCell<Source>` and `RefCell<HashMap<FileId, Bytes>>`. The comment states an
invariant that has been trivially true so far — the CLI is single-threaded — and a desktop app is
precisely the thing that stops it being true. A UI thread that touches the `Typesetter` while a
worker compiles is **undefined behaviour**, not a race that shows up as a wrong pixel.

**This is a hard architectural constraint, not a caution.** It decides the threading model below.

---

## The goal

> **md2pdf runs on the operator's Mac: pick files, pick a template, see the pages, act on what needed
> attention.** Identity (5) and packaging (6) are explicitly deferred.

---

## The decision: how do we work, given I cannot compile it?

### Option A · I write the whole adapter blind; you compile and paste back errors

- **Cost to me:** low. **Cost to you:** a paste-loop through egui API errors I cannot see, on code I
  cannot test.
- **[assumed]** dozens of round-trips. The worst version of this is me guessing at `egui::Context`
  method names from memory across a version boundary.

### Option B · You `cargo vendor` on the Mac and commit it; I build here as normal

One `cargo vendor` with network on your machine writes every dependency into `vendor/`, which I can
then compile offline exactly like the rest of the workspace.

- **Cost to you:** one command, then a large commit — **[assumed]** 150–400 MB for the `eframe` tree
  (winit, wgpu or glow, and their platform crates). Unmeasured; you would see the real number before
  committing it.
- **Cost to me:** none. Full typecheck, clippy, tests.
- **Against the goal:** best. It restores the loop that has worked all week.
- **Against the repo:** a large vendored tree is a real weight, and `INV-1`'s spirit is offline
  reproducibility — which vendoring serves rather than violates.

### Option C · Split the phase so almost nothing is written blind

**`md2pdf-app`** — a crate with **no egui dependency**: the app's whole state machine, the worker
thread, the read models, the Override plumbing. I build and test it here as normal.

**`md2pdf-gui`** — the egui shell. Widget functions that draw a read model and return an intent.
**[assumed]** 300–500 lines with no logic in them, which is the only part written blind.

- **Cost:** no extra work — it is the architecture the project already mandates (*functional core,
  imperative shell*), and the Command/Event contract was built for exactly this.
- **Against the goal:** meets it, and shrinks the blind surface to the part where a compile error is
  obvious and local rather than semantic.

**Chosen 2026-08-25: C, then B.** They compose — C is the right
architecture regardless, and B removes the remaining blind spot. C alone is workable; C plus a
paste-loop for one thin file is a very different proposition from A.

**What C costs you honestly:** one more crate, and a boundary I will be tempted to leak logic across.
The check is mechanical — `md2pdf-gui` should contain no `if` that decides anything about a document.

**The ordering B needs, confirmed:** `cargo vendor` can only vendor a declared dependency, so
`md2pdf-gui` and its `eframe` line must exist first. **[measured]** `/workspace` and the host's
`/projects/md2pdf` are the same inode, so a `cargo vendor` run on the Mac lands in this tree and is
usable here immediately.

**And the honest limit of B: it buys `cargo check`, not a running app.** **[measured]** this container
has zero GL/X11 libraries, so an `eframe` binary cannot link here whatever is vendored. Typechecking
is the blind spot that matters and vendoring closes it; launching the app is yours either way.

**A consequence for the gate:** `verify.sh` runs `cargo build --workspace`, which would try to link
`md2pdf-gui`. It needs `--exclude md2pdf-gui` on the build and test steps plus a `cargo check -p
md2pdf-gui` step, so the GUI is typechecked in the gate without being linked in it.

---

## The architecture

### The threading model, forced by constraint 2

```
UI thread                    worker thread (owns Typesetter, Review)
  egui draws read models  ──Command──▶  convert / probe / render / raster
  never touches Typesetter ◀──Event───  events + rastered pages
```

**One worker, one `Typesetter`, never shared.** A `std::sync::mpsc` channel each way; the UI thread
has no handle to touch. Since D3 that is belt *and* braces — the `Typesetter` is now genuinely
`Send + Sync` rather than asserted to be, so the channel design is about keeping the UI responsive
rather than about avoiding undefined behaviour.

**[assumed]** one worker is enough — a batch is already sequential and `comemo` is process-global, so
a second compiling thread would contend on the cache T31 measured rather than halve the time.

### What the app is, in read models

The event storm named these, and the GLOSSARY says the widget functions carry the same names:

| Read model | Drawn as | Source |
|---|---|---|
| `SourceList` | `source_list()` | the chosen files and their per-source state |
| `TemplateCatalogue` | `template_catalogue()` | 3e, including the **rejected** ones and why |
| `Preview` | `preview()` | `Compilation::raster` — *is* the output, by construction |
| `AttentionList` | `attention_list()` | 3f |
| `AdjustmentPanel` | `adjustment_panel()` | 3f's `OfferedFix`, only where an Element was named |
| `CollisionPrompt` | `collision_prompt()` | 3c2 |
| `BatchProgress` | `batch_progress()` | the engine's events |

**Never invented names** — `attention_list()`, not `WarningsPanel`.

### What phase 4 is *not*

**No markdown editing, ever.** A load-bearing stack constraint rather than a preference: egui's
weakest surface is text input, and md2pdf never exercises it. Any future editing request is a
framework decision, not a feature.

## Exit criteria

**The first four can only be checked on your machine**, which is itself worth stating: this is the
first phase where "done" is not something `verify.sh` can tell us.

1. The app opens, files are chosen, and PDFs are written where you asked.
2. The preview shows the rendered pages — the real ones, rastered from the compilation.
3. `attention_list()` shows what needed a judgment call; clicking an offer **changes the preview**.
4. A collision prompts rather than overwriting (`INV-3`).
5. ✅ **done** — `md2pdf-app` has no egui dependency; 15 tests, including the worker thread driven
   for real: a Job streaming events back, a document opened and re-decided through the channel, a
   page arriving as pixels, a stale click reported, and the thread ending when the handle drops.
6. ☐ `verify.sh` green with `md2pdf-gui` typechecked but not linked — waits on the vendor step.

## Doubts — audited

### D1 · Is one worker thread really enough? — **[assumed] yes, and the failure is visible**

A batch of 146 documents takes ~30s (**[measured]**, T31). During that the UI must stay responsive,
which one worker gives — but the *batch itself* is not parallel. If that is too slow the fix is
parallelism across documents, which contends on the process-global `comemo` cache and would need
measuring rather than assuming. Not now.

### D2 · Does rastering every page on every recompile cost too much? — **must be measured on the Mac**

**[measured]** an Override recompile is 8ms steady state (3f) — but that excludes rastering, and the
preview needs pixels. `raster` at 2x on A4 is ~1200×1700×4 bytes per page. **[assumed]** rastering
only the visible page is necessary and sufficient; rastering all of them on every keystroke-scale
change is the obvious way this feels slow.

### D3 · Does the `unsafe impl` become *safe* under this design? — **done 2026-08-25: it is gone**

It took the ten minutes it was worth, and the answer was better than either option offered.

Removing the `unsafe impl` outright does not compile: **[measured]** typst's `World` trait itself
requires `Send + Sync`. But the promise did not have to be dropped — it could be made **true**.
`RefCell` became `Mutex`, which is `Send + Sync` on its own, so the pair was deleted and the compiler
now enforces what the comment asserted. There is one `unsafe` left in `world.rs` and it is the word,
in a comment explaining its own absence.

**[measured]** the cost is nothing: interleaved min-of-3 over the corpus, `RefCell` 36.0s against
`Mutex` 34.7s — within noise, because both readers already cloned their value out and the lock is
uncontended by construction. All 34 contract tests unchanged.

**Worth noting what this was**: a latent UB risk that had been correct-by-accident since the CLI was
the only caller, sitting directly in the path of the phase about to introduce a second thread.

### D4 · What does the app do about templates the catalogue rejected? — **shows them**

3e carries rejections with reasons precisely so a UI can. **[assumed]** a disabled row with the
reason beside it, because a template that silently fails to appear is the complaint 3e was built to
prevent.

### D6 · The event storm's `CollisionPrompt` has nothing to hang on — **found while building**

**[measured]** the engine's contract has no `CollisionDetected` event. `output::plan` resolves every
collision *before any conversion begins*, from the `BlanketResolution` the caller supplied, and what
comes back afterwards is a `SourceSkipped`. The engine emits; it never asks.

So the app asks **before** the run — exactly as `--on-collision` does — and reports what was skipped
after. An interactive prompt-per-collision with apply-to-all, as the event storm describes it, needs
a new event *and* a way for an adapter to answer mid-Job, which is a real contract change rather
than a widget.

**[assumed]** the blanket answer is enough for phase 4 and possibly for good: a person converting a
folder has one intention about overwriting, and being asked fifty times is the failure
`BlanketResolution` was invented to prevent. Recorded so the gap between the storm and the build is
a decision rather than something nobody noticed.

### D5 · Is `eframe` the right choice at all? — **settled, and not being reopened**

Phase 1 chose Rust + egui, and the no-editing constraint is downstream of it. Recorded here only so
that "why not tauri" has an answer that is a decision rather than an omission.
