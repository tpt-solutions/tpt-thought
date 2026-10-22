// SPDX-License-Identifier: MIT OR Apache-2.0
//! The [`Observatory`] collects telemetry, metrics and the system timeline in one place. It
//! forwards every event to a [`TelemetrySink`] and keeps per-kind counts.

use crate::event::{EventKind, TelemetryEvent};
use crate::metric::Metrics;
use crate::sink::TelemetrySink;
use crate::snapshot::{
    ActiveThought, CostView, GraphSnapshot, HypothesisView, ObservatorySnapshot, ProviderView,
};
use crate::timeline::CycleTimeline;
use std::collections::BTreeMap;
use tpt_thought_core::Timestamp;

/// Telemetry, metrics and timeline for one running system.
#[derive(Debug)]
pub struct Observatory<S: TelemetrySink> {
    sink: S,
    metrics: Metrics,
    timeline: CycleTimeline,
    counts: BTreeMap<EventKind, u64>,
}

impl<S: TelemetrySink> Observatory<S> {
    /// An observatory that forwards events to `sink`.
    pub fn new(sink: S) -> Self {
        Self {
            sink,
            metrics: Metrics::new(),
            timeline: CycleTimeline::new(),
            counts: BTreeMap::new(),
        }
    }

    /// Forward `event` to the sink and count it. Emitting never changes cognitive state.
    pub fn emit(&mut self, event: &TelemetryEvent) {
        *self.counts.entry(event.kind).or_insert(0) += 1;
        self.sink.emit(event);
    }

    /// How many events of `kind` have been emitted.
    pub fn count(&self, kind: EventKind) -> u64 {
        self.counts.get(&kind).copied().unwrap_or(0)
    }

    /// The sink, for inspection.
    pub fn sink(&self) -> &S {
        &self.sink
    }

    /// The sink, mutably.
    pub fn sink_mut(&mut self) -> &mut S {
        &mut self.sink
    }

    /// The aggregated metrics.
    pub fn metrics(&self) -> &Metrics {
        &self.metrics
    }

    /// The aggregated metrics, mutably, for recording samples.
    pub fn metrics_mut(&mut self) -> &mut Metrics {
        &mut self.metrics
    }

    /// The sleep and reflection timeline.
    pub fn timeline(&self) -> &CycleTimeline {
        &self.timeline
    }

    /// The sleep and reflection timeline, mutably.
    pub fn timeline_mut(&mut self) -> &mut CycleTimeline {
        &mut self.timeline
    }

    /// Assemble a point-in-time snapshot from the state the caller holds. The observatory does not
    /// reach into thoughts itself, so the snapshot reflects exactly what the caller supplies.
    pub fn snapshot(
        &self,
        at: Timestamp,
        active_thoughts: Vec<ActiveThought>,
        graph: GraphSnapshot,
        hypotheses: Vec<HypothesisView>,
        providers: Vec<ProviderView>,
        cost: CostView,
    ) -> ObservatorySnapshot {
        ObservatorySnapshot {
            at,
            mode: self.timeline.current(),
            active_thoughts,
            graph,
            hypotheses,
            providers,
            cost,
            metrics: self.metrics.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sink::RecordingSink;
    use crate::timeline::SystemMode;

    #[test]
    fn emit_forwards_and_counts() {
        let mut obs = Observatory::new(RecordingSink::new());
        obs.emit(&TelemetryEvent::new(
            EventKind::ThoughtCreated,
            Timestamp(1),
        ));
        obs.emit(&TelemetryEvent::new(
            EventKind::ThoughtCreated,
            Timestamp(2),
        ));
        obs.emit(&TelemetryEvent::new(
            EventKind::ThoughtAborted,
            Timestamp(3),
        ));
        assert_eq!(obs.count(EventKind::ThoughtCreated), 2);
        assert_eq!(obs.count(EventKind::ThoughtEscalated), 0);
        let kinds: Vec<EventKind> = obs.sink().events().iter().map(|e| e.kind).collect();
        assert_eq!(
            kinds,
            [
                EventKind::ThoughtCreated,
                EventKind::ThoughtCreated,
                EventKind::ThoughtAborted
            ]
        );
    }

    #[test]
    fn snapshot_reflects_timeline_and_metrics() {
        let mut obs = Observatory::new(crate::sink::NoopSink);
        obs.timeline_mut()
            .enter(Timestamp(0), SystemMode::Sleep, Some("idle".into()));
        obs.metrics_mut()
            .record_occurrence(crate::metric::Metric::AbstentionRate, true);
        let snap = obs.snapshot(
            Timestamp(9),
            vec![],
            GraphSnapshot::default(),
            vec![],
            vec![],
            CostView::default(),
        );
        assert_eq!(snap.mode, Some(SystemMode::Sleep));
        assert_eq!(
            snap.metrics.rate(crate::metric::Metric::AbstentionRate),
            Some(1.0)
        );
    }
}
