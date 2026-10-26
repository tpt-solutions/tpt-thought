# Changelog

All notable changes to this crate are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/). The crate uses semantic versioning.
Repository-wide changes are recorded in the root `CHANGELOG.md`.

## [Unreleased]

### Added

- Hypothesis branches and a bounded `BranchSet`, guarding against branch explosion, with explore,
  suspend, reject, promote, prune, merge and test recording (`Branch`, `BranchSet`, `BranchError`).
- A fixed hypothesis lifecycle table (`HypothesisStatus`, `EDGES`, `IllegalHypothesisTransition`).
- Collapse to a single branch by named strategy: probability, utility, evidence, formal
  verification, ensemble agreement or human choice. Ties are reported, not resolved silently
  (`collapse`, `CollapseStrategy`, `CollapseError`).
- Contradictions between hypotheses, with attention, investigation gating and resolution by new
  evidence, context separation or conditional validity (`Contradiction`, `ContradictionLedger`,
  `handle`).
- Information gain in bits: `shannon_bits`, `expected_information_gain` and `should_investigate`.
- Evidence-gated belief pipeline: `Claim` moves one stage at a time through `ClaimStage`, with
  `PromotionPolicy`, `Criterion`, `EvidenceSummary`, stage history and demotion.
