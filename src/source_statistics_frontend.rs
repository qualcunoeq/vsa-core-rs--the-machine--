//! Controlled technical-language frontend for the source-derived statistics
//! catalog. It extracts only explicitly labeled quantities and fails closed on
//! underspecified or unsupported statistical language.

use crate::probability_pack::Rational;
use crate::source_formula_pack::FormulaRequest;
use crate::source_statistics_pack::DOMAIN;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FrontendStatus {
    Complete,
    Ambiguous,
    Unsupported,
    Missing,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StatisticsFrontendResult {
    pub status: FrontendStatus,
    pub formula: Option<String>,
    pub request: Option<FormulaRequest>,
    pub provenance_spans: Vec<String>,
    pub alternatives: Vec<String>,
    pub reasons: Vec<String>,
    pub replay_hash: String,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(value).expect("frontend serializes"))
    )
}

fn payload(result: &StatisticsFrontendResult) -> impl Serialize + '_ {
    (
        result.status,
        &result.formula,
        &result.request,
        &result.provenance_spans,
        &result.alternatives,
        &result.reasons,
    )
}

fn rational_token(token: &str) -> Option<Rational> {
    let cleaned = token.trim_matches(|character: char| {
        !character.is_ascii_digit() && character != '-' && character != '/'
    });
    if let Some((whole, fraction)) = cleaned.split_once('.') {
        if whole.is_empty() || fraction.is_empty() || !fraction.chars().all(|c| c.is_ascii_digit())
        {
            return None;
        }
        let sign = if whole.starts_with('-') { -1 } else { 1 };
        let whole_digits = whole.trim_start_matches('-');
        let numerator = whole_digits.parse::<i128>().ok()? * sign;
        let scale = 10_i128.checked_pow(fraction.len() as u32)?;
        let fractional = fraction.parse::<i128>().ok()? * sign;
        return Rational::new(numerator * scale + fractional, scale);
    }
    if let Some((numerator, denominator)) = cleaned.split_once('/') {
        return Rational::new(numerator.parse().ok()?, denominator.parse().ok()?);
    }
    Rational::new(cleaned.parse().ok()?, 1)
}

fn parse_explicit_list(segment: &str) -> Option<Vec<Rational>> {
    let normalized = segment
        .replace(['{', '}', '[', ']'], " ")
        .replace(" and ", ",")
        .replace(" AND ", ",");
    let mut values = Vec::new();
    for item in normalized.split(',') {
        let item = item.trim();
        if item.is_empty() {
            continue;
        }
        let tokens: Vec<&str> = item.split_whitespace().collect();
        if tokens.len() != 1 {
            return None;
        }
        values.push(rational_token(tokens[0])?);
    }
    (values.len() >= 2).then_some(values)
}

/// Parse a small, explicit count written either as an integer or a bounded
/// cardinal word.  This is deliberately not a general number-word parser:
/// the frontend only needs finite collection sizes for the source-backed
/// arithmetic-mean relation.
fn finite_count_token(token: &str) -> Option<Rational> {
    let normalized = token
        .trim_matches(|character: char| !character.is_ascii_alphanumeric() && character != '-')
        .to_ascii_lowercase();
    let value = match normalized.as_str() {
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
        "seventeen" => 17,
        "eighteen" => 18,
        "nineteen" => 19,
        "twenty" => 20,
        _ => return rational_token(&normalized),
    };
    Rational::new(value, 1)
}

/// Parse a bounded written fraction used as an explicitly declared total.
/// Unknown prose is rejected instead of being interpreted as a number.
fn finite_total_token(token: &str) -> Option<Rational> {
    let normalized = token
        .trim_matches(|character: char| {
            !character.is_ascii_alphanumeric() && character != '/' && character != '-'
        })
        .to_ascii_lowercase();
    match normalized.as_str() {
        "half" | "a-half" | "one-half" => Rational::new(1, 2),
        "quarter" | "a-quarter" | "one-quarter" => Rational::new(1, 4),
        "three-quarters" => Rational::new(3, 4),
        "third" | "one-third" => Rational::new(1, 3),
        _ => rational_token(&normalized),
    }
}

