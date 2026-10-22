// SPDX-License-Identifier: MIT OR Apache-2.0
//! Thought lifecycle state machine (spec section 7).
//!
//! Cognition is not linear, so the table permits loops (re-hypothesising, re-investigating,
//! reconsidering after an outcome). Transitions are driven by [`ThoughtEvent`]s and are pure
//! functions of `(state, event)`: the same input always yields the same transition.

use std::fmt;
use tpt_thought_core::{Thought, ThoughtState, Timestamp, TransitionKind};

/// An event that moves a thought through its lifecycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ThoughtEvent {
    /// Input arrived.
    Receive,
    /// Attention was allocated.
    Attend,
    /// Context gathering began.
    Contextualise,
    /// Memory association began.
    Associate,
    /// Hypothesis generation began (also: generate new hypotheses after a setback).
    Hypothesise,
    /// Evaluation began (also: reconsider after a changed intention or outcome).
    Evaluate,
    /// Evaluation reached an acceptable conclusion.
    Accept,
    /// Evaluation could not settle on a conclusion.
    MarkUncertain,
    /// Investigation began.
    Investigate,
    /// Re-evaluation after investigation began.
    ReEvaluate,
    /// An intention is being formed.
    Intend,
    /// Execution began.
    Act,
    /// The result is being observed.
    Observe,
    /// Learning from the outcome began.
    Learn,
    /// Consolidation began.
    Consolidate,
    /// The thought is finished.
    Complete,
}

/// The attempted transition is not permitted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IllegalTransition {
    /// State the thought was in.
    pub from: ThoughtState,
    /// Event that was rejected.
    pub event: ThoughtEvent,
}

impl fmt::Display for IllegalTransition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "event {:?} is not permitted in state {:?}",
            self.event, self.from
        )
    }
}

impl std::error::Error for IllegalTransition {}

use ThoughtEvent as E;
use ThoughtState as S;

/// The transition table: `(from, event) -> to`. Every row is a permitted edge.
pub const TRANSITIONS: &[(ThoughtState, ThoughtEvent, ThoughtState)] = &[
    (S::Created, E::Receive, S::Received),
    (S::Received, E::Attend, S::Attending),
    (S::Attending, E::Contextualise, S::Contextualising),
    (S::Contextualising, E::Associate, S::Associating),
    (S::Associating, E::Hypothesise, S::Hypothesising),
    (S::Hypothesising, E::Evaluate, S::Evaluating),
    (S::Evaluating, E::Accept, S::Accepted),
    (S::Evaluating, E::MarkUncertain, S::Uncertain),
    (S::Evaluating, E::Hypothesise, S::Hypothesising),
    (S::Uncertain, E::Investigate, S::Investigating),
    (S::Uncertain, E::Hypothesise, S::Hypothesising),
    (S::Investigating, E::ReEvaluate, S::ReEvaluating),
    (S::ReEvaluating, E::Accept, S::Accepted),
    (S::ReEvaluating, E::MarkUncertain, S::Uncertain),
    (S::ReEvaluating, E::Investigate, S::Investigating),
    (S::Accepted, E::Intend, S::Intending),
    (S::Intending, E::Act, S::Acting),
    (S::Intending, E::Evaluate, S::Evaluating),
    (S::Acting, E::Observe, S::Observing),
    (S::Observing, E::Learn, S::Learning),
    (S::Observing, E::Evaluate, S::Evaluating),
    (S::Learning, E::Consolidate, S::Consolidating),
    (S::Consolidating, E::Complete, S::Complete),
];

/// The state reached by applying `event` in `from`, or an error if not permitted.
pub fn next(from: ThoughtState, event: ThoughtEvent) -> Result<ThoughtState, IllegalTransition> {
    TRANSITIONS
        .iter()
        .find(|(f, e, _)| *f == from && *e == event)
        .map(|(_, _, to)| *to)
        .ok_or(IllegalTransition { from, event })
}

/// True if a direct transition `from -> to` exists.
pub fn can_transition(from: ThoughtState, to: ThoughtState) -> bool {
    TRANSITIONS.iter().any(|(f, _, t)| *f == from && *t == to)
}

/// Events permitted in `from`.
pub fn permitted_events(from: ThoughtState) -> impl Iterator<Item = ThoughtEvent> {
    TRANSITIONS
        .iter()
        .filter(move |(f, _, _)| *f == from)
        .map(|(_, e, _)| *e)
}

