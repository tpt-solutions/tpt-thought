// SPDX-License-Identifier: MIT OR Apache-2.0
//! Episode replay, prioritised by importance (spec section 48.2, todo Phase 9).

use crate::unit;
use std::cmp::Ordering;
use tpt_thought_core::{EpisodeId, ProvenanceId, Timestamp};

/// A recorded episode worth replaying during consolidation.
#[derive(Debug, Clone, PartialEq)]
pub struct Episode {
    /// Identity.
    pub id: EpisodeId,
    /// The topic the episode concerned; used as the memory and salience key.
    pub topic: String,
    /// When it happened.
    pub at: Timestamp,
    /// Prediction error, `0.0..=1.0`.
    pub prediction_error: f64,
    /// Salience at the time, `0.0..=1.0`.
    pub salience: f64,
    /// An open question the episode left unresolved, if any.
    pub open_question: Option<String>,
    /// Provenance of the episode.
    pub provenance: Vec<ProvenanceId>,
}

impl Episode {
    /// Replay priority: prediction errors matter most, then salience.
    pub fn importance(&self) -> f64 {
        unit(0.6 * unit(self.prediction_error) + 0.4 * unit(self.salience))
    }
}

/// Best-first ordering: higher importance, then more recent, then lower id.
fn priority_order(a: &Episode, b: &Episode) -> Ordering {
    b.importance()
        .total_cmp(&a.importance())
        .then(b.at.cmp(&a.at))
        .then(a.id.cmp(&b.id))
}

/// The replay queue. Episodes come out best first, so important experience is consolidated
/// before routine experience, and an interrupted replay resumes at the most important remainder.
#[derive(Debug, Clone, Default)]
pub struct ThoughtReplay {
    // Stored worst-first so that `pop` yields the best episode in constant time.
    queue: Vec<Episode>,
}

impl ThoughtReplay {
    /// A queue holding `episodes`.
    pub fn new(episodes: Vec<Episode>) -> Self {
        let mut replay = Self::default();
        replay.extend(episodes);
        replay
    }

    /// Add episodes, keeping the queue in priority order.
    pub fn extend(&mut self, episodes: Vec<Episode>) {
        self.queue.extend(episodes);
        self.queue.sort_by(|a, b| priority_order(b, a));
    }

    /// Remove and return the most important episode.
    pub fn pop(&mut self) -> Option<Episode> {
        self.queue.pop()
    }

    /// Episodes still queued, most important first.
    pub fn iter(&self) -> impl Iterator<Item = &Episode> {
        self.queue.iter().rev()
    }

    /// Number of queued episodes.
    pub fn len(&self) -> usize {
        self.queue.len()
    }

    /// True if nothing is queued.
    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tpt_thought_core::{IdGenerator, SeededEntropy};

    fn ep(id: EpisodeId, at: i64, error: f64, salience: f64) -> Episode {
        Episode {
            id,
            topic: "t".into(),
            at: Timestamp(at),
            prediction_error: error,
            salience,
            open_question: None,
            provenance: vec![],
        }
    }

    #[test]
    fn replays_most_important_first() {
        let ids = IdGenerator::new(SeededEntropy::new(7));
        let (a, b, c) = (ids.episode(), ids.episode(), ids.episode());
        let mut replay = ThoughtReplay::new(vec![
            ep(a, 1, 0.1, 0.1),
            ep(b, 2, 0.9, 0.2),
            ep(c, 3, 0.5, 0.9),
        ]);
        let order: Vec<_> = std::iter::from_fn(|| replay.pop()).map(|e| e.id).collect();
        assert_eq!(order, vec![c, b, a]);
    }

    #[test]
    fn ties_break_by_recency_then_id() {
        let ids = IdGenerator::new(SeededEntropy::new(8));
        let (a, b) = (ids.episode(), ids.episode());
        let mut replay = ThoughtReplay::new(vec![ep(a, 1, 0.5, 0.5), ep(b, 2, 0.5, 0.5)]);
        assert_eq!(replay.pop().unwrap().id, b);
    }

    #[test]
    fn non_finite_inputs_do_not_break_ordering() {
        let ids = IdGenerator::new(SeededEntropy::new(9));
        let e = ep(ids.episode(), 1, f64::NAN, 0.5);
        assert_eq!(e.importance(), 0.2);
    }
}
