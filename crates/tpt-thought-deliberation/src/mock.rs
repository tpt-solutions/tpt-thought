// SPDX-License-Identifier: MIT OR Apache-2.0
//! A scripted [`DeliberationEngine`] for tests and for exercising the loop without a real engine.

use crate::engine::{Capability, DeliberationEngine, DeliberationError};
use crate::investigation::{Investigation, InvestigationOutcome};
use crate::metacognition::Metacognition;
use crate::snapshot::DeliberationContext;
use std::collections::VecDeque;

/// Returns queued responses in order. An unscripted call returns [`DeliberationError::Failed`],
/// so a test notices when the loop asks for more steps than the script provides.
///
/// Only `investigate` and `reflect` are scripted. Every other capability stays
/// [`DeliberationError::Unsupported`], which lets tests check partial implementations.
#[derive(Debug, Default)]
pub struct ScriptedEngine {
    /// Responses to `investigate`, consumed front to back.
    pub investigations: VecDeque<Result<InvestigationOutcome, DeliberationError>>,
    /// Responses to `reflect`, consumed front to back.
    pub reflections: VecDeque<Result<Metacognition, DeliberationError>>,
    /// Every `investigate` request received, in order.
    pub requests: Vec<Investigation>,
    /// Every scripted capability invoked, in order.
    pub calls: Vec<Capability>,
}

impl DeliberationEngine for ScriptedEngine {
    fn investigate(
        &mut self,
        _ctx: &DeliberationContext,
        request: &Investigation,
    ) -> Result<InvestigationOutcome, DeliberationError> {
        self.calls.push(Capability::Investigate);
        self.requests.push(request.clone());
        self.investigations
            .pop_front()
            .unwrap_or_else(|| Err(DeliberationError::Failed("script exhausted".into())))
    }

    fn reflect(&mut self, _ctx: &DeliberationContext) -> Result<Metacognition, DeliberationError> {
        self.calls.push(Capability::Reflect);
        self.reflections
            .pop_front()
            .unwrap_or_else(|| Err(DeliberationError::Failed("script exhausted".into())))
    }
}