/// Whether applying `event` is reproducible. The lifecycle machine itself is always
/// deterministic; probabilistic behaviour belongs to the providers that choose the event.
pub const fn kind(_event: ThoughtEvent) -> TransitionKind {
    TransitionKind::Deterministic
}

/// Apply `event` to `thought`, updating its state and `updated_at`. On error the thought is
/// left unchanged.
pub fn apply(
    thought: &mut Thought,
    event: ThoughtEvent,
    now: Timestamp,
) -> Result<ThoughtState, IllegalTransition> {
    let to = next(thought.state, event)?;
    thought.state = to;
    thought.temporal.updated_at = Some(now);
    Ok(to)
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use tpt_thought_core::{
        IdGenerator, SeededEntropy, TextContent, ThoughtContent, ThoughtOrigin,
    };

    const ALL_EVENTS: [ThoughtEvent; 16] = [
        E::Receive,
        E::Attend,
        E::Contextualise,
        E::Associate,
        E::Hypothesise,
        E::Evaluate,
        E::Accept,
        E::MarkUncertain,
        E::Investigate,
        E::ReEvaluate,
        E::Intend,
        E::Act,
        E::Observe,
        E::Learn,
        E::Consolidate,
        E::Complete,
    ];

    fn thought() -> Thought {
        let ids = IdGenerator::new(SeededEntropy::new(1));
        Thought::new(
            ids.thought(),
            ThoughtOrigin::Internal,
            ThoughtContent::Text(TextContent {
                text: "t".into(),
                language: None,
            }),
            Timestamp(0),
        )
    }

    #[test]
    fn happy_path_reaches_complete() {
        let mut t = thought();
        for e in [
            E::Receive,
            E::Attend,
            E::Contextualise,
            E::Associate,
            E::Hypothesise,
            E::Evaluate,
            E::Accept,
            E::Intend,
            E::Act,
            E::Observe,
            E::Learn,
            E::Consolidate,
            E::Complete,
        ] {
            apply(&mut t, e, Timestamp(1)).unwrap();
        }
        assert!(t.state.is_terminal());
    }

    #[test]
    fn uncertain_loop_is_permitted() {
        assert_eq!(next(S::Evaluating, E::MarkUncertain), Ok(S::Uncertain));
        assert_eq!(next(S::Uncertain, E::Investigate), Ok(S::Investigating));
        assert_eq!(next(S::Investigating, E::ReEvaluate), Ok(S::ReEvaluating));
        assert_eq!(next(S::ReEvaluating, E::Investigate), Ok(S::Investigating));
    }

    #[test]
    fn illegal_transition_rejected_and_state_unchanged() {
        let mut t = thought();
        let err = apply(&mut t, E::Act, Timestamp(5)).unwrap_err();
        assert_eq!(
            err,
            IllegalTransition {
                from: S::Created,
                event: E::Act
            }
        );
        assert_eq!(t.state, S::Created);
        assert_eq!(t.temporal.updated_at, Some(Timestamp(0)));
    }

    #[test]
    fn complete_is_terminal() {
        assert_eq!(permitted_events(S::Complete).count(), 0);
    }

    #[test]
    fn table_is_deterministic() {
        // No (state, event) pair appears twice with different targets.
        for (i, (f1, e1, _)) in TRANSITIONS.iter().enumerate() {
            for (f2, e2, _) in &TRANSITIONS[i + 1..] {
                assert!(!(f1 == f2 && e1 == e2), "duplicate row for {f1:?}/{e1:?}");
            }
        }
    }

    #[test]
    fn every_state_reachable_from_created() {
        let mut seen = vec![S::Created];
        let mut i = 0;
        while i < seen.len() {
            for e in permitted_events(seen[i]).collect::<Vec<_>>() {
                let to = next(seen[i], e).unwrap();
                if !seen.contains(&to) {
                    seen.push(to);
                }
            }
            i += 1;
        }
        for s in S::ALL {
            assert!(seen.contains(&s), "{s:?} unreachable");
        }
    }

    proptest! {
        /// No sequence of events can produce an invalid state transition.
        #[test]
        fn no_invalid_transitions(events in proptest::collection::vec(0usize..16, 0..200)) {
            let mut t = thought();
            for idx in events {
                let before = t.state;
                let ev = ALL_EVENTS[idx];
                match apply(&mut t, ev, Timestamp(1)) {
                    Ok(after) => {
                        prop_assert!(can_transition(before, after));
                        prop_assert_eq!(t.state, after);
                    }
                    Err(_) => prop_assert_eq!(t.state, before),
                }
            }
        }

        /// Same input, same transition.
        #[test]
        fn transitions_are_deterministic(s in 0usize..17, e in 0usize..16) {
            prop_assert_eq!(next(S::ALL[s], ALL_EVENTS[e]), next(S::ALL[s], ALL_EVENTS[e]));
        }
    }
}

