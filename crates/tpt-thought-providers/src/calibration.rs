// SPDX-License-Identifier: MIT OR Apache-2.0
//! Calibration and outcome tracking (spec sections 67, 68, 119). Raw provider confidence is kept
//! separate from what the track record says it means: calibrated values come from binned hit
//! rates, and empirical reliability from the overall hit rate.

use crate::decision::{Answer, Decision};
use crate::types::ProviderId;
use std::collections::BTreeMap;
use tpt_thought_core::Probability;

/// Number of equal-width confidence bins used for calibration.
pub const BINS: usize = 10;

/// Default number of observed outcomes before a track record is trusted.
pub const DEFAULT_MIN_SAMPLES: u64 = 5;

#[derive(Debug, Clone, Copy, Default)]
struct Bin {
    count: u64,
    hits: u64,
}

#[derive(Debug, Clone, Default)]
struct Ledger {
    bins: [Bin; BINS],
    count: u64,
    hits: u64,
    brier_sum: f64,
}

/// Records what happened after providers made claims, and turns that history into calibration
/// and reliability estimates. Values stay `None` until enough outcomes exist.
#[derive(Debug, Clone)]
pub struct CalibrationTracker {
    min_samples: u64,
    ledgers: BTreeMap<ProviderId, Ledger>,
}

impl Default for CalibrationTracker {
    fn default() -> Self {
        Self::new(DEFAULT_MIN_SAMPLES)
    }
}

impl CalibrationTracker {
    /// A tracker that trusts a provider after `min_samples` outcomes.
    pub fn new(min_samples: u64) -> Self {
        Self {
            min_samples: min_samples.max(1),
            ledgers: BTreeMap::new(),
        }
    }

    /// Record that `provider` claimed `predicted` and the claim turned out `hit` or not.
    pub fn record(&mut self, provider: &ProviderId, predicted: Probability, hit: bool) {
        let ledger = self.ledgers.entry(provider.clone()).or_default();
        let index = bin_index(predicted);
        let outcome = f64::from(u8::from(hit));
        ledger.bins[index].count += 1;
        ledger.bins[index].hits += u64::from(hit);
        ledger.count += 1;
        ledger.hits += u64::from(hit);
        ledger.brier_sum += (predicted.get() - outcome).powi(2);
    }

    /// Record the outcome of an answered decision with a provider or human source. Abstentions and
    /// decisions without a raw confidence carry nothing to calibrate, so they are not recorded and
    /// `false` is returned.
    pub fn record_decision<T>(&mut self, decision: &Decision<T>, hit: bool) -> bool {
        let (Answer::Answered(_), Some(raw)) = (&decision.answer, decision.confidence.raw) else {
            return false;
        };
        self.record(&decision.source.provider_id(), raw, hit);
        true
    }

    /// Number of recorded outcomes for `provider`.
    pub fn sample_count(&self, provider: &ProviderId) -> u64 {
        self.ledgers.get(provider).map_or(0, |l| l.count)
    }

    /// Calibrated probability for a raw confidence: the observed hit rate of the raw confidence's
    /// bin. `None` until that bin has enough samples.
    pub fn calibrated(&self, provider: &ProviderId, raw: Probability) -> Option<Probability> {
        let bin = self.ledgers.get(provider)?.bins[bin_index(raw)];
        self.rate(bin.hits, bin.count)
    }

    /// Overall observed hit rate, the provider's empirical reliability. `None` until enough samples.
    pub fn empirical_reliability(&self, provider: &ProviderId) -> Option<Probability> {
        let ledger = self.ledgers.get(provider)?;
        self.rate(ledger.hits, ledger.count)
    }

    /// Mean squared error between claimed probabilities and outcomes (lower is better).
    pub fn brier_score(&self, provider: &ProviderId) -> Option<f64> {
        let ledger = self.ledgers.get(provider)?;
        (ledger.count > 0).then(|| ledger.brier_sum / ledger.count as f64)
    }

    /// Fill in the calibrated and empirical fields of a decision from the track record. Raw
    /// confidence is never overwritten.
    pub fn annotate<T>(&self, decision: &mut Decision<T>) {
        let provider = decision.source.provider_id();
        if let Some(raw) = decision.confidence.raw {
            decision.confidence.calibrated = self.calibrated(&provider, raw);
        }
        decision.confidence.empirical_reliability = self.empirical_reliability(&provider);
    }

    fn rate(&self, hits: u64, count: u64) -> Option<Probability> {
        if count < self.min_samples {
            return None;
        }
        Probability::new(hits as f64 / count as f64).ok()
    }
}

fn bin_index(p: Probability) -> usize {
    ((p.get() * BINS as f64) as usize).min(BINS - 1)
}
