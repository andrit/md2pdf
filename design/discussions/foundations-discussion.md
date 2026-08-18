Foundation done. Knowledgebase now holds 3 documents / 93 chunks — the designer doc plus the
  two artifacts I wrote.

  design/event-storm.md — 40 domain events, commands with actors, policies, 9 read models,
  hotspots, 8 bounded context candidates.

  design/GLOSSARY.md — one term per concept, Rust naming conventions (the skill's
  TS/Postgres/React layer table doesn't apply), synonym-drift table.

  What the storm actually surfaced
  
  The pipeline nouns needed splitting. Source (.md) → Markup (emitted Typst) → Compilation
  (paged output plus its Diagnostic) → Output (PDF on disk). Each has a distinct lifecycle, so
  none of them gets called "document."

  Compromise is the load-bearing noun. One recorded concession — shrunk-to-floor, rotated,
  clipped, image-skipped. The Diagnostic is just the set of them for one Compilation. Once
  that's the model, exception-driven batch preview, the attention gate, and Element-scope
  adjustment all fall out of it rather than needing separate machinery. Every read model in the
  app derives from the diagnostic or the source list; there's no UI state the pipeline doesn't
  already produce. 

  Conversion and Typesetting are both pure functions — text and values in, text and values out.
  All I/O sits in Job, Output, and PathBroker. Functional core / imperative shell falls out of
  the domain here rather than being imposed on it, which means the escalation ladder is
  testable with no filesystem and no window.

  I renamed the three adjustment "tiers" to Scopes. "Tier" means billing tier in every other
  project here, and md2pdf may yet become paid. Marked DISPUTED in the glossary with the
  resolution, rather than silently changed.

  Three things that block building, in order

  1. Verify Typst's measure()/layout() against the current version. The designer doc flags this
  itself. The entire measured-fit thesis — and the justification for Typst over a browser —
  rests on that API existing in the assumed shape. This wants a spike before any UI exists.
  2. ElementId has no scheme. The Diagnostic must name an element precisely enough that the
  user can act on it and a recompile can apply the override to that same element. Element-scope
  adjustment cannot ship without it. This is the largest unmodelled thing in the design.
  3. Fonts are unaddressed anywhere in the stack decisions. "GitHub fidelity" implies specific
  typefaces, and identical output across the three platforms requires fonts embedded with the
  app, not resolved from the system. It affects bundle size, licensing, and the cross-platform
  determinism argument that killed Tauri.

  Saved a project memory with the settled vocabulary and these three blockers. Natural next
  step is define-bounded-contexts for the Conversion → Typesetting → Review interfaces — but
  I'd argue the Typst API spike should come first, since a surprise there reshapes contexts
  1–3. 