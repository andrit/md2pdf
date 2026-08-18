#!/usr/bin/env python3
"""PreToolUse guardrail: designer instruction files are append-only.

Files under a `.workbench/designer/current/` directory ending in `.md` are the
designer<->AI interchange record. Per CLAUDE.md ("Replying to Markdown-Authored
Instructions"), replies are appended, never destructive. This hook enforces that
mechanically: it denies any Edit/Write that would remove or replace existing
content in such a file.

Contract (Claude Code PreToolUse hook):
  - stdin: JSON with tool_name and tool_input (file_path, old_string, new_string).
  - allow: exit 0 with no output.
  - deny:  exit 0, print JSON with hookSpecificOutput.permissionDecision = "deny".

Fail-open by design: on any parse error it allows the call. This is a safety net
around the model-followed CLAUDE.md convention, not the primary gate — better to
let an edit through than to wedge all edits on a malformed payload.
"""
import sys
import json
import os
import shutil
import datetime


def allow():
    sys.exit(0)


def deny(reason: str):
    print(json.dumps({
        "hookSpecificOutput": {
            "hookEventName": "PreToolUse",
            "permissionDecision": "deny",
            "permissionDecisionReason": reason,
        }
    }))
    sys.exit(0)


def archive_task_plan(path: str):
    """Snapshot the current task-plan file before it is edited or overwritten.

    Task-plan files are the AI operator's living working docs, so they are exempt
    from the append-only guard — but every prior version is preserved to a sibling
    `history/` folder first (a pre-git safety net). Archives live OUTSIDE
    `designer/current/` so they are neither ingested by the file watcher nor
    re-guarded. Fail-open: an archiving hiccup never blocks the edit.
    """
    try:
        if not os.path.isfile(path):
            return
        cur_dir = os.path.dirname(path)                                   # .../designer/current
        hist_dir = os.path.join(os.path.dirname(cur_dir), "history")      # .../designer/history
        os.makedirs(hist_dir, exist_ok=True)
        stem, ext = os.path.splitext(os.path.basename(path))
        ts = datetime.datetime.now().strftime("%Y%m%d-%H%M%S-%f")
        shutil.copy2(path, os.path.join(hist_dir, f"{stem}.{ts}{ext}"))
    except Exception:
        pass


def main():
    try:
        data = json.load(sys.stdin)
    except Exception:
        allow()

    tool = data.get("tool_name", "")
    tool_input = data.get("tool_input") or {}
    path = tool_input.get("file_path", "") or ""

    # Guard the designer interchange record: the phase inbox and feature discussions.
    # Everything else passes through.
    GUARDED = (".workbench/designer/current/", "/design/discussions/")
    if not path.endswith(".md") or not any(marker in path for marker in GUARDED):
        allow()

    # Task-plan files are exempt from the append-only guard (they're the AI's living
    # working docs) — but snapshot the prior version first, then allow the edit.
    if os.path.basename(path).startswith("task-plan-"):
        archive_task_plan(path)
        allow()

    if tool == "Write":
        # Overwriting an existing, non-empty designer file is destructive.
        try:
            if os.path.isfile(path) and os.path.getsize(path) > 0:
                deny(
                    f"Designer file is append-only: '{path}'. Use Edit to append "
                    "(keep the existing text and add after it); do not overwrite it with Write."
                )
        except OSError:
            allow()
        allow()

    if tool == "Edit":
        old = tool_input.get("old_string", "") or ""
        new = tool_input.get("new_string", "") or ""
        # Non-destructive iff the new text still contains the old text verbatim
        # (pure addition — nothing removed or rewritten).
        if old in new:
            allow()
        deny(
            f"Designer file is append-only: '{path}'. This Edit removes or replaces "
            "existing content (new_string does not contain old_string). Redo it as an "
            "append: keep old_string intact inside new_string and add your new text after it."
        )

    allow()


if __name__ == "__main__":
    main()
