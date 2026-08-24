# Plan — nothing was evicting the cache (T31)

**Written:** 2026-08-24 · **Closes:** flag **F3**, after five sightings.
**Blocks:** phase 3f, whose recompile loop is the same shape as the loop that kept dying.

Claims marked **[measured]** or **[assumed]**, per *Planning discipline* in `design/invariants.md`.

---

## Five sightings, no measurement

| When | What was running | What was concluded |
|---|---|---|
| 2026-08-22 | T26b exposure, ~200 renders through one `Typesetter` | "too many renders" |
| 2026-08-22 | same, one `Typesetter` per element | still killed — **placebo** |
| 2026-08-23 | T30 base-size comparison, 146 documents | "the corpus is large" |
| 2026-08-23 | same, a fresh `Typesetter` per probe | still killed — **placebo** |
| 2026-08-24 | the release batch, T26c | killed at ~141 of 146 |

**Every response was a workaround chosen from a guess.** Two of them changed how the work was
scoped rather than what was retained, and the runs that survived did so because they did less work.
Nothing ever plotted the curve.

## The curve — measured 2026-08-24

One long-lived `Typesetter`, the corpus a document at a time, RSS from `/proc/self/status`:

```
             no eviction        evict(5)
  1 doc         111 MiB          111 MiB
 41 docs       1629 MiB          563 MiB
 81 docs       2178 MiB          610 MiB
131 docs       3162 MiB          634 MiB
146 docs        killed           690 MiB
```

**~24 MiB per document, unbounded, and it is not the `Typesetter`.** `comemo`'s cache is
process-global — keyed by memoised call, not held by the `World` — so dropping a `Typesetter` frees
none of it. That is why both earlier fixes were placebo, and it is why this was reclassified from
`CONSTRAINT` to `DEFECT`: **[measured]** `comemo` 0.5.1 is in the graph via typst, typst's own CLI
evicts between compilations, and this project never called `evict` at all.

## What eviction costs — measured, because this is the half that could go wrong

`comemo` exists so 3f's recompile loop is fast. Evicting too eagerly throws that away. One real
document, recompiled six times, worst steady-state time:

| `evict` age | first compile | steady state |
|---|---|---|
| none | 737ms | **5ms** |
| 100 | 456ms | 4ms |
| 10 | 422ms | 5ms |
| **5** | 415ms | **4ms** |
| 1 | 474ms | 10ms |
| 0 | 454ms | **470ms** |

**Age 5 costs nothing.** The memoisation the long-lived `World` exists for is fully preserved — 4ms
against 5ms with no eviction at all. Only `evict(0)` destroys it, and it destroys it completely: a
hundred times slower, which is what "no cache" looks like.

So the number is a memory/speed dial with a wide flat region, and we are sitting in the middle of it
rather than at an edge. **[assumed]** any age from 2 to 100 would do; 5 is chosen because it is
comfortably inside the flat region at both ends.

## The change

1. **`comemo` becomes a direct dependency of `md2pdf-typeset`**, pinned `=0.5.1` exactly as the
   typst crates are. A version skew between the crate typst memoises through and the crate we evict
   through would silently stop evicting anything.
2. **`md2pdf_typeset::evict(max_age)`** — a thin, documented export. Not called inside `render`,
   because the right cadence belongs to the caller.
3. **`job.rs` calls it between documents**, with `EVICT_AGE = 5`.

**Why not inside `Typesetter`.** A batch wants eviction per document; 3f's recompile loop wants it
far less often, because there the *point* is that the previous compilation is still cached. Burying
the call would make that choice invisible at exactly the site that has to make it.

## Exit criteria

1. ✅ The batch that was dying completes: **146 documents, exit 0, 29.7s** — which is also the
   timing §6 last recorded, so the documented recipe works again.
2. ✅ Memory bounded: 3162 MiB and climbing at 131 documents → **690 MiB flat** at 146.
3. ✅ The recompile loop is not slowed: **[measured]** 4ms steady state at age 5, against 5ms with
   no eviction.
4. ✅ A gate: `eviction_still_evicts`. **Proved to fire.**
5. ✅ `verify.sh` green.

## Doubts — audited

### D1 · Can the memory bound be gated on? — **no, and two attempts proved it**

I wanted the gate to assert the curve directly. It cannot, and the reason is worth recording because
it looks like it should work:

| Attempt | In isolation | In the suite |
|---|---|---|
| absolute bound, 60 MiB | 0 MiB — passes | **81 MiB — fails, code correct** |
| paired: evicting vs not | 0 vs 124 | **82 vs 92 — collapses** |

`VmRSS` is process-wide, cargo runs a binary's tests concurrently, and the allocator reuses freed
pages — so the first measurement includes other tests' work, and in the second the evicting half
leaves the allocator warm for the unevicted half. **The curve is real and measured; it is not
measurable from inside a concurrent test binary.**

So the measurement stays a deliberate tool (`memory_growth_across_the_corpus`) and the *gate* asserts
something robust instead: that eviction has an observable effect at all. A cleared cache is ~100x
slower to recompile, which survives any machine noise, and it catches the failure that actually
threatens this — a `comemo` version skew that silently evicts nothing, which is precisely what the
exact version pin exists to prevent. It would also have caught both of F3's placebo fixes.

**[assumed]** the honest cost: nothing automatically re-checks the 690 MiB plateau. A regression that
kept eviction working but leaked elsewhere would need the tool run by hand.

### D2 · Does this make 3f safe? — **[assumed] no, and it is a different question**

The batch is bounded because it evicts between documents. 3f's loop recompiles *one* document
repeatedly, which is the case `comemo` handles well and which never grew in these measurements. The
risk there is a long session across *many* documents, and the cadence for that is 3f's to choose —
which is why `evict` is exported rather than hidden.

### D3 · Should the CLI expose the age? — **[assumed] no, not yet**

It is a performance dial with no user-visible meaning, and no one has wanted a different value. It
becomes a template or config question if 3f finds a reason. Recorded so it is a decision rather than
an oversight.
