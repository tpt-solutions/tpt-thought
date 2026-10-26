---
name: Feature request
about: Propose a capability for Thought
labels: enhancement
---

## Problem

<!-- The cognitive need this addresses. Reference spec.txt section numbers where relevant. -->

## Proposal

<!-- What the API or behaviour should look like. -->

## Boundary check

Thought must stay small (`docs/DEPENDENCY_POLICY.md`, `spec.txt` section 111):

- [ ] It does not add a model, database, LLM client, tensor code or network protocol to core.
- [ ] Any backend-specific behaviour goes through a provider trait, not a branch on a backend.
- [ ] Any sibling-repository dependency would live only in `tpt-thought-integrations`.
