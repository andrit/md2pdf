#!/usr/bin/env bash
# Enforces the two isolation boundaries that clippy cannot express, because clippy
# has no way to say "this lint applies everywhere EXCEPT this crate".
#
#   md2pdf-typeset  is the only crate that may link the typst crate
#   md2pdf-paths    is the only crate that may touch the filesystem
#
# The dependency graph already makes the first one impossible to violate by accident
# (typst is not in the other manifests). This catches the second, and catches someone
# "helpfully" adding typst to another Cargo.toml.
set -euo pipefail
cd "$(dirname "$0")/.."
fail=0

leaks=$(grep -rn --include='*.rs' -E '\buse +typst(_[a-z]+)?::|\btypst(_[a-z]+)?::' crates \
        | grep -v '^crates/md2pdf-typeset/' || true)
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
     | grep -v '^crates/md2pdf-paths/' || true)
if [ -n "$fs" ]; then
  echo "FAIL: filesystem access outside md2pdf-paths (PathBroker):"
  echo "$fs" | sed 's/^/  /'
  fail=1
fi

[ $fail -eq 0 ] && echo "boundaries OK: typst confined to md2pdf-typeset, fs confined to md2pdf-paths"
exit $fail
