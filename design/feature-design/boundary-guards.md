# Feature design — additional boundary guards

**Written:** 2026-08-20 · **Status:** approved 2026-08-20 — build G3, G1, G2 in that order as T22–T24. G4 and G5 declined.
**Relates to:** `design/invariants.md` (every guard below enforces a numbered invariant).

---

## Why more guards

`scripts/check-boundaries.sh` currently enforces two rules that clippy cannot express, because
clippy has no way to say *"this lint applies everywhere EXCEPT this crate"*:

- typst may only be linked by `md2pdf-typeset` (INV-10)
- `std::fs` may only be called by `md2pdf-paths` (INV-9)

Both have already earned their keep. The fs guard caught a real leak the moment filesystem code
existed, and the typst guard would have caught a manifest addition.

Several other invariants have **no enforcement at all** — they are promises held by memory. The
question this document answers is which of those are worth automating, and which would be ceremony.

**A guard earns its place when the violation is both tempting and invisible.** A rule that is
obvious to break and obvious to spot does not need a script.

---

## The five candidates

### G1 · No network — `INV-1`

**What:** no network-capable crate anywhere in the dependency graph, and no `std::net` in source.

**Mechanism — grep `Cargo.lock`, not the manifests.**

> **Corrected during this design.** The first draft proposed grepping `crates/*/Cargo.toml`, which
> only sees *direct* dependencies — and the realistic accident is transitive: a convenience crate
> quietly pulling in an HTTP stack. `Cargo.lock` lists the whole resolved graph, so it catches what
> a manifest grep cannot.

```
grep -E '^name = "(reqwest|ureq|hyper|tokio|native-tls|rustls|curl|isahc|attohttpc|surf)"' Cargo.lock
grep -rn --include='*.rs' '\bstd::net\b' crates
```

**Verified 2026-08-20: the graph is currently clean** — none of those crates appear, and
`typst-kit` is pinned with only the `embedded-fonts` feature, not `downloads`. So this guard starts
from a true state rather than immediately failing or grandfathering an exception.

**Why it is worth more than protection.** "No network, ever" is the product's most marketable
property — privacy, zero running cost, works on a plane. Today it is a promise. With this guard it
becomes an **audit**: *the binary contains no network stack, verified on every build*. That is a
materially stronger thing to say on a download page, and it is the kind of claim a cautious user
actually checks.

---

### G2 · The engine never learns what a UI is — `INV-8`

**What:** `domain`, `convert`, `typeset`, `paths`, `engine` may not depend on `eframe`, `egui`,
`winit`, or any windowing crate. Only the adapter crate may.

**Mechanism:** manifest grep, same shape as the existing typst rule — an allow-list of one crate.

**Why now rather than later.** The entire out-of-process-adapter design rests on this invariant,
and today it is protected only by the fact that **no adapter exists**. The moment phase 4 begins,
*"just import egui here for a second to get the preview working"* becomes a one-line temptation
with no alarm attached, and it is exactly the sort of shortcut that survives because it works.

Adding the guard before the adapter exists costs one grep. Adding it afterwards means first
cleaning up whatever leaked, against a working UI that people are reluctant to break.

**Open question:** the guard's value depends on where the adapter lives. If it is a crate in this
workspace (`md2pdf-gui`), the rule is exactly the shape above. If it is a separate repository, the
guard is trivially satisfied and close to useless. **This should be settled before building G2.**

---

### G3 · Determinism in the pure core — `INV-7`, and the one that enables a capability

**What:** no `SystemTime`, `Instant`, `rand`, `std::env`, or locale-dependent behaviour in
`domain`, `convert`, or `typeset`.

**Verified 2026-08-20 — the property already holds.** The same document rendered twice, 1.1 seconds
apart, produced **byte-identical PDFs** (14,972 bytes, no differing byte). No timestamp leaks into
the output; `TypstWorld::today()` returning `None` is doing real work.

That result changes what this guard *is*. It does not promise a property we lack — it **protects one
we already have**, and one that is easy to destroy with a single innocuous line.

**What it enables, today:**

- **Golden-hash tests.** A fixture corpus can assert a PDF's exact hash. Any rendering change
  anywhere becomes a single failing test naming the document, rather than something noticed by eye
  three phases later.
- **Cross-platform verification becomes a one-line comparison.** INV-7 says output is identical on
  macOS, Windows and Linux. With determinism, proving it is `sha256` on both machines. Without it,
  the invariant is untestable in principle.
- It is the precondition for a reproducible-build claim, should distribution ever want one.

This is the guard I would build first, because it is the only one that unlocks something rather
than merely preventing something.

---

### G4 · No subprocess spawning

**What:** no `std::process::Command` anywhere.

