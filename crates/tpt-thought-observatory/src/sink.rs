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

/// A sink that keeps every event in arrival order, for tests and in-process inspection.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct RecordingSink {
    events: Vec<TelemetryEvent>,
}

impl RecordingSink {
    /// An empty recording sink.
    pub fn new() -> Self {
        Self::default()
    }

    /// The recorded events, oldest first.
    pub fn events(&self) -> &[TelemetryEvent] {
        &self.events
    }

    /// Take the recorded events, leaving the sink empty.
    pub fn take(&mut self) -> Vec<TelemetryEvent> {
        std::mem::take(&mut self.events)
    }
}

impl TelemetrySink for RecordingSink {
    fn emit(&mut self, event: &TelemetryEvent) {
        self.events.push(event.clone());
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
}
