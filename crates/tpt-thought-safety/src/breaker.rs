// SPDX-License-Identifier: MIT OR Apache-2.0
//! Cognitive circuit breakers (spec section 48.4).
//!
//! Each breaker watches one failure mode. When its threshold is crossed it trips and stays open
//! until a caller resets it, so a runaway process cannot quietly recover on its own.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, VecDeque};
use std::fmt;

/// The failure modes guarded against.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BreakerKind {
    /// The same thought or step keeps recurring.
    Repetition,
    /// Too many thoughts or hypotheses are alive at once.
    Explosion,
    /// Escalation keeps climbing past the allowed level.
    RecursiveEscalation,
    /// Confidence keeps swinging up and down.
    ConfidenceOscillation,
    /// The same contradiction is raised again and again without resolution.
    ContradictionLoop,
    /// Accumulated compute exceeds its budget.
    ResourceExhaustion,
}

/// Why a breaker tripped.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Trip {
    /// A step fingerprint recurred `occurrences` times within the window.
    Repetition {
        /// The recurring fingerprint.
        fingerprint: u64,
        /// How many times it appeared in the window.
        occurrences: usize,
    },
    /// `count` items were alive, above `limit`.
    Explosion {
        /// Observed population.
        count: usize,
        /// Allowed population.
        limit: usize,
    },
    /// Escalation reached `level`, above `limit`.
    RecursiveEscalation {
        /// Observed level.
        level: u32,
        /// Highest allowed level.
        limit: u32,
    },
    /// Confidence changed direction `flips` times within the window.
    ConfidenceOscillation {
        /// Direction changes observed.
        flips: usize,
        /// Direction changes allowed.
        limit: usize,
    },
    /// The contradiction `pair` was raised `occurrences` times without resolution.
    ContradictionLoop {
        /// Identifier of the contradicting pair.
        pair: u64,
        /// Times it was raised.
        occurrences: usize,
    },
    /// Compute reached `used`, above `limit`.
    ResourceExhaustion {
        /// Compute consumed so far.
        used: f64,
        /// Compute allowed.
        limit: f64,
    },
}

impl Trip {
    /// Which breaker produced this trip.
    pub fn kind(&self) -> BreakerKind {
        match self {
            Self::Repetition { .. } => BreakerKind::Repetition,
            Self::Explosion { .. } => BreakerKind::Explosion,
            Self::RecursiveEscalation { .. } => BreakerKind::RecursiveEscalation,
            Self::ConfidenceOscillation { .. } => BreakerKind::ConfidenceOscillation,
            Self::ContradictionLoop { .. } => BreakerKind::ContradictionLoop,
            Self::ResourceExhaustion { .. } => BreakerKind::ResourceExhaustion,
        }
    }
}

impl fmt::Display for Trip {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Repetition {
                fingerprint,
                occurrences,
            } => write!(
                f,
                "repetition: step {fingerprint:016x} recurred {occurrences} times"
            ),
            Self::Explosion { count, limit } => {
                write!(f, "explosion: {count} live items exceeds limit {limit}")
            }
            Self::RecursiveEscalation { level, limit } => {
                write!(
                    f,
                    "recursive escalation: level {level} exceeds limit {limit}"
                )
            }
            Self::ConfidenceOscillation { flips, limit } => {
                write!(
                    f,
                    "confidence oscillation: {flips} direction changes exceeds {limit}"
                )
            }
            Self::ContradictionLoop { pair, occurrences } => write!(
                f,
                "contradiction loop: pair {pair:016x} raised {occurrences} times unresolved"
            ),
            Self::ResourceExhaustion { used, limit } => {
                write!(
                    f,
                    "resource exhaustion: {used} compute exceeds budget {limit}"
                )
            }
        }
    }
}

impl std::error::Error for Trip {}

