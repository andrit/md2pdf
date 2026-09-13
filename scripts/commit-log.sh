#!/usr/bin/env bash
# Keep design/commit-log.md's hashes true.
#
# The convention is to write an entry when *staging*, so the reasoning is captured while it
# is still in your head — which means the hash cannot be known yet and the entry goes in as
# `<pending>`. Nothing then fills it in afterwards unless something checks. This is that half.
#
#   ./scripts/commit-log.sh          report stale entries (exit 1 if any)
#   ./scripts/commit-log.sh --fix    fill them in from git log
#
# A `<pending>` entry is *correct* between staging and committing, so this reports only
# entries whose commit already exists — never the one you are about to make.
#
# This script was born here (2026-08-26) and ported to the workbench (2026-08-30), where it
# earned three more fixes the hard way: whitespace folding, the HEAD check that turns "hashes
# are current" into a gate that can fail, and typography folding after seven entries were found
# orphaned by an em dash, a backtick or an apostrophe. Synced back verbatim 2026-09-13; the
# workbench copy at scripts/commit-log.sh is the reference, keep the two identical.
set -uo pipefail
cd "$(dirname "$0")/.."

exec python3 - "${1:-}" <<'PY'
import re
import subprocess
import sys

fix = sys.argv[1] == "--fix"
path = "design/commit-log.md"


def parse(line):
    """The two entry forms. Returns (subject, rest) or None."""
    m = re.match(r"^- `<pending>` \*\*(?P<bold>.+?)\*\*(?P<rest>.*)$", line)
    if m:
        return m.group("bold"), m.group("rest")
    m = re.match(r"^- `<pending>` (?P<plain>[^*].*?)(?P<rest>\.?)$", line)
    if m:
        return m.group("plain"), m.group("rest")
    return None


def norm(s):
    """Fold the ways one subject gets typed two ways, then collapse whitespace, before comparing.

    Comparison only — the filled line always keeps the entry's own text.

    Whitespace: `.strip()` above already handles a *leading* space (`decf2fb " fix(tasks): …"`). This
    handles the same class inside the string: a subject typed or pasted with a double space matches
    an entry written with one. Caught 2026-08-31 when `6613bf8` was committed with three spaces
    before its last word.

    Typography: the entry is written in the log's prose, the subject is typed at the shell, and the
    prefix test orphaned seven entries that way before anyone looked (2026-09-13 audit): em dash
    typed as `-` (ab10dec, da2ba90, 375ca4d), backticks stripped (d5fc7f6), an apostrophe dropped
    (bdb350c). Each fold below answers one observed class; nothing is folded on speculation, and
    case is deliberately left alone — a lost letter (`458f616 "ix(compose)"`) is a typo, not a
    rendering, and must stay unmatched so it gets filled by hand with a note.
    """
    s = s.replace("\u2014", "-").replace("\u2013", "-")      # em dash, en dash
    s = re.sub(r"-{2,}", "-", s)                               # `--` typed for a dash
    s = s.replace("`", "")                                     # backticks around code terms
    s = s.replace("\u2019", "").replace("'", "")               # apostrophe, curly or straight: dropped
    s = s.replace("\u201c", '"').replace("\u201d", '"')       # curly double quotes
    return " ".join(s.split())


# Self-check, because this script's whole failure mode is going *quietly* blind to one form
# and reporting success — a silent pass looks exactly like a clean tree. Cheap, and it runs
# inside verify.sh.
for probe, want in [
    ("- `<pending>` **fix(tasks): a thing** (C28).", "fix(tasks): a thing"),
    ("- `<pending>` docs: a thing without bold", "docs: a thing without bold"),
    ("- `<pending>` docs: a thing with a stop.", "docs: a thing with a stop"),
]:
    got = parse(probe)
    if not got or got[0] != want:
        print(f"commit-log: the matcher is blind to a form it must see:\n  {probe}\n  got {got}")
        sys.exit(1)

if norm("a  b   c") != "a b c" or norm(" a b ") != "a b":
    print("commit-log: subject normalisation is broken")
    sys.exit(1)

# The seven real orphans, as (entry, commit) pairs: each must fold to equal. And one that must
# NOT — a normaliser that matches everything is a status line, not a gate.
for entry, commit in [
    ("feat(graph): R5 \u2014 project identity", "feat(graph): R5 - project identity"),
    ("docs(x): one \u2013 two", "docs(x): one -- two"),
    ("fix(db): the repair skipped every `self:*` handover", "fix(db): the repair skipped every self:* handover"),
    ("fix(rag): resolve the workbench's own rows", "fix(rag): resolve the workbenchs own rows"),
    ("fix(rag): resolve the workbench\u2019s own rows", "fix(rag): resolve the workbenchs own rows"),
    ("docs: say \u201cno\u201d", 'docs: say "no"'),
]:
    if norm(entry) != norm(commit):
        print(f"commit-log: normalisation is blind to a drift it must fold:\n  {entry!r}\n  {commit!r}")
        sys.exit(1)
