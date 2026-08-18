# Upgrading Typst

**Typst is pinned to `=0.15.1` and upgrading is a scheduled task, never a routine
`cargo update`.** This document is the runbook.

## Why this exists

Moving ~200 lines of spike code onto 0.15.1 produced **eight** breaking API changes:

| Was | Became |
|---|---|
| `FileId::new(None, vpath)` | `RootedPath::new(VirtualRoot::Project, vpath).intern()` |
| `VirtualPath::new(..) -> VirtualPath` | returns `Result<_, PathError>` |
| `Library::default()` | `Library::builder().build()`, needs `LibraryExt` in scope |
| `Introspector` inherent methods | it is a **trait**; must be imported |
| `typst_render::render(page, 2.0)` | `render(page, &RenderOptions)` |
| `typst::layout::Scalar` | `typst::utils::Scalar` |
| `VirtualPath::as_rootless_path` | deprecated → `get_without_slash` |
| `json.decode(bytes)` (markup) | removed |
| `today(Option<i64>)` | `today(Option<Duration>)` |

All of those were **compile errors** — the safe kind, because the compiler finds every
one. The dangerous kind is **silent behavioural change**: if a future `measure()`
stopped clamping to the available width, or `raw` blocks stopped wrapping, nothing
would fail to compile and the escalation ladder would quietly start making wrong
decisions on real documents.

`crates/md2pdf-typeset/tests/contract.rs` exists for exactly that, and it is the
centrepiece of this runbook.

## Release cadence

From crates.io: `0.14.0` Oct 2025 → `0.15.0` Jun 2026 → `0.15.1` Jul 2026. Roughly
**one breaking minor release every eight months**, with patches between. Typst is
pre-1.0, so breaking changes are expected and signposted. Budget **one day, twice a
year**. Reaching 1.0 would change this calculus and is worth re-reading this document
when it happens.

## What is pinned, and where

| Thing | Where | Value |
|---|---|---|
| `typst`, `typst-layout`, `typst-pdf`, `typst-render`, `typst-kit` | root `Cargo.toml` `[workspace.dependencies]` | `=0.15.1` — exact, not caret |
| Whole dependency graph | `Cargo.lock` | **committed** — md2pdf is an application |
| Rust toolchain | `rust-toolchain.toml` | `1.97.1`, so the Linux container and the macOS host compile identically |

## The blast radius is one crate

`md2pdf-typeset` is the only crate permitted to link the `typst` crate, and the
**dependency graph enforces it** — `md2pdf-domain` cannot import typst because typst
is not in its manifest. That is mechanical; a lint is something a person has to
remember to keep passing.

One distinction this boundary deliberately does *not* draw: Typst **markup syntax** is
a stable surface, and `md2pdf-convert` emits it freely as text. The typst **Rust
crate** is the unstable surface. Do not confuse them, or the anti-corruption layer
ends up in the wrong place.

## The procedure

1. **Read the changelog first.** `https://typst.app/docs/changelog/` — the docs sidebar
   lists every version. Note anything touching `measure`, `layout`, `context`, `page`,
   `raw`, introspection, or the `World` trait. Those are the load-bearing surfaces.
2. **Branch.** This will not be a one-commit change.
3. **Bump one version at a time** in `[workspace.dependencies]`. Do not jump 0.15 → 0.17
   in a single step; you lose the ability to attribute a behavioural change to a release.
4. **Fix the compile errors.** They should all be inside `md2pdf-typeset`. **If a compile
   error appears in any other crate, the anti-corruption layer has leaked** — fix the
   leak rather than the call site.
5. **Run the contract tests.** `cargo test -p md2pdf-typeset`.
   - *Decision assertions failing* (`Rung::Rotate` became `Rung::None`, etc.) means the
     semantics moved. **Stop and investigate.** Do not adjust the expectation to make it
     pass — that discards the only warning you get.
   - *Measurement pins failing* within a point or two usually means font shaping or
     metrics changed. Re-verify a real document by eye, then update the pinned numbers
     **in the same commit**, with the reason in the message.
6. **Re-run the benchmark**: `cd spikes/typst-embed && cargo run --release --bin bench`.
   Confirm the probe is still cheaper than a full-document layout and that an Override
   still costs single-digit milliseconds. Remember `comemo::evict(0)` before any timing —
   **the memo cache is global, not per-`World`**, and without eviction you measure
   ordering rather than code.
7. **Eyeball real output.** Render the font specimen and a document containing a wide
   table, a code block and an image. Numbers passing is not the same as the page looking
   right.
8. **Update the pin table above** if any API moved, so the next person sees the pattern
   rather than rediscovering it.

## Things that will bite

- **`comemo`'s cache is global.** Any benchmark or timing test must evict first.
- **`measure()` clamps.** `measure(el, width: avail).width` never exceeds `avail`, even
  for content that visibly draws off the page. The overflow predicate must stay
  `class is Atomic AND measure(el).width > available`. If a release changes the clamping,
  the ladder's predicate must change with it.
- **`layout()` forbids page configuration**; a plain `context` block does not. md2pdf
  deliberately uses `context` and computes available width itself. If `context` ever
  gains the same restriction, the single-pass fallback disappears — but the two-pass
  design already in place is unaffected.
- **Variable fonts.** 0.15.1 resolves weight and italic from the axes, so the FontBook
  ships variable TTFs with no static instances. Verify this still holds; a regression
  would show up as missing bold rather than as an error.
