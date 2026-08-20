# Development notes

Environment-specific traps that cost real time to diagnose. Not design decisions — those live in
`design/`.

## `linking with cc failed` — it is the OOM killer, not a flake

**Symptom:** `cargo test --workspace` fails intermittently with

```
error: linking with `cc` failed: exit status: 1
  = note: collect2: fatal error: ld terminated with signal 9 [Killed]
```

**Cause:** every test binary that exercises the real compiler links typst statically — roughly 250
crates. Three of them exist (`md2pdf-typeset`'s `contract`, `md2pdf-convert`'s `compiler`,
`md2pdf-engine`'s `walking_skeleton`), and linking two at once exhausts memory on a 4 GB machine.
`signal 9` is the tell; without looking for it the failure reads as a flake and gets retried.

**Already handled for the gate.** `scripts/verify.sh` runs its test step with `-j 1`, so links are
serial. Clippy has compiled the graph by then, so that step is mostly linking and serialising it
costs little.

**If you run cargo directly** and hit it:

```bash
cargo test --workspace -j 1
```

**Deliberately not done:** a repo-wide `jobs` limit in `.cargo/config.toml`. It would slow machines
with enough memory — the macOS host — to protect a container that already has the fix where it
matters. And the underlying cause is not a defect: testing against the real Typst compiler is what
caught the double-escaping bug, the unreachable clip rung, and the images that failed the whole
document. That is worth a slow link.

**Related constraint:** cargo builds one executable per file in `tests/`, so adding a new top-level
`tests/*.rs` that links typst adds another concurrent link. `md2pdf-convert` keeps its integration
tests as modules under `tests/compiler/` for this reason — add a module there rather than a new
file.

## Seeing rendered output

There is no PDF viewer and `pdftoppm` is not installed, so a PDF cannot be read directly. To look
at a page:

1. Call `Compilation::raster(page, 1.0)` and print the RGBA bytes as hex to stdout. **Not** to a
   file — `check-boundaries.sh` forbids `std::fs` outside `md2pdf-paths`, tests included.
2. Decode the hex and assemble a PNG by hand (`zlib` + `struct`: filter byte `0x00` per scanline,
   then IHDR/IDAT/IEND).

Worth the trouble: every test in this project asserts on extracted *text*, which is blind to
styling. Italic silently not rendering survived five green commits and was found in minutes by
looking at a page.

## Cargo invocations

The container keeps build artefacts outside the source tree, because the tree is a shared bind
mount and Linux and macOS artefacts must not collide:

```bash
export PATH=/home/user/.cargo/bin:$PATH
export CARGO_TARGET_DIR=/home/user/.cargo-target/md2pdf
```

Run `./scripts/verify.sh` from the workspace root.
