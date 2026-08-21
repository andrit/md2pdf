# Plan — the ladder census (T27)

**Written:** 2026-08-21 · **Purpose:** make `compromise-mechanism.md` §9 enforceable instead of
honour-system.

The rule says *every ladder change re-measures the baseline*. Nothing enforces that, nothing
notices when it is skipped, and the baseline is a snapshot with no history — so a drift spread
across three changes is invisible. That is the documented failure mode, left unguarded.

---

## What it is

**Golden hashes, but for decisions rather than bytes.**

The existing golden tests pin a rendered PDF's hash. They would catch a ladder change as an opaque
mismatch on whichever fixture happened to contain a wide table. They cannot say *"rotations went
from 84 to 12"*, which is the thing worth seeing.

Three pieces:

1. **A committed fixture corpus** — one document per rung and edge, in the repository.
   `documents/` is untracked, borrowed, and may be deleted at any time; **a baseline that cannot be
   reproduced is not a baseline.**
2. **A committed census file** listing, per fixture, the decisions the ladder made.
3. **A test that regenerates the census and compares it** to the committed one, failing with a
   readable diff and instructions.

**The census file's git history is the log.** Every ladder change leaves a diff showing exactly
which kinds moved, alongside the reasoning already captured in `commit-log.md`. There is no separate
log format to invent or maintain.

## Where it lives

| Thing | Path | Why |
|---|---|---|
| Fixtures | `crates/md2pdf-engine/tests/corpus/` | cargo builds a test target per `tests/*.rs` and per `tests/*/main.rs`; a directory of `.md` files is ignored, so **no new binary and no new concurrent link** |
| Census | `design/ladder-census.txt` | it is a design artefact whose history is the record; next to the mechanism doc it enforces |
| Test | a module of the existing `walking_skeleton` binary | same reason — no new link |

## One source of truth

The test does **not** hold the expected numbers. It regenerates the census from the fixtures and
compares against the committed file. So the file is the record and the test is the tripwire; they
cannot disagree, and updating the baseline is an explicit, reviewable edit rather than a number
changed inside a test.

Failure message states what changed, and that an intended change means committing the regenerated
census **in the same commit, saying why** — the lesson already learned from the golden hashes.

## The fixtures

One per outcome, each named for what it is for:

| Fixture | Intended decision |
|---|---|
| `fits.md` | Portrait, None |
| `shrink-slight.md` | Shrink, near base |
| `shrink-floor.md` | Shrink, at or near the floor |
| `rotate.md` | Landscape |
| `reflow.md` | Reflow — wide table |
| `reflow-hostile.md` | Reflow, 12 columns — the case R3 says may be unreadable |
| `image-scaled.md` + asset | Scale |
| `image-clipped.md` + asset | Clip — the only remaining route to that rung |
| `image-missing.md` | ImageMissing |
| `image-remote.md` | ImageSkipped |
| `unsupported.md` | UnsupportedConstruct — raw HTML |

## Exit criteria

1. The census covers **every** `CompromiseKind`, including `Clipped`. ✅
2. The test fails when the ladder changes, with a diff naming the kinds that moved. ✅
3. The census is stable across runs and processes. ✅
4. `verify.sh` green. ✅

---

## Built — 2026-08-21

`design/ladder-census.txt`, 12 fixtures, 4 tests. All four criteria met.

Criterion 2 was **demonstrated, not assumed** — a guard that has never failed is not known to be a
guard. A row appended to `rotate.md` produced:

```
  was: rotate.md               3 elements   1 rotated, 1 shrunk
  now: rotate.md               4 elements   1 rotated, 2 shrunk
```

…followed by the regenerate command and the instruction to commit the new reading *with* the change.
Reverting the fixture returned it to green.

### Two fixtures were not doing what their names claimed

Worth recording, because it is the failure mode this task exists to catch, found in the task itself.
Both were written from reasoning about widths and were wrong:

| Fixture | Intended | Actually did | Fixed by |
|---|---|---|---|
| `shrink-slight.md` | a small step down | **clean** — 429pt against 483pt available, it simply fit | 5 × 21ch → 518pt → **9.0pt** |
| `shrink-floor.md` | at the floor | **7.5pt**, one step above | 6 × 22ch → 649pt → **7.0pt** |

The second exposed something the ladder does that reasoning would not predict: a table's width is
**not linear in font size** — padding and gutters do not scale — so a 6 × 23ch table at 675pt does
not fit portrait even at 7pt, and rotates to `Landscape/None` instead. The portrait shrink band tops
out near **649pt**, not the 690pt that `483 / 0.7` predicts.

