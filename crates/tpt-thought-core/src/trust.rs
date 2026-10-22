// SPDX-License-Identifier: MIT OR Apache-2.0
//! Source classification of inputs (spec section 41). Anima ZKPA supplies identity and attestation;
//! this module only maps already-established source labels to a trust level.

use crate::content::SourceTrust;
use std::collections::HashMap;

/// Maps source labels to trust levels. Unlisted sources are [`SourceTrust::Untrusted`], so
/// fabricated or unknown sources never gain trust by default.
#[derive(Debug, Clone, Default)]
pub struct SourcePolicy {
    levels: HashMap<String, SourceTrust>,
}

impl SourcePolicy {
    /// A policy that trusts nothing.
    pub fn new() -> Self {
        Self::default()
    }

    /// Assign `trust` to `source`.
    pub fn set(&mut self, source: impl Into<String>, trust: SourceTrust) -> &mut Self {
        self.levels.insert(source.into(), trust);
        self
    }

    /// The trust level of `source`.
    pub fn classify(&self, source: &str) -> SourceTrust {
        self.levels.get(source).copied().unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_sources_are_untrusted() {
        let mut p = SourcePolicy::new();
        p.set("lidar", SourceTrust::Trusted);
        assert_eq!(p.classify("lidar"), SourceTrust::Trusted);
        assert_eq!(p.classify("web-form"), SourceTrust::Untrusted);
    }
}
