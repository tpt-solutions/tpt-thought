// SPDX-License-Identifier: MIT OR Apache-2.0
//! Belief re-evaluation with accumulated evidence, and confidence recalibration (spec
//! sections 43, 44, 67; todo Phase 9).

use crate::{extend_provenance, unit};
use serde::{Deserialize, Serialize};
use tpt_thought_core::ProvenanceId;

/// Number of equal-width confidence bins used for calibration.
pub const CALIBRATION_BINS: usize = 10;
/// Samples a bin needs before its empirical accuracy overrides the raw prediction.
const MIN_BIN_SAMPLES: u32 = 3;

/// Where a belief sits in its lifecycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BeliefStatus {
    /// Not yet supported enough to hold as a belief.
    Candidate,
    /// Held as a belief.
    Believed,
    /// Was believed; evidence has since weakened it.
    Demoted,
}

/// Thresholds for belief promotion and demotion.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BeliefConfig {
    /// Confidence at or above which a candidate is promoted.
    pub promote_at: f64,
    /// Confidence at or below which a believed claim is demoted.
    pub demote_at: f64,
    /// Supporting observations required before promotion.
    pub min_support: u32,
}

impl Default for BeliefConfig {
    fn default() -> Self {
        Self {
            promote_at: 0.8,
            demote_at: 0.3,
            min_support: 3,
        }
    }
}

/// A claim accumulating evidence for and against.
#[derive(Debug, Clone, PartialEq)]
pub struct BeliefRecord {
    /// The claim.
    pub key: String,
    /// Supporting observations.
    pub support: u32,
    /// Contradicting observations.
    pub contradiction: u32,
    /// Confidence from the evidence, `0.0..=1.0`, with Laplace smoothing.
    pub confidence: f64,
    /// Lifecycle status.
    pub status: BeliefStatus,
    /// Provenance of all evidence. Preserved through promotion and demotion.
    pub provenance: Vec<ProvenanceId>,
}

impl BeliefRecord {
    /// A new candidate with no evidence.
    pub fn new(key: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            support: 0,
            contradiction: 0,
            confidence: 0.5,
            status: BeliefStatus::Candidate,
            provenance: Vec::new(),
        }
    }

    /// Record one observation.
    pub fn observe(&mut self, supports: bool, provenance: &[ProvenanceId]) {
        if supports {
            self.support += 1;
        } else {
            self.contradiction += 1;
        }
        extend_provenance(&mut self.provenance, provenance);
    }

    /// Recompute confidence and status. Returns the new status if it changed.
    pub fn reevaluate(&mut self, cfg: &BeliefConfig) -> Option<BeliefStatus> {
        let total = f64::from(self.support + self.contradiction);
        self.confidence = (f64::from(self.support) + 1.0) / (total + 2.0);
        let next = match self.status {
            BeliefStatus::Believed if self.confidence > cfg.demote_at => BeliefStatus::Believed,
            BeliefStatus::Believed => BeliefStatus::Demoted,
            _ if self.confidence >= cfg.promote_at && self.support >= cfg.min_support => {
                BeliefStatus::Believed
            }
            BeliefStatus::Demoted => BeliefStatus::Demoted,
            BeliefStatus::Candidate => BeliefStatus::Candidate,
        };
        if next == self.status {
            None
        } else {
            self.status = next;
            Some(next)
        }
    }
}

/// One calibration observation: a stated confidence and whether the claim proved correct.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CalibrationSample {
    /// Stated confidence, `0.0..=1.0`.
    pub predicted: f64,
    /// Whether the claim was correct.
    pub correct: bool,
}

/// Statistics for one confidence bin.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct BinStat {
    /// Samples in the bin.
    pub count: u32,
    /// Mean stated confidence in the bin.
    pub mean_predicted: f64,
    /// Observed accuracy in the bin.
    pub empirical_accuracy: f64,
}

/// Confidence calibration fitted from samples.
#[derive(Debug, Clone, PartialEq)]
pub struct Calibration {
    bins: Vec<BinStat>,
    expected_error: f64,
    samples: usize,
}

impl Default for Calibration {
    fn default() -> Self {
        Self {
            bins: vec![BinStat::default(); CALIBRATION_BINS],
            expected_error: 0.0,
            samples: 0,
        }
    }
}

