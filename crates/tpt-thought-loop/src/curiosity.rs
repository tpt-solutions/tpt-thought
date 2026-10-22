// SPDX-License-Identifier: MIT OR Apache-2.0
//! The curiosity driver (spec section 79).
//!
//! Curiosity is the drive to investigate something novel that might be informative. The drive is
//! the product of novelty and expected information gain, and an event fires when it reaches the
//! threshold. Nothing fires for a zero drive, however low the threshold.

use serde::{Deserialize, Serialize};
use std::fmt;

/// The threshold was outside `0.0..=1.0` or not finite.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct InvalidThreshold(pub f64);

impl fmt::Display for InvalidThreshold {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "curiosity threshold {} is not in 0.0..=1.0", self.0)
    }
}

impl std::error::Error for InvalidThreshold {}

/// Raised when curiosity reaches the threshold: a question worth investigating.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CuriosityEvent {
    /// The question the drive is about.
    pub question: String,
    /// Novelty times information gain, in `0.0..=1.0`.
    pub drive: f64,
}

/// Decides when curiosity fires.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct CuriosityDriver {
    threshold: f64,
}

impl CuriosityDriver {
    /// A driver that fires at `threshold`.
    pub fn new(threshold: f64) -> Result<Self, InvalidThreshold> {
        if (0.0..=1.0).contains(&threshold) {
            Ok(Self { threshold })
        } else {
            Err(InvalidThreshold(threshold))
        }
    }

    /// The threshold at which curiosity fires.
    pub fn threshold(&self) -> f64 {
        self.threshold
    }

    /// Check the drive for `question`. Novelty and information gain are each clamped to
    /// `0.0..=1.0` first, so out-of-range inputs cannot inflate the drive.
    pub fn check(
        &self,
        novelty: f64,
        information_gain: f64,
        question: &str,
    ) -> Option<CuriosityEvent> {
        let drive = novelty.clamp(0.0, 1.0) * information_gain.clamp(0.0, 1.0);
        (drive > 0.0 && drive >= self.threshold).then(|| CuriosityEvent {
            question: question.to_owned(),
            drive,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fires_at_or_above_threshold() {
        let driver = CuriosityDriver::new(0.25).unwrap();
        let event = driver.check(0.5, 0.5, "why?").unwrap();
        assert_eq!(event.question, "why?");
        assert_eq!(event.drive, 0.25);
        assert!(driver.check(0.5, 0.4, "why?").is_none());
    }

    #[test]
    fn zero_drive_never_fires() {
        let driver = CuriosityDriver::new(0.0).unwrap();
        assert!(driver.check(0.0, 1.0, "q").is_none());
        assert!(driver.check(1.0, 0.0, "q").is_none());
    }

    #[test]
    fn out_of_range_inputs_are_clamped() {
        let driver = CuriosityDriver::new(1.0).unwrap();
        let event = driver.check(7.0, 7.0, "q").unwrap();
        assert_eq!(event.drive, 1.0);
    }

    #[test]
    fn rejects_invalid_threshold() {
        assert!(CuriosityDriver::new(1.5).is_err());
        assert!(CuriosityDriver::new(f64::NAN).is_err());
    }
}
