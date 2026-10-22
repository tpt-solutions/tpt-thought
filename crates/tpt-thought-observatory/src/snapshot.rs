// SPDX-License-Identifier: MIT OR Apache-2.0
//! Observatory data model (spec section 99): serialisable views of active thoughts, the thought
//! graph, hypotheses, providers, cost and state. These types are plain data for a future UI.
//! The Observatory UI itself belongs to Anima Observatory and is not implemented here.

use crate::metric::Metrics;
use crate::timeline::SystemMode;
use serde::{Deserialize, Serialize};
use tpt_thought_core::{HypothesisId, Probability, ThoughtId, ThoughtState, Timestamp};

/// One thought as shown in the observatory's "active thoughts" panel.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ActiveThought {
    /// The thought.
    pub id: ThoughtId,
    /// Its lifecycle state.
    pub state: ThoughtState,
    /// Attention score as last computed.
    pub attention: f64,
    /// Its calibrated (or raw, if uncalibrated) confidence, if known.
    pub confidence: Option<Probability>,
    /// Number of hypotheses it currently holds.
    pub hypothesis_count: u32,
    /// The provider currently serving it, if any.
    pub active_provider: Option<String>,
    /// Compute spent on it so far, in provider-defined units.
    pub compute_cost: f64,
}

/// A typed edge in the thought graph snapshot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GraphEdgeView {
    /// The source thought.
    pub from: ThoughtId,
    /// The target thought.
    pub to: ThoughtId,
    /// The relation label, such as `supports`, `contradicts` or `causes`.
    pub relation: String,
}

/// The thought graph as shown in the observatory: branches, evidence, contradictions and outcomes.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct GraphSnapshot {
    /// Thoughts shown as nodes.
    pub nodes: Vec<ThoughtId>,
    /// Typed edges between the nodes.
    pub edges: Vec<GraphEdgeView>,
}

/// A hypothesis as shown in the observatory.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HypothesisView {
    /// The hypothesis.
    pub id: HypothesisId,
    /// Its lifecycle status label, such as `testing` or `supported`.
    pub status: String,
    /// Its current probability, if estimated.
    pub probability: Option<Probability>,
}

/// A provider as shown in the observatory.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderView {
    /// Provider name.
    pub name: String,
    /// Whether it is currently serving requests.
    pub active: bool,
    /// Inferences issued through it.
    pub inferences: u64,
}

/// Aggregate cost across the system, in the same units as the budget.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct CostView {
    /// Compute, in provider-defined units.
    pub compute: f64,
    /// Monetary or abstract cost.
    pub money: f64,
    /// Wall-clock latency in nanoseconds.
    pub latency_nanos: u64,
}

/// A point-in-time view of the whole observatory.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ObservatorySnapshot {
    /// When the snapshot was taken.
    pub at: Timestamp,
    /// The current system mode, if known.
    pub mode: Option<SystemMode>,
    /// Active thoughts.
    pub active_thoughts: Vec<ActiveThought>,
    /// The thought graph.
    pub graph: GraphSnapshot,
    /// Hypotheses under consideration.
    pub hypotheses: Vec<HypothesisView>,
    /// Providers known to the system.
    pub providers: Vec<ProviderView>,
    /// Aggregate cost.
    pub cost: CostView,
    /// Aggregated cognitive metrics.
    pub metrics: Metrics,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tpt_thought_core::{IdGenerator, SeededEntropy};

    #[test]
    fn snapshot_round_trips_through_json() {
        let ids = IdGenerator::new(SeededEntropy::new(7));
        let a = ids.thought();
        let b = ids.thought();
        let snapshot = ObservatorySnapshot {
            at: Timestamp(1_000),
            mode: Some(SystemMode::Reflection),
            active_thoughts: vec![ActiveThought {
                id: a,
                state: ThoughtState::Hypothesising,
                attention: 0.75,
                confidence: Probability::new(0.4).ok(),
                hypothesis_count: 3,
                active_provider: Some("local".into()),
                compute_cost: 2.5,
            }],
            graph: GraphSnapshot {
                nodes: vec![a, b],
                edges: vec![GraphEdgeView {
                    from: a,
                    to: b,
                    relation: "supports".into(),
                }],
            },
            hypotheses: vec![HypothesisView {
                id: ids.hypothesis(),
                status: "testing".into(),
                probability: None,
            }],
            providers: vec![ProviderView {
                name: "local".into(),
                active: true,
                inferences: 4,
            }],
            cost: CostView {
                compute: 2.5,
                money: 0.0,
                latency_nanos: 900,
            },
            metrics: Metrics::new(),
        };
        let json = serde_json::to_string(&snapshot).unwrap();
        let back: ObservatorySnapshot = serde_json::from_str(&json).unwrap();
        assert_eq!(back, snapshot);
    }
}
