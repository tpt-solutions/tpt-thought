// SPDX-License-Identifier: MIT OR Apache-2.0
//! Resource budgets across all dimensions in spec section 63.
//!
//! `tpt-thought-core` owns [`CognitiveBudget`], which covers only some dimensions. Rather than
//! widen that core type, this module defines the full eight-dimension budget and converts from
//! the core type, so no core API changes are needed.

use serde::{Deserialize, Serialize};
use tpt_thought_core::CognitiveBudget;

/// One resource dimension of a cognitive budget (spec section 63).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Dimension {
    /// Wall-clock time, in nanoseconds.
    Time,
    /// Compute, in provider-defined units.
    Compute,
    /// Energy, in provider-defined units.
    Energy,
    /// Monetary or abstract cost.
    Money,
    /// Memory, in bytes.
    Memory,
    /// Simultaneous branches.
    Branches,
    /// Calls to model providers.
    ModelCalls,
    /// Experiments run.
    Experiments,
}

impl Dimension {
    /// Every dimension, in spec order.
    pub const ALL: [Dimension; 8] = [
        Self::Time,
        Self::Compute,
        Self::Energy,
        Self::Money,
        Self::Memory,
        Self::Branches,
        Self::ModelCalls,
        Self::Experiments,
    ];
}

/// Limits per dimension. `None` means unbounded in that dimension.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct ResourceBudget {
    /// Limit on [`Dimension::Time`], in nanoseconds.
    pub time_nanos: Option<f64>,
    /// Limit on [`Dimension::Compute`].
    pub compute: Option<f64>,
    /// Limit on [`Dimension::Energy`].
    pub energy: Option<f64>,
    /// Limit on [`Dimension::Money`].
    pub money: Option<f64>,
    /// Limit on [`Dimension::Memory`], in bytes.
    pub memory_bytes: Option<f64>,
    /// Limit on [`Dimension::Branches`].
    pub branches: Option<f64>,
    /// Limit on [`Dimension::ModelCalls`].
    pub model_calls: Option<f64>,
    /// Limit on [`Dimension::Experiments`].
    pub experiments: Option<f64>,
}

/// Amounts consumed per dimension. Unlike [`ResourceBudget`], every dimension has a value.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct ResourceUsage {
    /// Nanoseconds spent.
    pub time_nanos: f64,
    /// Compute spent.
    pub compute: f64,
    /// Energy spent.
    pub energy: f64,
    /// Money spent.
    pub money: f64,
    /// Bytes held.
    pub memory_bytes: f64,
    /// Branches open.
    pub branches: f64,
    /// Model calls made.
    pub model_calls: f64,
    /// Experiments run.
    pub experiments: f64,
}

/// The first dimension that would exceed its limit.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BudgetExhausted {
    /// The exhausted dimension.
    pub dimension: Dimension,
    /// The amount that would be used.
    pub used: f64,
    /// The limit.
    pub limit: f64,
}

impl ResourceBudget {
    /// No limits in any dimension.
    pub const UNLIMITED: ResourceBudget = ResourceBudget {
        time_nanos: None,
        compute: None,
        energy: None,
        money: None,
        memory_bytes: None,
        branches: None,
        model_calls: None,
        experiments: None,
    };

    /// The limit for `dimension`, if any.
    pub fn limit(&self, dimension: Dimension) -> Option<f64> {
        match dimension {
            Dimension::Time => self.time_nanos,
            Dimension::Compute => self.compute,
            Dimension::Energy => self.energy,
            Dimension::Money => self.money,
            Dimension::Memory => self.memory_bytes,
            Dimension::Branches => self.branches,
            Dimension::ModelCalls => self.model_calls,
            Dimension::Experiments => self.experiments,
        }
    }
}

impl ResourceUsage {
    /// The amount used in `dimension`.
    pub fn amount(&self, dimension: Dimension) -> f64 {
        match dimension {
            Dimension::Time => self.time_nanos,
            Dimension::Compute => self.compute,
            Dimension::Energy => self.energy,
            Dimension::Money => self.money,
            Dimension::Memory => self.memory_bytes,
            Dimension::Branches => self.branches,
            Dimension::ModelCalls => self.model_calls,
            Dimension::Experiments => self.experiments,
        }
    }