/// Recognize only the explicit finite relation
/// `the sum of N <items> is S ... mean`.  It supplies the same typed
/// `sum`/`count` inputs as the source-declared arithmetic-mean record and does
/// not infer individual observations, ranges, filters, or distributions.
fn natural_sum_count_mean(text: &str) -> Option<(BTreeMap<String, Rational>, String)> {
    let lower = text.to_ascii_lowercase();
    if !(lower.contains("mean") || lower.contains("average")) {
        return None;
    }
    let sum_start = lower.find("sum of")? + "sum of".len();
    let sentence_end = text[sum_start..]
        .find(|character: char| matches!(character, '.' | '?' | ';'))
        .map(|offset| sum_start + offset)
        .unwrap_or(text.len());
    let clause = &text[sum_start..sentence_end];
    let lower_clause = clause.to_ascii_lowercase();
    let is_offset = lower_clause.find(" is ")?;
    let left = clause[..is_offset].trim();
    let right = clause[is_offset + " is ".len()..].trim();
    let mut left_tokens = left.split_whitespace();
    if left_tokens.clone().next() == Some("the") {
        // `the` is a determiner, not part of the count.
        left_tokens.next();
    }
    let count_token = left_tokens.next()?;
    let count = finite_count_token(count_token)?;
    if count.numerator <= 0 || count.denominator != 1 {
        return None;
    }
    // Require a collection noun after the count.  This prevents a generic
    // phrase such as `sum of x is ...` from becoming a finite mean request.
    let collection = left_tokens
        .next()?
        .trim_matches(|character: char| !character.is_ascii_alphabetic());
    const COLLECTIONS: &[&str] = &[
        "number",
        "numbers",
        "value",
        "values",
        "term",
        "terms",
        "scores",
        "observations",
        "items",
        "quantities",
        "measurements",
    ];
    if !COLLECTIONS.contains(&collection.to_ascii_lowercase().as_str()) {
        return None;
    }
    let total_token = right.split_whitespace().next()?;
    let total = finite_total_token(total_token)?;
    Some((
        BTreeMap::from([(String::from("sum"), total), (String::from("count"), count)]),
        format!("natural-sum-count-span:{}..{}", sum_start, sentence_end),
    ))
}

/// Extract a finite numeric enumeration only when the surrounding clause is
/// list-shaped. Arbitrary numbers in a word problem are never treated as
/// observations by this helper.
fn parse_natural_numeric_list(segment: &str) -> Option<Vec<Rational>> {
    let lower = segment.to_ascii_lowercase();
    let rejected = [
        "graph",
        "table",
        "[asy]",
        "prime",
        "possible",
        "variable",
        "unknown",
        "from",
        "through",
        "increase",
        "decrease",
        "change",
        "average speed",
        "average rate",
    ];
    if rejected.iter().any(|marker| lower.contains(marker)) {
        return None;
    }

    let mut values = Vec::new();
    let mut residual = String::with_capacity(segment.len());
    let chars: Vec<char> = segment.chars().collect();
    let mut index = 0;
    while index < chars.len() {
        let starts_number = chars[index].is_ascii_digit()
            || (chars[index] == '-'
                && chars
                    .get(index + 1)
                    .is_some_and(|character| character.is_ascii_digit()));
        if !starts_number {
            residual.push(chars[index]);
            index += 1;
            continue;
        }
        let start = index;
        if chars[index] == '-' {
            index += 1;
        }
        while chars
            .get(index)
            .is_some_and(|character| character.is_ascii_digit())
        {
            index += 1;
        }
        if chars.get(index) == Some(&'.')
            && chars
                .get(index + 1)
                .is_some_and(|character| character.is_ascii_digit())
        {
            index += 1;
            while chars
                .get(index)
                .is_some_and(|character| character.is_ascii_digit())
            {
                index += 1;
            }
        }
        if chars.get(index) == Some(&'/')
            && chars
                .get(index + 1)
                .is_some_and(|character| character.is_ascii_digit())
        {
            index += 1;
            while chars
                .get(index)
                .is_some_and(|character| character.is_ascii_digit())
            {
                index += 1;
            }
        }
        let token: String = chars[start..index].iter().collect();
        values.push(rational_token(&token)?);
        residual.push(' ');
    }
    if values.len() < 2 {
        return None;
    }
    let allowed_words = [
        "and",
        "degrees",
        "degree",
        "circ",
        "fahrenheit",
        "celsius",
        "scores",
        "temperatures",
        "values",
        "were",
        "are",
    ];
    if residual
        .split(|character: char| !character.is_ascii_alphabetic())
        .filter(|word| !word.is_empty())
        .any(|word| !allowed_words.contains(&word.to_ascii_lowercase().as_str()))
    {
        return None;
    }
    Some(values)
}

