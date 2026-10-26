# Contributing to TPT Thought

## Licence of contributions

Unless you state otherwise, any contribution you intentionally submit for inclusion in this
project is licensed under **MIT OR Apache-2.0**, as in the rest of the repository, without any
additional terms or conditions.

## Rules that keep Thought small

Read `docs/DEPENDENCY_POLICY.md` before adding a dependency or a crate. In short:

- Create a crate only when a real API or dependency boundary exists.
- `tpt-thought-core` depends on no TPT runtime crates and no backends.
- Never branch on a specific backend. Go through provider traits.
- Sibling-repo dependencies live only in `tpt-thought-integrations`.
- Libraries are runtime-agnostic: no tokio, native `async fn` in traits.
- Dependencies must be MIT-compatible. Apache-2.0-only crates are rejected by `cargo deny`.

## Reporting issues

This project accepts **issues only, not pull requests**. Please open an issue using one of the
templates (bug, feature request or integration gap). Pull requests are closed without review.

Before reporting a bug, run the same checks CI runs. From the repository root:

```
scripts/check.ps1     # Windows (PowerShell)
scripts/check.sh      # Linux and macOS
```

Both run `cargo fmt --check`, `cargo clippy -D warnings`, `cargo test` and `cargo deny check`.

Source files start with `// SPDX-License-Identifier: MIT OR Apache-2.0`. Unsafe code is forbidden.
