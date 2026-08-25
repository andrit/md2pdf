# Plan — the attention gate and Overrides (3f)

**Written:** 2026-08-25 · **Phase:** 3f, the last engine phase. Phase 4 is the app.
**Rests on:** T31 (`comemo` eviction — the recompile loop is the shape that died five times before
anything evicted the cache).

Claims marked **[measured]** or **[assumed]**, per *Planning discipline* in `design/invariants.md`.
Options laid out in full and scored against the goal, per *Cost is not a reason to decline an
option*.

---

## The goal

> **"47 converted cleanly, 3 need your attention" is real, and an Element Override changes the
> output and recompiles fast enough to feel immediate.**

That is the roadmap's exit for 3f. It is also the payoff for everything the ladder does: the engine
has been recording *where it made a judgment call* since T13, and nothing yet offers to revisit one.

## Where this stands

**[measured]** what already exists:

| | State |
|---|---|
| `Diagnostic` / `Compromise` | built, sealed per document, carries `ElementId` |
| `Flagged` | `Diagnostic::is_flagged()`, and the CLI already prints a count |
| `DecisionMap::apply_override` | **built, and it is a trap — see below** |
| Recompile speed | **[measured]** 4ms steady state for probe+render of a 90-element document (T31) |
| `AttentionList` | does not exist |
| `Override` (the type) | does not exist |

**[measured]** the CLI already says *"1 converted cleanly"* and lists flagged documents. What it
cannot do is say *what* was compromised in a way a person can act on, or act on it.

## The defect this plan starts from

`DecisionMap::apply_override` sets the two axes directly:

```rust
pub fn apply_override(&mut self, id, orientation: Option<Orientation>, reduction: Option<Reduction>)
```

**Setting `orientation = Landscape` does not re-measure.** The size in that Decision was chosen
against the *portrait* width, and the GLOSSARY is explicit:

> RE-MEASURE in landscape; do not inherit the portrait size. Carrying the Floor size over is a bug.

So the method as it stands lets a caller commit exactly the bug T14 was written to fix. It has no
callers yet, which is the only reason it has never done so. **[assumed]** the same applies to
"permit below Floor": the point of that Override is to find a size the floor forbade, and no size
exists until something measures for one.

**This is the design question of 3f, not a detail.**

---

## The decision: what *is* an Override?

### Option A · An overridden **outcome** — what the code does today

The user's choice replaces the Decision. No re-probe; render immediately.

- **Cost:** near zero. It is already written.
- **Speed:** fastest possible — render only, **[measured]** ~2ms of the 4ms.
- **Correct for:** "allow clip", which needs no measurement.
- **Wrong for:** "force landscape" and "permit below Floor", both of which need a size that only
  measurement can supply. Ships the bug above.

### Option B · An overridden **constraint**, then re-probe that Element

The Override changes the *inputs* to the ladder for one Element — its permitted orientation, its
floor — and the ladder runs again for that Element alone.

- **Cost:** ~2 days. `Override` becomes a domain type, the probe learns to take per-Element
  constraints, and the engine grows a loop that holds a Conversion open across recompiles.
- **Speed:** **[assumed]** one Element's probe is far cheaper than a document's; must be measured,
  because it is the number the exit criterion turns on.
- **Correct for:** all three, by construction. The Override says *what the user permits*; the ladder
  still decides *what that permits*, which is the separation the whole engine rests on.

### Option C · Outcome for clip, constraint for the other two

Whichever is cheaper per control.

- **Cost:** ~1.5 days, and two mental models for one concept.
- **[assumed]** the saving is illusory: the loop, the type and the plumbing are needed for B anyway,
  and the only thing C avoids is one re-probe for the cheapest case.

**Recommendation: B.** Not on cost — it is the most expensive of the three — but because A ships a
known bug and C keeps two meanings of the word "Override" alive in one codebase. B is also the only
one that keeps `apply_override`'s caller honest, because there stops being a way to set an
orientation without a size to go with it.

**What B costs you honestly:** `DecisionMap::apply_override` changes shape or goes away, and the
GLOSSARY's *"an Override is an entry written into this map, indistinguishable in kind from an engine
Decision"* becomes half-true — the Override *produces* such an entry, via the ladder, rather than
being one. That sentence needs correcting, which is a documentation change to a settled decision and
should be visible rather than quiet.

---

## The change (Option B)

### 1 · `AttentionList` — the read model

In `md2pdf-domain`, because it is a value derived from a Diagnostic and phase 4 draws it as
`attention_list()` (`GLOSSARY`, naming table).

```rust
pub struct AttentionList { pub items: Vec<Attention> }
pub struct Attention {
    pub source: PathBuf,
    pub id: ElementId,
    pub what: CompromiseKind,     // what md2pdf did
    pub offers: Vec<OfferedFix>,  // what it can do instead
}
```

**`offers` is the whole point.** Element scope is offered *only where the Diagnostic named an
Element* — md2pdf is not a layout editor, it offers a fix exactly where the engine already admitted
to a compromise. A `Reflowed` table offers "force landscape"; a `ShrunkToFloor` offers "permit
below floor" and "force landscape"; a `Clipped` element offers both and is the only one that ever
lost content.

### 2 · `Override` — the domain type

