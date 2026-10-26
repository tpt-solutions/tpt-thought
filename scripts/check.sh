#!/usr/bin/env sh
# SPDX-License-Identifier: MIT OR Apache-2.0
# Runs the same checks as CI. Run from anywhere; the script moves to the repository root.
set -eu

cd "$(dirname "$0")/.."

echo "==> cargo fmt --check"
cargo fmt --all --check

echo "==> cargo clippy"
cargo clippy --workspace --all-targets -- -D warnings

echo "==> cargo test"
cargo test --workspace

echo "==> cargo deny check"
cargo deny check

echo "All checks passed."
