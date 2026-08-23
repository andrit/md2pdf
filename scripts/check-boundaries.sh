#!/usr/bin/env bash
# Enforces the two isolation boundaries that clippy cannot express, because clippy
# has no way to say "this lint applies everywhere EXCEPT this crate".
#
#   md2pdf-typeset  is the only crate that may link the typst crate      (INV-10)
#   md2pdf-paths    is the only crate that may touch the filesystem       (INV-9)
#   the pure core   must contain nothing nondeterministic                 (INV-7)
#   nothing anywhere may reach the network                                 (INV-1)
#   the engine never learns what a window is                              (INV-8)
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

# INV-1: no network, ever. No accounts, no telemetry, no fetching — which is what makes
# running cost ~zero and lets md2pdf work on a plane.
#
# Greps **Cargo.lock**, not the manifests. The realistic accident is not someone adding
# HTTP deliberately; it is a convenience crate pulling in a stack transitively, and only
# the lock file sees the resolved graph. Verified clean at 308 crates when this was
# written.
#
# Honest limit: this is a denylist, so a network stack nobody listed passes silently. It
# raises the cost of adding one; it does not make it impossible. `cargo-deny` would do
# this properly by understanding the graph, and is not used because a gate that needs a
# network fetch to install is not a gate that runs everywhere.
net_crates='reqwest|ureq|hyper|tokio|native-tls|rustls|openssl|curl|isahc|attohttpc'
net_crates="$net_crates|surf|socket2|mio|h2|quinn|async-std|smol|awc|tungstenite"
netdeps=$(grep -nE "^name = \"($net_crates)\"" Cargo.lock || true)
if [ -n "$netdeps" ]; then
  echo "FAIL: a network-capable crate is in the dependency graph (INV-1):"
  echo "$netdeps" | sed 's/^/  /'
  fail=1
fi

netsrc=$(grep -rn --include='*.rs' -E '\bstd::net\b' crates | strip_comments || true)
if [ -n "$netsrc" ]; then
  echo "FAIL: network access in source (INV-1):"
  echo "$netsrc" | sed 's/^/  /'
  fail=1
fi

# INV-8: the engine does not know what a window is. Only an adapter may.
#
# The out-of-process-adapter design rests entirely on this, and today it is protected by
# nothing but the absence of a UI. The moment phase 4 starts, "just import egui here for
# a second to get the preview working" is a one-line change with no alarm attached — and
# it is exactly the kind of shortcut that survives because it works.
#
# Cheap now, expensive later: adding this before the adapter exists costs one grep.
# Adding it afterwards means cleaning up whatever leaked, against a working UI that
# nobody wants to break.
#
# Scoped to the five core crates rather than the workspace, because the adapter is an
# in-workspace crate (`md2pdf-cli` today, `md2pdf-gui` on the host later) and is
# *supposed* to link a toolkit. That is also this guard's honest limit: it sees direct
# dependencies of those five, not a toolkit arriving transitively. The realistic accident
# here is a direct import, not a transitive one — unlike the network rule, which greps
# the lock file for exactly that reason.
core_crates='crates/md2pdf-domain crates/md2pdf-convert crates/md2pdf-typeset'
core_crates="$core_crates crates/md2pdf-paths crates/md2pdf-engine"
ui='eframe|egui|egui_extras|egui-winit|winit|wgpu|glutin|tao|wry|iced|druid|slint|dioxus'
ui="$ui|gtk|gtk4|sdl2|softbuffer|raw-window-handle|accesskit|muda|arboard|rfd"

ui_manifests=$(grep -nE "^[[:space:]]*($ui)[[:space:]]*=" $(printf '%s/Cargo.toml ' $core_crates) || true)
if [ -n "$ui_manifests" ]; then
  echo "FAIL: a UI toolkit in a core crate's manifest — the engine must not know what a window is (INV-8):"
  echo "$ui_manifests" | sed 's/^/  /'
  fail=1
fi

ui_src=$(grep -rn --include='*.rs' -E "\\buse +($ui)::|\\b($ui)::" $core_crates | strip_comments || true)
if [ -n "$ui_src" ]; then
  echo "FAIL: a UI toolkit referenced in core source (INV-8):"
  echo "$ui_src" | sed 's/^/  /'
  fail=1
fi

[ $fail -eq 0 ] && echo "boundaries OK: typst confined, fs confined, core deterministic, no network, no UI in the engine"
exit $fail
