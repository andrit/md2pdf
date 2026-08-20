#!/usr/bin/env bash
# The single command CI runs and you run. Separate steps, so a failure names the
# gate that broke rather than reporting "build failed".
set -uo pipefail
cd "$(dirname "$0")/.."
rc=0
step() { echo; echo "=== $1 ==="; shift; "$@" || { echo "^^ FAILED"; rc=1; }; }

step "boundaries"  ./scripts/check-boundaries.sh
step "fmt"         cargo fmt --all -- --check
step "clippy"      cargo clippy --workspace --all-targets -- -D warnings

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
step "test"        cargo test --workspace -j 1
step "build"       cargo build --workspace
echo; [ $rc -eq 0 ] && echo "verify: PASS" || echo "verify: FAIL"
exit $rc
