// SPDX-License-Identifier: MIT OR Apache-2.0
//! Deterministic state tests and property tests over the lifecycle transition table (spec 95).

mod common;

use proptest::prelude::*;
use tpt_thought_core::{
    StructuredValue, Thought, ThoughtContent, ThoughtOrigin, ThoughtState, Timestamp,
};
use tpt_thought_state::{apply, can_transition, next, permitted_events, ThoughtEvent};

const ALL_EVENTS: [ThoughtEvent; 16] = [
    ThoughtEvent::Receive,
    ThoughtEvent::Attend,
    ThoughtEvent::Contextualise,
    ThoughtEvent::Associate,
    ThoughtEvent::Hypothesise,
    ThoughtEvent::Evaluate,
    ThoughtEvent::Accept,
    ThoughtEvent::MarkUncertain,
    ThoughtEvent::Investigate,
    ThoughtEvent::ReEvaluate,
    ThoughtEvent::Intend,
    ThoughtEvent::Act,
    ThoughtEvent::Observe,
    ThoughtEvent::Learn,
    ThoughtEvent::Consolidate,
    ThoughtEvent::Complete,
];

fn fresh() -> Thought {
    let ids = common::ids(1);
    Thought::new(
        ids.thought(),
        ThoughtOrigin::Observation,
        ThoughtContent::Structured(StructuredValue::Null),
        Timestamp(0),
    )
}

#[test]
fn happy_path_reaches_complete_in_order() {
    let mut t = fresh();
    let path = [
        ThoughtEvent::Receive,
        ThoughtEvent::Attend,
        ThoughtEvent::Contextualise,
        ThoughtEvent::Associate,
        ThoughtEvent::Hypothesise,
        ThoughtEvent::Evaluate,
        ThoughtEvent::Accept,
    ];
    for (i, event) in path.into_iter().enumerate() {
        apply(&mut t, event, Timestamp(i as i64 + 1)).expect("legal step");
    }
    assert_eq!(t.state, ThoughtState::Accepted);
    assert_eq!(t.temporal.updated_at, Some(Timestamp(path.len() as i64)));
}

#[test]
fn illegal_event_leaves_thought_unchanged() {
    let mut t = fresh();
    let before = t.clone();
    let err = apply(&mut t, ThoughtEvent::Complete, Timestamp(9)).expect_err("illegal");
    assert_eq!(err.from, ThoughtState::Created);
    assert_eq!(err.event, ThoughtEvent::Complete);
    assert_eq!(t, before, "a rejected event must not touch the thought");
}

#[test]
fn identical_event_sequences_produce_identical_states() {
    let script = [
        ThoughtEvent::Receive,
        ThoughtEvent::Attend,
        ThoughtEvent::Attend,
        ThoughtEvent::Contextualise,
        ThoughtEvent::Hypothesise,
    ];
    let run = || {
        let mut t = fresh();
        script
            .iter()
            .map(|e| apply(&mut t, *e, Timestamp(5)).ok())
            .collect::<Vec<_>>()
    };
    assert_eq!(run(), run());
}

#[test]
fn every_permitted_event_succeeds_from_its_state() {
    for state in ALL_STATES {
        for event in permitted_events(state) {
            let to = next(state, event).expect("permitted event must transition");
            assert!(
                can_transition(state, to),
                "{state:?} -> {to:?} must be an edge"
            );
        }
    }
}

const ALL_STATES: [ThoughtState; 17] = [
    ThoughtState::Created,
    ThoughtState::Received,
    ThoughtState::Attending,
    ThoughtState::Contextualising,
    ThoughtState::Associating,
    ThoughtState::Hypothesising,
    ThoughtState::Evaluating,
    ThoughtState::Accepted,
    ThoughtState::Uncertain,
    ThoughtState::Investigating,
    ThoughtState::ReEvaluating,
    ThoughtState::Intending,
    ThoughtState::Acting,
    ThoughtState::Observing,
    ThoughtState::Learning,
    ThoughtState::Consolidating,
    ThoughtState::Complete,
];

proptest! {
    /// For any event sequence, every accepted step is a table edge and every rejected step leaves
    /// the state where it was. Nothing can move the lifecycle off the table.
    #[test]
    fn arbitrary_sequences_never_take_an_invalid_edge(
        indices in proptest::collection::vec(0usize..ALL_EVENTS.len(), 0..64)
    ) {
        let mut t = fresh();
        for (step, i) in indices.iter().enumerate() {
            let event = ALL_EVENTS[*i];
            let before = t.state;
            match apply(&mut t, event, Timestamp(step as i64)) {
                Ok(to) => {
                    prop_assert!(can_transition(before, to));
                    prop_assert_eq!(to, next(before, event).expect("accepted"));
                    prop_assert_eq!(t.state, to);
                }
                Err(err) => {
                    prop_assert_eq!(err.from, before);
                    prop_assert_eq!(t.state, before);
                }
            }
        }
    }

    /// Complete is terminal: once reached, no further event is accepted.
    #[test]
    fn complete_is_terminal(
        tail in proptest::collection::vec(0usize..ALL_EVENTS.len(), 0..16)
    ) {
        prop_assert_eq!(permitted_events(ThoughtState::Complete).count(), 0);
        let mut t = fresh();
        for event in path_to(ThoughtState::Created, ThoughtState::Complete) {
            apply(&mut t, event, Timestamp(0)).expect("legal");
        }
        prop_assert_eq!(t.state, ThoughtState::Complete);
        for i in tail {
            prop_assert!(apply(&mut t, ALL_EVENTS[i], Timestamp(2)).is_err());
            prop_assert_eq!(t.state, ThoughtState::Complete);
        }
    }
}

/// A shortest sequence of events from `from` to `to`, found by breadth-first search over the table.
fn path_to(from: ThoughtState, to: ThoughtState) -> Vec<ThoughtEvent> {
    let mut queue = std::collections::VecDeque::from([(from, Vec::new())]);
    let mut seen = vec![from];
    while let Some((state, path)) = queue.pop_front() {
        if state == to {
            return path;
        }
        for event in permitted_events(state) {
            let next = next(state, event).expect("permitted");
            if !seen.contains(&next) {
                seen.push(next);
                let mut extended = path.clone();
                extended.push(event);
                queue.push_back((next, extended));
            }
        }
    }
    panic!("{to:?} is unreachable from {from:?}");
}
