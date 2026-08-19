//! Bounded language-to-request frontend for source-derived sequences.
//!
//! This frontend only recognizes explicitly stated finite arithmetic and
//! geometric sequence inputs.  It emits a typed formula request; it never
//! evaluates a formula or authorizes an answer.

use crate::probability_pack::Rational;
use crate::source_formula_pack::FormulaRequest;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SequenceFrontendStatus {
    Complete,
    Ambiguous,
    Missing,
    Unsupported,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SequenceFrontendResult {
    pub status: SequenceFrontendStatus,
    pub request: Option<FormulaRequest>,
    pub evidence: Vec<String>,
    pub unresolved: Vec<String>,
    pub provenance: Vec<String>,
    pub replay_hash: String,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn rational(value: &str) -> Option<Rational> {
    value
        .trim()
        .parse::<i128>()
        .ok()
        .map(|n| Rational::new(n, 1).unwrap())
}

fn integer_token(value: &str) -> Option<Rational> {
    let cleaned = value.trim_matches(|character: char| {
        !character.is_ascii_digit() && character != '-'
    });
    cleaned
        .parse::<i128>()
        .ok()
        .map(|number| Rational::new(number, 1).unwrap())
}

fn ordinal_word(value: &str) -> Option<i128> {
    let normalized = value.trim_matches(|character: char| !character.is_ascii_alphabetic());
    let direct = [
        ("first", 1),
        ("second", 2),
        ("third", 3),
        ("fourth", 4),
        ("fifth", 5),
        ("sixth", 6),
        ("seventh", 7),
        ("eighth", 8),
        ("ninth", 9),
        ("tenth", 10),
        ("eleventh", 11),
        ("twelfth", 12),
        ("thirteenth", 13),
        ("fourteenth", 14),
        ("fifteenth", 15),
        ("sixteenth", 16),
        ("seventeenth", 17),
        ("eighteenth", 18),
        ("nineteenth", 19),
        ("twentieth", 20),
        ("thirtieth", 30),
        ("fortieth", 40),
        ("fiftieth", 50),
        ("sixtieth", 60),
        ("seventieth", 70),
        ("eightieth", 80),
        ("ninetieth", 90),
    ];
    direct
        .iter()
        .find(|(word, _)| *word == normalized)
        .map(|(_, number)| *number)
}

fn ordinal_number(text: &str) -> Option<i128> {
    for token in text.split_whitespace() {
        let trimmed: String = token
            .chars()
            .filter(|character| character.is_ascii_alphanumeric() || *character == '-')
            .collect();
        let digits = trimmed.trim_end_matches(|character: char| {
            matches!(character, 's' | 't' | 'n' | 'd' | 'r' | 'h')
        });
        if digits != trimmed.as_str() {
            if let Ok(number) = digits.parse::<i128>() {
                return Some(number);
            }
        }
        if let Some(number) = ordinal_word(&trimmed) {
            return Some(number);
        }
    }
    None
}

/// Find an ordinal that is syntactically attached to the requested `term`.
/// Earlier terms in a statement (for example, "first and thirteenth terms")
/// are definitions, not the requested output.  If no term-local ordinal is
/// present we refuse rather than treating an incidental ordinal as the target.
fn requested_ordinal(text: &str) -> Option<i128> {
    let tokens: Vec<&str> = text.split_whitespace().collect();
    let mut candidates = Vec::new();
    for (index, token) in tokens.iter().enumerate() {
        let normalized: String = token
            .chars()
            .filter(|character| character.is_ascii_alphabetic())
            .collect();
        if !normalized.starts_with("term") {
            continue;
        }
        // A definition such as "the first three terms ... are" or "the
        // first and thirteenth terms ... are" supplies inputs, not the
        // requested output index.
        let is_definition = tokens[index + 1..tokens.len().min(index + 6)]
            .iter()
            .map(|next| next.trim_matches(|character: char| !character.is_ascii_alphabetic()))
            .any(|next| next == "are" || next == "is");
        if is_definition {
            continue;
        }
        for previous in (index.saturating_sub(3)..index).rev() {
            if let Some(number) = ordinal_number(tokens[previous]) {
                candidates.push(number);
                break;
            }
        }
    }
    candidates.into_iter().last()
}

fn explicit_integer_list(segment: &str) -> Option<Vec<Rational>> {
    let normalized = segment.replace(" and ", ",");
    let mut values = Vec::new();
    for item in normalized.split(',') {
        let item = item.trim().trim_end_matches("...").trim();
        if item.is_empty() {
            continue;
        }
        // Natural-language qualifiers such as "respectively" are not list
        // entries.  Ignore a qualifier only when it contains no numeric
        // token; a mixed or malformed numeric entry still causes rejection.
        if !item.chars().any(|character| character.is_ascii_digit()) {
            continue;
        }
        values.push(integer_token(item)?);
    }
    (values.len() >= 2).then_some(values)
}

/// Lower natural-language arithmetic-sequence statements when the sequence
/// terms and requested numeric index determine a unique finite nth-term
/// request.  This is a frontend bridge only; generic source-formula execution
/// remains responsible for the arithmetic result.
pub fn formalize_sequence_terms_text(
    text: &str,
    case_id: &str,
    domain: &str,
) -> SequenceFrontendResult {
    let lower = text.to_ascii_lowercase();
    let provenance = vec![format!("source-sequence-terms-frontend:{case_id}")];
    let unsupported = [
        "geometric",
        "infinite",
        "converges",
        "recurrence",
        "sum of",
        "series",
        "difference between",
        "how many",
        "solve for",
        "angles",
        "trapezoid",
        "hexagon",
        "two sequences",
    ];
    if !lower.contains("arithmetic sequence")
        || unsupported.iter().any(|marker| lower.contains(marker))
    {
        return output(SequenceFrontendResult {
            status: SequenceFrontendStatus::Unsupported,
            request: None,
            evidence: Vec::new(),
            unresolved: vec!["request is outside explicit arithmetic nth-term scope".into()],
            provenance,
            replay_hash: String::new(),
        });
    }
    let Some(target) = requested_ordinal(&lower) else {
        return output(SequenceFrontendResult {
            status: SequenceFrontendStatus::Missing,
            request: None,
            evidence: Vec::new(),
            unresolved: vec!["requested term index is not explicit".into()],
            provenance,
            replay_hash: String::new(),
        });
    };
    if target <= 0 {
        return output(SequenceFrontendResult {
            status: SequenceFrontendStatus::Unsupported,
            request: None,
            evidence: Vec::new(),
            unresolved: vec!["term index must be positive".into()],
            provenance,
            replay_hash: String::new(),
        });
    }

    let mut a1 = None;
    let mut difference = None;
    let mut evidence = Vec::new();
    if let Some(marker) = lower.find("first three terms") {
        let start = marker + "first three terms".len();
        let end = text[start..]
            .find(|character: char| matches!(character, '.' | ';' | '?'))
            .map(|offset| start + offset)
            .unwrap_or(text.len());
        if let Some(values) = explicit_integer_list(&text[start..end]) {
            let delta = values[1].sub(&values[0]);
            if delta == Some(values[2].sub(&values[1]).unwrap()) {
                a1 = values.first().cloned();
                difference = delta;
                evidence.push(format!("first-three-terms-span:{start}..{end}"));
            }
        }
    }
    if a1.is_none() && lower.contains("first and thirteenth terms") {
        let Some(start) = lower.find("are") else {
            return output(SequenceFrontendResult {
                status: SequenceFrontendStatus::Ambiguous,
                request: None,
                evidence,
                unresolved: vec!["two stated terms are not bound".into()],
                provenance,
                replay_hash: String::new(),
            });
        };
        let end = text[start + 3..]
            .find(|character: char| matches!(character, '.' | ';' | '?'))
            .map(|offset| start + 3 + offset)
            .unwrap_or(text.len());
        if let Some(values) = explicit_integer_list(&text[start + 3..end]) {
            if values.len() == 2 {
                let gap = values[1].sub(&values[0]).unwrap();
                a1 = Some(values[0].clone());
                difference = gap.div(&Rational::new(12, 1).unwrap());
                evidence.push(format!(
                    "first-thirteenth-terms-span:{}..{end}",
                    start + 3
                ));
            }
        }
    }
    if a1.is_none() {
        if let Some(marker) = lower.find("sequence") {
            let start = marker + "sequence".len();
            let end = text[start..]
                .find(|character: char| matches!(character, '.' | ';' | '?'))
                .map(|offset| start + offset)
                .unwrap_or(text.len());
            if let Some(values) = explicit_integer_list(&text[start..end]) {
                if values.len() < 3 {
                    return output(SequenceFrontendResult {
                        status: SequenceFrontendStatus::Ambiguous,
                        request: None,
                        evidence,
                        unresolved: vec![
                            "fewer than three explicit sequence terms are available".into(),
                        ],
                        provenance,
                        replay_hash: String::new(),
                    });
                }
                let delta = values[1].sub(&values[0]);
                if values
                    .windows(2)
                    .all(|pair| pair[1].sub(&pair[0]) == Some(delta.clone().unwrap()))
                {
                    a1 = values.first().cloned();
                    difference = delta;
                    evidence.push(format!("sequence-terms-span:{start}..{end}"));
                }
            }
        }
    }
    let (Some(a1), Some(difference)) = (a1, difference) else {
        return output(SequenceFrontendResult {
            status: SequenceFrontendStatus::Ambiguous,
            request: None,
            evidence,
            unresolved: vec!["terms do not determine one arithmetic progression".into()],
            provenance,
            replay_hash: String::new(),
        });
    };
    let mut inputs = BTreeMap::new();
    inputs.insert("a1".into(), a1);
    inputs.insert("d".into(), difference);
    inputs.insert("n".into(), Rational::new(target, 1).unwrap());
    output(SequenceFrontendResult {
        status: SequenceFrontendStatus::Complete,
        request: Some(FormulaRequest {
            formula: "arithmetic_nth_term".into(),
            inputs,
            domain: domain.into(),
            ambiguity: None,
            provenance: provenance.clone(),
        }),
        evidence,
        unresolved: Vec::new(),
        provenance,
        replay_hash: String::new(),
    })
}

fn find_value(text: &str, labels: &[&str]) -> Option<(String, Rational)> {
    for label in labels {
        let Some(start) = text.find(label) else {
            continue;
        };
        let tail = &text[start + label.len()..];
        let token = tail
            .trim_start_matches(|c: char| c == ' ' || c == ':' || c == '=')
            .split(|c: char| !c.is_ascii_digit() && c != '-')
            .next()
            .unwrap_or_default();
        if let Some(value) = rational(token) {
            return Some(((*label).into(), value));
        }
    }
    None
}

fn output(result: SequenceFrontendResult) -> SequenceFrontendResult {
    let mut result = result;
    result.replay_hash.clear();
    result.replay_hash = digest(&result);
    result
}

pub fn replay_verified(result: &SequenceFrontendResult) -> bool {
    let mut copy = result.clone();
    let hash = copy.replay_hash.clone();
    copy.replay_hash.clear();
    hash == digest(&copy) && !result.provenance.is_empty()
}

pub fn formalize_sequence_text(text: &str, case_id: &str) -> SequenceFrontendResult {
    let lower = text.to_ascii_lowercase();
    let provenance = vec![format!("source-sequence-frontend:{case_id}")];
    let mut unresolved = Vec::new();
    if [
        "infinite",
        "converges",
        "convergence",
        "limit",
        "recurrence",
    ]
    .iter()
    .any(|term| lower.contains(term))
    {
        return output(SequenceFrontendResult {
            status: SequenceFrontendStatus::Unsupported,
            request: None,
            evidence: Vec::new(),
            unresolved: vec![
                "infinite or recurrence semantics are outside the finite catalog".into(),
            ],
            provenance,
            replay_hash: String::new(),
        });
    }
    let arithmetic = lower.contains("arithmetic") || lower.contains("common difference");
    let geometric = lower.contains("geometric") || lower.contains("common ratio");
    if !arithmetic && !geometric {
        return output(SequenceFrontendResult {
            status: SequenceFrontendStatus::Unsupported,
            request: None,
            evidence: Vec::new(),
            unresolved: vec!["no supported finite sequence family is stated".into()],
            provenance,
            replay_hash: String::new(),
        });
    }
    if arithmetic && geometric {
        unresolved.push("sequence family is not uniquely identified".into());
    }
    let partial =
        lower.contains("sum of") || lower.contains("partial sum") || lower.contains("series sum");
    let nth =
        lower.contains("nth term") || lower.contains("n-th term") || lower.contains("term number");
    if partial == nth {
        unresolved.push("requested finite operation is not uniquely identified".into());
    }
    let Some((a1_label, a1)) = find_value(&lower, &["first term", "first value", "a1", "a_1"])
    else {
        unresolved.push("first term is not explicitly bound".into());
        return output(SequenceFrontendResult {
            status: if unresolved.len() > 1 {
                SequenceFrontendStatus::Ambiguous
            } else {
                SequenceFrontendStatus::Missing
            },
            request: None,
            evidence: Vec::new(),
            unresolved,
            provenance,
            replay_hash: String::new(),
        });
    };
    let Some((n_label, n)) = find_value(&lower, &["n =", "n is", "term number", "term n"]) else {
        unresolved.push("positive term index is not explicitly bound".into());
        return output(SequenceFrontendResult {
            status: SequenceFrontendStatus::Missing,
            request: None,
            evidence: vec![a1_label],
            unresolved,
            provenance,
            replay_hash: String::new(),
        });
    };
    let mut inputs = BTreeMap::from([("a1".into(), a1), ("n".into(), n)]);
    let (formula, parameter_label) = if arithmetic {
        let Some((label, value)) = find_value(&lower, &["common difference", "difference", "d ="])
        else {
            unresolved.push("common difference is not explicitly bound".into());
            return output(SequenceFrontendResult {
                status: SequenceFrontendStatus::Missing,
                request: None,
                evidence: vec![a1_label, n_label],
                unresolved,
                provenance,
                replay_hash: String::new(),
            });
        };
        inputs.insert("d".into(), value);
        (
            if partial {
                "arithmetic_partial_sum"
            } else {
                "arithmetic_nth_term"
            },
            label,
        )
    } else if geometric {
        let Some((label, value)) = find_value(&lower, &["common ratio", "ratio", "r ="]) else {
            unresolved.push("common ratio is not explicitly bound".into());
            return output(SequenceFrontendResult {
                status: SequenceFrontendStatus::Missing,
                request: None,
                evidence: vec![a1_label, n_label],
                unresolved,
                provenance,
                replay_hash: String::new(),
            });
        };
        inputs.insert("r".into(), value);
        (
            if partial {
                "geometric_partial_sum"
            } else {
                "geometric_nth_term"
            },
            label,
        )
    } else {
        ("", String::new())
    };
    if !unresolved.is_empty() {
        return output(SequenceFrontendResult {
            status: SequenceFrontendStatus::Ambiguous,
            request: None,
            evidence: vec![a1_label, n_label, parameter_label],
            unresolved,
            provenance,
            replay_hash: String::new(),
        });
    }
    output(SequenceFrontendResult {
        status: SequenceFrontendStatus::Complete,
        request: Some(FormulaRequest {
            formula: formula.into(),
            inputs,
            domain: "source_catalog_sequences_series".into(),
            ambiguity: None,
            provenance: provenance.clone(),
        }),
        evidence: vec![a1_label, n_label, parameter_label],
        unresolved,
        provenance,
        replay_hash: String::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn binds_arithmetic_nth_term_without_evaluating() {
        let result = formalize_sequence_text(
            "An arithmetic sequence has first term = 3, common difference = 4; find the nth term for n = 5.",
            "test-arithmetic",
        );
        assert_eq!(result.status, SequenceFrontendStatus::Complete);
        assert_eq!(
            result.request.as_ref().unwrap().formula,
            "arithmetic_nth_term"
        );
        assert!(replay_verified(&result));
    }

    #[test]
    fn refuses_infinite_language() {
        let result = formalize_sequence_text(
            "Determine whether the infinite geometric series converges.",
            "test-infinite",
        );
        assert_eq!(result.status, SequenceFrontendStatus::Unsupported);
        assert!(replay_verified(&result));
    }

    #[test]
    fn binds_natural_language_terms_and_requested_target() {
        let result = formalize_sequence_terms_text(
            "The first three terms of an arithmetic sequence are 1, 10 and 19, respectively. What is the value of the 21st term?",
            "test-terms",
            "external-source-sequence-shadow",
        );
        assert_eq!(result.status, SequenceFrontendStatus::Complete);
        let request = result.request.as_ref().unwrap();
        assert_eq!(request.formula, "arithmetic_nth_term");
        assert_eq!(request.inputs.get("a1"), Some(&Rational::new(1, 1).unwrap()));
        assert_eq!(request.inputs.get("d"), Some(&Rational::new(9, 1).unwrap()));
        assert_eq!(request.inputs.get("n"), Some(&Rational::new(21, 1).unwrap()));
        assert!(replay_verified(&result));
    }

    #[test]
    fn binds_two_distant_terms_without_using_definition_ordinal() {
        let result = formalize_sequence_terms_text(
            "The first and thirteenth terms of an arithmetic sequence are 5 and 29, respectively. What is the fiftieth term?",
            "test-distant-terms",
            "external-source-sequence-shadow",
        );
        assert_eq!(result.status, SequenceFrontendStatus::Complete);
        let request = result.request.as_ref().unwrap();
        assert_eq!(request.inputs.get("a1"), Some(&Rational::new(5, 1).unwrap()));
        assert_eq!(request.inputs.get("d"), Some(&Rational::new(2, 1).unwrap()));
        assert_eq!(request.inputs.get("n"), Some(&Rational::new(50, 1).unwrap()));
        assert!(replay_verified(&result));
    }

    #[test]
    fn preserves_ambiguous_or_unsupported_sequence_language() {
        let missing = formalize_sequence_terms_text(
            "The first three terms of an arithmetic sequence are 1, 4 and 7, respectively. Find the term.",
            "test-missing-target",
            "external-source-sequence-shadow",
        );
        assert_eq!(missing.status, SequenceFrontendStatus::Missing);
        assert!(replay_verified(&missing));

        let unsupported = formalize_sequence_terms_text(
            "Determine whether the infinite geometric series converges.",
            "test-unsupported-sequence",
            "external-source-sequence-shadow",
        );
        assert_eq!(unsupported.status, SequenceFrontendStatus::Unsupported);
        assert!(replay_verified(&unsupported));
    }
}
