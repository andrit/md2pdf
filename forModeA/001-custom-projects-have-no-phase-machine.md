# 001 — `custom` projects have no phase machine, and `task_list` is broken

**Status:** open
**Origin:** md2pdf, 2026-08-19. Surfaced while recording completed work across a multi-session build.

## Problem

Two separate issues, both of which leave a `custom` project unable to record its own SDLC progress
in the workbench. Together they mean md2pdf's phase and task state lives **entirely** in
checked-in design documents, and the workbench has no visibility into a project that is four
phases deep with eight committed tasks.

### A. `custom` seeds zero phases, so nothing can hold progress

`project.json` for type `custom` defines no SDLC phase list (`total_phases: 0`). Consequences:

- `session_start` reports *"Project has not started — call phase_advance to begin"* indefinitely,
  which reads as a skipped step rather than an absent feature. It is misleading on every session
  start of every custom project.
- `phase_advance` has nothing to advance into.
- `set_current_phase` validates its index against the project type's phase list, so it rejects
  every index — including 0. There is no way to record a phase at all.

The universal advance gate in `CLAUDE.md` (`/phase-audit`, with `exempt_phase_ids`) is written in
terms of phase indices, so it also has nothing to attach to for these projects.

### B. `task_list` returns a Postgres error for every call

```
mcp__workbench__task_list {}                                  -> error
mcp__workbench__task_list { state: "planned" }                -> error
mcp__workbench__task_list { phase_index: 0, type: "sdlc" }    -> error

PostgresError: could not determine data type of parameter $1
```

Every argument combination fails, including no arguments, so this is not a usage problem — a bind
parameter is untyped in the query. This blocks the natural workaround for (A): mirroring a
project's own phases into workbench tasks, since `task_create` carries a `phase_index`.

`session_start` returns `open_tasks` without error, so the defect appears specific to `task_list`'s
query rather than the tasks table.

## Proposed change

1. **Fix `task_list`** — cast or type the bind parameter. Small, and independently valuable: it is
   currently unusable for every project, not just custom ones.
2. **Let `custom` projects define their own phases.** Either a default generic phase list for
   `custom`, or support for a project-supplied phase list (e.g. read from the project's own
   `.workbench/` or a `phases` key in `project.json`) that `set_current_phase` and `phase_advance`
   validate against.
3. **Failing that, make the absence honest.** If custom projects are deliberately phase-less, then
   `session_start` should say so — *"project type `custom` defines no SDLC phases"* — rather than
   *"has not started"*. The current wording invites a session to try to fix a non-problem, which is
   what happened here.

## What *not* to do

- Do not migrate md2pdf to a different project type to dodge this. `custom` was chosen deliberately;
  md2pdf is meant to be the evidence that later justifies a `desktop` type (roadmap C26), and
  forcing it into `web`/`cli` now would corrupt that evidence.
- Do not silently auto-create phases for existing custom projects — md2pdf's phases are already
  recorded in `design/roadmap.md`, and a second, disagreeing source of truth is worse than none.

## Acceptance criteria

- `task_list` returns results for at least one argument combination, including no arguments.
- Either a custom project can record a current phase and advance through it, **or** `session_start`
  states plainly that the type has no phases and stops reporting "not started".
- md2pdf can then either mirror `design/roadmap.md` into the workbench, or keep that document as the
  acknowledged system of record — but the choice becomes deliberate rather than forced.
