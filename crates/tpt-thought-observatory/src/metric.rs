// SPDX-License-Identifier: MIT OR Apache-2.0
//! Cognitive metrics and deterministic aggregation (spec section 97).

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// A cognitive metric. Rate metrics are recorded as `1.0` for an occurrence and `0.0` for a
/// non-occurrence, so their mean is the rate. Every other metric is recorded as its raw value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum Metric {
    /// Nanoseconds from thought creation to completion (or to the latest observation).
    ThoughtLatency,
    /// Depth of a thought in the spawn tree.
    ThoughtDepth,
    /// Number of branches created.
    BranchCount,
    /// Number of provider inferences issued.
    InferenceCount,
    /// Rate of escalations per decision point (occurrence = 1.0).
    EscalationRate,
    /// Rate of abstentions per decision point (occurrence = 1.0).
    AbstentionRate,
    /// Rate of correct predictions (occurrence = 1.0).
    PredictionAccuracy,
    /// Magnitude of prediction error.
    PredictionError,
    /// Rate of belief revisions per evaluation (occurrence = 1.0).
    BeliefRevisionRate,
    /// Rate of memory retrievals that returned relevant memory (occurrence = 1.0).
    MemoryRetrievalRate,
    /// Confidence stated with a decision, in `0.0..=1.0`.
    DecisionConfidence,
    /// Absolute gap between stated confidence and observed outcome frequency.
    CalibrationError,
    /// Compute, money or energy spent, in provider-defined units.
    CognitiveCost,
    /// Expected or realised uncertainty reduction.
    InformationGain,
    /// Rate of actions that succeeded (occurrence = 1.0).
    ActionSuccessRate,
}

impl Metric {
    /// Every metric, in declaration order.
    pub const ALL: [Metric; 15] = [
        Self::ThoughtLatency,
        Self::ThoughtDepth,
        Self::BranchCount,
        Self::InferenceCount,
        Self::EscalationRate,
        Self::AbstentionRate,
        Self::PredictionAccuracy,
        Self::PredictionError,
        Self::BeliefRevisionRate,
        Self::MemoryRetrievalRate,
        Self::DecisionConfidence,
        Self::CalibrationError,
        Self::CognitiveCost,
        Self::InformationGain,
        Self::ActionSuccessRate,
    ];

    /// True if this metric is recorded as a 0/1 occurrence and read back as a rate.
    pub const fn is_rate(self) -> bool {
        matches!(
            self,
            Self::EscalationRate
                | Self::AbstentionRate
                | Self::PredictionAccuracy
                | Self::BeliefRevisionRate
                | Self::MemoryRetrievalRate
                | Self::ActionSuccessRate
        )
    }
}

/// Running summary of one metric.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Summary {
    /// Number of samples.
    pub count: u64,
    /// Sum of samples.
    pub sum: f64,
    /// Smallest sample.
    pub min: f64,
    /// Largest sample.
    pub max: f64,
}

impl Summary {
    fn new(value: f64) -> Self {
        Self {
            count: 1,
            sum: value,
            min: value,
            max: value,
        }
    }

    fn add(&mut self, value: f64) {
        self.count += 1;
        self.sum += value;
        self.min = self.min.min(value);
        self.max = self.max.max(value);
    }

    /// The arithmetic mean, which is the rate for rate metrics.
    pub fn mean(&self) -> f64 {
        self.sum / self.count as f64
    }
}

/// Deterministic metric aggregation. Each metric keeps a running summary, and the map is
/// ordered by [`Metric`], so the same sequence of `record` calls always yields the same output.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Metrics {
    summaries: BTreeMap<Metric, Summary>,
}

impl Metrics {
    /// An empty set of metrics.
    pub fn new() -> Self {
        Self::default()
    }

    /// Record one sample. Non-finite values are rejected and return `false`.
    pub fn record(&mut self, metric: Metric, value: f64) -> bool {
        if !value.is_finite() {
            return false;
        }
        self.summaries
            .entry(metric)
            .and_modify(|s| s.add(value))
            .or_insert_with(|| Summary::new(value));
        true
    }

    /// Record an occurrence (`true` = 1.0) for a rate metric.
    pub fn record_occurrence(&mut self, metric: Metric, occurred: bool) {
        self.record(metric, if occurred { 1.0 } else { 0.0 });
    }

    /// The summary of `metric`, if any samples were recorded.
    pub fn summary(&self, metric: Metric) -> Option<&Summary> {
        self.summaries.get(&metric)
    }

    /// The mean of `metric`, if any samples were recorded.
    pub fn mean(&self, metric: Metric) -> Option<f64> {
        self.summary(metric).map(Summary::mean)
    }

    /// The rate of `metric`. Identical to [`Metrics::mean`]; named for rate metrics.
    pub fn rate(&self, metric: Metric) -> Option<f64> {
        self.mean(metric)
    }

    /// Iterate over recorded metrics in their stable order.
    pub fn iter(&self) -> impl Iterator<Item = (Metric, &Summary)> {
        self.summaries.iter().map(|(m, s)| (*m, s))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mean_and_bounds_are_aggregated() {
        let mut m = Metrics::new();
        for v in [2.0, 4.0, 9.0] {
            assert!(m.record(Metric::CognitiveCost, v));
        }
        let s = m.summary(Metric::CognitiveCost).unwrap();
        assert_eq!(s.count, 3);
        assert_eq!(s.sum, 15.0);
        assert_eq!(s.min, 2.0);
        assert_eq!(s.max, 9.0);
        assert_eq!(m.mean(Metric::CognitiveCost), Some(5.0));
    }

    #[test]
    fn rates_are_occurrence_means() {
        let mut m = Metrics::new();
        for escalated in [true, false, false, true] {
            m.record_occurrence(Metric::EscalationRate, escalated);
        }
        assert_eq!(m.rate(Metric::EscalationRate), Some(0.5));
        assert!(Metric::EscalationRate.is_rate());
        assert!(!Metric::CognitiveCost.is_rate());
    }

    #[test]
    fn non_finite_samples_are_rejected() {
        let mut m = Metrics::new();
        assert!(!m.record(Metric::PredictionError, f64::NAN));
        assert!(!m.record(Metric::PredictionError, f64::INFINITY));
        assert_eq!(m.summary(Metric::PredictionError), None);
    }

    #[test]
    fn iteration_order_is_stable() {
        let mut m = Metrics::new();
        m.record(Metric::ActionSuccessRate, 1.0);
        m.record(Metric::ThoughtLatency, 1.0);
        let order: Vec<Metric> = m.iter().map(|(k, _)| k).collect();
        assert_eq!(order, [Metric::ThoughtLatency, Metric::ActionSuccessRate]);
        assert_eq!(Metric::ALL.len(), 15);
    }
}
