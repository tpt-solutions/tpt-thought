// SPDX-License-Identifier: MIT OR Apache-2.0
//! Hypotheses (spec sections 15, 16, 29, 37, 43, 44 and 64): branching explanations, their
//! lifecycle, collapse to a single choice, contradictions, information gain, and the pipeline
//! by which a considered claim may eventually become a belief.
//!
//! Considering a hypothesis never makes it a belief. Promotion is sequential and evidence-gated,
//! see [`belief::Claim`].

pub mod belief;
pub mod branch;
pub mod collapse;
pub mod contradiction;
pub mod information;
pub mod lifecycle;

pub use belief::{
    Claim, ClaimStage, Criterion, EvidenceSummary, PolicyError, PromotionError, PromotionPolicy,
    StageChange,
};
pub use branch::{Branch, BranchError, BranchSet};
pub use collapse::{collapse, CollapseError, CollapseStrategy};
pub use contradiction::{
    handle as handle_contradiction, Contradiction, ContradictionAction, ContradictionError,
    ContradictionLedger, Resolution,
};
pub use information::{expected_information_gain, shannon_bits, should_investigate};
pub use lifecycle::{HypothesisStatus, IllegalHypothesisTransition};
