// SPDX-License-Identifier: MIT OR Apache-2.0
//! TPT Thought facade: re-exports the public API of the Thought crates.
//!
//! ```
//! use tpt_thought::{core::*, state};
//!
//! let ids = IdGenerator::new(SeededEntropy::new(1));
//! let mut t = Thought::new(
//!     ids.thought(),
//!     ThoughtOrigin::Internal,
//!     ThoughtContent::Text(TextContent { text: "hello".into(), language: None }),
//!     Timestamp(0),
//! );
//! state::apply(&mut t, state::ThoughtEvent::Receive, Timestamp(1)).unwrap();
//! assert_eq!(t.state, ThoughtState::Received);
//! ```

pub use tpt_thought_attention as attention;
pub use tpt_thought_consolidation as consolidation;
pub use tpt_thought_core as core;
pub use tpt_thought_deliberation as deliberation;
pub use tpt_thought_graph as graph;
pub use tpt_thought_hypothesis as hypothesis;
pub use tpt_thought_intention as intention;
pub use tpt_thought_loop as cognitive_loop;
pub use tpt_thought_observatory as observatory;
pub use tpt_thought_policy as policy;
pub use tpt_thought_prediction as prediction;
pub use tpt_thought_provenance as provenance;
pub use tpt_thought_providers as providers;
pub use tpt_thought_reflection as reflection;
pub use tpt_thought_safety as safety;
pub use tpt_thought_state as state;
pub use tpt_thought_workspace as workspace;
