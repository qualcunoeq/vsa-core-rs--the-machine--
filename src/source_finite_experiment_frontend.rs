//! Bounded natural-language frontend for explicit finite die experiments.
//!
//! The contract accepts one explicitly uniform integer die through the
//! validated uniform-die frontend, or two independent explicitly uniform
//! integer dice with one simple joint event.  It does not infer fairness,
//! independence, sample-space size, or event scope from ordinary language.
//! The typed request stops at a finite exact probability operation.

use crate::probability_pack::{
    evaluate_probability, ProbabilityArtifact, ProbabilityOperation, ProbabilityRequest,
    ProbabilityStatus, Rational,
};
use crate::uniform_die_frontend::{
    execute as execute_single_die, EventPredicate, ExecutionResult as SingleExecutionResult,
    FrontendStatus as SingleFrontendStatus, UniformDieRequest,
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
pub enum JointPredicate {
    SumEquals(i64),
    SumLessThan(i64),
    SumGreaterThan(i64),
    AtLeastOneEquals(i64),
    DifferenceEquals(i64),
    ProductEven,
    ProductOdd,
    SumPrime,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum FiniteExperimentRequest {
    Single(UniformDieRequest),
    Two {
        sides: u32,
        predicate: JointPredicate,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FrontendResult {
    pub status: FrontendStatus,
    pub request: Option<FiniteExperimentRequest>,
    pub unresolved: Vec<String>,
    pub provenance: Vec<String>,
    pub replay_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExecutionResult {
    pub status: FrontendStatus,
    pub value: Option<Rational>,
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

fn status_from_single(status: SingleFrontendStatus) -> FrontendStatus {
    match status {
        SingleFrontendStatus::Complete => FrontendStatus::Complete,
        SingleFrontendStatus::Ambiguous => FrontendStatus::Ambiguous,
        SingleFrontendStatus::Missing => FrontendStatus::Missing,
        SingleFrontendStatus::Unsupported => FrontendStatus::Unsupported,
    }
}

fn output(
    status: FrontendStatus,
    request: Option<FiniteExperimentRequest>,
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

fn parse_integer(token: &str) -> Option<i64> {
    let normalized = token.trim_matches(|c: char| !c.is_ascii_digit() && c != '-');
    if normalized.is_empty() || normalized == "-" {
        return None;
    }
    normalized.parse().ok()
}

fn number_word(token: &str) -> Option<u32> {
    Some(match token {
        "one" => 1,
        "two" => 2,
        "three" => 3,
        "four" => 4,
        "five" => 5,
        "six" => 6,
        "seven" => 7,
        "eight" => 8,
        "nine" => 9,
        "ten" => 10,
        "eleven" => 11,
        "twelve" => 12,
        "thirteen" => 13,
        "fourteen" => 14,
        "fifteen" => 15,
        "sixteen" => 16,
        "twenty" => 20,
        "thirty" => 30,
        "forty" => 40,
        "fifty" => 50,
        "sixty" => 60,
        "seventy" => 70,
        "eighty" => 80,
        "ninety" => 90,
        _ => return None,
    })
}

fn side_candidates(lower: &str) -> Vec<u32> {
    let mut candidates = Vec::new();
    let tokens = lower
        .split_whitespace()
        .map(|token| token.trim_matches(|c: char| c == ',' || c == '.' || c == ';' || c == ':'))
        .collect::<Vec<_>>();
    for (index, token) in tokens.iter().enumerate() {
        if let Some(prefix) = token.strip_suffix("-sided") {
            if let Some(value) = parse_integer(prefix)
                .and_then(|value| u32::try_from(value).ok())
                .or_else(|| number_word(prefix))
            {
                candidates.push(value);
            }
        } else if *token == "sided" && index > 0 {
            if let Some(value) = parse_integer(tokens[index - 1])
                .map(|value| value as u32)
                .or_else(|| number_word(tokens[index - 1]))
            {
                candidates.push(value);
            }
        }
    }
    for (word, value) in [("tetrahedral", 4), ("octahedral", 8), ("dodecahedral", 12)] {
        if lower.contains(word) {
            candidates.push(value);
        }
    }
    candidates.sort_unstable();
    candidates.dedup();
    candidates.retain(|value| (2..=1000).contains(value));
    candidates
}

fn has_explicit_uniformity(lower: &str) -> bool {
    [
        "fair ",
        "standard ",
        "uniform ",
        "equally likely",
        "equally-likely",
        "equal chance",
    ]
    .iter()
    .any(|marker| lower.contains(marker))
}

fn find_integer_after(lower: &str, markers: &[&str]) -> Option<i64> {
    for marker in markers {
        let Some(start) = lower.find(marker) else {
            continue;
        };
        let rest = &lower[start + marker.len()..];
        for token in rest.split_whitespace() {
            if let Some(value) = parse_integer(token) {
                return Some(value);
            }
        }
    }
    None
}

fn one_joint_predicate(lower: &str) -> Result<JointPredicate, FrontendStatus> {
    let mut candidates = Vec::new();
    if lower.contains("sum") && lower.contains("prime") {
        candidates.push(JointPredicate::SumPrime);
    }
    if !lower.contains("sum is less than") && !lower.contains("sum is greater than") {
        if let Some(value) = find_integer_after(
            lower,
            &[
                "sum equals ",
                "sum is ",
                "sum of the two numbers is ",
                "sum of the numbers is ",
            ],
        ) {
            candidates.push(JointPredicate::SumEquals(value));
        }
    }
    if let Some(value) = find_integer_after(
        lower,
        &[
            "sum less than ",
            "sum is less than ",
            "sum of the two is less than ",
        ],
    ) {
        candidates.push(JointPredicate::SumLessThan(value));
    }
    if let Some(value) = find_integer_after(
        lower,
        &[
            "sum greater than ",
            "sum is greater than ",
            "sum of the two is greater than ",
        ],
    ) {
        candidates.push(JointPredicate::SumGreaterThan(value));
    }
    if let Some(value) = find_integer_after(
        lower,
        &[
            "at least one die shows ",
            "at least one die show ",
            "at least one shows ",
        ],
    ) {
        candidates.push(JointPredicate::AtLeastOneEquals(value));
    }
    if let Some(value) = find_integer_after(lower, &["differ by ", "difference is "]) {
        candidates.push(JointPredicate::DifferenceEquals(value.abs()));
    }
    if lower.contains("product") && lower.contains("even") {
        candidates.push(JointPredicate::ProductEven);
    }
    if lower.contains("product") && lower.contains("odd") {
        candidates.push(JointPredicate::ProductOdd);
    }
    candidates.dedup();
    match candidates.as_slice() {
        [predicate] => Ok(predicate.clone()),
        [] => Err(FrontendStatus::Missing),
        _ => Err(FrontendStatus::Ambiguous),
    }
}

fn one_die_predicate(lower: &str) -> Result<EventPredicate, FrontendStatus> {
    let mut candidates = Vec::new();
    if lower.contains("negative outcome") || lower.contains("negative result") {
        candidates.push(EventPredicate::Negative);
    }
    if let Some(value) = find_integer_after(
        lower,
        &["less than or equal to ", "at most ", "no greater than "],
    ) {
        candidates.push(EventPredicate::LessThanOrEqual(value));
    } else if let Some(value) = find_integer_after(lower, &["less than ", "below ", "under "]) {
        candidates.push(EventPredicate::LessThan(value));
    }
    if let Some(value) = find_integer_after(lower, &["not equal to ", "anything except "]) {
        candidates.push(EventPredicate::NotEqual(value));
    } else if let Some(value) = find_integer_after(lower, &["rolling a ", "rolling an ", "equals "])
    {
        candidates.push(EventPredicate::Equal(value));
    }
    if lower.contains("even outcome")
        || lower.contains("even number")
        || lower.contains("even value")
        || lower.contains("even result")
    {
        candidates.push(EventPredicate::Even);
    }
    if lower.contains("odd outcome")
        || lower.contains("odd number")
        || lower.contains("odd value")
        || lower.contains("odd result")
    {
        candidates.push(EventPredicate::Odd);
    }
    if let Some(value) = find_integer_after(lower, &["multiple of ", "divisible by "]) {
        if value == 0 {
            return Err(FrontendStatus::Unsupported);
        }
        candidates.push(EventPredicate::MultipleOf(value));
    }
    candidates.dedup();
    match candidates.as_slice() {
        [predicate] => Ok(predicate.clone()),
        [] => Err(FrontendStatus::Missing),
        _ => Err(FrontendStatus::Ambiguous),
    }
}

/// Formalize one or two explicitly uniform integer dice and one finite event.
pub fn formalize(text: &str, case_id: &str) -> FrontendResult {
    let lower = text.to_ascii_lowercase();
    let provenance = vec![
        format!("finite-experiment-frontend:{case_id}"),
        format!("source-span:0..{}", text.len()),
        "explicit-finite-die-grammar".into(),
    ];
    if (lower.contains("three ") || lower.contains("four ") || lower.contains("five "))
        && lower.contains("dice")
        || lower.contains("without replacement")
        || lower.contains("loaded ")
        || lower.contains("biased ")
        || lower.contains("repeated roll")
        || lower.contains("repeatedly")
        || lower.contains("times")
        || lower.contains("expected value")
    {
        return output(
            FrontendStatus::Unsupported,
            None,
            vec!["experiment is outside the bounded one/two-die event contract".into()],
            provenance,
        );
    }
    let dice_count = if lower.contains("two dice")
        || lower.contains("two ") && lower.contains("dice")
        || lower.contains("pair of") && lower.contains("dice")
    {
        2
    } else if lower.contains("die") {
        1
    } else {
        return output(
            FrontendStatus::Ambiguous,
            None,
            vec!["explicit one- or two-die experiment is required".into()],
            provenance,
        );
    };
    if !has_explicit_uniformity(&lower) {
        return output(
            FrontendStatus::Ambiguous,
            None,
            vec!["fairness or uniformity must be explicit".into()],
            provenance,
        );
    }
    let sides = side_candidates(&lower);
    if sides.len() != 1 {
        return output(
            if sides.is_empty() {
                FrontendStatus::Missing
            } else {
                FrontendStatus::Ambiguous
            },
            None,
            vec!["exactly one bounded die size is required".into()],
            provenance,
        );
    }
    let sides = sides[0];
    if dice_count == 1 {
        return match one_die_predicate(&lower) {
            Ok(event) => output(
                FrontendStatus::Complete,
                Some(FiniteExperimentRequest::Single(UniformDieRequest {
                    sides,
                    event,
                    provenance: provenance.clone(),
                })),
                Vec::new(),
                provenance,
            ),
            Err(status) => output(
                status,
                None,
                vec!["exactly one supported event predicate is required".into()],
                provenance,
            ),
        };
    }
    if lower.contains("labeled") && !lower.contains("labels 1") && !lower.contains("numbered 1") {
        return output(
            FrontendStatus::Unsupported,
            None,
            vec!["labeled two-die outcomes require explicit contiguous integer labels".into()],
            provenance,
        );
    }
    match one_joint_predicate(&lower) {
        Ok(predicate) => output(
            FrontendStatus::Complete,
            Some(FiniteExperimentRequest::Two { sides, predicate }),
            Vec::new(),
            provenance,
        ),
        Err(status) => output(
            status,
            None,
            vec!["exactly one supported joint event is required".into()],
            provenance,
        ),
    }
}

fn is_prime(value: i64) -> bool {
    if value < 2 {
        return false;
    }
    (2..=((value as f64).sqrt() as i64)).all(|divisor| value % divisor != 0)
}

fn matches_joint(predicate: &JointPredicate, left: i64, right: i64) -> bool {
    match predicate {
        JointPredicate::SumEquals(value) => left + right == *value,
        JointPredicate::SumLessThan(value) => left + right < *value,
        JointPredicate::SumGreaterThan(value) => left + right > *value,
        JointPredicate::AtLeastOneEquals(value) => left == *value || right == *value,
        JointPredicate::DifferenceEquals(value) => (left - right).abs() == *value,
        JointPredicate::ProductEven => (left * right) % 2 == 0,
        JointPredicate::ProductOdd => (left * right) % 2 != 0,
        JointPredicate::SumPrime => is_prime(left + right),
    }
}

fn execute_single(request: &UniformDieRequest) -> ExecutionResult {
    let result: SingleExecutionResult = execute_single_die(request);
    finish_execution(ExecutionResult {
        status: status_from_single(result.status),
        value: result.value,
        assumptions: result.assumptions,
        reasons: result.reasons,
        provenance: result.provenance,
        replay_hash: String::new(),
    })
}

fn execute_two(sides: u32, predicate: &JointPredicate, provenance: Vec<String>) -> ExecutionResult {
    let total = (sides as i128) * (sides as i128);
    let probabilities = (0..total)
        .map(|_| Rational::new(1, total).unwrap())
        .collect::<Vec<_>>();
    let outcomes = (1..=sides)
        .flat_map(|left| (1..=sides).map(move |right| format!("({left},{right})")))
        .collect::<Vec<_>>();
    let event = (1..=sides)
        .flat_map(|left| (1..=sides).map(move |right| (left, right)))
        .enumerate()
        .filter_map(|(index, (left, right))| {
            matches_joint(predicate, left as i64, right as i64).then_some(index)
        })
        .collect::<Vec<_>>();
    let complement = (0..outcomes.len())
        .filter(|index| !event.contains(index))
        .collect::<Vec<_>>();
    let request = ProbabilityRequest {
        operation: ProbabilityOperation::Complement,
        domain: "finite_exact_probability".into(),
        outcomes,
        probabilities,
        values: Vec::new(),
        event_a: Some(complement),
        event_b: None,
        partition: Vec::new(),
        conditional_values: Vec::new(),
        prior_probability: None,
        likelihood: None,
        evidence: None,
        ambiguity: None,
        provenance: provenance.clone(),
    };
    let result = evaluate_probability(&request);
    let value = match result.artifact {
        Some(ProbabilityArtifact::Scalar(value)) => Some(value),
        _ => None,
    };
    finish_execution(ExecutionResult {
        status: match result.status {
            ProbabilityStatus::Complete => FrontendStatus::Complete,
            ProbabilityStatus::Ambiguous => FrontendStatus::Ambiguous,
            ProbabilityStatus::Missing => FrontendStatus::Missing,
            _ => FrontendStatus::Unsupported,
        },
        value,
        assumptions: result.assumptions,
        reasons: result.reasons,
        provenance,
        replay_hash: String::new(),
    })
}

pub fn execute(result: &FrontendResult) -> Option<ExecutionResult> {
    if result.status != FrontendStatus::Complete || !replay_verified(result) {
        return None;
    }
    let request = result.request.as_ref()?;
    Some(match request {
        FiniteExperimentRequest::Single(request) => execute_single(request),
        FiniteExperimentRequest::Two { sides, predicate } => {
            execute_two(*sides, predicate, result.provenance.clone())
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_dice_sum_is_exact_and_replayable() {
        let result = formalize(
            "Two fair six-sided dice are rolled. What is the probability that their sum is 7?",
            "test-sum",
        );
        assert_eq!(result.status, FrontendStatus::Complete);
        assert!(replay_verified(&result));
        let execution = execute(&result).unwrap();
        assert_eq!(execution.value, Some(Rational::new(1, 6).unwrap()));
        assert!(execution_replay_verified(&execution));
        let mut tampered = execution.clone();
        tampered.replay_hash.push('x');
        assert!(!execution_replay_verified(&tampered));
    }

    #[test]
    fn two_dice_without_explicit_fairness_is_ambiguous() {
        let result = formalize(
            "Two six-sided dice are rolled. What is the probability that their sum is 7?",
            "test-ambiguous",
        );
        assert_eq!(result.status, FrontendStatus::Ambiguous);
        assert!(replay_verified(&result));
        assert!(execute(&result).is_none());
    }

    #[test]
    fn multi_die_and_repeated_events_are_unsupported() {
        for text in [
            "Three fair six-sided dice are rolled. What is the probability that the sum is 9?",
            "A fair six-sided die is rolled repeatedly. What is the probability of a six?",
        ] {
            let result = formalize(text, "test-unsupported");
            assert_eq!(result.status, FrontendStatus::Unsupported);
            assert!(replay_verified(&result));
            assert!(execute(&result).is_none());
        }
    }

    #[test]
    fn single_die_reuses_validated_frontend() {
        let result = formalize(
            "A fair six-sided die is rolled. What is the probability of an even outcome?",
            "test-single",
        );
        assert_eq!(result.status, FrontendStatus::Complete);
        assert_eq!(
            execute(&result).unwrap().value,
            Some(Rational::new(1, 2).unwrap())
        );
    }
}
