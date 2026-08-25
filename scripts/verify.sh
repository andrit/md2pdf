#!/usr/bin/env bash
# The single command CI runs and you run. Separate steps, so a failure names the
# gate that broke rather than reporting "build failed".
set -uo pipefail
cd "$(dirname "$0")/.."
rc=0
step() { echo; echo "=== $1 ==="; shift; "$@" || { echo "^^ FAILED"; rc=1; }; }

step "boundaries"  ./scripts/check-boundaries.sh
# Safe to gate on: it reports only entries whose commit already exists, so the one you
# are about to make is still legitimately `<pending>`. Nothing ran it until 2026-08-23,
# which is how eight entries drifted after being committed — the exact drift it was
# written to catch.
step "commit-log"  ./scripts/commit-log.sh
step "fmt"         cargo fmt --all -- --check

# `md2pdf-gui` is excluded from every step that *links*, and only from those.
#
# It is the one crate that cannot be linked in the container: `eframe` needs system GL
# and X11 libraries this image does not have, and `ldconfig` finds none. That is a fact
# about the image, not about the code — so the gate typechecks it (`check` does not
# link) and leaves the linking to the host, which is also the only place it can run.
#
# Excluding it from `clippy`/`test`/`build` rather than skipping it entirely is the
# distinction that matters: a type error in the GUI still turns this red.
GUI=md2pdf-gui
step "clippy"      cargo clippy --workspace --exclude $GUI --all-targets -- -D warnings
step "gui-check"   cargo check -p $GUI --all-targets

# `-j 1` for the test step, and only the test step.
#
# Each test binary that links typst statically pulls in ~250 crates, and linking two of
# them at once exhausts memory on a 4 GB machine — `ld` is killed by the OOM killer,
# which surfaces as an intermittent "linking with cc failed" that reads as a flake until
# you look for signal 9. It has now bitten three times, and gets likelier with every
# crate that gains an integration test.
#
# Costs little: clippy --all-targets above has already compiled the graph, so this step
# is mostly linking, and serialising the links is the point.
step "test"        cargo test --workspace --exclude $GUI -j 1
step "build"       cargo build --workspace --exclude $GUI
echo; [ $rc -eq 0 ] && echo "verify: PASS" || echo "verify: FAIL"
exit $rc