/// Thresholds for every breaker. The defaults are conservative starting points, to be tuned per
/// deployment and benchmarked (spec section 96).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BreakerConfig {
    /// Number of recent steps kept for repetition detection.
    pub repetition_window: usize,
    /// Occurrences of one fingerprint within the window that trip the breaker.
    pub repetition_limit: usize,
    /// Live items allowed before an explosion trips.
    pub max_population: usize,
    /// Highest escalation level allowed.
    pub max_escalation_level: u32,
    /// Number of recent confidence readings kept.
    pub oscillation_window: usize,
    /// Direction changes within the window that trip the breaker.
    pub oscillation_flips: usize,
    /// Smallest confidence change counted as movement.
    pub oscillation_min_delta: f64,
    /// Unresolved repeats of one contradiction that trip the breaker.
    pub contradiction_limit: usize,
    /// Total compute allowed before exhaustion trips.
    pub max_compute: f64,
}

impl Default for BreakerConfig {
    fn default() -> Self {
        Self {
            repetition_window: 16,
            repetition_limit: 4,
            max_population: 64,
            max_escalation_level: 4,
            oscillation_window: 8,
            oscillation_flips: 4,
            oscillation_min_delta: 0.05,
            contradiction_limit: 3,
            max_compute: 1_000.0,
        }
    }
}

/// The combined guard: one breaker per [`BreakerKind`], with state that can be inspected and reset.
#[derive(Debug, Clone)]
pub struct CircuitBreakers {
    config: BreakerConfig,
    recent_steps: VecDeque<u64>,
    recent_confidence: VecDeque<f64>,
    contradictions: BTreeMap<u64, usize>,
    compute: f64,
    trips: BTreeMap<BreakerKind, Trip>,
}

impl CircuitBreakers {
    /// A closed guard with the given thresholds.
    pub fn new(config: BreakerConfig) -> Self {
        Self {
            config,
            recent_steps: VecDeque::new(),
            recent_confidence: VecDeque::new(),
            contradictions: BTreeMap::new(),
            compute: 0.0,
            trips: BTreeMap::new(),
        }
    }

    /// The thresholds in use.
    pub fn config(&self) -> &BreakerConfig {
        &self.config
    }

    /// Record a step's fingerprint (for example a hash of the thought's content and parent).
    pub fn record_step(&mut self, fingerprint: u64) -> Result<(), Trip> {
        self.recent_steps.push_back(fingerprint);
        while self.recent_steps.len() > self.config.repetition_window {
            self.recent_steps.pop_front();
        }
        let occurrences = self
            .recent_steps
            .iter()
            .filter(|&&s| s == fingerprint)
            .count();
        if occurrences >= self.config.repetition_limit {
            self.open(Trip::Repetition {
                fingerprint,
                occurrences,
            });
        }
        self.check_kind(BreakerKind::Repetition)
    }

    /// Report how many items are currently alive.
    pub fn observe_population(&mut self, count: usize) -> Result<(), Trip> {
        if count > self.config.max_population {
            self.open(Trip::Explosion {
                count,
                limit: self.config.max_population,
            });
        }
        self.check_kind(BreakerKind::Explosion)
    }

    /// Report the current escalation level.
    pub fn observe_escalation(&mut self, level: u32) -> Result<(), Trip> {
        if level > self.config.max_escalation_level {
            self.open(Trip::RecursiveEscalation {
                level,
                limit: self.config.max_escalation_level,
            });
        }
        self.check_kind(BreakerKind::RecursiveEscalation)
    }

    /// Report a confidence reading in `0.0..=1.0`.
    pub fn observe_confidence(&mut self, confidence: f64) -> Result<(), Trip> {
        self.recent_confidence.push_back(confidence);
        while self.recent_confidence.len() > self.config.oscillation_window {
            self.recent_confidence.pop_front();
        }
        let flips = self.direction_changes();
        if flips >= self.config.oscillation_flips {
            self.open(Trip::ConfidenceOscillation {
                flips,
                limit: self.config.oscillation_flips,
            });
        }
        self.check_kind(BreakerKind::ConfidenceOscillation)
    }

