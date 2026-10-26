// SPDX-License-Identifier: MIT OR Apache-2.0
//! Adapter from Thought telemetry to the `tpt-pulse` event stream (spec sections 53, 116; todo.md
//! Phase 10 `PulseAdapter`).
//!
//! Pulse transports events and does not define what they mean. This adapter takes the
//! observatory's [`TelemetryEvent`]s, which already carry Thought's meaning, and publishes each
//! one to a [`tpt_pulse_core::EventStream`]. Every subscriber receives its own copy in publication
//! order. Thought does not depend on a network protocol, and the stream runs in-process.

use tpt_pulse_core::EventStream;
use tpt_thought_observatory::{TelemetryEvent, TelemetrySink};

/// A telemetry sink that publishes each event to a Pulse event stream.
///
/// Clones of the stream share subscribers, so the caller keeps one clone to subscribe with and
/// hands the other to the sink. Publishing never fails the cognitive operation that emitted the
/// event, even when no subscriber is listening.
#[derive(Clone, Default)]
pub struct PulseSink {
    stream: EventStream<TelemetryEvent>,
}

impl PulseSink {
    /// A sink that publishes to `stream`.
    pub fn new(stream: EventStream<TelemetryEvent>) -> Self {
        Self { stream }
    }

    /// The stream this sink publishes to. Subscribe here to receive events.
    pub fn stream(&self) -> &EventStream<TelemetryEvent> {
        &self.stream
    }
}

impl TelemetrySink for PulseSink {
    fn emit(&mut self, event: &TelemetryEvent) {
        self.stream.publish(event.clone());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tpt_thought_core::Timestamp;
    use tpt_thought_observatory::EventKind;

    fn event(kind: EventKind) -> TelemetryEvent {
        TelemetryEvent::new(kind, Timestamp(0))
    }

    #[test]
    fn emit_publishes_to_every_subscriber_in_order() {
        let stream = EventStream::new();
        let first = stream.subscribe();
        let second = stream.subscribe();
        let mut sink = PulseSink::new(stream);

        sink.emit(&event(EventKind::ThoughtCreated));
        sink.emit(&event(EventKind::ThoughtAttended));

        let kinds = |sub: tpt_pulse_core::Subscriber<TelemetryEvent>| {
            sub.map(|e| e.kind).collect::<Vec<_>>()
        };
        drop(sink);
        assert_eq!(
            kinds(first),
            vec![EventKind::ThoughtCreated, EventKind::ThoughtAttended]
        );
        assert_eq!(
            kinds(second),
            vec![EventKind::ThoughtCreated, EventKind::ThoughtAttended]
        );
    }

    #[test]
    fn emit_without_subscribers_does_not_fail() {
        let mut sink = PulseSink::default();
        sink.emit(&event(EventKind::ThoughtCreated));
        assert_eq!(sink.stream().subscriber_count(), 0);
    }

    #[test]
    fn subscriber_sees_events_from_a_sink_clone() {
        let sink = PulseSink::default();
        let subscriber = sink.stream().subscribe();
        let mut emitter = sink.clone();

        emitter.emit(&event(EventKind::ThoughtCreated));

        assert_eq!(subscriber.recv().unwrap().kind, EventKind::ThoughtCreated);
    }
}