if norm("fix(compose): mount").startswith(norm("ix(compose): mount")) or norm("ix(compose): mount") == norm("fix(compose): mount"):
    print("commit-log: normalisation folds a lost letter — it is too wide")
    sys.exit(1)

log = subprocess.run(
    ["git", "log", "--format=%h\t%s"], capture_output=True, text=True
).stdout.splitlines()
# `.strip()` on the subject: this repo has commits whose message was pasted with a leading
# space (`decf2fb " fix(tasks): …"`), which would match nothing and sit `<pending>` forever
# while this script reported success. Silent drift from a character nobody can see.
commits = [
    (h, s.strip()) for h, s in (line.split("\t", 1) for line in log if "\t" in line)
]


lines = open(path).read().split("\n")
out, stale, ambiguous = [], [], []

for line in lines:
    # Bold is optional: code commits carry a **bold subject** and a body, docs commits are
    # listed by subject alone. Matching only the bold form would be blind to most entries.
    parsed = parse(line)
    if not parsed:
        out.append(line)
        continue
    subject, rest = parsed
    bold = line.startswith("- `<pending>` **")

    hits = [(h, s) for h, s in commits if norm(s).startswith(norm(subject))]
    if len(hits) == 1:
        h, s = hits[0]
        stale.append(f"  {h}  {s}")
        filled = (f"- `{h}` **{subject}**{rest}" if bold else f"- `{h}` {subject}{rest}")
        out.append(filled if fix else line)
    else:
        if len(hits) > 1:
            ambiguous.append(f"  {subject!r} matches {len(hits)} commits: {hits}")
        out.append(line)

if ambiguous:
    print("cannot resolve — the subject is not unique:")
    print("\n".join(ambiguous))
    sys.exit(1)

content = "\n".join(out)
if fix:
    open(path, "w").write(content)


def head_unrecorded(text):
    """The OTHER direction: does the newest commit have an entry at all?

    Everything above walks entries → commits, so it can only see an entry it can already match.
    An entry whose subject drifted from its commit subject matches nothing, lands in the `len(hits)
    == 0` branch, and is passed over in silence — while this script prints "hashes are current".
    That is this tool wearing the same disguise it exists to catch: a check reporting success
    because its failure is unrepresentable.

    Caught the hard way on 2026-08-31. An entry was written as ``keep `.claude.json` inside…`` and
    committed as ``keep .claude.json inside…``; the matcher needs the entry subject to be a verbatim
    PREFIX of the commit subject, so the backticks orphaned it permanently and nothing said a word.

    A `<pending>` entry for the commit you are ABOUT to make is correct and must stay silent, so the
    question is asked of HEAD, which is already committed: either an entry carries its hash, or a
    pending entry still matches its subject and will fill at the next staging. Neither means the
    reasoning for that commit is not in the log and never will be.
    """
    if not commits:
        return None
    h, subject = commits[0]
    if re.search(rf"^- `{re.escape(h)}` ", text, re.M):
        return None
    for line in text.split("\n"):
        parsed = parse(line)
        if parsed and norm(subject).startswith(norm(parsed[0])):
            return None
    return (
        f"commit-log: HEAD has no entry.\n"
        f"  {h}  {subject}\n"
        f"  No entry carries that hash, and no `<pending>` entry's subject is a prefix of it.\n"
        f"  Either the entry was never written, or its subject drifted from the commit subject —\n"
        f"  the matcher is a prefix test, so backticks or an edited word orphan it permanently."
    )


# Self-check, same reasoning as the matcher probes: a detector that silently stops detecting is
# indistinguishable from a clean tree.
if commits:
    _h = commits[0][0]
    if head_unrecorded(f"- `{_h}` **anything**") is not None:
        print("commit-log: the HEAD check cannot see a filled entry — it is blind")
        sys.exit(1)
    if head_unrecorded("- `deadbee` **unrelated**") is None:
        print("commit-log: the HEAD check cannot fail — it is a status line, not a gate")
        sys.exit(1)

head_msg = head_unrecorded(content)

if fix:
    if stale:
        print(f"commit-log: filled in {len(stale)} hash(es)")
        print("\n".join(stale))
    else:
        print("commit-log: nothing to fill")
    if head_msg:
        print(head_msg)
        sys.exit(1)
    sys.exit(0)

if stale:
    print(f"commit-log: {len(stale)} entry(s) still `<pending>` whose commit exists:")
    print("\n".join(stale))
    print("\nfill them in:  ./scripts/commit-log.sh --fix")
    if head_msg:
        print(head_msg)
    sys.exit(1)

if head_msg:
    print(head_msg)
    sys.exit(1)

print("commit-log: hashes are current, and HEAD has an entry")
sys.exit(0)
PY