/// The thought lifecycle API (spec section 121): `create` is `Thought::new` /
/// `Thought::from_observation`; the rest are provided here. Each step applies exactly one
/// lifecycle event, so calling out of order returns [`IllegalTransition`].
pub trait ThoughtLifecycle {
    /// Input received (`Created -> Received`).
    fn receive(&mut self, now: Timestamp) -> Result<ThoughtState, IllegalTransition>;
    /// Begin attending (`Received -> Attending`).
    fn attend(&mut self, now: Timestamp) -> Result<ThoughtState, IllegalTransition>;
    /// Begin gathering context (`Attending -> Contextualising`).
    fn contextualise(&mut self, now: Timestamp) -> Result<ThoughtState, IllegalTransition>;
    /// Associate memories (`Contextualising -> Associating`). Memories are added only if the
    /// transition succeeds.
    fn associate(
        &mut self,
        memories: impl IntoIterator<Item = tpt_thought_core::MemoryRef>,
        now: Timestamp,
    ) -> Result<ThoughtState, IllegalTransition>;
    /// Finish (`Consolidating -> Complete`).
    fn complete(&mut self, now: Timestamp) -> Result<ThoughtState, IllegalTransition>;
}

impl ThoughtLifecycle for Thought {
    fn receive(&mut self, now: Timestamp) -> Result<ThoughtState, IllegalTransition> {
        apply(self, ThoughtEvent::Receive, now)
    }

    fn attend(&mut self, now: Timestamp) -> Result<ThoughtState, IllegalTransition> {
        apply(self, ThoughtEvent::Attend, now)
    }

    fn contextualise(&mut self, now: Timestamp) -> Result<ThoughtState, IllegalTransition> {
        apply(self, ThoughtEvent::Contextualise, now)
    }

    fn associate(
        &mut self,
        memories: impl IntoIterator<Item = tpt_thought_core::MemoryRef>,
        now: Timestamp,
    ) -> Result<ThoughtState, IllegalTransition> {
        let state = apply(self, ThoughtEvent::Associate, now)?;
        self.memories.extend(memories);
        Ok(state)
    }

    fn complete(&mut self, now: Timestamp) -> Result<ThoughtState, IllegalTransition> {
        apply(self, ThoughtEvent::Complete, now)
    }
}

#[cfg(test)]
mod lifecycle_tests {
    use super::*;
    use tpt_thought_core::*;

    fn obs(ids: &IdGenerator) -> Observation {
        Observation {
            id: ids.observation(),
            content: Box::new(ThoughtContent::Structured(StructuredValue::Null)),
            source: "s".into(),
            trust: SourceTrust::Trusted,
            observed_at: Timestamp(5),
        }
    }

    #[test]
    fn from_observation_through_association() {
        let ids = IdGenerator::new(SeededEntropy::new(1));
        let mut t = Thought::from_observation(ids.thought(), obs(&ids), Timestamp(6));
        assert_eq!(t.state, ThoughtState::Created);
        assert_eq!(t.temporal.observed_at, Some(Timestamp(5)));
        t.receive(Timestamp(7)).unwrap();
        t.attend(Timestamp(8)).unwrap();
        t.contextualise(Timestamp(9)).unwrap();
        let m = ContentReference {
            store: "anima-sse".into(),
            id: "M1".into(),
        };
        t.associate([m.clone()], Timestamp(10)).unwrap();
        assert_eq!(t.state, ThoughtState::Associating);
        assert_eq!(t.memories, vec![m]);
        assert_eq!(t.temporal.updated_at, Some(Timestamp(10)));
    }

    #[test]
    fn out_of_order_calls_fail_without_side_effects() {
        let ids = IdGenerator::new(SeededEntropy::new(1));
        let mut t = Thought::from_observation(ids.thought(), obs(&ids), Timestamp(6));
        let m = ContentReference {
            store: "s".into(),
            id: "x".into(),
        };
        assert!(t.associate([m], Timestamp(7)).is_err());
        assert!(t.memories.is_empty());
        assert!(t.complete(Timestamp(7)).is_err());
        assert_eq!(t.state, ThoughtState::Created);
    }
}