Finding this needed per-element sizes, which the census deliberately omits. Rather than the
throwaway id-matching probe **R4** describes, `describe_the_fixtures` was added as a second ignored
test — the inspection counterpart to the tripwire, and the tool for keeping fixtures honest.

### Placed as a module, not a fourth binary

`tests/walking_skeleton.rs` became `tests/walking_skeleton/main.rs` with `census.rs` beside it — the
arrangement `md2pdf-convert/tests/compiler/` already uses, and the target name is unchanged, so
`docs/development.md`'s commands still work. D3 predicted no new link; this is how that was kept.

---

## Doubts — audited before building

Recorded whether they survived or not, because "I considered it and it was fine" is only worth
anything if the considering was real.

### D1 · Can `Clipped` still be reached at all? — **VERIFIED by measurement 2026-08-21**

After T26a every table reflows, so `Clip` is reachable only by an element with no alternate form:
an image. If no fixture can reach it, the census cannot cover every kind and the exit criteria are
unmeetable.

*Measured, not reasoned:*

```
A4: portrait avail=483pt  landscape avail=730pt  image floor=0.25

2000px image: natural=2000pt  landscape ratio=0.365  ->  Landscape / Scale
3000px image: natural=3000pt  landscape ratio=0.243  ->  Landscape / Clip
```

Two facts fall out. **One image pixel is one point of natural width**, and the clip threshold at A4
is therefore `730 / 0.25 = 2920pt` — an image must exceed **2920px wide**.

**Consequence:** the clip fixture needs an asset wider than 2920px, built to that measured
threshold rather than guessed. The 2000×300 image used in earlier tests does *not* clip; it scales.
A solid-colour PNG at 3000px compresses to a trivial size, so committing it costs nothing.

### D2 · Are ladder decisions deterministic across processes? — **VERIFIED by measurement 2026-08-21**

The census is worthless if it flickers. PDF *bytes* were shown deterministic across three separate
processes, but that is a different claim: decisions come from `measure()` compared against floors in
floating point, where an element sitting exactly on a boundary could plausibly tip either way.

An earlier draft of this plan called it verified on the strength of two runs of *different* code,
which proves nothing. **Re-checked properly** — same binary, same input, two runs:

```
run 1: reflowed 25, rotated 84, shrunk_to_floor 163, unsupported 2
run 2: reflowed 25, rotated 84, shrunk_to_floor 163, unsupported 2
```

Identical. The event streams differ only in the output directory path, as expected. Decisions are
stable across processes.

### D3 · Does adding fixtures risk another linker OOM? — **VERIFIED by existing evidence, no**

Three test binaries already link typst; a fourth would make the OOM worse.

Cargo creates a test target only for `tests/*.rs` and `tests/*/main.rs`. `md2pdf-typeset/tests/`
already contains a `fixtures/` directory holding three PNGs and has never produced an extra target
— that is the same arrangement, already demonstrated in this repository rather than assumed. The
census itself goes into the existing `walking_skeleton` binary as a module, adding no link.

### D4 · Should the census drive the CLI or the engine? — **not verified, decided on reasoning**

The CLI is what users run, so a subprocess census would test more. But the census is about **the
ladder**, and routing it through argument parsing and event serialisation would make a CLI change
able to turn the ladder tripwire red. Using the engine API keeps the instrument pointed at the thing
it measures. The CLI already has its own tests.

### D5 · Will it be brittle in an unhelpful way? — **not verified, accepted**

Every legitimate ladder change turns it red — that is the point, not a defect. The risk is *volume*:
if one change flips every fixture, the diff stops being informative.

Unverifiable in advance; it depends on changes not yet made. Mitigated by keeping the corpus small
(~11 fixtures, one per outcome) so a diff stays readable, and by the census being line-oriented
text so `git diff` shows precisely which fixture moved.

### D6 · Does the fixture corpus replace the real one? — **no, and they answer different questions**

The 146-document corpus measures *reality* — the mix of documents a person actually has, which is
what R1's 48% flagged figure is about. The fixture corpus measures *the mechanism* — one clean case
per rung.

Both stay. The fixture census is the tripwire; the real-corpus baseline in `compromise-mechanism.md`
§6 remains a periodic, manual measurement, and remains unreproducible if `documents/` is deleted.
That is a known limitation, not one this task fixes.