fn natural_numeric_list_mean(text: &str) -> Option<(Vec<Rational>, String)> {
    let lower = text.to_ascii_lowercase();
    let nouns = ["scores", "temperatures", "values"];
    let (noun_start, noun) = nouns
        .iter()
        .filter_map(|noun| lower.find(noun).map(|start| (start, *noun)))
        .min_by_key(|(start, _)| *start)?;
    let after_noun = noun_start + noun.len();
    let (relation_offset, relation) = [" were", " are"]
        .iter()
        .filter_map(|relation| {
            lower[after_noun..]
                .find(relation)
                .map(|offset| (offset, *relation))
        })
        .min_by_key(|(offset, _)| *offset)?;
    let start = after_noun + relation_offset + relation.len();
    let rest = &text[start..];
    let end = rest
        .find(|character: char| matches!(character, '.' | '?' | ';'))
        .unwrap_or(rest.len());
    let segment = rest[..end].trim();
    let values = parse_natural_numeric_list(segment)?;
    Some((
        values,
        format!("natural-list-span:{}..{}", start, start + end),
    ))
}

fn single_symbol(token: &str) -> Option<String> {
    let symbol: String = token
        .chars()
        .filter(|character| character.is_ascii_alphabetic())
        .collect();
    (symbol.len() == 1).then_some(symbol)
}

/// Recognize only an explicitly stated equality of two finite means with one
/// unknown additive value on the right. This is a source-backed equation
/// contract, not a general symbolic equation solver.
fn mean_equality_unknown(text: &str) -> Option<(BTreeMap<String, Rational>, String)> {
    let lower = text.to_ascii_lowercase();
    let mean_pos = lower.find("mean")?;
    let left_of = lower[mean_pos..].find(" of ")? + mean_pos + 4;
    let connector = [" is equal to the mean", " is equal to the average"]
        .iter()
        .filter_map(|marker| {
            lower[left_of..]
                .find(marker)
                .map(|offset| (offset, *marker))
        })
        .min_by_key(|(offset, _)| *offset)?;
    let connector_start = left_of + connector.0;
    let left_segment = text[left_of..connector_start].trim();
    let left_values = parse_explicit_list(&left_segment.replace(" and ", ","))?;
    let right_start = connector_start + connector.1.len();
    let right_of = lower[right_start..].find(" of ")? + right_start + 4;
    let right_end = text[right_of..]
        .find(|character: char| matches!(character, '.' | '?' | ';'))
        .map(|offset| right_of + offset)
        .unwrap_or(text.len());
    let right_segment = text[right_of..right_end]
        .replace(" and ", ",")
        .replace(" AND ", ",");
    let right_items: Vec<&str> = right_segment
        .split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .collect();
    if right_items.len() != 2 {
        return None;
    }
    let mut known_sum = None;
    let mut unknown_symbol = None;
    for item in right_items {
        if let Some(value) = rational_token(item) {
            if known_sum.is_some() {
                return None;
            }
            known_sum = Some(value);
        } else if let Some(symbol) = single_symbol(item) {
            if unknown_symbol.is_some() {
                return None;
            }
            unknown_symbol = Some(symbol);
        } else {
            return None;
        }
    }
    let known_sum = known_sum?;
    let unknown_symbol = unknown_symbol?;
    let inputs = BTreeMap::from([
        (
            "left_sum".into(),
            left_values
                .iter()
                .fold(Rational::zero(), |acc, value| acc.add(value).unwrap()),
        ),
        (
            "left_count".into(),
            Rational::new(left_values.len() as i128, 1).expect("left list is non-empty"),
        ),
        ("right_known_sum".into(), known_sum),
        (
            "right_count".into(),
            Rational::new(2, 1).expect("right list has two entries"),
        ),
    ]);
    Some((
        inputs,
        format!(
            "mean-equality-span:{}..{};unknown:{}",
            left_of, right_end, unknown_symbol
        ),
    ))
}

