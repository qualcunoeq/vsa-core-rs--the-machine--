//! Bounded natural-language frontend for exact uniform integer-die events.
//!
//! This is intentionally narrower than the finite-probability frontend.  It
//! accepts only an explicitly standard/fair die whose faces are the integers
//! `1..=N`, plus one simple event predicate.  It lowers to the existing exact
//! probability pack; it never treats a labeled or merely "six-sided" die as
//! uniform by convention.

use crate::probability_pack::{
    evaluate_probability, ProbabilityArtifact, ProbabilityOperation, ProbabilityRequest,
    ProbabilityStatus, Rational,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FrontendStatus {
    Complete,
    Ambiguous,
    Missing,
    Unsupported,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum EventPredicate {
    Negative,
    LessThan(i64),
    LessThanOrEqual(i64),
    Equal(i64),
    NotEqual(i64),
    Even,
    Odd,
    MultipleOf(i64),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UniformDieRequest {
    pub sides: u32,
    pub event: EventPredicate,
    pub provenance: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FrontendResult {
    pub status: FrontendStatus,
    pub request: Option<UniformDieRequest>,
    pub unresolved: Vec<String>,
    pub provenance: Vec<String>,
    pub replay_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExecutionResult {
    pub status: FrontendStatus,
    pub value: Option<Rational>,
    pub probability: Option<ProbabilityArtifact>,
    pub assumptions: Vec<String>,
    pub reasons: Vec<String>,
    pub provenance: Vec<String>,
    pub replay_hash: String,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn finish(mut result: FrontendResult) -> FrontendResult {
    result.replay_hash.clear();
    result.replay_hash = digest(&result);
    result
}

fn finish_execution(mut result: ExecutionResult) -> ExecutionResult {
    result.replay_hash.clear();
    result.replay_hash = digest(&result);
    result
}

pub fn replay_verified(result: &FrontendResult) -> bool {
    let mut copy = result.clone();
    let hash = copy.replay_hash.clone();
    copy.replay_hash.clear();
    hash == digest(&copy) && !result.provenance.is_empty()
}

pub fn execution_replay_verified(result: &ExecutionResult) -> bool {
    let mut copy = result.clone();
    let hash = copy.replay_hash.clone();
    copy.replay_hash.clear();
    hash == digest(&copy) && !result.provenance.is_empty()
}

fn output(
    status: FrontendStatus,
    request: Option<UniformDieRequest>,
    unresolved: Vec<String>,
    provenance: Vec<String>,
) -> FrontendResult {
    finish(FrontendResult {
        status,
        request,
        unresolved,
        provenance,
        replay_hash: String::new(),
    })
}

fn parse_positive_integer_after(text: &str, marker: &str) -> Option<u32> {
    let start = text.find(marker)? + marker.len();
    let digits: String = text[start..]
        .trim_start()
        .chars()
        .take_while(char::is_ascii_digit)
        .collect();
    let sides = digits.parse().ok()?;
    (2..=1000).contains(&sides).then_some(sides)
}

fn parse_sides(text: &str) -> Vec<u32> {
    let markers = [
        "standard ",
        "fair ",
        "uniform ",
        "with ",
        "of ",
        "each of ",
        "each of the ",
    ];
    let mut values = Vec::new();
    for marker in markers {
        if let Some(value) = parse_positive_integer_after(text, marker) {
            let start = text.find(marker).unwrap() + marker.len();
            let tail = &text[start..];
            let plausible = match marker {
                "standard " | "fair " | "uniform " => {
                    tail.contains("-sided die")
                        || tail.contains("-sided")
                        || tail.contains(" faces")
                }
                "with " => tail.contains("faces"),
                "of " => {
                    tail.contains("equally likely faces") || tail.contains("equally-likely faces")
                }
                "each of " | "each of the " => tail.contains("faces") || tail.contains("outcomes"),
                _ => false,
            };
            if plausible {
                values.push(value);
            }
        }
    }
    values.sort_unstable();
    values.dedup();
    values
}

fn parse_integer_after(text: &str, marker: &str) -> Option<i64> {
    let start = text.find(marker)? + marker.len();
    let rest = text[start..].trim_start();
    let mut token = String::new();
    for (index, character) in rest.chars().enumerate() {
        if index == 0 && matches!(character, '-' | '−') {
            token.push('-');
        } else if character.is_ascii_digit() {
            token.push(character);
        } else {
            break;
        }
    }
    (!token.is_empty() && token != "-").then(|| token.replace('−', "-").parse().ok())?
}

fn has_contiguous_integer_labels(text: &str, sides: u32) -> bool {
    [
        format!("faces labeled 1–{sides}"),
        format!("faces labeled 1-{sides}"),
        format!("faces labeled 1 to {sides}"),
    ]
    .iter()
    .any(|marker| text.contains(marker))
}

fn parse_event(text: &str) -> Result<EventPredicate, FrontendStatus> {
    let lower = text.to_ascii_lowercase();
    let mut candidates = Vec::new();
    if lower.contains("negative number")
        || lower.contains("negative result")
        || lower.contains("negative value")
        || lower.contains("negative outcome")
    {
        candidates.push(EventPredicate::Negative);
    }
    let at_most_marker = [
        "less than or equal to",
        "at most",
        "no greater than",
        "not exceeding",
    ]
    .iter()
    .find(|marker| lower.contains(**marker));
    if let Some(marker) = at_most_marker {
        let value = parse_integer_after(&lower, marker).ok_or(FrontendStatus::Missing)?;
        candidates.push(EventPredicate::LessThanOrEqual(value));
    } else if let Some(marker) = ["less than", "below", "under", "fewer than"]
        .iter()
        .find(|marker| lower.contains(**marker))
    {
        let value = parse_integer_after(&lower, marker).ok_or(FrontendStatus::Missing)?;
        candidates.push(EventPredicate::LessThan(value));
    }
    if lower.contains("not rolling") || lower.contains("not roll") {
        if let Some(value) = parse_integer_after(&lower, "not rolling a ")
            .or_else(|| parse_integer_after(&lower, "not rolling an "))
            .or_else(|| parse_integer_after(&lower, "not roll a "))
        {
            candidates.push(EventPredicate::NotEqual(value));
        }
    } else if let Some(marker) = ["not equal to ", "anything except ", "other than "]
        .iter()
        .find(|marker| lower.contains(**marker))
    {
        if let Some(value) = parse_integer_after(&lower, marker) {
            candidates.push(EventPredicate::NotEqual(value));
        }
    } else if lower.contains("rolling an ") || lower.contains("rolling a ") {
        let marker = if lower.contains("rolling an ") {
            "rolling an "
        } else {
            "rolling a "
        };
        if let Some(value) = parse_integer_after(&lower, marker) {
            candidates.push(EventPredicate::Equal(value));
        }
    } else if let Some(marker) = ["roll of ", "outcome equals ", "value equals "]
        .iter()
        .find(|marker| lower.contains(**marker))
    {
        if let Some(value) = parse_integer_after(&lower, marker) {
            candidates.push(EventPredicate::Equal(value));
        }
    }
    if lower.contains("even number")
        || lower.contains("even outcome")
        || lower.contains("even value")
        || lower.contains("even result")
    {
        candidates.push(EventPredicate::Even);
    }
    if lower.contains("odd number")
        || lower.contains("odd outcome")
        || lower.contains("odd value")
        || lower.contains("odd result")
    {
        candidates.push(EventPredicate::Odd);
    }
    if let Some(marker) = ["multiple of ", "divisible by "]
        .iter()
        .find(|marker| lower.contains(**marker))
    {
        let value = parse_integer_after(&lower, marker).ok_or(FrontendStatus::Missing)?;
        if value == 0 {
            return Err(FrontendStatus::Unsupported);
        }
        candidates.push(EventPredicate::MultipleOf(value));
    }
    if candidates.len() != 1 {
        return Err(if candidates.is_empty() {
            FrontendStatus::Missing
        } else {
            FrontendStatus::Ambiguous
        });
    }
    Ok(candidates.pop().unwrap())
}

/// Parse one explicitly uniform integer-die event.
pub fn formalize(text: &str, case_id: &str) -> FrontendResult {
    let lower = text.to_ascii_lowercase();
    let provenance = vec![
        format!("uniform-die-frontend:{case_id}"),
        format!("source-span:0..{}", text.len()),
        "explicit-uniform-integer-die-grammar".into(),
    ];
    if lower.contains("two dice")
        || lower.contains("three dice")
        || lower.contains("sum")
        || lower.contains("odds")
        || lower.contains("probability of an event")
        || lower.contains("empirical")
        || lower.contains("repeated rolls")
        || lower.contains("over 10 rolls")
        || lower.contains("times")
        || lower.contains("special")
        || lower.contains("without replacement")
        || lower.contains("repeatedly")
    {
        return output(
            FrontendStatus::Unsupported,
            None,
            vec!["request is outside one uniform integer-die event".into()],
            provenance,
        );
    }
    let explicit_uniformity = lower.contains("standard ")
        || lower.contains("fair ")
        || lower.contains("uniform ")
        || lower.contains("equally likely")
        || lower.contains("equally-likely")
        || lower.contains("equal chance");
    if !explicit_uniformity || !lower.contains("die") {
        return output(
            FrontendStatus::Ambiguous,
            None,
            vec!["uniformity and finite die size must be explicit".into()],
            provenance,
        );
    }
    let side_candidates = parse_sides(&lower);
    if side_candidates.len() > 1 {
        return output(
            FrontendStatus::Ambiguous,
            None,
            vec!["multiple possible die sizes were stated".into()],
            provenance,
        );
    }
    let Some(sides) = side_candidates.first().copied() else {
        let size_evidence =
            lower.contains("-sided") || lower.contains(" faces") || lower.contains(" outcomes");
        return output(
            if size_evidence {
                FrontendStatus::Missing
            } else {
                FrontendStatus::Ambiguous
            },
            None,
            vec!["a bounded number of sides is required".into()],
            provenance,
        );
    };
    if lower.contains("labeled") && !has_contiguous_integer_labels(&lower, sides) {
        return output(
            FrontendStatus::Unsupported,
            None,
            vec!["labeled faces are supported only when the contiguous integer range 1..=N is explicit".into()],
            provenance,
        );
    }
    let event = match parse_event(&lower) {
        Ok(event) => event,
        Err(status) => {
            return output(
                status,
                None,
                vec!["exactly one supported event predicate is required".into()],
                provenance,
            )
        }
    };
    let request = UniformDieRequest {
        sides,
        event,
        provenance: provenance.clone(),
    };
    output(
        FrontendStatus::Complete,
        Some(request),
        Vec::new(),
        provenance,
    )
}

fn event_matches(event: &EventPredicate, value: i64) -> bool {
    match event {
        EventPredicate::Negative => value < 0,
        EventPredicate::LessThan(bound) => value < *bound,
        EventPredicate::LessThanOrEqual(bound) => value <= *bound,
        EventPredicate::Equal(target) => value == *target,
        EventPredicate::NotEqual(target) => value != *target,
        EventPredicate::Even => value % 2 == 0,
        EventPredicate::Odd => value % 2 != 0,
        EventPredicate::MultipleOf(divisor) => value % divisor == 0,
    }
}

pub fn execute(request: &UniformDieRequest) -> ExecutionResult {
    let provenance = request.provenance.clone();
    let outcomes: Vec<String> = (1..=request.sides).map(|value| value.to_string()).collect();
    let probabilities = outcomes
        .iter()
        .map(|_| Rational::new(1, request.sides as i128).unwrap())
        .collect::<Vec<_>>();
    let event_indices = (1..=request.sides as i64)
        .filter(|value| event_matches(&request.event, *value))
        .map(|value| (value - 1) as usize)
        .collect::<Vec<_>>();
    let complement_indices = (0..request.sides as usize)
        .filter(|index| !event_indices.contains(index))
        .collect::<Vec<_>>();
    let probability_request = ProbabilityRequest {
        // The existing pack exposes complement, so evaluate the desired
        // event as one minus its explicitly enumerated complement.  This
        // preserves reuse of the validated probability backend without
        // adding a one-off event evaluator.
        operation: ProbabilityOperation::Complement,
        domain: "finite_exact_probability".into(),
        outcomes,
        probabilities,
        values: (1..=request.sides as i64).collect(),
        event_a: Some(complement_indices),
        event_b: None,
        partition: Vec::new(),
        conditional_values: Vec::new(),
        prior_probability: None,
        likelihood: None,
        evidence: None,
        ambiguity: None,
        provenance: provenance.clone(),
    };
    let result = evaluate_probability(&probability_request);
    let (status, value, probability, reasons) = match result.status {
        ProbabilityStatus::Complete => {
            let scalar = match &result.artifact {
                Some(ProbabilityArtifact::Scalar(value)) => Some(value.clone()),
                _ => None,
            };
            (
                FrontendStatus::Complete,
                scalar,
                result.artifact,
                result.reasons,
            )
        }
        ProbabilityStatus::Unsupported => (FrontendStatus::Unsupported, None, None, result.reasons),
        ProbabilityStatus::Ambiguous => (FrontendStatus::Ambiguous, None, None, result.reasons),
        _ => (FrontendStatus::Missing, None, None, result.reasons),
    };
    finish_execution(ExecutionResult {
        status,
        value,
        probability,
        assumptions: vec!["integer faces are uniformly distributed over 1..=sides".into()],
        reasons,
        provenance,
        replay_hash: String::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn standard_die_event_is_exact_and_replayable() {
        let frontend = formalize(
            "What is the probability of rolling a number less than 7 on a standard 12-sided die?",
            "test",
        );
        assert_eq!(frontend.status, FrontendStatus::Complete);
        assert!(replay_verified(&frontend));
        let execution = execute(frontend.request.as_ref().unwrap());
        assert_eq!(execution.status, FrontendStatus::Complete);
        assert_eq!(execution.value, Rational::new(1, 2));
        assert!(execution_replay_verified(&execution));
    }

    #[test]
    fn labeled_and_multi_die_requests_fail_closed() {
        assert_eq!(
            formalize(
                "What is the probability of rolling an even number on a special labeled 6-sided die?",
                "test"
            )
            .status,
            FrontendStatus::Unsupported
        );
        assert_eq!(
            formalize(
                "What is the probability that two dice have sum less than 8?",
                "test"
            )
            .status,
            FrontendStatus::Unsupported
        );
        assert_eq!(
            formalize(
                "What is the probability of rolling an even number on a standard 6-sided die with faces labeled A-F?",
                "test"
            )
            .status,
            FrontendStatus::Unsupported
        );
    }

    #[test]
    fn missing_uniformity_is_not_inferred() {
        assert_eq!(
            formalize(
                "What is the probability of rolling an even number on a 6-sided die?",
                "test"
            )
            .status,
            FrontendStatus::Ambiguous
        );
    }

    #[test]
    fn contiguous_integer_labels_preserve_uniform_semantics() {
        let frontend = formalize(
            "What is the probability of rolling an 11 on a standard 12-sided die with faces labeled 1–12?",
            "test",
        );
        assert_eq!(frontend.status, FrontendStatus::Complete);
        let execution = execute(frontend.request.as_ref().unwrap());
        assert_eq!(execution.value, Rational::new(1, 12));
        assert!(execution_replay_verified(&execution));
    }

    #[test]
    fn shifted_uniformity_and_event_phrasings_remain_typed() {
        let cases = [
            (
                "A fair 8-sided die is rolled. What is the chance of an outcome at most 3?",
                Rational::new(3, 8),
            ),
            (
                "A die with 10 equally likely faces is rolled. Find the probability of a value divisible by 5.",
                Rational::new(1, 5),
            ),
            (
                "A die with 12 faces, each equally likely, is rolled. What is the probability of a roll below 7?",
                Rational::new(1, 2),
            ),
            (
                "For a uniform 6-sided die, find the probability of a roll of 4.",
                Rational::new(1, 6),
            ),
        ];
        for (text, expected) in cases {
            let frontend = formalize(text, "shifted");
            assert_eq!(frontend.status, FrontendStatus::Complete, "{text}");
            assert!(replay_verified(&frontend));
            let execution = execute(frontend.request.as_ref().unwrap());
            assert_eq!(execution.status, FrontendStatus::Complete, "{text}");
            assert_eq!(execution.value, expected, "{text}");
            assert!(execution_replay_verified(&execution));
        }
    }

    #[test]
    fn shifted_language_does_not_relax_boundaries() {
        let ambiguous = [
            "A 10-sided die is rolled. Find the probability of a value at most 3.",
            "A die with 10 faces is rolled. Find the probability of an even value.",
        ];
        for text in ambiguous {
            assert_eq!(
                formalize(text, "boundary").status,
                FrontendStatus::Ambiguous
            );
        }
        let unsupported = [
            "Two fair 6-sided dice are rolled. Find the probability of a sum below 8.",
            "A fair 6-sided die has faces labeled A-F. Find the probability of an even result.",
            "A fair 6-sided die is rolled repeatedly. Find the probability of exactly two even results.",
        ];
        for text in unsupported {
            assert_eq!(
                formalize(text, "boundary").status,
                FrontendStatus::Unsupported
            );
        }
    }
}
