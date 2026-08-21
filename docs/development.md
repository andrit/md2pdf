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

## Performance: wall clock in this container lies

**Measured 2026-08-21**, after a 146-document run appeared to take 31 minutes.

| What | Wall | CPU |
|---|---|---|
| 146 documents, debug, first run | **31m** | 5m20s |
| 30 documents, debug, from `/workspace` | 1m16s | 1m17s |
| 30 documents, debug, from `/tmp` | **35m** | 1m25s |
| 5 documents, debug, twice in a row | 5.4s | 5.6s |
| **146 documents, release** | **22s** | 19s |

Two conclusions, both the opposite of the obvious one:

**1. The container is descheduled for long stretches.** Wall clock ran 6–28× ahead of CPU time in
some runs and matched it exactly in others, on identical work. The bind mount was suspected and
**exonerated** — reading from `/workspace` was the *fast* case; the slow run read from local
overlay. What predicts the stall is when the run happened, not what it touched.

> **So compare `user`, not `real`.** A wall-clock figure from this container is not evidence about
> md2pdf.

**2. Release is ~16× faster than debug**, which is where the rest of the apparent slowness lived.
146 documents convert in **22 seconds**. Benchmark the release binary or do not benchmark.

### Where the CPU actually goes

`cargo test -p md2pdf-engine --test walking_skeleton profile_the_phases -- --ignored --nocapture`

```
design__event-storm.md   100 elements, 14 atomic   convert 1ms   probe 8298ms   render 930ms   pdf 136ms
micro-saas-ideas.md      360 elements, 45 atomic   convert 12ms  probe 2999ms   render 1550ms  pdf 204ms
design__kano.md           39 elements,  1 atomic   convert 0ms   probe  290ms   render  199ms  pdf  39ms
```

**The ProbePass dominates — up to 88%** — and not in proportion to element count: `event-storm`
has *fewer* atomic elements than `micro-saas` and takes three times longer in the probe. The cost
tracks how far each element walks down the escalation ladder, because every rung is another linear
scan in 0.5pt steps, and since T14 an element that reaches landscape is scanned **twice**.

The spike predicted this and named the fix: binary search would cut ~7 measurements to ~3.

**Not doing it.** At 22 seconds for 146 documents in release, there is nothing to fix. This is
recorded so that *if* the probe ever shows up in a real profile, the target is already known and
nobody has to rediscover it.

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