/// Parse only an explicitly enumerated finite list whose arithmetic mean is
/// requested.  This bridge deliberately refuses ranges, filtering, symbolic
/// entries, and optimization/constraint problems; those require separate
/// semantics rather than a convenient sum/count guess.
pub fn formalize_finite_list_mean_text(text: &str) -> StatisticsFrontendResult {
    let lower = text.to_ascii_lowercase();
    if !(lower.contains("mean") || lower.contains("average")) {
        return result(
            FrontendStatus::Missing,
            None,
            None,
            vec![text.into()],
            Vec::new(),
            vec!["no mean or average target was stated".into()],
        );
    }
    let rejected_semantics = [
        "from",
        "through",
        "prime",
        "multiple",
        "positive",
        "negative",
        "median",
        "largest",
        "smallest",
        "expression",
        "variable",
        "unknown",
        "reciprocal",
        "added to",
        "list becomes",
    ];
    if rejected_semantics
        .iter()
        .any(|marker| lower.contains(marker))
    {
        return result(
            FrontendStatus::Unsupported,
            None,
            None,
            vec![text.into()],
            Vec::new(),
            vec![
                "mean request requires range, filtering, symbolic, or optimization semantics"
                    .into(),
            ],
        );
    }
    if lower.contains("by how much")
        || lower.contains("average increase")
        || lower.contains("average decrease")
        || lower.contains("average speed")
        || lower.contains("average rate")
    {
        return result(
            FrontendStatus::Unsupported,
            None,
            None,
            vec![text.into()],
            Vec::new(),
            vec!["request asks for a derived change or rate, not a finite-list mean".into()],
        );
    }
    if let Some((inputs, span)) = mean_equality_unknown(text) {
        return with_request("mean_equality_unknown", inputs, vec![span]);
    }
    if let Some((inputs, span)) = natural_sum_count_mean(text) {
        return with_request("arithmetic_mean", inputs, vec![span]);
    }
    if let Some((values, span)) = natural_numeric_list_mean(text) {
        let sum = values.iter().fold(Rational::zero(), |acc, value| {
            acc.add(value).expect("finite rational sum remains exact")
        });
        let count = Rational::new(values.len() as i128, 1).expect("non-empty list");
        return with_request(
            "arithmetic_mean",
            BTreeMap::from([("sum".into(), sum), ("count".into(), count)]),
            vec![span],
        );
    }
    let candidates = if let (Some(start), Some(end)) = (lower.find('{'), lower.rfind('}')) {
        (end > start).then(|| &text[start..=end])
    } else if let Some(start) = lower.find("scores are") {
        let start = start + "scores are".len();
        let end = text[start..]
            .find(|character: char| matches!(character, '?' | '.' | ';'))
            .map(|offset| start + offset)
            .unwrap_or(text.len());
        Some(&text[start..end])
    } else if let Some(start) = lower.find("mean of") {
        let start = start + "mean of".len();
        let end = text[start..]
            .find(|character: char| matches!(character, '?' | '.' | ';'))
            .map(|offset| start + offset)
            .unwrap_or(text.len());
        Some(&text[start..end])
    } else if let Some(start) = lower.find("average of") {
        let start = start + "average of".len();
        let end = text[start..]
            .find(|character: char| matches!(character, '?' | '.' | ';'))
            .map(|offset| start + offset)
            .unwrap_or(text.len());
        Some(&text[start..end])
    } else {
        None
    };
    let Some(segment) = candidates else {
        return result(
            FrontendStatus::Ambiguous,
            None,
            None,
            vec![text.into()],
            vec!["arithmetic_mean".into()],
            vec!["mean target exists but no explicit finite list was located".into()],
        );
    };
    let segment_end = ["what is", "what's", "calculate", "compute", "find "]
        .iter()
        .filter_map(|marker| segment.to_ascii_lowercase().find(marker))
        .min();
    let segment = segment_end.map(|end| &segment[..end]).unwrap_or(segment);
    let structural_segment = segment.replace(" and ", ",");
    if structural_segment
        .chars()
        .any(|character| character.is_ascii_alphabetic())
    {
        let lower_segment = structural_segment.to_ascii_lowercase();
        let symbolic = lower_segment.contains('x')
            || lower_segment.contains('+')
            || lower_segment.contains("variable")
            || lower_segment.contains("expression");
        if !symbolic {
            return result(
                FrontendStatus::Ambiguous,
                None,
                None,
                vec![text.into()],
                vec!["arithmetic_mean".into()],
                vec!["a mean target exists but the list contents are not explicit".into()],
            );
        }
        return result(
            FrontendStatus::Unsupported,
            None,
            None,
            vec![text.into()],
            Vec::new(),
            vec!["symbolic or word-valued list entries require another capability".into()],
        );
    }
    let Some(values) = parse_explicit_list(segment) else {
        return result(
            FrontendStatus::Ambiguous,
            None,
            None,
            vec![text.into()],
            vec!["arithmetic_mean".into()],
            vec!["list syntax is not an explicit finite numeric enumeration".into()],
        );
    };
    let sum = values
        .iter()
        .fold(Rational::new(0, 1).expect("zero is valid"), |acc, value| {
            acc.add(value).unwrap()
        });
    let count = Rational::new(values.len() as i128, 1).expect("list count is positive");
    with_request(
        "arithmetic_mean",
        BTreeMap::from([("sum".into(), sum), ("count".into(), count)]),
        vec![format!("explicit-list-span:{segment}")],
    )
}

