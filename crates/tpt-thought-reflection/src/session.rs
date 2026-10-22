// SPDX-License-Identifier: MIT OR Apache-2.0
use crate::artifact::ReflectionArtifact;
use crate::mode::CognitiveMode;
use crate::trigger::ReflectionTrigger;
use serde::{Deserialize, Serialize};
use tpt_thought_core::Timestamp;

/// Explicit bounds on one reflection session. These are the circuit breakers that stop
/// reflection from running away (spec section 48.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReflectionLimits {
    /// Most thoughts that may be reflected on in one session.
    pub max_steps: u32,
    /// Most artifacts that may be recorded in one session.
    pub max_artifacts: u32,
    /// Wall-clock budget for the whole session, in nanoseconds.
    pub max_duration_nanos: i64,
}

impl Default for ReflectionLimits {
    fn default() -> Self {
        Self {
            max_steps: 16,
            max_artifacts: 64,
            max_duration_nanos: 5_000_000_000,
        }
    }
}

/// Which limit ended a session early.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Halt {
    /// `max_steps` was reached.
    StepLimit,
    /// `max_artifacts` was reached.
    ArtifactLimit,
    /// `max_duration_nanos` was exceeded.
    DurationLimit,
}

/// How a session ended, or whether it is still going.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionStatus {
    /// Still accepting steps and artifacts.
    Running,
    /// Finished normally.
    Completed,
    /// Terminated by a circuit breaker.
    Halted(Halt),
}

/// Why a session refused a call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionError {
    /// The session already completed normally.
    Finished,
    /// A circuit breaker has already halted the session.
    Halted(Halt),
}

impl std::fmt::Display for SessionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Finished => write!(f, "reflection session already completed"),
            Self::Halted(h) => write!(f, "reflection session halted: {h:?}"),
        }
    }
}

impl std::error::Error for SessionError {}

/// One bounded reflection run, started by a [`ReflectionTrigger`].
///
/// Every call is checked against the limits, so a session always terminates: either it
/// completes, or a circuit breaker halts it.
#[derive(Debug, Clone, PartialEq)]
pub struct ReflectionSession {
    trigger: ReflectionTrigger,
    mode: CognitiveMode,
    limits: ReflectionLimits,
    started_at: Timestamp,
    steps: u32,
    artifacts: Vec<ReflectionArtifact>,
    status: SessionStatus,
}

impl ReflectionSession {
    /// Start a session in [`CognitiveMode::Reflection`].
    pub fn new(trigger: ReflectionTrigger, limits: ReflectionLimits, now: Timestamp) -> Self {
        Self {
            trigger,
            mode: CognitiveMode::Reflection,
            limits,
            started_at: now,
            steps: 0,
            artifacts: Vec::new(),
            status: SessionStatus::Running,
        }
    }

    /// Why this session was started.
    pub fn trigger(&self) -> &ReflectionTrigger {
        &self.trigger
    }

    /// The mode the session runs in.
    pub fn mode(&self) -> CognitiveMode {
        self.mode
    }

    /// The limits in force.
    pub fn limits(&self) -> ReflectionLimits {
        self.limits
    }

    /// Current status.
    pub fn status(&self) -> SessionStatus {
        self.status
    }

    /// Steps taken so far.
    pub fn steps(&self) -> u32 {
        self.steps
    }

    /// Artifacts recorded so far, in order.
    pub fn artifacts(&self) -> &[ReflectionArtifact] {
        &self.artifacts
    }

    /// Start the next step (one thought under reflection). Returns the zero-based step index.
    pub fn begin_step(&mut self, now: Timestamp) -> Result<u32, SessionError> {
        self.check_running(now)?;
        if self.steps >= self.limits.max_steps {
            return Err(self.halt(Halt::StepLimit));
        }
        let index = self.steps;
        self.steps += 1;
        Ok(index)
    }

    /// Record an artifact produced by the current step.
    pub fn record(
        &mut self,
        artifact: ReflectionArtifact,
        now: Timestamp,
    ) -> Result<(), SessionError> {
        self.check_running(now)?;
        if self.artifacts.len() >= self.limits.max_artifacts as usize {
            return Err(self.halt(Halt::ArtifactLimit));
        }
        self.artifacts.push(artifact);
        Ok(())
    }

    /// Mark the session finished normally.
    pub fn complete(&mut self) -> Result<(), SessionError> {
        match self.status {
            SessionStatus::Running => {
                self.status = SessionStatus::Completed;
                Ok(())
            }
            SessionStatus::Completed => Err(SessionError::Finished),
            SessionStatus::Halted(h) => Err(SessionError::Halted(h)),
        }
    }

    /// Split into trigger, final status and artifacts.
    pub(crate) fn into_parts(self) -> (ReflectionTrigger, SessionStatus, Vec<ReflectionArtifact>) {
        (self.trigger, self.status, self.artifacts)
    }

    fn check_running(&mut self, now: Timestamp) -> Result<(), SessionError> {
        match self.status {
            SessionStatus::Running => {}
            SessionStatus::Completed => return Err(SessionError::Finished),
            SessionStatus::Halted(h) => return Err(SessionError::Halted(h)),
        }
        if now.0 - self.started_at.0 > self.limits.max_duration_nanos {
            return Err(self.halt(Halt::DurationLimit));
        }
        Ok(())
    }

    fn halt(&mut self, halt: Halt) -> SessionError {
        self.status = SessionStatus::Halted(halt);
        SessionError::Halted(halt)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tpt_thought_core::{IdGenerator, SeededEntropy};

    fn session(limits: ReflectionLimits) -> ReflectionSession {
        let ids = IdGenerator::new(SeededEntropy::new(3));
        let trigger = ReflectionTrigger::Periodic {
            targets: vec![ids.thought()],
        };
        ReflectionSession::new(trigger, limits, Timestamp(0))
    }

    #[test]
    fn step_limit_halts_the_session() {
        let mut s = session(ReflectionLimits {
            max_steps: 2,
            ..ReflectionLimits::default()
        });
        assert_eq!(s.begin_step(Timestamp(1)), Ok(0));
        assert_eq!(s.begin_step(Timestamp(1)), Ok(1));
        assert_eq!(
            s.begin_step(Timestamp(1)),
            Err(SessionError::Halted(Halt::StepLimit))
        );
        assert_eq!(s.status(), SessionStatus::Halted(Halt::StepLimit));
    }

    #[test]
    fn duration_limit_halts_the_session() {
        let mut s = session(ReflectionLimits {
            max_duration_nanos: 100,
            ..ReflectionLimits::default()
        });
        assert_eq!(
            s.begin_step(Timestamp(101)),
            Err(SessionError::Halted(Halt::DurationLimit))
        );
    }

    #[test]
    fn halted_session_refuses_further_calls() {
        let mut s = session(ReflectionLimits {
            max_steps: 0,
            ..ReflectionLimits::default()
        });
        let _ = s.begin_step(Timestamp(0));
        assert_eq!(s.complete(), Err(SessionError::Halted(Halt::StepLimit)));
    }

    #[test]
    fn completed_session_refuses_steps() {
        let mut s = session(ReflectionLimits::default());
        s.complete().unwrap();
        assert_eq!(s.begin_step(Timestamp(0)), Err(SessionError::Finished));
    }
}
