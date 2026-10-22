// SPDX-License-Identifier: MIT OR Apache-2.0
use crate::artifact::ReflectionArtifact;
use crate::review::CognitiveReview;
use crate::session::{ReflectionLimits, ReflectionSession};
use crate::trigger::{ReflectionTrigger, TriggerSignal};
use std::fmt;
use tpt_thought_core::{Clock, Thought, ThoughtId, Timestamp};

/// Thresholds and schedule that decide when reflection starts, and the limits it runs under.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ReflectionPolicy {
    /// Uncertainty at or above this value fires an [`ReflectionTrigger::Uncertainty`].
    pub uncertainty_threshold: f64,
    /// Prediction error at or above this value fires an [`ReflectionTrigger::OutcomeMismatch`].
    pub mismatch_threshold: f64,
    /// Reflect at least this often, in nanoseconds, when there are recent thoughts.
    pub periodic_interval_nanos: i64,
    /// Bounds applied to every session.
    pub limits: ReflectionLimits,
}

impl Default for ReflectionPolicy {
    fn default() -> Self {
        Self {
            uncertainty_threshold: 0.7,
            mismatch_threshold: 0.5,
            periodic_interval_nanos: 60_000_000_000,
            limits: ReflectionLimits::default(),
        }
    }
}

/// Decides when to reflect and runs bounded reflection sessions.
///
/// Thought decides *when*, *why* and *what*; the reasoning comes from the [`ReflectionReasoner`].
#[derive(Debug, Clone, Copy, Default)]
pub struct ReflectionEngine {
    policy: ReflectionPolicy,
}

/// The supplier of reflective reasoning (Eve in production, a stub in tests).
pub trait ReflectionReasoner {
    /// Produce artifacts for one thought under reflection. Must return structured artifacts,
    /// never a model's private chain-of-thought.
    fn reflect(
        &mut self,
        step: &ReflectionStep<'_>,
    ) -> Result<Vec<ReflectionArtifact>, ReasonerError>;
}

/// The thought being reflected on, and why.
#[derive(Debug, Clone, Copy)]
pub struct ReflectionStep<'a> {
    /// Why the session started.
    pub trigger: &'a ReflectionTrigger,
    /// The thought under reflection.
    pub thought: &'a Thought,
    /// Zero-based index of this step within the session.
    pub index: u32,
}

/// The reasoner failed. The message is a short diagnostic, not reasoning text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReasonerError {
    message: String,
}

impl ReasonerError {
    /// A reasoner failure with a short diagnostic.
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    /// The diagnostic.
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for ReasonerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for ReasonerError {}

impl ReflectionEngine {
    /// An engine with the given policy.
    pub fn new(policy: ReflectionPolicy) -> Self {
        Self { policy }
    }

    /// The policy in force.
    pub fn policy(&self) -> &ReflectionPolicy {
        &self.policy
    }

    /// Decide whether to reflect now, and on what.
    ///
    /// Priority when several conditions hold: error, outcome mismatch, contradiction, goal
    /// change, uncertainty, then the periodic schedule. Returns `None` when nothing fires.
    pub fn decide(
        &self,
        signals: &[TriggerSignal],
        recent: &[ThoughtId],
        last_reflection: Option<Timestamp>,
        now: Timestamp,
    ) -> Option<ReflectionTrigger> {
        let p = &self.policy;

        let errors = collect(signals, |s| match s {
            TriggerSignal::Error { thought } => Some(vec![*thought]),
            _ => None,
        });
        if !errors.is_empty() {
            return Some(ReflectionTrigger::Error { targets: errors });
        }

        let mismatches: Vec<(ThoughtId, f64)> = signals
            .iter()
            .filter_map(|s| match s {
                TriggerSignal::OutcomeMismatch { thought, error }
                    if *error >= p.mismatch_threshold =>
                {
                    Some((*thought, *error))
                }
                _ => None,
            })
            .collect();
        if !mismatches.is_empty() {
            let error = max_of(mismatches.iter().map(|m| m.1));
            let targets = dedup(mismatches.iter().map(|m| m.0).collect());
            return Some(ReflectionTrigger::OutcomeMismatch { targets, error });
        }

        let contradictions = collect(signals, |s| match s {
            TriggerSignal::Contradiction { a, b } => Some(vec![*a, *b]),
            _ => None,
        });
        if !contradictions.is_empty() {
            return Some(ReflectionTrigger::Contradiction {
                targets: contradictions,
            });
        }

        let goals = collect(signals, |s| match s {
            TriggerSignal::GoalChange { thought } => Some(vec![*thought]),
            _ => None,
        });
        if !goals.is_empty() {
            return Some(ReflectionTrigger::GoalChange { targets: goals });
        }

        let uncertain: Vec<(ThoughtId, f64)> = signals
            .iter()
            .filter_map(|s| match s {
                TriggerSignal::Uncertainty {
                    thought,
                    uncertainty,
                } if *uncertainty >= p.uncertainty_threshold => Some((*thought, *uncertainty)),
                _ => None,
            })
            .collect();
        if !uncertain.is_empty() {
            let uncertainty = max_of(uncertain.iter().map(|u| u.1));
            let targets = dedup(uncertain.iter().map(|u| u.0).collect());
            return Some(ReflectionTrigger::Uncertainty {
                targets,
                uncertainty,
            });
        }

        let last = last_reflection.unwrap_or(Timestamp::EPOCH);
        if !recent.is_empty() && now.0 - last.0 >= p.periodic_interval_nanos {
            return Some(ReflectionTrigger::Periodic {
                targets: dedup(recent.to_vec()),
            });
        }

        None
    }