fn labeled_value(text: &str, labels: &[&str]) -> Option<(String, Rational)> {
    let tokens: Vec<&str> = text.split_whitespace().collect();
    for (index, token) in tokens.iter().enumerate() {
        let normalized = token.trim_matches(|character: char| {
            !character.is_ascii_alphanumeric()
                && character != '_'
                && character != '='
                && character != '-'
        });
        for label in labels {
            if normalized == *label {
                // Accept an explicit separator as its own token, but never
                // infer a value from unlabeled prose.  This covers
                // `sum = 30` and `sum : 30` while retaining the same
                // fail-closed label boundary as `sum=30`.
                let mut value_index = index + 1;
                while matches!(tokens.get(value_index), Some(&"=" | &":")) {
                    value_index += 1;
                }
                if let Some(next) = tokens
                    .get(value_index)
                    .and_then(|value| rational_token(value))
                {
                    return (
                        format!("{label} {}", tokens[index + 1..=value_index].join(" ")),
                        next,
                    )
                        .into();
                }
            }
            let prefix = format!("{label}=");
            if let Some(value) = normalized.strip_prefix(&prefix) {
                if let Some(parsed) = rational_token(value) {
                    return (normalized.to_string(), parsed).into();
                }
            }
            let prefix = format!("{label}:");
            if let Some(value) = normalized.strip_prefix(&prefix) {
                if let Some(parsed) = rational_token(value) {
                    return (normalized.to_string(), parsed).into();
                }
            }
        }
    }
    None
}

