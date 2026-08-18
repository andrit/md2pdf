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
step "test"        cargo test --workspace
step "build"       cargo build --workspace
echo; [ $rc -eq 0 ] && echo "verify: PASS" || echo "verify: FAIL"
exit $rc