impl Calibration {
    /// Fit bins from `samples`, and compute the expected calibration error.
    pub fn fit(samples: &[CalibrationSample]) -> Self {
        let mut sums = [(0u32, 0.0f64, 0u32); CALIBRATION_BINS];
        for s in samples {
            let p = unit(s.predicted);
            let idx = bin_index(p);
            sums[idx].0 += 1;
            sums[idx].1 += p;
            sums[idx].2 += u32::from(s.correct);
        }
        let n = samples.len();
        let bins: Vec<BinStat> = sums
            .iter()
            .map(|&(count, predicted_sum, correct)| {
                if count == 0 {
                    BinStat::default()
                } else {
                    BinStat {
                        count,
                        mean_predicted: predicted_sum / f64::from(count),
                        empirical_accuracy: f64::from(correct) / f64::from(count),
                    }
                }
            })
            .collect();
        let expected_error = if n == 0 {
            0.0
        } else {
            bins.iter()
                .map(|b| {
                    f64::from(b.count) / n as f64 * (b.mean_predicted - b.empirical_accuracy).abs()
                })
                .sum()
        };
        Self {
            bins,
            expected_error,
            samples: n,
        }
    }

    /// Adjust a raw confidence toward the observed accuracy of its bin, when the bin has
    /// enough samples. Otherwise the raw value is returned unchanged.
    pub fn adjust(&self, raw: f64) -> f64 {
        let p = unit(raw);
        let bin = self.bins[bin_index(p)];
        if bin.count >= MIN_BIN_SAMPLES {
            bin.empirical_accuracy
        } else {
            p
        }
    }

    /// The bins, lowest confidence first.
    pub fn bins(&self) -> &[BinStat] {
        &self.bins
    }

    /// Expected calibration error, `0.0` is perfectly calibrated.
    pub fn expected_error(&self) -> f64 {
        self.expected_error
    }

    /// Number of samples the fit used.
    pub fn samples(&self) -> usize {
        self.samples
    }
}

fn bin_index(p: f64) -> usize {
    ((p * CALIBRATION_BINS as f64) as usize).min(CALIBRATION_BINS - 1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tpt_thought_core::{IdGenerator, SeededEntropy};

    #[test]
    fn promotion_requires_support_and_keeps_provenance() {
        let ids = IdGenerator::new(SeededEntropy::new(40));
        let cfg = BeliefConfig::default();
        let mut b = BeliefRecord::new("valve sticks when cold");
        let evidence: Vec<_> = (0..3).map(|_| ids.provenance()).collect();
        for p in &evidence {
            b.observe(true, std::slice::from_ref(p));
        }
        assert_eq!(b.reevaluate(&cfg), Some(BeliefStatus::Believed));
        assert_eq!(b.provenance, evidence);
    }

    #[test]
    fn contradictions_demote_a_belief() {
        let ids = IdGenerator::new(SeededEntropy::new(41));
        let cfg = BeliefConfig::default();
        let mut b = BeliefRecord::new("x");
        for _ in 0..3 {
            b.observe(true, &[ids.provenance()]);
        }
        b.reevaluate(&cfg);
        for _ in 0..10 {
            b.observe(false, &[ids.provenance()]);
        }
        assert_eq!(b.reevaluate(&cfg), Some(BeliefStatus::Demoted));
        assert_eq!(b.provenance.len(), 13);
    }

    #[test]
    fn reevaluate_reports_only_changes() {
        let cfg = BeliefConfig::default();
        let mut b = BeliefRecord::new("x");
        assert_eq!(b.reevaluate(&cfg), None);
    }

    #[test]
    fn calibration_is_perfect_when_predictions_match_outcomes() {
        let samples: Vec<_> = (0..10)
            .map(|i| CalibrationSample {
                predicted: 0.9,
                correct: i < 9,
            })
            .collect();
        let c = Calibration::fit(&samples);
        assert!(c.expected_error() < 1e-9);
        assert_eq!(c.samples(), 10);
    }

    #[test]
    fn overconfident_bin_is_pulled_toward_observed_accuracy() {
        let samples: Vec<_> = (0..10)
            .map(|i| CalibrationSample {
                predicted: 0.95,
                correct: i < 5,
            })
            .collect();
        let c = Calibration::fit(&samples);
        assert!((c.adjust(0.95) - 0.5).abs() < 1e-9);
        // Empty bins leave the raw value alone.
        assert_eq!(c.adjust(0.1), 0.1);
        assert!(c.expected_error() > 0.4);
    }
}
