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

pub use tpt_thought_core as core;
pub use tpt_thought_provenance as provenance;
pub use tpt_thought_state as state;
