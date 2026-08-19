# Plan — finish the escalation ladder (phase 3b2, T14)

**Written:** 2026-08-19 · **Follows:** 3b (images), which is where the gap surfaced.
**Goal:** all four rungs of the ladder are reachable, and an element that cannot fit is clipped and
visibly marked rather than silently overflowing.

Not an images task — it affects **tables as much as images**, which is why it was lifted out of 3b.

---

## What is actually broken

Three things, and the GLOSSARY already says what the behaviour should be. This is code failing to
match a settled design, not a design question.

**1. `Rung::Clip` is unreachable code.** `probe.rs` can only emit `none`, `shrink`, or `rotate`.
`harvest.rs` parses `"clip"`, `render.rs` implements it with a visible red marker, `Rung::Clip`
exists in the domain — and nothing ever produces it. The spike flagged clipping as unprobed; this is
why.

**2. Rotation never re-measures, so the ladder's own rule is violated.** The GLOSSARY is explicit:

> → RE-MEASURE in landscape; do not inherit the portrait size
> → still over? → clip, with a visible marker
>
> *The re-measure step matters: landscape offers far more width… Carrying the Floor size over is a
> bug.*

`render.rs` emits `#page(flipped: true, margin: …)[body]` at natural size and hopes. The RenderPass
**never measures** — that is the two-pass design — so "re-measure in landscape" can only happen in
the ProbePass, and it does not happen at all. An element too wide even for landscape overflows with
no marker, no Compromise, and no way for the user to know.

**3. `Rung::Rotate`'s doc comment claims the RenderPass re-measures.** Corrected in T13 to say it
does not; T14 makes the claim true by moving the work to where it belongs.

---

## The decision: the ladder has two axes, not four steps

The ladder reads as a linear sequence, but mechanically it is **orientation** and **reduction**, and
rotation exists only to change the available width that reduction is measured against.

```rust
pub struct Decision {
    pub id: ElementId,
    pub orientation: Orientation,   // Portrait | Landscape
    pub reduction: Reduction,       // None | Shrink { size_pt } | Scale { factor } | Clip
    pub natural_pt: f64,
    pub available_pt: f64,          // the width the reduction was decided against
}
```

**Why this rather than widening `Rung`.** The flat enum cannot express "rotated *and* shrunk to
8pt", which is precisely the common case after a re-measure — and the finished product needs it. The
stack decisions specify Element-scope overrides as:

> force landscape, allow clip, permit below-floor for this element

Three **independent** toggles. On a flat enum, "force landscape" has to guess which composite rung to
write, and "landscape + below-floor" is inexpressible. On two axes each override sets one field.
That the override vocabulary and the decision model agree is the strongest evidence the split is
right.

*Cost:* `Rung` is named in `decision.rs`, `harvest.rs`, `render.rs`, `diagnostic.rs`,
`DecisionMap::apply_override`, and a dozen tests. Real, but mechanical, and the compiler finds every
site.

*Alternative considered:* keep `Rung` and add `Rotate { then: Box<Rung> }`. Rejected — it encodes
the same two axes in a shape that cannot be pattern-matched on one axis, and an override would have
to rebuild the nesting.

## The algorithm

Entirely in the ProbePass, per the GLOSSARY ("decided entirely in the ProbePass"):

```
wrappable                      -> Portrait, None
fits portrait                  -> Portrait, None
reduces to fit within floor    -> Portrait, Shrink|Scale
otherwise, re-measure against landscape width:
    fits landscape at base     -> Landscape, None
    reduces within floor there -> Landscape, Shrink|Scale
    otherwise                  -> Landscape, Clip
```

The landscape width is arithmetic, not a second layout: `page_height_pt - 2 × margin_pt`. Add
`Template::available_landscape_pt()` beside the existing `available_pt()`, so neither pass has to
know the rule.

**Cost:** one extra measurement pass, and only for elements that already failed portrait — rare in
a real document. The probe deliberately avoids `layout()` and that stays true.

## Diagnostic

Two axes mean an element can make **two** concessions at once. Emit one `Compromise` per non-default
axis rather than inventing composite kinds: an element that rotated *and* shrank produces `Rotated`
and `ShrunkToFloor { size_pt }`. The attention list then reads "rotated, and shrunk to 7pt", which
is what the user needs to know, and no new `CompromiseKind` variants are required.

## Tests

- A table that fits: portrait, no reduction — unchanged.
- A table that fits after shrinking: portrait + `Shrink`, unchanged.
- **A table too wide for portrait but fine in landscape at base size: `Landscape` + `None`** — the
  re-measure the GLOSSARY demands, and currently absent. It must *not* inherit the portrait floor.
- A table needing reduction even in landscape: `Landscape` + `Shrink`.
- **A table too wide for landscape at its floor: `Landscape` + `Clip`** — makes the fourth rung
  reachable for the first time.
- The clipped render carries the visible marker.
- Images take the same path via `Scale` rather than `Shrink`.
- Overrides set one axis without disturbing the other — the property the split exists for.
- Visual: a clipped element looks clipped and its marker is legible.

## Exit criteria

1. All four reductions and both orientations are reachable from the probe.
2. An element too wide for landscape is clipped **and marked**, with a `Compromise` recorded.
3. Rotation re-measures; a rotated element does not inherit its portrait size.
4. `verify.sh` green; `/phase-audit` run or explicitly waived, as in 3a and 3b.

## Deliberately not here

- **Binary search for the shrink step.** Still ~7 measurements per element in a 0.5pt linear scan;
  noted in the spike as a 3× saving, and still not worth taking until the probe shows up in a
  profile.
- **Overrides themselves.** The two-axis model is what makes them expressible; building the
  affordance is 3f.