fn result(
    status: FrontendStatus,
    formula: Option<String>,
    request: Option<FormulaRequest>,
    spans: Vec<String>,
    alternatives: Vec<String>,
    reasons: Vec<String>,
) -> StatisticsFrontendResult {
    let mut output = StatisticsFrontendResult {
        status,
        formula,
        request,
        provenance_spans: spans,
        alternatives,
        reasons,
        replay_hash: String::new(),
    };
    let replay_hash = digest(&payload(&output));
    output.replay_hash = replay_hash;
    output
}

fn with_request(
    formula: &str,
    inputs: BTreeMap<String, Rational>,
    spans: Vec<String>,
) -> StatisticsFrontendResult {
    result(
        FrontendStatus::Complete,
        Some(formula.into()),
        Some(FormulaRequest {
            formula: formula.into(),
            inputs,
            domain: DOMAIN.into(),
            ambiguity: None,
            provenance: spans.clone(),
        }),
        spans,
        Vec::new(),
        Vec::new(),
    )
}

/// Parse a deliberately bounded set of labeled finite-statistics statements.
/// Unlabeled prose is never converted into a fact by lexical resemblance.
pub fn formalize_statistics_text(text: &str) -> StatisticsFrontendResult {
    let lower = text.to_ascii_lowercase();
    let unsupported_marker = [
        "continuous",
        "sample standard deviation",
        "confidence interval",
        "hypothesis test",
        "regression",
        "normal distribution",
        "density",
    ];
    if unsupported_marker
        .iter()
        .any(|marker| lower.contains(marker))
    {
        return result(
            FrontendStatus::Unsupported,
            None,
            None,
            vec![text.into()],
            Vec::new(),
            vec!["request is outside the finite source-statistics catalog".into()],
        );
    }
    let has_weighted = lower.contains("weighted") || lower.contains("weight");
    let has_mean = lower.contains("mean") || lower.contains("average");
    let has_bernoulli = lower.contains("bernoulli") || lower.contains("binary outcome");
    let has_binomial = lower.contains("binomial");
    if has_mean && has_weighted {
        let weighted_sum = labeled_value(&lower, &["weighted_sum", "weighted-sum"]);
        let total_weight = labeled_value(&lower, &["total_weight", "total-weight"]);
        if let (Some((sum_span, sum)), Some((weight_span, weight))) = (weighted_sum, total_weight) {
            return with_request(
                "weighted_mean",
                BTreeMap::from([
                    ("weighted_sum".into(), sum),
                    ("total_weight".into(), weight),
                ]),
                vec![sum_span, weight_span],
            );
        }
    }
    if has_mean && !has_weighted {
        let sum = labeled_value(&lower, &["sum"]);
        let count = labeled_value(&lower, &["count", "n"]);
        if let (Some((sum_span, sum)), Some((count_span, count))) = (sum, count) {
            return with_request(
                "arithmetic_mean",
                BTreeMap::from([("sum".into(), sum), ("count".into(), count)]),
                vec![sum_span, count_span],
            );
        }
    }
    if has_bernoulli && lower.contains("variance") {
        if let Some((span, probability)) = labeled_value(&lower, &["p", "probability"]) {
            return with_request(
                "bernoulli_variance",
                BTreeMap::from([("p".into(), probability)]),
                vec![span],
            );
        }
    }
    if has_binomial {
        let n = labeled_value(&lower, &["n", "trials"]);
        let probability = labeled_value(&lower, &["p", "probability"]);
        let formula = if lower.contains("variance") {
            "binomial_variance"
        } else if lower.contains("expected") || lower.contains("mean") {
            "binomial_expected_value"
        } else {
            return result(
                FrontendStatus::Ambiguous,
                None,
                None,
                vec![text.into()],
                vec!["binomial_expected_value".into(), "binomial_variance".into()],
                vec!["requested binomial output is not identified".into()],
            );
        };
        if let (Some((n_span, n)), Some((p_span, probability))) = (n, probability) {
            return with_request(
                formula,
                BTreeMap::from([("n".into(), n), ("p".into(), probability)]),
                vec![n_span, p_span],
            );
        }
    }
    if has_mean {
        return result(
            FrontendStatus::Ambiguous,
            None,
            None,
            vec![text.into()],
            vec!["arithmetic_mean".into(), "weighted_mean".into()],
            vec!["mean is present but the required labeled quantities do not identify one formulation".into()],
        );
    }
    result(
        FrontendStatus::Missing,
        None,
        None,
        vec![text.into()],
        Vec::new(),
        vec!["no supported finite-statistics target was identified".into()],
    )
}

