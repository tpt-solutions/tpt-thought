// SPDX-License-Identifier: MIT OR Apache-2.0
//! Pattern detection, skill formation, cognitive reflexes and abstraction (spec sections 86-89).
//!
//! Everything here is a pure function of recorded attempts and deliberations, so the same
//! inputs always produce the same patterns. Reflections are structured artifacts (strategies,
//! policies, skills), never a model's private chain of thought.

use crate::{extend_provenance, Association, Episode};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use tpt_thought_core::{EpisodeId, ProvenanceId};

/// One attempt at a strategy and its result.
#[derive(Debug, Clone, PartialEq)]
pub struct Attempt {
    /// Strategy used.
    pub strategy: String,
    /// Whether it achieved its outcome.
    pub succeeded: bool,
    /// Signature of the error, when it failed in a recognisable way.
    pub error_signature: Option<String>,
    /// Provenance of the attempt.
    pub provenance: Vec<ProvenanceId>,
}

/// One deliberation: a question and the answer it settled on.
#[derive(Debug, Clone, PartialEq)]
pub struct Deliberation {
    /// The question deliberated on.
    pub question: String,
    /// The answer reached.
    pub answer: String,
    /// Provenance of the deliberation.
    pub provenance: Vec<ProvenanceId>,
}

/// Thresholds for pattern detection.
#[derive(Debug, Clone, PartialEq)]
pub struct PatternConfig {
    /// Attempts needed before a strategy gets a verdict.
    pub min_samples: u32,
    /// Success ratio at or above which a strategy works.
    pub works_ratio: f64,
    /// Success ratio at or below which a strategy fails.
    pub fails_ratio: f64,
    /// Successes needed before a working strategy becomes a skill.
    pub skill_successes: u32,
    /// Occurrences needed before an error counts as a repeated mistake.
    pub mistake_repeats: u32,
    /// Identical deliberations needed before they become a learned policy.
    pub reflex_repeats: u32,
    /// Route hint for skills: skills are routed through Intellect as a fast provider (§88).
    pub fast_route_hint: String,
}

impl Default for PatternConfig {
    fn default() -> Self {
        Self {
            min_samples: 3,
            works_ratio: 0.8,
            fails_ratio: 0.2,
            skill_successes: 3,
            mistake_repeats: 2,
            reflex_repeats: 3,
            fast_route_hint: "intellect:fast".into(),
        }
    }
}

/// Whether a strategy works.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    /// Succeeds reliably.
    Works,
    /// Fails reliably.
    Fails,
    /// Neither reliable nor reliably failing.
    Mixed,
}

/// The track record of one strategy.
#[derive(Debug, Clone, PartialEq)]
pub struct StrategyReport {
    /// Strategy name.
    pub strategy: String,
    /// Successful attempts.
    pub successes: u32,
    /// Failed attempts.
    pub failures: u32,
    /// The verdict.
    pub verdict: Verdict,
    /// Provenance of the attempts.
    pub provenance: Vec<ProvenanceId>,
}

/// An error that recurs often enough to be a known mistake.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepeatedMistake {
    /// The error signature.
    pub signature: String,
    /// How often it occurred.
    pub count: u32,
    /// Provenance of the occurrences.
    pub provenance: Vec<ProvenanceId>,
}

/// A deliberation repeated until it is a learned policy (cognitive reflex, §89).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LearnedPolicy {
    /// The question.
    pub question: String,
    /// The answer it consistently reaches.
    pub answer: String,
    /// How often it was reached.
    pub count: u32,
    /// Provenance of the deliberations.
    pub provenance: Vec<ProvenanceId>,
}

/// How a skill is executed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SkillRoute {
    /// Routed through Intellect as a fast provider, with this hint (§88).
    FastProvider {
        /// Provider hint.
        hint: String,
    },
}

/// A reusable skill formed from repeated success (§88).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Skill {
    /// Skill name (the strategy it came from).
    pub name: String,
    /// How it is routed.
    pub route: SkillRoute,
    /// Successes that justified it.
    pub evidence_count: u32,
    /// Provenance of the evidence.
    pub provenance: Vec<ProvenanceId>,
}

/// Everything pattern detection found in one pass.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Detection {
    /// Strategy track records.
    pub strategies: Vec<StrategyReport>,
    /// Repeated mistakes.
    pub mistakes: Vec<RepeatedMistake>,
    /// Learned policies.
    pub reflexes: Vec<LearnedPolicy>,
    /// Skills.
    pub skills: Vec<Skill>,
}

