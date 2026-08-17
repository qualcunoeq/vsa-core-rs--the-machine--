//! Narrow technical-language frontend for the bounded finite Markov extensions.
//!
//! It requires an explicit transition matrix, row-stochastic convention, and
//! operation. Hitting requests additionally require an initial distribution
//! and explicit target/avoid indices. It never infers a matrix from a graph.

use crate::finite_markov_hitting_pack::HittingRequest;
use crate::finite_markov_stationary_pack::StationaryRequest;
use crate::probability_pack::Rational;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MarkovFrontendStatus {
    Complete,
    Ambiguous,
    Missing,
    Unsupported,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum MarkovFrontendRequest {
    Stationary(StationaryRequest),
    Hitting(HittingRequest),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MarkovFrontendResult {
    pub status: MarkovFrontendStatus,
    pub request: Option<MarkovFrontendRequest>,
    pub unresolved: Vec<String>,
    pub provenance: Vec<String>,
    pub replay_hash: String,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(value).expect("markov frontend serializes"))
    )
}

fn finish(mut result: MarkovFrontendResult) -> MarkovFrontendResult {
    result.replay_hash.clear();
    result.replay_hash = digest(&result);
    result
}

pub fn replay_verified(result: &MarkovFrontendResult) -> bool {
    let mut copy = result.clone();
    let hash = copy.replay_hash.clone();
    copy.replay_hash.clear();
    hash == digest(&copy) && !result.provenance.is_empty()
}

fn rational(token: &str) -> Option<Rational> {
    let token = token.trim_matches(|character: char| {
        !character.is_ascii_digit() && character != '-' && character != '/'
    });
    if let Some((numerator, denominator)) = token.split_once('/') {
        return Rational::new(numerator.parse().ok()?, denominator.parse().ok()?);
    }
    Rational::new(token.parse().ok()?, 1)
}

fn bracketed_after(text: &str, marker: &str, nested: bool) -> Option<String> {
    let lower = text.to_ascii_lowercase();
    let start = lower.find(&marker.to_ascii_lowercase())?;
    let open = text[start..].find('[')? + start;
    let mut depth = 0usize;
    for (offset, character) in text[open..].char_indices() {
        match character {
            '[' => depth += 1,
            ']' => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some(text[open + 1..open + offset].to_string());
                }
            }
            _ => {}
        }
        if !nested && depth == 1 && character == ']' {
            break;
        }
    }
    None
}

fn parse_vector(text: &str, marker: &str) -> Option<Vec<Rational>> {
    let body = bracketed_after(text, marker, false)?;
    body.split(',').map(rational).collect()
}

fn parse_matrix(text: &str) -> Option<Vec<Vec<Rational>>> {
    let body = bracketed_after(text, "transition", true)?;
    let mut rows = Vec::new();
    let mut current = String::new();
    let mut depth = 0usize;
    for character in body.chars() {
        match character {
            '[' => {
                depth += 1;
                if depth > 1 {
                    current.push(character);
                }
            }
            ']' => {
                if depth == 0 {
                    return None;
                }
                depth -= 1;
                if depth == 0 {
                    rows.push(
                        current
                            .split(',')
                            .map(rational)
                            .collect::<Option<Vec<_>>>()?,
                    );
                    current.clear();
                } else {
                    current.push(character);
                }
            }
            ',' if depth == 0 => {}
            _ if depth > 0 => current.push(character),
            _ if !character.is_whitespace() => return None,
            _ => {}
        }
    }
    (!rows.is_empty()).then_some(rows)
}

fn index_after(text: &str, marker: &str) -> Option<usize> {
    let lower = text.to_ascii_lowercase();
    let marker = marker.to_ascii_lowercase();
    for (offset, _) in lower.match_indices(&marker) {
        let start = offset + marker.len();
        let candidate = text[start..].trim_start_matches(|character: char| {
            character == '=' || character == ':' || character.is_whitespace()
        });
        if let Some(number) = candidate
            .split(|character: char| !character.is_ascii_digit())
            .next()
            .filter(|number| !number.is_empty())
        {
            if let Ok(value) = number.parse() {
                return Some(value);
            }
        }
    }
    None
}

fn result(
    status: MarkovFrontendStatus,
    request: Option<MarkovFrontendRequest>,
    unresolved: Vec<String>,
    provenance: Vec<String>,
) -> MarkovFrontendResult {
    finish(MarkovFrontendResult {
        status,
        request,
        unresolved,
        provenance,
        replay_hash: String::new(),
    })
}