impl StatisticsFrontendResult {
    pub fn replay_verified(&self) -> bool {
        self.replay_hash == digest(&payload(self))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labeled_mean_is_typed_and_ambiguous_mean_is_preserved() {
        let complete = formalize_statistics_text("Find the mean from sum=30 and count=5.");
        assert_eq!(complete.status, FrontendStatus::Complete);
        assert_eq!(complete.formula.as_deref(), Some("arithmetic_mean"));
        assert!(complete.replay_verified());
        let shifted = formalize_statistics_text("Using count : 5, compute the mean from sum = 30.");
        assert_eq!(shifted.status, FrontendStatus::Complete);
        assert_eq!(shifted.formula.as_deref(), Some("arithmetic_mean"));
        assert!(shifted.replay_verified());
        let ambiguous = formalize_statistics_text("Find the average from total=30 and count=5.");
        assert_eq!(ambiguous.status, FrontendStatus::Ambiguous);
        assert!(ambiguous.replay_verified());
    }

    #[test]
    fn explicit_list_mean_lowers_without_inventing_range_or_filter_semantics() {
        let complete = formalize_finite_list_mean_text(
            "Jeff's scores are 89, 92, 88, 95 and 91. What is the arithmetic mean?",
        );
        assert_eq!(complete.status, FrontendStatus::Complete);
        let request = complete.request.as_ref().unwrap();
        assert_eq!(request.inputs["sum"], Rational::new(455, 1).unwrap());
        assert_eq!(request.inputs["count"], Rational::new(5, 1).unwrap());
        assert!(complete.replay_verified());

        let external_style = formalize_finite_list_mean_text(
            "A learner's four quiz scores are 81, 87, 94 and 98. What is the arithmetic mean of these four scores?",
        );
        assert_eq!(
            external_style.status,
            FrontendStatus::Complete,
            "{external_style:?}"
        );

        let temperature_list = formalize_finite_list_mean_text(
            "The noon temperatures for seven consecutive days were 80°, 79°, 81°, 85°, 87°, 89°, and 87° Fahrenheit. What is the mean noon temperature?",
        );
        assert_eq!(temperature_list.status, FrontendStatus::Complete);
        let temperature_request = temperature_list.request.as_ref().unwrap();
        assert_eq!(
            temperature_request.inputs["sum"],
            Rational::new(588, 1).unwrap()
        );
        assert_eq!(
            temperature_request.inputs["count"],
            Rational::new(7, 1).unwrap()
        );
        assert!(temperature_list.replay_verified());

        let latex_temperature_list = formalize_finite_list_mean_text(
            r#"The noon temperatures for seven consecutive days were $80^{\circ}$, $79^{\circ}$, $81^{\circ}$, $85^{\circ}$, $87^{\circ}$, $89^{\circ}$, and $87^{\circ}$ Fahrenheit. What is the mean noon temperature?"#,
        );
        assert_eq!(latex_temperature_list.status, FrontendStatus::Complete);
        assert!(latex_temperature_list.replay_verified());

        let derived_change = formalize_finite_list_mean_text(
            "Scores were 87, 83, and 88. By how much will the average increase after a score of 90?",
        );
        assert_eq!(derived_change.status, FrontendStatus::Unsupported);
        assert!(derived_change.replay_verified());

        let range = formalize_finite_list_mean_text(
            "What is the arithmetic mean of the integers from -4 through 5?",
        );
        assert_eq!(range.status, FrontendStatus::Unsupported);
        assert!(range.replay_verified());

        let symbolic =
            formalize_finite_list_mean_text("The arithmetic mean of x + 8, 15, and 2x is 24.");
        assert_ne!(symbolic.status, FrontendStatus::Complete);
        assert!(symbolic.replay_verified());
    }

    #[test]
    fn finite_mean_equality_binds_one_unknown_without_general_symbolic_solving() {
        let complete = formalize_finite_list_mean_text(
            "The mean (average) of 6, 9 and 18 is equal to the mean (average) of 12 and y. What is the value of y?",
        );
        assert_eq!(complete.status, FrontendStatus::Complete, "{complete:?}");
        assert_eq!(complete.formula.as_deref(), Some("mean_equality_unknown"));
        let request = complete.request.as_ref().unwrap();
        assert_eq!(request.inputs["left_sum"], Rational::new(33, 1).unwrap());
        assert_eq!(request.inputs["left_count"], Rational::new(3, 1).unwrap());
        assert_eq!(
            request.inputs["right_known_sum"],
            Rational::new(12, 1).unwrap()
        );
        assert_eq!(request.inputs["right_count"], Rational::new(2, 1).unwrap());
        assert!(complete.replay_verified());

        let alternate = formalize_finite_list_mean_text(
            "The mean of 5,8 and 17 is equal to the mean of 12 and y. What is the value of y?",
        );
        assert_eq!(alternate.status, FrontendStatus::Complete, "{alternate:?}");
        assert!(alternate.replay_verified());

        let ambiguous = formalize_finite_list_mean_text(
            "The mean of a list is equal to the mean of another list. Find the unknown value.",
        );
        assert_ne!(ambiguous.status, FrontendStatus::Complete);
        assert!(ambiguous.replay_verified());
    }

    #[test]
    fn natural_sum_and_count_mean_binds_declared_total_without_inventing_values() {
        let complete = formalize_finite_list_mean_text(
            "The sum of four numbers is one-half. What is the mean of the four numbers?",
        );
        assert_eq!(complete.status, FrontendStatus::Complete, "{complete:?}");
        assert_eq!(complete.formula.as_deref(), Some("arithmetic_mean"));
        let request = complete.request.as_ref().expect("sum/count request");
        assert_eq!(request.inputs["sum"], Rational::new(1, 2).unwrap());
        assert_eq!(request.inputs["count"], Rational::new(4, 1).unwrap());
        assert!(complete.replay_verified());

        let numeric =
            formalize_finite_list_mean_text("The sum of 6 values is 30. Find their average.");
        assert_eq!(numeric.status, FrontendStatus::Complete, "{numeric:?}");
        assert_eq!(
            numeric.request.unwrap().inputs["sum"],
            Rational::new(30, 1).unwrap()
        );
    }

    #[test]
    fn natural_sum_count_mean_rejects_unstated_collection_or_total() {
        for text in [
            "The sum of x is 30. Find the mean.",
            "The sum of four numbers is unknown. Find the mean.",
            "The sum of four probabilities is one-half. Find the mean.",
        ] {
            let result = formalize_finite_list_mean_text(text);
            assert_ne!(result.status, FrontendStatus::Complete, "{text}");
            assert!(result.replay_verified());
        }
    }
}
