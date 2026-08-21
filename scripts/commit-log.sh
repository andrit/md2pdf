#!/usr/bin/env bash
# Keep design/commit-log.md's hashes true.
#
# The convention is to write an entry when *staging*, so the reasoning is captured while
# it is still in your head — which means the hash cannot be known yet and the entry goes
# in as `<pending>`. Nothing then filled it in afterwards, and seven entries sat pending
# across seven commits before anyone noticed. This is that missing half.
#
#   ./scripts/commit-log.sh          report stale entries (exit 1 if any)
#   ./scripts/commit-log.sh --fix    fill them in from git log
#
# A `<pending>` entry is *correct* between staging and committing, so this reports only
# entries whose commit already exists — never the one you are about to make.
set -uo pipefail
cd "$(dirname "$0")/.."

exec python3 - "${1:-}" <<'PY'
import re
import subprocess
import sys

fix = sys.argv[1] == "--fix"
path = "design/commit-log.md"

log = subprocess.run(
    ["git", "log", "--format=%h\t%s"], capture_output=True, text=True
).stdout.splitlines()
commits = [tuple(line.split("\t", 1)) for line in log if "\t" in line]

lines = open(path).read().split("\n")
out, stale, ambiguous = [], [], []

for line in lines:
    m = re.match(r"^- `<pending>` \*\*(.+?)\*\*(.*)$", line)
    if not m:
        out.append(line)
        continue
    subject, rest = m.group(1), m.group(2)

    # The bold text is the commit subject minus its task suffix, so match the whole
    # thing. Matching the task tag alone is ambiguous — "(T27)" named both the commit
    # that planned the census and the one that built it.
    hits = [(h, s) for h, s in commits if s.startswith(subject)]
    if len(hits) == 1:
        h, s = hits[0]
        stale.append(f"  {h}  {s}")
        out.append(f"- `{h}` **{subject}**{rest}" if fix else line)
    else:
        if len(hits) > 1:
            ambiguous.append(f"  {subject!r} matches {len(hits)} commits: {hits}")
        out.append(line)

if ambiguous:
    print("cannot resolve — the subject is not unique:")
    print("\n".join(ambiguous))
    sys.exit(1)

if not stale:
    print("commit-log: hashes are current")
    sys.exit(0)

if fix:
    open(path, "w").write("\n".join(out))
    print(f"commit-log: filled in {len(stale)} hash(es)")
    print("\n".join(stale))
    sys.exit(0)

print(f"commit-log: {len(stale)} entry(s) still `<pending>` whose commit exists:")
print("\n".join(stale))
print("\nfill them in:  ./scripts/commit-log.sh --fix")
sys.exit(1)
PY