    /// Record that the contradiction `pair` was raised again without being resolved.
    pub fn record_contradiction(&mut self, pair: u64) -> Result<(), Trip> {
        let occurrences = {
            let n = self.contradictions.entry(pair).or_insert(0);
            *n += 1;
            *n
        };
        if occurrences >= self.config.contradiction_limit {
            self.open(Trip::ContradictionLoop { pair, occurrences });
        }
        self.check_kind(BreakerKind::ContradictionLoop)
    }

    /// Mark the contradiction `pair` as resolved, clearing its count.
    pub fn resolve_contradiction(&mut self, pair: u64) {
        self.contradictions.remove(&pair);
    }

    /// Charge `cost` units of compute against the budget.
    pub fn charge(&mut self, cost: f64) -> Result<(), Trip> {
        self.compute += cost;
        if self.compute > self.config.max_compute {
            self.open(Trip::ResourceExhaustion {
                used: self.compute,
                limit: self.config.max_compute,
            });
        }
        self.check_kind(BreakerKind::ResourceExhaustion)
    }

    /// The combined check: `Err` with the first open trip, if any breaker is open.
    pub fn check(&self) -> Result<(), Trip> {
        match self.trips.values().next() {
            Some(trip) => Err(trip.clone()),
            None => Ok(()),
        }
    }

    /// True if any breaker is open.
    pub fn is_open(&self) -> bool {
        !self.trips.is_empty()
    }

    /// The open trip for `kind`, if it has tripped.
    pub fn trip(&self, kind: BreakerKind) -> Option<&Trip> {
        self.trips.get(&kind)
    }

    /// Every open trip, in breaker order.
    pub fn trips(&self) -> impl Iterator<Item = &Trip> {
        self.trips.values()
    }

    /// Close one breaker and clear the state it accumulated. Other breakers are untouched.
    pub fn reset(&mut self, kind: BreakerKind) {
        self.trips.remove(&kind);
        match kind {
            BreakerKind::Repetition => self.recent_steps.clear(),
            BreakerKind::Explosion | BreakerKind::RecursiveEscalation => {}
            BreakerKind::ConfidenceOscillation => self.recent_confidence.clear(),
            BreakerKind::ContradictionLoop => self.contradictions.clear(),
            BreakerKind::ResourceExhaustion => self.compute = 0.0,
        }
    }

    /// Close every breaker and clear all accumulated state.
    pub fn reset_all(&mut self) {
        for kind in [
            BreakerKind::Repetition,
            BreakerKind::Explosion,
            BreakerKind::RecursiveEscalation,
            BreakerKind::ConfidenceOscillation,
            BreakerKind::ContradictionLoop,
            BreakerKind::ResourceExhaustion,
        ] {
            self.reset(kind);
        }
    }

    fn open(&mut self, trip: Trip) {
        self.trips.entry(trip.kind()).or_insert(trip);
    }

    fn check_kind(&self, kind: BreakerKind) -> Result<(), Trip> {
        match self.trips.get(&kind) {
            Some(trip) => Err(trip.clone()),
            None => Ok(()),
        }
    }