```rust
pub struct Override { pub id: ElementId, pub permit: Permit }
pub enum Permit { Landscape, BelowFloor { to_pt: f64 }, Clip }
```

A *permission*, not an outcome. The ladder still decides.

### 3 · The probe takes per-Element constraints

`probe(elements, template)` becomes `probe(elements, template, overrides)`. An Element with an
Override is measured against the width its permission implies and floored where its permission
allows. **Everything else is unchanged**, which is what keeps this from becoming a second ladder.

### 4 · The recompile loop, in `md2pdf-engine`

A `Review` holding one document's Conversion, its Template, and its current DecisionMap. Applying an
Override re-probes and re-renders; the Conversion is *not* redone, because the markdown has not
changed and re-parsing is the expensive part that `comemo` cannot help with.

**T31 is the precondition.** This loop is the shape that was OOM-killed five times before anything
evicted the cache, and its cadence is deliberately *not* the batch's — see `plan-comemo.md` D2.

### 5 · A CLI surface, minimal and honest

3f has no UI; phase 4 is the UI. But an engine capability with no way to exercise it is a capability
nobody has seen. `--attention` prints the list; **[assumed]** that is enough to prove the model
without inventing a CLI interaction the app will replace.

## Exit criteria — **all met 2026-08-25**

1. ✅ `--attention` on `documents/design-docs`: **"3 converted cleanly, 12 need your attention."**
   Every item names what was done — *"wrapped its cells instead of shrinking"* — and what could be
   allowed instead. It writes nothing.
2. ✅ An Override changes the decision **and the rendered bytes**, asserted on the PDF rather than
   the decision, because a decision that changed without changing the page changed nothing.
3. ✅ **Measured, and the assumption was wrong — see below.**
4. ✅ `forcing_landscape_measures_against_the_landscape_width` — the size comes from the ladder
   measuring under the permission, never from the decision being replaced.
5. ✅ Refused at two levels: `Review::apply` returns false for an Element this document lacks, and
   the ProbePass ignores an Override whose content hash no longer matches.
6. ✅ Census unchanged, goldens unmoved, `verify.sh` green.

## The latency assumption was wrong, and by 15x

D2 assumed under 100ms from T31's 4ms recompile figure. **[measured]** the first attempt:

```
100 elements | open 4134ms | override + render 1567ms
```

**Re-probing the document to move one table re-measures all hundred of them.** The ProbePass builds
*one* Typst document containing the measurement code for every Element, and `comemo` keys on the
source — so changing one Element's constants invalidates the whole thing. T31's 4ms was recompiling
the *same* source, where every measurement is a cache hit; nothing in that number was about changing
anything.

Fixed by re-probing **only the Elements whose permissions changed** and splicing the results in. That
is sound only because Element decisions do not depend on each other — which is exactly what D1's test
was written to check, before there was a reason to rely on it.

```
100 elements | open 881ms | first use of an option 328ms | thereafter 8ms
```

**Two numbers, because they are two different clicks.** `comemo` keys on the probe source and each
distinct permit produces a distinct one, so the first time a user tries "landscape" costs ~330ms and
every later use of it costs 8ms. A person toggling between options feels the first pause and nothing
after. The test asserts on the steady state and **reports the cold number rather than averaging it
away**.

## Doubts — audited

### D1 · Does re-probing one Element give a different answer than probing all of them? — **[measured] no, and the whole fix rests on it**

The probe measures Elements independently — **[measured]**, that is why `probe` takes a slice and
`overflow_of` can pass one Element. **[assumed]** therefore a single-Element re-probe is identical
to a full one for that Element. If that is wrong, every override is subtly inconsistent with the
page around it, and exit criterion 2 would not show it because one page looks fine on its own.
Test: re-probe one Element and compare against the full probe's answer for it —
`probing_one_element_agrees_with_probing_the_document`. It passes, which is what made the latency fix
above legitimate rather than a shortcut. **It was written before it was needed**, which is the only
reason the fix took minutes instead of an afternoon of doubt.

### D2 · Is <100ms achievable? — **[measured] yes, but not the way this doubt assumed**

It says the ingredients say yes, citing T31's 4ms. **That reasoning was wrong**: 4ms was recompiling
an unchanged source, and an Override changes it. The first implementation took 1567ms. The doubt was
right to demand a number and wrong about what the number would be — see above.

### D3 · What happens to `apply_override`? — **[assumed] it becomes private or goes**

It is the trap. Once the probe takes Overrides, writing an axis directly has no legitimate caller.
Leaving it public is leaving a loaded foot-gun in the domain crate for phase 4 to find.

### D4 · Does an Override survive a re-run? — **no, and that is the design**

Job and Element scope are *"this Job only"* (`GLOSSARY`). Persistence is Template scope, which is
`template.toml` and belongs to the author, not the reviewer. **[assumed]** a user who wants a
permanent change edits the template — which 3e made possible. Recorded because "why did my override
vanish" is a support question waiting to happen, and the answer should be a decision rather than an
oversight.

### D5 · Does the AttentionList need page numbers? — **not in 3f**

`Compromise.page` exists and is always `None`; filling it needs the RenderPass to report where an
Element landed, which is real work. **[assumed]** a reviewer working from a list of elements does
not need one until the app can jump to a page — phase 4's problem, and R4 already tracks it.