    /// Run a bounded reflection session over `trigger`'s targets, drawing thoughts from
    /// `thoughts`. Targets that are not present are skipped. The session always terminates,
    /// and the returned review says whether a circuit breaker or a reasoner failure ended it.
    pub fn run<R: ReflectionReasoner>(
        &self,
        trigger: ReflectionTrigger,
        thoughts: &[Thought],
        reasoner: &mut R,
        clock: &impl Clock,
    ) -> CognitiveReview {
        let targets = trigger.targets().to_vec();
        let mut session = ReflectionSession::new(trigger, self.policy.limits, clock.now());
        let mut reviewed = Vec::new();
        let mut failure = None;

        'targets: for id in targets {
            let Some(thought) = thoughts.iter().find(|t| t.id == id) else {
                continue;
            };
            let Ok(index) = session.begin_step(clock.now()) else {
                break;
            };
            reviewed.push(id);
            let step = ReflectionStep {
                trigger: session.trigger(),
                thought,
                index,
            };
            match reasoner.reflect(&step) {
                Ok(artifacts) => {
                    for artifact in artifacts {
                        if session.record(artifact, clock.now()).is_err() {
                            break 'targets;
                        }
                    }
                }
                Err(e) => {
                    failure = Some(e.message().to_owned());
                    break;
                }
            }
        }

        if session.status() == crate::session::SessionStatus::Running {
            let _ = session.complete();
        }
        CognitiveReview::from_session(session, reviewed, failure)
    }
}

fn collect(
    signals: &[TriggerSignal],
    pick: impl Fn(&TriggerSignal) -> Option<Vec<ThoughtId>>,
) -> Vec<ThoughtId> {
    dedup(signals.iter().filter_map(pick).flatten().collect())
}

fn dedup(ids: Vec<ThoughtId>) -> Vec<ThoughtId> {
    let mut out = Vec::with_capacity(ids.len());
    for id in ids {
        if !out.contains(&id) {
            out.push(id);
        }
    }
    out
}

