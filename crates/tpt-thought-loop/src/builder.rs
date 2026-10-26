// SPDX-License-Identifier: MIT OR Apache-2.0
//! Assembling a [`CognitiveLoop`] from parts, with defaults for everything optional.

use crate::action::ActionSink;
use crate::config::LoopConfig;
use crate::cycle::{CognitiveLoop, LoopError};
use crate::null::{NullActionSink, NullEngine, NullEstimator};
use crate::slow::Estimator;
use tpt_thought_deliberation::DeliberationEngine;
use tpt_thought_observatory::TelemetrySink;
use tpt_thought_policy::PolicySet;
use tpt_thought_providers::ProviderRegistry;
use tpt_thought_workspace::MemoryStore;

/// Builds a [`CognitiveLoop`] step by step.
///
/// Every part has a default, so `CognitiveLoop::builder().build()` yields a loop that runs with
/// no providers, no deliberation, no estimates and no executor. Each of those is replaced by
/// calling the matching method, which may change the builder's type parameters.
pub struct LoopBuilder<E, A, S> {
    config: LoopConfig,
    registry: ProviderRegistry,
    engine: E,
    estimator: A,
    sink: S,
    memory: Option<Box<dyn MemoryStore>>,
    telemetry: Option<Box<dyn TelemetrySink>>,
    policy: Option<PolicySet>,
}

impl CognitiveLoop<NullEngine, NullEstimator, NullActionSink> {
    /// A builder with every part at its default. See [`LoopBuilder`].
    pub fn builder() -> LoopBuilder<NullEngine, NullEstimator, NullActionSink> {
        LoopBuilder {
            config: LoopConfig::default(),
            registry: ProviderRegistry::new(),
            engine: NullEngine,
            estimator: NullEstimator,
            sink: NullActionSink,
            memory: None,
            telemetry: None,
            policy: None,
        }
    }
}

impl<E, A, S> LoopBuilder<E, A, S> {
    /// Use `config` in place of the default.
    pub fn config(mut self, config: LoopConfig) -> Self {
        self.config = config;
        self
    }

    /// Use `registry` in place of an empty one.
    pub fn registry(mut self, registry: ProviderRegistry) -> Self {
        self.registry = registry;
        self
    }

    /// Use `store` for memory retrieval. The default store holds nothing.
    pub fn memory(mut self, store: impl MemoryStore + 'static) -> Self {
        self.memory = Some(Box::new(store));
        self
    }

    /// Use `policy` in place of [`crate::default_policy`], which always runs the full slow path.
    pub fn policy(mut self, policy: PolicySet) -> Self {
        self.policy = Some(policy);
        self
    }

    /// Forward telemetry to `sink` as well as the built-in recorder.
    pub fn telemetry(mut self, sink: impl TelemetrySink + 'static) -> Self {
        self.telemetry = Some(Box::new(sink));
        self
    }

    /// Use `engine` for deep investigation in place of [`NullEngine`].
    pub fn engine<E2: DeliberationEngine>(self, engine: E2) -> LoopBuilder<E2, A, S> {
        LoopBuilder {
            config: self.config,
            registry: self.registry,
            engine,
            estimator: self.estimator,
            sink: self.sink,
            memory: self.memory,
            telemetry: self.telemetry,
            policy: self.policy,
        }
    }

    /// Use `estimator` for quantitative estimates in place of [`NullEstimator`].
    pub fn estimator<A2: Estimator>(self, estimator: A2) -> LoopBuilder<E, A2, S> {
        LoopBuilder {
            config: self.config,
            registry: self.registry,
            engine: self.engine,
            estimator,
            sink: self.sink,
            memory: self.memory,
            telemetry: self.telemetry,
            policy: self.policy,
        }
    }

    /// Use `sink` to execute actions in place of [`NullActionSink`].
    pub fn sink<S2: ActionSink>(self, sink: S2) -> LoopBuilder<E, A, S2> {
        LoopBuilder {
            config: self.config,
            registry: self.registry,
            engine: self.engine,
            estimator: self.estimator,
            sink,
            memory: self.memory,
            telemetry: self.telemetry,
            policy: self.policy,
        }
    }
}

impl<E: DeliberationEngine, A: Estimator, S: ActionSink> LoopBuilder<E, A, S> {
    /// Assemble the loop. Fails if the configuration's thresholds are out of range.
    pub fn build(self) -> Result<CognitiveLoop<E, A, S>, LoopError> {
        let mut lp = CognitiveLoop::new(
            self.config,
            self.registry,
            self.engine,
            self.estimator,
            self.sink,
        )?;
        if let Some(store) = self.memory {
            lp = lp.with_boxed_memory(store);
        }
        if let Some(sink) = self.telemetry {
            lp = lp.with_telemetry(sink);
        }
        if let Some(policy) = self.policy {
            lp = lp.with_policy(policy);
        }
        Ok(lp)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_builder_builds_a_loop() {
        let lp = CognitiveLoop::builder().build();
        assert!(lp.is_ok());
    }

    #[test]
    fn builder_rejects_out_of_range_config() {
        let config = LoopConfig {
            accept_at: 2.0,
            ..LoopConfig::default()
        };
        assert!(CognitiveLoop::builder().config(config).build().is_err());
    }
}