/// Parse one explicit stationary or target-before-avoid request.
pub fn formalize(text: &str, case_id: &str) -> MarkovFrontendResult {
    let lower = text.to_ascii_lowercase();
    let provenance = vec![
        format!("finite-markov-frontend:{case_id}"),
        format!("source-span:0..{}", text.len()),
        "explicit-transition-matrix-grammar".into(),
    ];
    if [
        "continuous",
        "infinite",
        "approx",
        "spectral",
        "mixing",
        "expected time",
        "limit",
    ]
    .iter()
    .any(|marker| lower.contains(marker))
    {
        return result(
            MarkovFrontendStatus::Unsupported,
            None,
            vec!["request exceeds the bounded exact finite Markov frontend".into()],
            provenance,
        );
    }
    // “Invariant distribution” is a bounded alias only when the requested
    // object is explicitly a distribution; bare invariant language remains
    // unresolved rather than being routed by vocabulary alone.
    let stationary = lower.contains("stationary")
        || (lower.contains("invariant") && lower.contains("distribution"));
    let hitting = (lower.contains("hitting") || lower.contains("reach")) && lower.contains("avoid");
    if stationary && hitting {
        return result(
            MarkovFrontendStatus::Ambiguous,
            None,
            vec!["stationary and hitting operations are both present".into()],
            provenance,
        );
    }
    if !stationary && !hitting {
        return result(
            MarkovFrontendStatus::Missing,
            None,
            vec!["a unique stationary or target-before-avoid operation is required".into()],
            provenance,
        );
    }
    if !(lower.contains("row-stochastic") || lower.contains("row stochastic")) {
        return result(
            MarkovFrontendStatus::Ambiguous,
            None,
            vec!["row-stochastic convention is not explicitly declared".into()],
            provenance,
        );
    }
    let Some(transition) = parse_matrix(text) else {
        return result(
            MarkovFrontendStatus::Missing,
            None,
            vec!["an explicit transition matrix is required".into()],
            provenance,
        );
    };
    if stationary {
        return result(
            MarkovFrontendStatus::Complete,
            Some(MarkovFrontendRequest::Stationary(StationaryRequest {
                domain: "finite_exact_markov_stationary".into(),
                transition,
                row_stochastic: Some(true),
                ambiguity: None,
                provenance: provenance.clone(),
            })),
            Vec::new(),
            provenance,
        );
    }
    let Some(initial) = parse_vector(text, "initial") else {
        return result(
            MarkovFrontendStatus::Missing,
            None,
            vec!["hitting requests require an explicit initial distribution".into()],
            provenance,
        );
    };
    let (Some(target), Some(avoid)) = (index_after(text, "target"), index_after(text, "avoid"))
    else {
        return result(
            MarkovFrontendStatus::Missing,
            None,
            vec!["hitting requests require explicit target and avoid indices".into()],
            provenance,
        );
    };
    result(
        MarkovFrontendStatus::Complete,
        Some(MarkovFrontendRequest::Hitting(HittingRequest {
            domain: "finite_exact_markov_hitting".into(),
            transition,
            initial,
            target_states: vec![target],
            avoid_states: vec![avoid],
            row_stochastic: Some(true),
            ambiguity: None,
            provenance: provenance.clone(),
        })),
        Vec::new(),
        provenance,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_stationary_matrix() {
        let result = formalize(
            "Find the stationary distribution for a row-stochastic transition=[[3/4,1/4],[1/2,1/2]].",
            "stationary",
        );
        assert_eq!(result.status, MarkovFrontendStatus::Complete);
        assert!(matches!(
            result.request,
            Some(MarkovFrontendRequest::Stationary(_))
        ));
        assert!(replay_verified(&result));
    }

    #[test]
    fn preserves_missing_hitting_context() {
        let result = formalize(
            "Find the hitting probability for a row-stochastic transition=[[1,0],[0,1]] with initial=[1,0].",
            "hitting",
        );
        assert_eq!(result.status, MarkovFrontendStatus::Missing);
        assert!(replay_verified(&result));
    }

    #[test]
    fn accepts_explicit_invariant_distribution_alias() {
        let result = formalize(
            "Compute the invariant distribution of a row-stochastic transition=[[3/4,1/4],[1/2,1/2]].",
            "invariant",
        );
        assert_eq!(result.status, MarkovFrontendStatus::Complete);
        assert!(matches!(
            result.request,
            Some(MarkovFrontendRequest::Stationary(_))
        ));
        assert!(replay_verified(&result));
    }
}
