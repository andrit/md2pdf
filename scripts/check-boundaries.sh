#!/usr/bin/env bash
# Enforces the two isolation boundaries that clippy cannot express, because clippy
# has no way to say "this lint applies everywhere EXCEPT this crate".
#
#   md2pdf-typeset  is the only crate that may link the typst crate      (INV-10)
#   md2pdf-paths    is the only crate that may touch the filesystem       (INV-9)
#   the pure core   must contain nothing nondeterministic                 (INV-7)
#
# The dependency graph already makes the first one impossible to violate by accident
# (typst is not in the other manifests). This catches the second, and catches someone
# "helpfully" adding typst to another Cargo.toml.
set -euo pipefail
cd "$(dirname "$0")/.."
fail=0

# Drop matches that are comments rather than code.
#
# `grep -rn` emits `path:line:content`, so this strips lines whose content begins with
# `//`, `//!` or `*`. Both source greps below have now fired on their own explanatory
# prose — once on a Cargo.toml comment, once on a doc comment describing the very rule
# being enforced. A guard that cannot tell code from a sentence about code trains people
# to reword comments, which is exactly the wrong lesson.
#
# Conservative on purpose: a trailing comment on a real code line is still reported.
# Over-reporting is a nuisance; under-reporting would defeat the guard.
# `|| true` is repeated at each call site: `pipefail` propagates a no-match exit from
# any stage, so catching it only inside the function is not enough.
strip_comments() { grep -vE '^[^:]+:[0-9]+:[[:space:]]*(//|\*)' || true; }

leaks=$(grep -rn --include='*.rs' -E '\buse +typst(_[a-z]+)?::|\btypst(_[a-z]+)?::' crates \
        | grep -v '^crates/md2pdf-typeset/' | strip_comments || true)
if [ -n "$leaks" ]; then
  echo "FAIL: typst referenced outside md2pdf-typeset (the anti-corruption layer):"
  echo "$leaks" | sed 's/^/  /'
  fail=1
fi

# Match a dependency *key* at line start (`typst = ...`, `typst-pdf = ...`), not the
# word "typst" wherever it appears. The previous bare grep flagged its own explanatory
# comments, and would flag `md2pdf-typeset = { path = ... }` — a legitimate dependency
# on the anti-corruption layer, which is the supported way to reach typst.
manifests=$(grep -lE '^[[:space:]]*typst(-[a-z]+)?[[:space:]]*=' crates/*/Cargo.toml \
            | grep -v 'md2pdf-typeset' || true)
if [ -n "$manifests" ]; then
  echo "FAIL: typst added to a manifest outside md2pdf-typeset:"
  echo "$manifests" | sed 's/^/  /'
  fail=1
fi

fs=$(grep -rn --include='*.rs' -E '\bstd::fs\b|\bFile::(open|create)\b' crates \
     | grep -v '^crates/md2pdf-paths/' | strip_comments || true)
if [ -n "$fs" ]; then
  echo "FAIL: filesystem access outside md2pdf-paths (PathBroker):"
  echo "$fs" | sed 's/^/  /'
  fail=1
fi

# INV-7: identical output on every platform, which is only testable if the pure core
# is deterministic. Verified 2026-08-20: three separate processes produced byte-identical
# PDFs, which is what makes the golden-hash tests in md2pdf-convert possible. One
# `SystemTime::now()` in a PDF timestamp would silently end that.
#
# md2pdf-paths is deliberately NOT listed: `testing::TempDir` uses the clock for unique
# directory names, which is test scaffolding and never reaches output.
nondet=$(grep -rn --include='*.rs' -E '\b(SystemTime|Instant|std::env|rand::|thread_rng)\b' \
         crates/md2pdf-domain crates/md2pdf-convert crates/md2pdf-typeset \
         | strip_comments || true)
if [ -n "$nondet" ]; then
  echo "FAIL: nondeterminism in the pure core (breaks reproducible output, INV-7):"
  echo "$nondet" | sed 's/^/  /'
  fail=1
fi

[ $fail -eq 0 ] && echo "boundaries OK: typst confined, fs confined, pure core deterministic"
exit $fail
