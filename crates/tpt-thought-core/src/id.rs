// SPDX-License-Identifier: MIT OR Apache-2.0
//! Globally unique, cryptographically strong identifiers (spec section 73).
//!
//! IDs are 128 random bits. Entropy is injectable so tests can be deterministic.

use serde::{de, Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;
use std::sync::Mutex;

/// Error returned when an entropy source cannot supply random bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EntropyError;

impl fmt::Display for EntropyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("entropy source unavailable")
    }
}

impl std::error::Error for EntropyError {}

/// A source of random bytes. Production code should use [`OsEntropy`]; tests may use
/// [`SeededEntropy`] for reproducible IDs.
pub trait EntropySource: Send {
    /// Fill `buf` with random bytes, reporting failure instead of panicking.
    ///
    /// The default implementation calls [`fill`](Self::fill) and never fails. Sources that can
    /// fail (such as the operating system) should override it.
    fn try_fill(&mut self, buf: &mut [u8]) -> Result<(), EntropyError> {
        self.fill(buf);
        Ok(())
    }

    /// Fill `buf` with random bytes.
    ///
    /// # Panics
    ///
    /// Panics if the source cannot supply bytes. Use [`try_fill`](Self::try_fill) to handle
    /// that case.
    fn fill(&mut self, buf: &mut [u8]);
}

/// Cryptographically strong entropy from the operating system.
#[cfg(feature = "os-entropy")]
#[derive(Debug, Default, Clone, Copy)]
pub struct OsEntropy;

#[cfg(feature = "os-entropy")]
impl EntropySource for OsEntropy {
    fn try_fill(&mut self, buf: &mut [u8]) -> Result<(), EntropyError> {
        getrandom::fill(buf).map_err(|_| EntropyError)
    }

    fn fill(&mut self, buf: &mut [u8]) {
        self.try_fill(buf)
            .expect("operating system entropy source unavailable");
    }
}

/// Deterministic, **non-cryptographic** entropy (SplitMix64) for tests and replay.
#[derive(Debug, Clone)]
pub struct SeededEntropy {
    state: u64,
}

impl SeededEntropy {
    /// Create a deterministic source from `seed`.
    pub fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
}

impl EntropySource for SeededEntropy {
    fn fill(&mut self, buf: &mut [u8]) {
        for chunk in buf.chunks_mut(8) {
            let bytes = self.next_u64().to_le_bytes();
            chunk.copy_from_slice(&bytes[..chunk.len()]);
        }
    }
}

/// Error parsing an ID from text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseIdError(&'static str);

impl fmt::Display for ParseIdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid id: {}", self.0)
    }
}

impl std::error::Error for ParseIdError {}

fn to_hex(bytes: &[u8; 16]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn from_hex(s: &str) -> Result<[u8; 16], ParseIdError> {
    if s.len() != 32 || !s.is_ascii() {
        return Err(ParseIdError("expected 32 hex characters"));
    }
    let mut out = [0u8; 16];
    for (i, byte) in out.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&s[i * 2..i * 2 + 2], 16)
            .map_err(|_| ParseIdError("non-hex character"))?;
    }
    Ok(out)
}

macro_rules! define_id {
    ($(#[$doc:meta])* $name:ident, $prefix:literal) => {
        $(#[$doc])*
        #[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub struct $name([u8; 16]);

        impl $name {
            /// Textual prefix identifying the kind of ID (makes IDs traceable in logs).
            pub const PREFIX: &'static str = $prefix;

            /// Construct from raw bytes.
            pub const fn from_bytes(bytes: [u8; 16]) -> Self {
                Self(bytes)
            }

            /// The raw bytes.
            pub const fn as_bytes(&self) -> &[u8; 16] {
                &self.0
            }

            /// Generate using the given entropy source.
            pub fn generate(entropy: &mut dyn EntropySource) -> Self {
                let mut b = [0u8; 16];
                entropy.fill(&mut b);
                Self(b)
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}:{}", $prefix, to_hex(&self.0))
            }
        }

        impl fmt::Debug for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                fmt::Display::fmt(self, f)
            }
        }

        impl std::str::FromStr for $name {
            type Err = ParseIdError;
            fn from_str(s: &str) -> Result<Self, Self::Err> {
                let rest = s
                    .strip_prefix($prefix)
                    .and_then(|r| r.strip_prefix(':'))
                    .ok_or(ParseIdError("wrong or missing prefix"))?;
                from_hex(rest).map(Self)
            }
        }

        impl Serialize for $name {
            fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
                s.collect_str(self)
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                let s = String::deserialize(d)?;
                s.parse().map_err(de::Error::custom)
            }
        }
    };
}