**Why:** pandoc was rejected specifically to avoid a ~150MB sidecar, and "one executable, no
dependencies" carries the distribution story. This is the guard most likely to be violated
*reasonably* — shelling out to `pdftoppm` or a system font tool is an easy local fix that works
perfectly on the machine where it was written and breaks packaging everywhere else.

**Lower priority:** the temptation is real but conspicuous. Nobody adds a subprocess by accident.

---

### G5 · Domain purity — `INV-13`

**What:** `md2pdf-domain`'s manifest may list only `serde` and `thiserror`.

**Why:** everything depends on the domain, so a leak there contaminates the whole tree, and it is
where "just this once" is most tempting because it is where the types live.

**Lowest priority, and honestly close to self-enforcing:** the manifest is three lines long, and
any addition to it is conspicuous in review. This is a rule better kept by the doc comment already
in `lib.rs` than by a script.

---

## Doubts

Recorded because a plan without them is a sales pitch.

**D1 · ~~Grep is the wrong tool; `cargo-deny` exists~~ — RESOLVED 2026-08-20, doubt yields.**
The reasoning was sound: `cargo-deny`'s `bans` section is built for exactly G1/G2/G5 and understands
the dependency graph instead of pattern-matching text.

**Checked: it is not installed**, and adding a `cargo install` step to `verify.sh` breaks CLAUDE.md's
rule that *CI runs the same command you run locally* — a gate that only works after a network fetch
is not a gate that works everywhere. Hand-rolled greps are therefore the correct choice here, not
merely a defensible one. If `cargo-deny` is ever adopted for another reason, move G1 into it then.

**D2 · Both existing guards have fired on their own prose. Twice.**
Once on a Cargo.toml comment containing the word "typst", once on a doc comment explaining the
`std::fs` rule. Each new grep is a new opportunity for the same failure, and a *blocking* guard that
false-positives stops all work until someone reasons out why a sentence is a violation.

*Mitigation:* the `strip_comments` helper now exists and must be used by every new source grep from
the start, and every guard must ship with a **planted-violation negative control** proving it still
catches the real thing. That is now the pattern; it should be an explicit requirement, not a habit.

**D3 · G4 and G5 may be ceremony.**
Applying the test at the top of this document — *tempting and invisible* — G1, G2 and G3 clearly
qualify: transitive network deps are invisible, a UI import is tempting under deadline, and a
`SystemTime` call is both. G4's violation is conspicuous. G5's manifest is three lines. Building all
five because five is a satisfying number would be exactly the speculative generality
`invariants.md` exists to prevent.

**D4 · Guards are load-bearing in CI, so a false positive is expensive.**
Every guard added increases the chance that a green build is blocked by the guard rather than by a
defect. Three guards is a reasonable ceiling for a project this size; five starts to look like a
policy layer.

**D5 · G2's shape is unsettled** until the adapter's location is decided (see G2, open question).

**D6 · None of this is tested against a real violation yet.**
The claims above are reasoned, not demonstrated, apart from the two things that were verified
empirically (the clean lock file, and PDF determinism). Each guard's negative control is part of
building it, not part of proposing it.

---

## Recommendation

**Build G3, G1, G2 — in that order. Do not build G4 or G5.**

- **G3 first**, because it is the only one that enables something: golden-hash tests and a testable
  INV-7, both available immediately since determinism already holds.
- **G1 second**, because it converts the product's strongest claim into an auditable fact, and the
  graph is currently clean so it starts honest.
- **G2 third, and before phase 4 begins**, because that is when it stops being free — but only once
  the adapter's location is settled.
- **G4 and G5 not at all**, unless something actually violates them. Record the reasoning in
  `invariants.md` so the decision is not re-litigated.

Each guard ships with: comment stripping from the start, a planted-violation negative control, and
a one-line entry in the script's header explaining which invariant it enforces.

## Not in scope

**Glossary synonym drift.** The GLOSSARY carries explicit `⚠️ NOT` lists — never "render", never
"output dir", never "overflow policy" — and vocabulary drift is the exact failure it exists to
prevent. It is also the only rule in the project with no enforcement whatsoever, which makes it
tempting.

It is excluded because it would grep **prose**, where D2's false-positive problem is not an edge
case but the normal case: any document explaining the rule necessarily contains the banned words.
A guard that must be taught to ignore the documentation of itself is the wrong shape. If drift
becomes a real problem, the answer is a review checklist, not a script.


---

## Build sequence — T22, T23, T24 *(approved 2026-08-20)*

Three successive builds, smallest blast radius first. Each ships with the same three things, which
are now a **requirement rather than a habit**, because both existing guards have fired on their own
prose (D2):

