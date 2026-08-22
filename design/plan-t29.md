# Plan — the reflow alternate must actually fit (T29)

**Written:** 2026-08-22 · **Follows:** T26b, which put 152 of 198 compromised elements onto the
reflow rung. **Closes:** the honesty gap in `Reflow`; part of **R3**, **R5**, and flag **F1**.

Claims marked **[measured]** or **[assumed]**, per *Planning discipline* in `design/invariants.md`.

---

## The defect

`Reflow` sits immediately before `Clip` because the alternate is supposed to always fit. **It does
not.** With unbreakable cell content it overflows the page *and* the cells overprint each other into
mush — while the ladder records `Reflowed`, which downstream reads as *handled*.

**[measured]** 4 elements in the corpus render this way **today**, and T26b added 3 more by moving
elements onto the rung. 7 of 198.

This is the mechanism's central risk in its purest form: the output is wrong, the record says it is
fine, and nothing fails.

## The cause, and where it actually lives

Long runs of characters with no break opportunity — Typst cannot wrap them, so the cell is at least
as wide as the run, whatever the column spec says. **[measured]** all four specs tried in T26a2
overflow on such content, including today's all-`1fr`.

**Where those runs come from, measured over every table cell in the corpus** — and this corrects the
guess recorded in T26b's re-vetting, which said links were the problem:

| Count | Kind | Example |
|---|---|---|
| **196** | **inline code** | `` `completeSubmission(existingId)` `` |
| 28 | other long run | trailing punctuation on inline code |
| 15 | markdown link | `[stripe.com/docs](https://…)` — display text is short, does **not** overflow |
| 13 | path in plain text | `/api/proposals/[id]/finalize` |
| 3 | bare URL in plain text | |

**Inline code is 77% of it.** Links are 6% and mostly harmless, because what renders is the display
text. So the design does not need to survive markup surgery inside `#link("…")`, which is what I
previously claimed made this hard. That claim was wrong.

## The fix

**Give long runs a break opportunity — a zero-width space (`U+200B`) — in the alternate only.**

**[measured]** it works, in both forms that matter:

```
plain text, unbroken     overflows: YES
plain text + ZWSP        overflows: no
#raw, unbroken           overflows: YES
#raw + ZWSP              overflows: no
```

That `#raw` breaks at a ZWSP is the load-bearing fact, since inline code is the dominant case, and it
is not obvious — raw text is otherwise rendered verbatim.

### Alternate only, and why that matters

The body must keep its unbroken runs. Its natural width is what the probe measures to decide the
rung; if the body could wrap mid-token, a wide table would measure narrow and the ladder would make
different — wrong — decisions. **[measured]** the census is the check: it must not move.

### How, without string surgery on markup

`emit.rs` currently builds one `cells` string and uses it for both forms. **Emit the table's cells a
second time with a `breakable` flag set**, and use that for the alternate:

- `Emitter` gains `breakable: bool`.
- `Event::Text` and `Event::Code` insert breaks into long runs **before escaping**, so a break can
  never land inside an escape sequence. `U+200B` is not ASCII punctuation, so `escape` passes it
  through untouched.
- The second emitter's `pending` compromises and `images` are **discarded** — the first pass already
  recorded them, and double-counting them would inflate every Diagnostic.

Re-emitting is the point: both forms come from the same code path, so they cannot drift, and no
parser has to understand Typst markup. **[assumed]** the cost is irrelevant — it is tables only, and
conversion is ~1ms per document against a ~150ms probe.

### Constants, tunable

Insert a break every `N` characters inside any run longer than `M`. Starting values `M = 24`,
`N = 12` **[assumed]** — chosen so ordinary words are never touched and the measured problem set
(runs of 30+) always is. Tuned by eye with T26c, not derived.

## Build the overflow oracle first — flag F1

**Nothing in this project can currently answer "did that run off the page".** Typst does not report
overflow; the only way to know is to render, raster, and look for ink past the margin. That has been
written as throwaway code **four times now** — T26a's clip triage, T26a2's column experiment, T26b's
exposure measurement, and this plan's ZWSP check.

T29 cannot be verified without it, and after T26b the rung it watches carries 77% of all compromises.
So it stops being throwaway:

- `overflows(compilation) -> bool` as a test helper beside `describe_the_fixtures`.
- A fixture whose cells hold a long unbreakable run, asserted **not** to overflow — the regression
  test for this task.
- **[assumed]** a `--check-overflow` CLI mode is worth it eventually; not in this task.

**This is the deliverable that outlives T29.** The fix is ten lines; the instrument is the thing that
means the next silent regression is caught.

## Exit criteria

1. Long runs get break opportunities in the alternate, never in the body.
2. The **census does not move** — this changes appearance, not rungs.
3. The overflow oracle exists as a committed helper, with a fixture that would fail without the fix.
4. **Corpus re-measured: the 7 overflowing elements go to 0** — the number that defines success.
5. `a_reflowed_table_is_unchanged` regenerated deliberately; the reason recorded.
6. `verify.sh` green.

---

## Doubts — audited

### D1 · Does a ZWSP survive into the PDF text layer? — **not verified, and it is a real cost**

Copying a path out of the PDF would paste with invisible characters in it. **[assumed]** acceptable:
the alternative is text running off the page, and the alternate form is only used when the table
would otherwise not fit at all.

Worth flagging rather than hiding — raised as a `DEBT` flag when built, because someone will
eventually paste a broken path and wonder why.

### D2 · Could breaking make output *worse* anywhere? — **not verified, bounded**

A break inserted mid-identifier is ugly where the table would have fitted anyway. Bounded by `M`:
only runs longer than 24 characters are touched, and only in the alternate, which is only rendered
when the table did not fit.

### D3 · Does this remove the need for `Clip`? — **no, and that is worth stating**

With breaks available almost any table can be made to fit, so `Clip` becomes even less reachable for
tables than it already is. It remains reachable for images. **[measured]** the census covers it via
`image-clipped.md`, which is why that fixture exists.

### D4 · Is the 7-element figure exact? — **it is a floor, not an exact count**

The oracle checks page 1 only, at 1 pt/px. A table overflowing solely on a later page would be
missed. **[assumed]** rare, since overflow comes from a wide row and the widest is usually early.
Building the oracle properly is the chance to fix this — check every page.