    /// Add `delta` to this usage, dimension by dimension.
    pub fn plus(&self, delta: &ResourceUsage) -> ResourceUsage {
        ResourceUsage {
            time_nanos: self.time_nanos + delta.time_nanos,
            compute: self.compute + delta.compute,
            energy: self.energy + delta.energy,
            money: self.money + delta.money,
            memory_bytes: self.memory_bytes + delta.memory_bytes,
            branches: self.branches + delta.branches,
            model_calls: self.model_calls + delta.model_calls,
            experiments: self.experiments + delta.experiments,
        }
    }

    /// The first dimension (in spec order) where this usage exceeds `budget`, if any.
    pub fn check(&self, budget: &ResourceBudget) -> Result<(), BudgetExhausted> {
        for dimension in Dimension::ALL {
            if let Some(limit) = budget.limit(dimension) {
                let used = self.amount(dimension);
                if used > limit {
                    return Err(BudgetExhausted {
                        dimension,
                        used,
                        limit,
                    });
                }
            }
        }
        Ok(())
    }

    /// Charge `delta` against `budget`. Usage is only updated if the result stays within every
    /// limit, so a refused charge leaves the usage unchanged.
    pub fn charge(
        &mut self,
        delta: &ResourceUsage,
        budget: &ResourceBudget,
    ) -> Result<(), BudgetExhausted> {
        let next = self.plus(delta);
        next.check(budget)?;
        *self = next;
        Ok(())
    }
}

impl From<CognitiveBudget> for ResourceBudget {
    /// Maps the core budget onto the matching dimensions. `max_iterations` has no dimension in
    /// spec section 63 and is not carried over.
    fn from(core: CognitiveBudget) -> Self {
        Self {
            time_nanos: core.max_latency_nanos.map(|n| n as f64),
            compute: core.max_compute,
            money: core.max_cost,
            branches: core.max_branches.map(f64::from),
            model_calls: core.max_external_calls.map(f64::from),
            ..Self::UNLIMITED
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn budget_with_calls(limit: f64) -> ResourceBudget {
        ResourceBudget {
            model_calls: Some(limit),
            ..ResourceBudget::UNLIMITED
        }
    }

    #[test]
    fn unlimited_budget_never_exhausts() {
        let usage = ResourceUsage {
            energy: 1e12,
            ..Default::default()
        };
        assert!(usage.check(&ResourceBudget::UNLIMITED).is_ok());
    }

    #[test]
    fn reports_the_exhausted_dimension() {
        let budget = budget_with_calls(2.0);
        let usage = ResourceUsage {
            model_calls: 3.0,
            ..Default::default()
        };
        assert_eq!(
            usage.check(&budget),
            Err(BudgetExhausted {
                dimension: Dimension::ModelCalls,
                used: 3.0,
                limit: 2.0,
            })
        );
    }

    #[test]
    fn refused_charge_leaves_usage_unchanged() {
        let budget = budget_with_calls(2.0);
        let mut usage = ResourceUsage {
            model_calls: 1.0,
            ..Default::default()
        };
        let delta = ResourceUsage {
            model_calls: 2.0,
            ..Default::default()
        };
        assert!(usage.charge(&delta, &budget).is_err());
        assert_eq!(usage.model_calls, 1.0);
        let small = ResourceUsage {
            model_calls: 1.0,
            ..Default::default()
        };
        assert!(usage.charge(&small, &budget).is_ok());
        assert_eq!(usage.model_calls, 2.0);
    }

    #[test]
    fn core_budget_maps_to_matching_dimensions() {
        let core = CognitiveBudget {
            max_latency_nanos: Some(500),
            max_compute: Some(4.0),
            max_cost: Some(1.5),
            max_branches: Some(3),
            max_iterations: Some(9),
            max_external_calls: Some(7),
        };
        let b = ResourceBudget::from(core);
        assert_eq!(b.time_nanos, Some(500.0));
        assert_eq!(b.compute, Some(4.0));
        assert_eq!(b.money, Some(1.5));
        assert_eq!(b.branches, Some(3.0));
        assert_eq!(b.model_calls, Some(7.0));
        assert_eq!(b.energy, None);
        assert_eq!(b.memory_bytes, None);
        assert_eq!(b.experiments, None);
    }
}