1. comment stripping via the existing `strip_comments` helper, from the first line of the grep;
2. a **planted-violation negative control**, run and reported — a guard nobody has seen fail is not
   known to work;
3. a one-line entry in `check-boundaries.sh`'s header naming the invariant it enforces.

### ✅ T22 · G3 — determinism, and the golden-hash tests it unlocks *(built 2026-08-20)*

**First, because it is the only guard that enables something.** Determinism was verified to hold
already (byte-identical PDFs 1.1 seconds apart), so this protects a live property and cashes it in
the same task.

- **Guard:** no `SystemTime`, `Instant`, `rand`, `std::env::var`, or `chrono` in `md2pdf-domain`,
  `md2pdf-convert`, `md2pdf-typeset`.
  - **Exemption, deliberate and narrow:** `md2pdf-paths` is *not* in the list — `testing::TempDir`
    uses `SystemTime` for unique names, which is test scaffolding and never touches output.
- **Cash it in:** a golden-hash test over the existing fixture corpus. One `sha256` per document,
  asserted. Any rendering change anywhere becomes a single failing test naming the document.
- **Negative control:** add a `SystemTime::now()` to `md2pdf-convert`; the guard must fail. Change
  one glyph in a fixture; the golden test must fail.

**Doubt carried in:** golden hashes are only as useful as the discipline of updating them
deliberately. A hash that gets refreshed reflexively whenever it goes red is worse than no test,
because it looks like coverage. The failure message must say *"if this change was intended, update
the hash and say why in the commit"* — the test's own instructions are the mitigation.

**Second doubt — largely yields.** The hash pins the whole PDF, so a Typst upgrade turns every
golden test red at once. With a handful of fixtures that is a handful of lines to update, and a
Typst upgrade is already a scheduled, reviewed task with its own runbook (`docs/typst-upgrade.md`) —
so red golden tests are the *correct* signal, not noise. Keep the fixture count small enough that
this stays true.

**Third doubt — RESOLVED 2026-08-20, and it was the one that mattered.** The determinism check on
2026-08-20 ran twice *in a single process*, which is not what a golden test does: a stored hash is
compared against a value produced by a **different run**, so any per-process nondeterminism (hash
seeds, allocator addresses leaking into output) would have passed that check and failed as a golden
test — after the work was done.

**Checked properly: three separate processes produced the identical hash** (`03a3cda8db8f9ce4`,
17,644 bytes) over a document with a heading, emphasis, bold, an embedded image and a table. Golden
hashes are viable.

Two useful side-findings: `md2pdf_domain::fnv1a` is sufficient as the digest — this is change
detection, not tamper resistance, the same reasoning as `ElementId` — so **G3 needs no new
dependency**; and the pure core currently contains **no** `SystemTime`, `Instant`, `rand` or
`std::env` at all, so the guard starts from a true state.

### T23 · G1 — no network

- **Guard:** grep **`Cargo.lock`** for network-capable crates (`reqwest`, `ureq`, `hyper`, `tokio`,
  `native-tls`, `rustls`, `curl`, `isahc`, `attohttpc`, `surf`), plus `std::net` in source.
  The lock file is the point: it catches the transitive case a manifest grep cannot see.
- **Verified precondition:** the graph is currently clean, so this starts honest.
- **Negative control:** add `ureq` to a crate's manifest, run `cargo check` to update the lock, and
  confirm the guard fails. Then revert *and confirm the lock file is restored* — this is the one
  negative control that mutates a committed artefact, so cleanup must be checked, not assumed.

**Doubt carried in:** the crate list is a denylist, and denylists are always incomplete — a network
stack nobody listed passes silently. `cargo-deny` would do this properly by understanding the graph
(D1). The mitigation is honesty in the failure message: this guard raises the cost of adding a
network dependency; it does not make it impossible.

### T24 · G2 — the engine never learns what a UI is

**Last, and gated.** Do not build until the adapter's location is settled (G2's open question): if
the egui adapter is a crate in this workspace the guard is an allow-list of one, exactly like the
typst rule; if it lives in a separate repository the guard is trivially satisfied and close to
useless.

- **Guard:** no `eframe`, `egui`, `winit`, `wgpu` in any manifest except the adapter's.
- **Timing:** before phase 4 begins. This is the whole point — the guard is free now and expensive
  once a working UI exists that people are reluctant to break.
- **Negative control:** add `egui` to `md2pdf-engine`'s manifest; the guard must fail.

### Not being built

**G4 (no subprocess)** and **G5 (domain purity)** are declined, per D3: their violations are
conspicuous rather than invisible, and building all five because five is a satisfying number is the
speculative generality `invariants.md` exists to prevent. Recorded here so the decision is not
re-litigated — if something ever violates them, that is new evidence and the decision can change.