define_id!(
    /// Identifies a [`Thought`](crate::Thought).
    ThoughtId,
    "thought"
);
define_id!(
    /// Identifies an episode.
    EpisodeId,
    "episode"
);
define_id!(
    /// Identifies a hypothesis.
    HypothesisId,
    "hypothesis"
);
define_id!(
    /// Identifies a prediction.
    PredictionId,
    "prediction"
);
define_id!(
    /// Identifies an intention.
    IntentionId,
    "intention"
);
define_id!(
    /// Identifies an outcome.
    OutcomeId,
    "outcome"
);
define_id!(
    /// Identifies an observation.
    ObservationId,
    "observation"
);
define_id!(
    /// Identifies a provenance record.
    ProvenanceId,
    "provenance"
);

/// Thread-safe ID generator wrapping an [`EntropySource`].
pub struct IdGenerator {
    entropy: Mutex<Box<dyn EntropySource>>,
}

macro_rules! generator_method {
    ($(#[$doc:meta])* $method:ident, $ty:ident) => {
        $(#[$doc])*
        pub fn $method(&self) -> $ty {
            let mut guard = self.entropy.lock().unwrap_or_else(|e| e.into_inner());
            $ty::generate(guard.as_mut())
        }
    };
}

impl IdGenerator {
    /// Wrap an entropy source.
    pub fn new(entropy: impl EntropySource + 'static) -> Self {
        Self {
            entropy: Mutex::new(Box::new(entropy)),
        }
    }

    /// Generator backed by OS entropy.
    #[cfg(feature = "os-entropy")]
    pub fn os() -> Self {
        Self::new(OsEntropy)
    }

    generator_method!(
        /// Generate a [`ThoughtId`].
        thought,
        ThoughtId
    );
    generator_method!(
        /// Generate an [`EpisodeId`].
        episode,
        EpisodeId
    );
    generator_method!(
        /// Generate a [`HypothesisId`].
        hypothesis,
        HypothesisId
    );
    generator_method!(
        /// Generate a [`PredictionId`].
        prediction,
        PredictionId
    );
    generator_method!(
        /// Generate an [`IntentionId`].
        intention,
        IntentionId
    );
    generator_method!(
        /// Generate an [`OutcomeId`].
        outcome,
        OutcomeId
    );
    generator_method!(
        /// Generate an [`ObservationId`].
        observation,
        ObservationId
    );
    generator_method!(
        /// Generate a [`ProvenanceId`].
        provenance,
        ProvenanceId
    );
}

impl fmt::Debug for IdGenerator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("IdGenerator { .. }")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seeded_is_deterministic() {
        let a = IdGenerator::new(SeededEntropy::new(7));
        let b = IdGenerator::new(SeededEntropy::new(7));
        assert_eq!(a.thought(), b.thought());
        assert_ne!(a.thought(), a.thought());
    }

    #[test]
    fn display_parse_round_trip() {
        let id = IdGenerator::new(SeededEntropy::new(1)).episode();
        let s = id.to_string();
        assert!(s.starts_with("episode:"));
        assert_eq!(s.parse::<EpisodeId>().unwrap(), id);
    }

    #[test]
    fn rejects_wrong_kind_and_garbage() {
        let id = IdGenerator::new(SeededEntropy::new(1))
            .thought()
            .to_string();
        assert!(id.parse::<EpisodeId>().is_err());
        assert!("thought:zz".parse::<ThoughtId>().is_err());
    }

    #[cfg(feature = "os-entropy")]
    #[test]
    fn os_entropy_unique() {
        let g = IdGenerator::os();
        assert_ne!(g.thought(), g.thought());
    }

    struct FailingEntropy;

    impl EntropySource for FailingEntropy {
        fn try_fill(&mut self, _buf: &mut [u8]) -> Result<(), EntropyError> {
            Err(EntropyError)
        }

        fn fill(&mut self, _buf: &mut [u8]) {
            panic!("fill must not be reached through try_fill");
        }
    }

    #[test]
    fn try_fill_reports_source_failure() {
        let mut buf = [0u8; 16];
        assert_eq!(FailingEntropy.try_fill(&mut buf), Err(EntropyError));
    }

    #[test]
    fn default_try_fill_matches_fill() {
        let mut via_fill = SeededEntropy::new(7);
        let mut via_try = SeededEntropy::new(7);
        let (mut a, mut b) = ([0u8; 32], [0u8; 32]);
        via_fill.fill(&mut a);
        via_try
            .try_fill(&mut b)
            .expect("seeded entropy never fails");
        assert_eq!(a, b);
    }
}
