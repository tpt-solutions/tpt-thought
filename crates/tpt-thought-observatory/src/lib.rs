// SPDX-License-Identifier: MIT OR Apache-2.0
//! Observability for TPT Thought (spec sections 97 to 99).
//!
//! This crate provides:
//! - structured telemetry events with stable names ([`EventKind`], [`TelemetryEvent`]);
//! - cognitive metrics with deterministic aggregation ([`Metric`], [`Metrics`]);
//! - a telemetry sink shape with no required backend ([`TelemetrySink`], [`NoopSink`],
//!   [`RecordingSink`]);
//! - serialisable observatory data ([`ObservatorySnapshot`]) and a sleep and reflection timeline
//!   ([`CycleTimeline`]).
//!
//! The Observatory UI is an Anima Observatory concern and is out of scope for this crate. The
//! data types here are what that UI would read.

pub mod event;
pub mod metric;
pub mod observatory;
pub mod sink;
pub mod snapshot;
pub mod timeline;

pub use event::{EventKind, TelemetryEvent};
pub use metric::{Metric, Metrics, Summary};
pub use observatory::Observatory;
pub use sink::{NoopSink, RecordingSink, TelemetrySink};
pub use snapshot::{
    ActiveThought, CostView, GraphEdgeView, GraphSnapshot, HypothesisView, ObservatorySnapshot,
    ProviderView,
};
pub use timeline::{CycleTimeline, SystemMode, TimelineEntry};
