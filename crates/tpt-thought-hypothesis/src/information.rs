// SPDX-License-Identifier: MIT OR Apache-2.0
//! Information gain (spec section 37). Gain is measured in bits as the expected reduction in
//! Shannon entropy over the hypotheses after observing an outcome.

const TOLERANCE: f64 = 1e-9;

/// Shannon entropy of a distribution, in bits. Zero-probability entries contribute nothing.
/// Returns `None` if any entry is negative or not finite.
pub fn shannon_bits(dist: &[f64]) -> Option<f64> {
    if dist.iter().any(|p| !p.is_finite() || *p < 0.0) {
        return None;
    }
    Some(
        dist.iter()
            .filter(|p| **p > 0.0)
            .map(|p| -p * p.log2())
            .sum::<f64>(),
    )
}

/// Expected information gain, in bits, from testing a set of hypotheses.
///
/// `prior[h]` is the probability of hypothesis `h`. `likelihood[h][o]` is `P(outcome o | h)`.
/// The gain is `H(prior) - sum_o P(o) H(posterior | o)`. Returns `None` if the prior or any
/// likelihood row is not a distribution over the same outcomes.
pub fn expected_information_gain(prior: &[f64], likelihood: &[Vec<f64>]) -> Option<f64> {
    if prior.is_empty() || prior.len() != likelihood.len() {
        return None;
    }
    if (prior.iter().sum::<f64>() - 1.0).abs() > TOLERANCE {
        return None;
    }
    let outcomes = likelihood.first()?.len();
    if outcomes == 0 || likelihood.iter().any(|row| row.len() != outcomes) {
        return None;
    }
    for row in likelihood {
        if (row.iter().sum::<f64>() - 1.0).abs() > TOLERANCE {
            return None;
        }
    }
    let prior_entropy = shannon_bits(prior)?;
    let mut posterior_entropy = 0.0;
    for o in 0..outcomes {
        let joint: Vec<f64> = prior
            .iter()
            .zip(likelihood)
            .map(|(p, row)| p * row[o])
            .collect();
        let p_outcome: f64 = joint.iter().sum();
        if p_outcome <= 0.0 {
            continue;
        }
        let posterior: Vec<f64> = joint.iter().map(|j| j / p_outcome).collect();
        posterior_entropy += p_outcome * shannon_bits(&posterior)?;
    }
    Some((prior_entropy - posterior_entropy).max(0.0))
}

/// Whether testing is worth its cost: investigate only when the expected gain exceeds the cost
/// (spec section 37).
pub fn should_investigate(expected_gain: f64, cost: f64) -> bool {
    expected_gain > cost
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entropy_of_fair_coin_is_one_bit() {
        assert!((shannon_bits(&[0.5, 0.5]).unwrap() - 1.0).abs() < 1e-12);
        assert_eq!(shannon_bits(&[1.0, 0.0]).unwrap(), 0.0);
        assert!(shannon_bits(&[-0.1, 1.1]).is_none());
    }

    #[test]
    fn perfect_test_gains_full_entropy() {
        // Two equally likely hypotheses, an outcome that identifies each: 1 bit.
        let gain = expected_information_gain(&[0.5, 0.5], &[vec![1.0, 0.0], vec![0.0, 1.0]]);
        assert!((gain.unwrap() - 1.0).abs() < 1e-12);
    }

    #[test]
    fn uninformative_test_gains_nothing() {
        let gain = expected_information_gain(&[0.3, 0.7], &[vec![0.5, 0.5], vec![0.5, 0.5]]);
        assert!(gain.unwrap().abs() < 1e-12);
    }

    #[test]
    fn malformed_inputs_are_rejected() {
        assert!(expected_information_gain(&[0.6, 0.6], &[vec![1.0], vec![1.0]]).is_none());
        assert!(expected_information_gain(&[0.5, 0.5], &[vec![1.0, 0.0]]).is_none());
        assert!(
            expected_information_gain(&[0.5, 0.5], &[vec![0.9, 0.0], vec![0.0, 1.0]]).is_none()
        );
    }

    #[test]
    fn investigate_only_when_gain_exceeds_cost() {
        assert!(should_investigate(1.0, 0.5));
        assert!(!should_investigate(0.5, 0.5));
        assert!(!should_investigate(0.1, 0.5));
    }
}
