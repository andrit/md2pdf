# forModeA — md2pdf's outbox to the workbench

This folder is this project's **outbox to Mode A** (the workbench). When work inside this project
(Mode B) surfaces something that belongs *upstream* — a scaffold gap, a project-type template fix, a
reusable pattern worth escalating, a feature or support request on the workbench itself — write it
here as a numbered proposal instead of losing it at the end of a session.

## Convention

- One file per proposal: `NNN-short-slug.md`, numbered in order.
- Each proposal is self-contained: problem, origin, proposed change, what *not* to do, acceptance
  criteria. A Mode A session must be able to pick it up cold, with no memory of this project.
- Declare a status line in the file so the workbench can tell live requests from settled ones:

  ```
  **Status:** open
  ```

  `open` (awaiting a decision) · `accepted` (agreed, not yet built) · `done` · `declined`.
  A file with no status line is treated as `open`.
- Nothing here changes this project's runtime. It is purely a handoff surface.

## How it reaches the workbench

A workbench (Mode A) session sees outstanding proposals from every project in its `session_start`
response, under `inbound_requests`, and in its orientation summary. Nothing needs to be sent — write
the file and it will be picked up.

**Replies usually cannot come back into this file.** Most projects are mounted read-only in Mode A,
so the workbench can read a proposal but not write to it. Expect a resolution to arrive as a change
to the workbench itself, with the reply recorded workbench-side or appended here later from a
session running inside this project.

## Why a folder rather than chat

A proposal discovered mid-session survives the session, is reviewable, and accumulates into a real
backlog the workbench can work through. In a product context this same shape is an **outbox for
support and feature requests** — the mechanism generalises well beyond this workbench.

## Open items

<!-- Add a line per proposal, newest last. -->
- [001](001-custom-projects-have-no-phase-machine.md) — **open** · `custom` projects seed zero SDLC phases so progress cannot be recorded, and `task_list` errors on every call.