    /// Count sign changes between successive confidence movements at least
    /// `oscillation_min_delta`.
    fn direction_changes(&self) -> usize {
        let mut flips = 0;
        let mut last_sign = 0i8;
        let readings: Vec<f64> = self.recent_confidence.iter().copied().collect();
        for pair in readings.windows(2) {
            let delta = pair[1] - pair[0];
            if delta.abs() < self.config.oscillation_min_delta {
                continue;
            }
            let sign = if delta > 0.0 { 1 } else { -1 };
            if last_sign != 0 && sign != last_sign {
                flips += 1;
            }
            last_sign = sign;
        }
        flips
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn guard() -> CircuitBreakers {
        CircuitBreakers::new(BreakerConfig::default())
    }

    #[test]
    fn repetition_trips_and_resets() {
        let mut g = guard();
        for _ in 0..3 {
            assert!(g.record_step(7).is_ok());
        }
        let trip = g.record_step(7).unwrap_err();
        assert_eq!(
            trip,
            Trip::Repetition {
                fingerprint: 7,
                occurrences: 4
            }
        );
        assert!(g.is_open());
        assert!(g.check().is_err());
        g.reset(BreakerKind::Repetition);
        assert!(g.check().is_ok());
        assert!(g.record_step(7).is_ok(), "history was cleared on reset");
    }

    #[test]
    fn varied_steps_do_not_trip_repetition() {
        let mut g = guard();
        for step in 0..100 {
            assert!(g.record_step(step).is_ok());
        }
    }

    #[test]
    fn explosion_trips_and_resets() {
        let mut g = guard();
        assert!(g.observe_population(64).is_ok());
        let trip = g.observe_population(65).unwrap_err();
        assert_eq!(trip.kind(), BreakerKind::Explosion);
        g.reset(BreakerKind::Explosion);
        assert!(g.observe_population(10).is_ok());
    }

    #[test]
    fn escalation_trips_and_resets() {
        let mut g = guard();
        assert!(g.observe_escalation(4).is_ok());
        assert!(matches!(
            g.observe_escalation(5),
            Err(Trip::RecursiveEscalation { level: 5, limit: 4 })
        ));
        g.reset(BreakerKind::RecursiveEscalation);
        assert!(g.observe_escalation(1).is_ok());
    }

    #[test]
    fn oscillation_trips_and_resets() {
        let mut g = guard();
        let mut tripped_at = None;
        for (i, c) in [0.2, 0.8, 0.2, 0.8, 0.2, 0.8].into_iter().enumerate() {
            if g.observe_confidence(c).is_err() {
                tripped_at = Some(i);
                break;
            }
        }
        assert_eq!(
            tripped_at,
            Some(5),
            "fourth direction change needs five readings"
        );
        g.reset(BreakerKind::ConfidenceOscillation);
        assert!(g.check().is_ok());
    }

    #[test]
    fn steady_confidence_is_not_oscillation() {
        let mut g = guard();
        for c in [0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9] {
            assert!(g.observe_confidence(c).is_ok());
        }
    }

    #[test]
    fn contradiction_loop_trips_and_resolution_clears() {
        let mut g = guard();
        assert!(g.record_contradiction(9).is_ok());
        assert!(g.record_contradiction(9).is_ok());
        assert!(g.record_contradiction(9).is_err());
        g.reset(BreakerKind::ContradictionLoop);

        assert!(g.record_contradiction(5).is_ok());
        g.resolve_contradiction(5);
        assert!(g.record_contradiction(5).is_ok());
        assert!(g.record_contradiction(5).is_ok());
    }

    #[test]
    fn resource_exhaustion_trips_and_resets() {
        let mut g = guard();
        assert!(g.charge(600.0).is_ok());
        assert!(matches!(
            g.charge(500.0),
            Err(Trip::ResourceExhaustion { .. })
        ));
        g.reset(BreakerKind::ResourceExhaustion);
        assert!(g.charge(600.0).is_ok(), "budget restarts after reset");
    }

    #[test]
    fn breakers_are_independent() {
        let mut g = guard();
        let _ = g.observe_population(1_000);
        assert!(g.trip(BreakerKind::Explosion).is_some());
        assert!(g.trip(BreakerKind::Repetition).is_none());
        g.reset(BreakerKind::Repetition);
        assert!(
            g.is_open(),
            "resetting another kind must not close explosion"
        );
        g.reset_all();
        assert!(!g.is_open());
    }

    #[test]
    fn trip_display_names_the_reason() {
        let t = Trip::Explosion { count: 9, limit: 4 };
        assert_eq!(t.to_string(), "explosion: 9 live items exceeds limit 4");
    }
}
