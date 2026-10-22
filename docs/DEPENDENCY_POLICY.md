# Dependency policy

## Allowed everywhere

- Licences: MIT, MIT/Apache-2.0 dual, BSD-2/3, ISC, Zlib, Unlicense, CC0, Unicode, Blue Oak.
- Not allowed: Apache-2.0-only, copyleft. Enforced by `deny.toml`.

## `tpt-thought-core`

May depend on: `serde`, `serde_json`, `minicbor-serde`, `getrandom` (optional, OS entropy).

Must not depend on: any TPT crate (Anima, Eve, Intellect, Augur, Axiom, Infer, Pulse), any
model/vendor SDK, any async runtime, any database.

## Other `tpt-thought-*` crates

Depend on `tpt-thought-core` and, where needed, sibling Thought crates. Provenance additionally
uses `sha2` for integrity hashes. No async runtime in any library crate.

## `tpt-thought-integrations`

The only crate that may depend on sibling TPT repositories
(`github.com/TPT-Solutions/<repo>`), each behind its own feature flag so the core stays
reusable.

## Banned

Vendor SDKs (OpenAI, Anthropic, Ollama, Hugging Face, ...) and `tokio` are denied in
`deny.toml`. Vendor access belongs in adapters behind provider traits.

## Distribution: git-only

Thought is distributed from git, not crates.io. The sibling dependencies are git dependencies,
and crates.io does not accept those. Every release is tagged in this repository.

## CI git authentication

The sibling repositories (`github.com/TPT-Solutions/*`) are public and fetch anonymously, so CI
needs no credentials today.

If a sibling becomes private, set the `TPT_GIT_TOKEN` repository secret to a fine-grained token
with read-only contents access. The CI step then rewrites GitHub URLs for that organisation to use
it. The step is skipped when the secret is absent.

## Sibling dependencies on hold

Sibling dependencies are not yet in the build. See `INTEGRATION_*.md`. Before any sibling
dependency is added, its licence and its async runtime requirements must be checked against this
policy.