fn max_of(values: impl Iterator<Item = f64>) -> f64 {
    values.fold(f64::MIN, f64::max)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::artifact::{KnowledgeGap, Label};
    use crate::review::Recommendation;
    use crate::session::{Halt, SessionStatus};
    use tpt_thought_core::{
        IdGenerator, ManualClock, SeededEntropy, StructuredValue, ThoughtContent, ThoughtOrigin,
    };

    struct Fixture {
        ids: IdGenerator,
    }

    impl Fixture {
        fn new() -> Self {
            Self {
                ids: IdGenerator::new(SeededEntropy::new(11)),
            }
        }

        fn thought(&self) -> Thought {
            Thought::new(
                self.ids.thought(),
                ThoughtOrigin::Internal,
                ThoughtContent::Structured(StructuredValue::Null),
                Timestamp(0),
            )
        }
    }

    fn engine() -> ReflectionEngine {
        ReflectionEngine::default()
    }

    /// Returns one knowledge gap per step, or fails when asked.
    struct GapReasoner {
        gap_per_step: usize,
        fail: bool,
    }

    impl ReflectionReasoner for GapReasoner {
        fn reflect(
            &mut self,
            step: &ReflectionStep<'_>,
        ) -> Result<Vec<ReflectionArtifact>, ReasonerError> {
            if self.fail {
                return Err(ReasonerError::new("reasoner unavailable"));
            }
            Ok((0..self.gap_per_step)
                .map(|_| {
                    ReflectionArtifact::KnowledgeGap(KnowledgeGap {
                        thought: step.thought.id,
                        name: Label::new("thermal baseline").unwrap(),
                        missing: vec![],
                    })
                })
                .collect())
        }
    }

    #[test]
    fn error_fires_first() {
        let f = Fixture::new();
        let (a, b) = (f.ids.thought(), f.ids.thought());
        let signals = [
            TriggerSignal::Uncertainty {
                thought: a,
                uncertainty: 0.99,
            },
            TriggerSignal::Error { thought: b },
        ];
        let got = engine().decide(&signals, &[a], None, Timestamp(1));
        assert_eq!(got, Some(ReflectionTrigger::Error { targets: vec![b] }));
    }

    #[test]
    fn uncertainty_fires_only_above_threshold() {
        let f = Fixture::new();
        let t = f.ids.thought();
        let low = [TriggerSignal::Uncertainty {
            thought: t,
            uncertainty: 0.2,
        }];
        assert_eq!(engine().decide(&low, &[], None, Timestamp(1)), None);

        let high = [TriggerSignal::Uncertainty {
            thought: t,
            uncertainty: 0.9,
        }];
        assert_eq!(
            engine().decide(&high, &[], None, Timestamp(1)),
            Some(ReflectionTrigger::Uncertainty {
                targets: vec![t],
                uncertainty: 0.9
            })
        );
    }

    #[test]
    fn contradiction_targets_both_sides() {
        let f = Fixture::new();
        let (a, b) = (f.ids.thought(), f.ids.thought());
        let got = engine().decide(
            &[TriggerSignal::Contradiction { a, b }],
            &[],
            None,
            Timestamp(1),
        );
        assert_eq!(
            got,
            Some(ReflectionTrigger::Contradiction {
                targets: vec![a, b]
            })
        );
    }

    #[test]
    fn goal_change_fires() {
        let f = Fixture::new();
        let t = f.ids.thought();
        let got = engine().decide(
            &[TriggerSignal::GoalChange { thought: t }],
            &[],
            None,
            Timestamp(1),
        );
        assert_eq!(
            got,
            Some(ReflectionTrigger::GoalChange { targets: vec![t] })
        );
    }

    #[test]
    fn outcome_mismatch_fires_above_threshold() {
        let f = Fixture::new();
        let t = f.ids.thought();
        let got = engine().decide(
            &[TriggerSignal::OutcomeMismatch {
                thought: t,
                error: 0.8,
            }],
            &[],
            None,
            Timestamp(1),
        );
        assert_eq!(
            got,
            Some(ReflectionTrigger::OutcomeMismatch {
                targets: vec![t],
                error: 0.8
            })
        );
        let small = engine().decide(
            &[TriggerSignal::OutcomeMismatch {
                thought: t,
                error: 0.1,
            }],
            &[],
            None,
            Timestamp(1),
        );
        assert_eq!(small, None);
    }

    #[test]
    fn periodic_fires_when_due_and_only_with_recent_thoughts() {
        let f = Fixture::new();
        let t = f.ids.thought();
        let interval = ReflectionPolicy::default().periodic_interval_nanos;
        let e = engine();
        assert_eq!(
            e.decide(&[], &[t], Some(Timestamp(0)), Timestamp(interval - 1)),
            None
        );
        assert_eq!(
            e.decide(&[], &[t], Some(Timestamp(0)), Timestamp(interval)),
            Some(ReflectionTrigger::Periodic { targets: vec![t] })
        );
        assert_eq!(
            e.decide(&[], &[], Some(Timestamp(0)), Timestamp(interval * 2)),
            None
        );
    }

    #[test]
    fn review_has_structured_components() {
        let f = Fixture::new();
        let t = f.thought();
        let trigger = ReflectionTrigger::Error {
            targets: vec![t.id],
        };
        let mut reasoner = GapReasoner {
            gap_per_step: 2,
            fail: false,
        };
        let clock = ManualClock::new(Timestamp(0));
        let review = engine().run(trigger, std::slice::from_ref(&t), &mut reasoner, &clock);

        assert_eq!(review.status, SessionStatus::Completed);
        assert_eq!(review.reviewed, vec![t.id]);
        assert_eq!(review.failure, None);
        assert_eq!(review.artifacts.len(), 2);
        assert_eq!(review.gaps.gaps.len(), 2);
        assert_eq!(
            review.metacognition.recommendation,
            Recommendation::AskForHelp
        );
    }

    #[test]
    fn reflection_terminates_at_artifact_limit() {
        let f = Fixture::new();
        let thoughts: Vec<Thought> = (0..3).map(|_| f.thought()).collect();
        let targets = thoughts.iter().map(|t| t.id).collect();
        let eng = ReflectionEngine::new(ReflectionPolicy {
            limits: ReflectionLimits {
                max_artifacts: 4,
                ..ReflectionLimits::default()
            },
            ..ReflectionPolicy::default()
        });
        let mut reasoner = GapReasoner {
            gap_per_step: 10,
            fail: false,
        };
        let clock = ManualClock::new(Timestamp(0));
        let review = eng.run(
            ReflectionTrigger::Periodic { targets },
            &thoughts,
            &mut reasoner,
            &clock,
        );
        assert_eq!(review.artifacts.len(), 4);
        assert_eq!(review.status, SessionStatus::Halted(Halt::ArtifactLimit));
    }

    #[test]
    fn reflection_terminates_at_step_limit() {
        let f = Fixture::new();
        let thoughts: Vec<Thought> = (0..5).map(|_| f.thought()).collect();
        let targets = thoughts.iter().map(|t| t.id).collect();
        let eng = ReflectionEngine::new(ReflectionPolicy {
            limits: ReflectionLimits {
                max_steps: 2,
                ..ReflectionLimits::default()
            },
            ..ReflectionPolicy::default()
        });
        let mut reasoner = GapReasoner {
            gap_per_step: 1,
            fail: false,
        };
        let clock = ManualClock::new(Timestamp(0));
        let review = eng.run(
            ReflectionTrigger::Periodic { targets },
            &thoughts,
            &mut reasoner,
            &clock,
        );
        assert_eq!(review.reviewed.len(), 2);
        assert_eq!(review.status, SessionStatus::Halted(Halt::StepLimit));
    }

    /// Advances the clock on every step, so the duration breaker is the one that fires.
    struct SlowReasoner<'a> {
        clock: &'a ManualClock,
    }

    impl ReflectionReasoner for SlowReasoner<'_> {
        fn reflect(
            &mut self,
            _step: &ReflectionStep<'_>,
        ) -> Result<Vec<ReflectionArtifact>, ReasonerError> {
            self.clock.advance(1_000);
            Ok(vec![])
        }
    }

    #[test]
    fn reflection_terminates_at_duration_limit() {
        let f = Fixture::new();
        let thoughts: Vec<Thought> = (0..50).map(|_| f.thought()).collect();
        let targets = thoughts.iter().map(|t| t.id).collect();
        let eng = ReflectionEngine::new(ReflectionPolicy {
            limits: ReflectionLimits {
                max_duration_nanos: 2_500,
                ..ReflectionLimits::default()
            },
            ..ReflectionPolicy::default()
        });
        let clock = ManualClock::new(Timestamp(0));
        let mut reasoner = SlowReasoner { clock: &clock };
        let review = eng.run(
            ReflectionTrigger::Periodic { targets },
            &thoughts,
            &mut reasoner,
            &clock,
        );
        assert_eq!(review.status, SessionStatus::Halted(Halt::DurationLimit));
        assert!(review.reviewed.len() < 50);
    }

    #[test]
    fn reasoner_failure_is_reported_not_hidden() {
        let f = Fixture::new();
        let t = f.thought();
        let mut reasoner = GapReasoner {
            gap_per_step: 0,
            fail: true,
        };
        let clock = ManualClock::new(Timestamp(0));
        let review = engine().run(
            ReflectionTrigger::Error {
                targets: vec![t.id],
            },
            std::slice::from_ref(&t),
            &mut reasoner,
            &clock,
        );
        assert_eq!(review.failure.as_deref(), Some("reasoner unavailable"));
        assert!(review.artifacts.is_empty());
    }

    #[test]
    fn unknown_targets_are_skipped() {
        let f = Fixture::new();
        let missing = f.ids.thought();
        let mut reasoner = GapReasoner {
            gap_per_step: 1,
            fail: false,
        };
        let clock = ManualClock::new(Timestamp(0));
        let review = engine().run(
            ReflectionTrigger::Error {
                targets: vec![missing],
            },
            &[],
            &mut reasoner,
            &clock,
        );
        assert!(review.reviewed.is_empty());
        assert_eq!(review.status, SessionStatus::Completed);
    }
}
