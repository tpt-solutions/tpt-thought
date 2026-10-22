// SPDX-License-Identifier: MIT OR Apache-2.0
//! Fan-out of prediction errors to the subsystems that learn from them (spec section 18).
//!
//! The hooks are a trait, so this crate depends on none of memory, learning, belief, attention
//! or planning. Each hook defaults to doing nothing; implementors override what they handle.

use crate::error::PredictionError;
use crate::ladder::{ResponseAction, ResponsePlan};

/// Subsystems that react to a prediction error.
pub trait ErrorFanOut {
    /// Reinforce or weaken the memories that supported the prediction.
    fn reinforce_memory(&mut self, _error: &PredictionError) {}
    /// Queue the error as a candidate for active learning.
    fn request_active_learning(&mut self, _error: &PredictionError) {}
    /// Revise the belief the prediction rested on.
    fn update_belief(&mut self, _error: &PredictionError) {}
    /// Adjust confidence in the model or provider that made the prediction.
    fn adjust_confidence(&mut self, _error: &PredictionError) {}
    /// Raise attention on the input that surprised the system.
    fn raise_attention(&mut self, _error: &PredictionError) {}
    /// Revise plans that depend on the prediction.
    fn update_planning(&mut self, _error: &PredictionError) {}
}

/// Deliver `error` to `sink`. Memory and confidence always learn from an error. The response
/// plan then decides the rest: `UpdateBelief` revises beliefs, `RaiseAttention` and
/// `Investigate` raise attention and queue active learning, and `SuspendModel` and
/// `ReevaluateAssumptions` revise planning (and beliefs, for reevaluation).
pub fn dispatch(error: &PredictionError, plan: &ResponsePlan, sink: &mut impl ErrorFanOut) {
    sink.reinforce_memory(error);
    sink.adjust_confidence(error);
    for action in &plan.actions {
        match action {
            ResponseAction::Continue => {}
            ResponseAction::UpdateBelief => sink.update_belief(error),
            ResponseAction::RaiseAttention => sink.raise_attention(error),
            ResponseAction::Investigate => sink.request_active_learning(error),
            ResponseAction::SuspendModel => sink.update_planning(error),
            ResponseAction::ReevaluateAssumptions => {
                sink.update_belief(error);
                sink.update_planning(error);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ladder::{ErrorLevel, ResponseLadder};
    use tpt_thought_core::{IdGenerator, SeededEntropy, Timestamp};

    #[derive(Default)]
    struct Recorder {
        calls: Vec<&'static str>,
    }

    impl ErrorFanOut for Recorder {
        fn reinforce_memory(&mut self, _: &PredictionError) {
            self.calls.push("memory");
        }
        fn request_active_learning(&mut self, _: &PredictionError) {
            self.calls.push("learning");
        }
        fn update_belief(&mut self, _: &PredictionError) {
            self.calls.push("belief");
        }
        fn adjust_confidence(&mut self, _: &PredictionError) {
            self.calls.push("confidence");
        }
        fn raise_attention(&mut self, _: &PredictionError) {
            self.calls.push("attention");
        }
        fn update_planning(&mut self, _: &PredictionError) {
            self.calls.push("planning");
        }
    }

    fn error(magnitude: f64) -> PredictionError {
        let ids = IdGenerator::new(SeededEntropy::new(5));
        PredictionError {
            prediction: ids.prediction(),
            observation: ids.observation(),
            source_thought: ids.thought(),
            content_error: magnitude,
            timing_error: 0.0,
            timing_offset_nanos: None,
            observed_at: Timestamp(0),
        }
    }

    fn run(magnitude: f64) -> (ErrorLevel, Vec<&'static str>) {
        let e = error(magnitude);
        let plan = ResponseLadder::default().plan(&e);
        let mut r = Recorder::default();
        dispatch(&e, &plan, &mut r);
        (plan.level, r.calls)
    }

    #[test]
    fn small_errors_only_reinforce_and_adjust_confidence() {
        let (level, calls) = run(0.01);
        assert_eq!(level, ErrorLevel::Small);
        assert_eq!(calls, vec!["memory", "confidence"]);
    }

    #[test]
    fn moderate_errors_update_belief() {
        let (_, calls) = run(0.2);
        assert_eq!(calls, vec!["memory", "confidence", "belief"]);
    }

    #[test]
    fn large_errors_raise_attention_and_learn() {
        let (_, calls) = run(0.5);
        assert_eq!(calls, vec!["memory", "confidence", "attention", "learning"]);
    }

    #[test]
    fn catastrophic_errors_revise_planning_and_assumptions() {
        let (_, calls) = run(0.9);
        assert_eq!(
            calls,
            vec!["memory", "confidence", "planning", "belief", "planning"]
        );
    }

    #[test]
    fn default_hooks_are_no_ops() {
        struct Nothing;
        impl ErrorFanOut for Nothing {}
        let e = error(1.0);
        let plan = ResponseLadder::default().plan(&e);
        dispatch(&e, &plan, &mut Nothing);
    }
}