/// Detect strategies, repeated mistakes, learned policies and skills.
pub fn detect(
    attempts: &[Attempt],
    deliberations: &[Deliberation],
    cfg: &PatternConfig,
) -> Detection {
    let mut by_strategy: BTreeMap<&str, (u32, u32, Vec<ProvenanceId>)> = BTreeMap::new();
    let mut by_signature: BTreeMap<&str, (u32, Vec<ProvenanceId>)> = BTreeMap::new();
    for a in attempts {
        let entry = by_strategy.entry(&a.strategy).or_default();
        if a.succeeded {
            entry.0 += 1;
        } else {
            entry.1 += 1;
        }
        extend_provenance(&mut entry.2, &a.provenance);
        if let (false, Some(sig)) = (a.succeeded, &a.error_signature) {
            let entry = by_signature.entry(sig).or_default();
            entry.0 += 1;
            extend_provenance(&mut entry.1, &a.provenance);
        }
    }

    let mut strategies = Vec::new();
    let mut skills = Vec::new();
    for (name, (successes, failures, provenance)) in by_strategy {
        let total = successes + failures;
        if total < cfg.min_samples {
            continue;
        }
        let ratio = f64::from(successes) / f64::from(total);
        let verdict = if ratio >= cfg.works_ratio {
            Verdict::Works
        } else if ratio <= cfg.fails_ratio {
            Verdict::Fails
        } else {
            Verdict::Mixed
        };
        if verdict == Verdict::Works && successes >= cfg.skill_successes {
            skills.push(Skill {
                name: name.to_owned(),
                route: SkillRoute::FastProvider {
                    hint: cfg.fast_route_hint.clone(),
                },
                evidence_count: successes,
                provenance: provenance.clone(),
            });
        }
        strategies.push(StrategyReport {
            strategy: name.to_owned(),
            successes,
            failures,
            verdict,
            provenance,
        });
    }

    let mistakes = by_signature
        .into_iter()
        .filter(|(_, (count, _))| *count >= cfg.mistake_repeats)
        .map(|(sig, (count, provenance))| RepeatedMistake {
            signature: sig.to_owned(),
            count,
            provenance,
        })
        .collect();

    let mut by_pair: BTreeMap<(&str, &str), (u32, Vec<ProvenanceId>)> = BTreeMap::new();
    for d in deliberations {
        let entry = by_pair.entry((&d.question, &d.answer)).or_default();
        entry.0 += 1;
        extend_provenance(&mut entry.1, &d.provenance);
    }
    let reflexes = by_pair
        .into_iter()
        .filter(|(_, (count, _))| *count >= cfg.reflex_repeats)
        .map(|((question, answer), (count, provenance))| LearnedPolicy {
            question: question.to_owned(),
            answer: answer.to_owned(),
            count,
            provenance,
        })
        .collect();

    Detection {
        strategies,
        mistakes,
        reflexes,
        skills,
    }
}

/// The kind of knowledge an abstraction yields (spec section 87).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AbstractionKind {
    /// A recurring fact about the world.
    Semantic,
    /// A strategy that works.
    Procedural,
    /// A causal relationship.
    CausalModel,
    /// A rule of thumb, including what to avoid.
    Heuristic,
    /// A learned answer to a recurring question.
    Policy,
    /// A reusable skill.
    Skill,
}

/// Knowledge distilled from episodes and patterns. Carries the provenance it was built from,
/// so promotion never severs the link back to evidence.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Knowledge {
    /// What kind of knowledge this is.
    pub kind: AbstractionKind,
    /// Human-readable statement of the knowledge.
    pub statement: String,
    /// Episodes it was abstracted from.
    pub sources: Vec<EpisodeId>,
    /// Provenance of the evidence.
    pub provenance: Vec<ProvenanceId>,
}

