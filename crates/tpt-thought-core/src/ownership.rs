// SPDX-License-Identifier: MIT OR Apache-2.0
//! Thought ownership (spec section 40). Identity and permissions are delegated to Anima ZKPA; this
//! crate only records the state.

use serde::{Deserialize, Serialize};

/// Who may see or act on a thought.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThoughtOwnership {
    /// Visible only to the owning agent.
    #[default]
    Private,
    /// Shared with other agents.
    Shared,
    /// Responsibility handed to another agent.
    Delegated {
        /// Opaque identity of the delegate (resolved by the identity provider).
        to: String,
    },
    /// Published for general consumption.
    Published,
    /// Retained read-only.
    Archived,
}
