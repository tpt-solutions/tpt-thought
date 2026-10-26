// SPDX-License-Identifier: MIT OR Apache-2.0
//! Telemetry sink abstraction (spec section 98). No backend is required: sinks are a shape that
//! a tracing, Pulse or file exporter can implement in its own crate.

use crate::event::TelemetryEvent;

/// Receives telemetry events. The method shape follows `tracing` event emission (borrowed event,
/// no return value) so an adapter is a few lines, but this trait has no `tracing` dependency.
pub trait TelemetrySink {
    /// Record one event. Must not fail the cognitive operation that emitted it.
    fn emit(&mut self, event: &TelemetryEvent);
}

impl<S: TelemetrySink + ?Sized> TelemetrySink for &mut S {
    fn emit(&mut self, event: &TelemetryEvent) {
        (**self).emit(event);
    }
}

impl<S: TelemetrySink + ?Sized> TelemetrySink for Box<S> {
    fn emit(&mut self, event: &TelemetryEvent) {
        (**self).emit(event);
    }
}

/// A sink that discards every event.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NoopSink;

impl TelemetrySink for NoopSink {
    fn emit(&mut self, _event: &TelemetryEvent) {}
}

/// Default number of events a [`RecordingSink`] keeps before discarding the oldest.
pub const DEFAULT_RECORDING_CAPACITY: usize = 10_000;

/// A sink that keeps recent events in arrival order, for tests and in-process inspection.
///
/// The sink is bounded: once it holds more than its capacity, the oldest events are discarded
/// and counted in [`RecordingSink::dropped`]. Trimming happens in batches once the buffer
/// reaches twice the capacity, so each `emit` stays amortised constant time.
#[derive(Debug, Clone, PartialEq)]
pub struct RecordingSink {
    events: Vec<TelemetryEvent>,
    capacity: usize,
    dropped: u64,
}

impl Default for RecordingSink {
    fn default() -> Self {
        Self::new()
    }
}

impl RecordingSink {
    /// An empty sink that keeps up to [`DEFAULT_RECORDING_CAPACITY`] events.
    pub fn new() -> Self {
        Self::with_capacity(DEFAULT_RECORDING_CAPACITY)
    }

    /// An empty sink that keeps up to `capacity` of the most recent events.
    ///
    /// A capacity of zero keeps nothing but still counts every event as dropped.
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            events: Vec::new(),
            capacity,
            dropped: 0,
        }
    }

    /// The retained events, oldest first.
    pub fn events(&self) -> &[TelemetryEvent] {
        &self.events
    }

    /// Number of events discarded because the sink was full.
    pub fn dropped(&self) -> u64 {
        self.dropped
    }

    /// The maximum number of events this sink retains between trims.
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Take the retained events, leaving the sink empty. The dropped count is kept.
    pub fn take(&mut self) -> Vec<TelemetryEvent> {
        std::mem::take(&mut self.events)
    }

    fn trim(&mut self) {
        let excess = self.events.len().saturating_sub(self.capacity);
        if excess > 0 {
            self.events.drain(..excess);
            self.dropped += excess as u64;
        }
    }
}

impl TelemetrySink for RecordingSink {
    fn emit(&mut self, event: &TelemetryEvent) {
        self.events.push(event.clone());
        if self.events.len() >= self.capacity.saturating_mul(2).max(1) {
            self.trim();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::EventKind;
    use tpt_thought_core::Timestamp;

    #[test]
    fn recording_sink_keeps_order() {
        let mut sink = RecordingSink::new();
        let kinds = [
            EventKind::ThoughtCreated,
            EventKind::ThoughtAttended,
            EventKind::ThoughtAborted,
        ];
        for (i, k) in kinds.into_iter().enumerate() {
            sink.emit(&TelemetryEvent::new(k, Timestamp(i as i64)));
        }
        let got: Vec<EventKind> = sink.events().iter().map(|e| e.kind).collect();
        assert_eq!(got, kinds);
        assert_eq!(sink.take().len(), 3);
        assert!(sink.events().is_empty());
    }

    #[test]
    fn sinks_forward_through_references_and_boxes() {
        fn drive(mut s: impl TelemetrySink) {
            s.emit(&TelemetryEvent::new(
                EventKind::ThoughtCreated,
                Timestamp(0),
            ));
        }
        let mut rec = RecordingSink::new();
        drive(&mut rec);
        assert_eq!(rec.events().len(), 1);
        drive(Box::new(NoopSink));
    }

    #[test]
    fn recording_sink_is_bounded_and_keeps_newest() {
        let mut sink = RecordingSink::with_capacity(3);
        for i in 0..10 {
            sink.emit(&TelemetryEvent::new(
                EventKind::ThoughtCreated,
                Timestamp(i),
            ));
        }
        assert!(sink.events().len() <= 2 * sink.capacity());
        assert!(sink.events().len() >= sink.capacity());
        assert_eq!(sink.events().last().map(|e| e.at), Some(Timestamp(9)));
        assert_eq!(sink.events().len() as u64 + sink.dropped(), 10);
    }

    #[test]
    fn zero_capacity_keeps_nothing() {
        let mut sink = RecordingSink::with_capacity(0);
        sink.emit(&TelemetryEvent::new(
            EventKind::ThoughtCreated,
            Timestamp(0),
        ));
        assert!(sink.events().is_empty());
        assert_eq!(sink.dropped(), 1);
    }
}