/// Abstract knowledge from detected patterns, episodes and causal links.
pub fn abstract_knowledge(
    detection: &Detection,
    episodes: &[Episode],
    causal: &[&Association],
) -> Vec<Knowledge> {
    let mut out = Vec::new();
    for s in &detection.strategies {
        match s.verdict {
            Verdict::Works => out.push(Knowledge {
                kind: AbstractionKind::Procedural,
                statement: format!("prefer strategy {}", s.strategy),
                sources: Vec::new(),
                provenance: s.provenance.clone(),
            }),
            Verdict::Fails => out.push(Knowledge {
                kind: AbstractionKind::Heuristic,
                statement: format!("avoid strategy {}", s.strategy),
                sources: Vec::new(),
                provenance: s.provenance.clone(),
            }),
            Verdict::Mixed => {}
        }
    }
    for m in &detection.mistakes {
        out.push(Knowledge {
            kind: AbstractionKind::Heuristic,
            statement: format!("guard against {}", m.signature),
            sources: Vec::new(),
            provenance: m.provenance.clone(),
        });
    }
    for r in &detection.reflexes {
        out.push(Knowledge {
            kind: AbstractionKind::Policy,
            statement: format!("answer '{}' with '{}'", r.question, r.answer),
            sources: Vec::new(),
            provenance: r.provenance.clone(),
        });
    }
    for s in &detection.skills {
        out.push(Knowledge {
            kind: AbstractionKind::Skill,
            statement: format!("skill {}", s.name),
            sources: Vec::new(),
            provenance: s.provenance.clone(),
        });
    }
    for a in causal {
        out.push(Knowledge {
            kind: AbstractionKind::CausalModel,
            statement: format!("{} causes {}", a.from, a.to),
            sources: Vec::new(),
            provenance: a.provenance.clone(),
        });
    }

    let mut by_topic: BTreeMap<&str, (Vec<EpisodeId>, Vec<ProvenanceId>)> = BTreeMap::new();
    for e in episodes {
        let entry = by_topic.entry(&e.topic).or_default();
        entry.0.push(e.id);
        extend_provenance(&mut entry.1, &e.provenance);
    }
    for (topic, (sources, provenance)) in by_topic {
        if sources.len() >= 2 {
            out.push(Knowledge {
                kind: AbstractionKind::Semantic,
                statement: format!("{topic} recurs"),
                sources,
                provenance,
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use tpt_thought_core::{IdGenerator, SeededEntropy};

    fn attempt(strategy: &str, succeeded: bool, sig: Option<&str>, p: ProvenanceId) -> Attempt {
        Attempt {
            strategy: strategy.into(),
            succeeded,
            error_signature: sig.map(Into::into),
            provenance: vec![p],
        }
    }

    #[test]
    fn reliable_success_becomes_a_skill_routed_to_fast_provider() {
        let ids = IdGenerator::new(SeededEntropy::new(30));
        let attempts: Vec<_> = (0..4)
            .map(|_| attempt("cache-first", true, None, ids.provenance()))
            .collect();
        let d = detect(&attempts, &[], &PatternConfig::default());
        assert_eq!(d.strategies[0].verdict, Verdict::Works);
        assert_eq!(d.skills.len(), 1);
        assert_eq!(
            d.skills[0].route,
            SkillRoute::FastProvider {
                hint: "intellect:fast".into()
            }
        );
        assert_eq!(d.skills[0].provenance.len(), 4);
    }

    #[test]
    fn mixed_strategies_get_no_skill() {
        let ids = IdGenerator::new(SeededEntropy::new(31));
        let attempts = vec![
            attempt("s", true, None, ids.provenance()),
            attempt("s", false, None, ids.provenance()),
            attempt("s", true, None, ids.provenance()),
            attempt("s", false, None, ids.provenance()),
        ];
        let d = detect(&attempts, &[], &PatternConfig::default());
        assert_eq!(d.strategies[0].verdict, Verdict::Mixed);
        assert!(d.skills.is_empty());
    }

    #[test]
    fn repeated_errors_and_deliberations_are_identified() {
        let ids = IdGenerator::new(SeededEntropy::new(32));
        let attempts = vec![
            attempt("s", false, Some("timeout"), ids.provenance()),
            attempt("s", false, Some("timeout"), ids.provenance()),
        ];
        let delib: Vec<_> = (0..3)
            .map(|_| Deliberation {
                question: "retry?".into(),
                answer: "no".into(),
                provenance: vec![ids.provenance()],
            })
            .collect();
        let d = detect(&attempts, &delib, &PatternConfig::default());
        assert_eq!(d.mistakes[0].count, 2);
        assert_eq!(d.reflexes[0].answer, "no");
    }

    #[test]
    fn abstraction_keeps_provenance() {
        let ids = IdGenerator::new(SeededEntropy::new(33));
        let p = ids.provenance();
        let d = detect(
            &[
                attempt("s", false, None, p),
                attempt("s", false, None, p),
                attempt("s", false, None, p),
            ],
            &[],
            &PatternConfig::default(),
        );
        let k = abstract_knowledge(&d, &[], &[]);
        assert_eq!(k[0].kind, AbstractionKind::Heuristic);
        assert_eq!(k[0].provenance, vec![p]);
    }
}
