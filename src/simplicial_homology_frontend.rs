//! Controlled technical-language frontend for bounded simplicial homology.
//!
//! The accepted surface is intentionally explicit: vertices and simplices
//! must be enumerated, the operation must be unique, and the coefficient field
//! must be stated.  Related vocabulary without those fields remains ambiguous
//! or unsupported rather than being routed by keyword alone.

use crate::simplicial_homology_pack::{HomologyOperation, SimplicialComplexRequest};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FrontendStatus {
    Complete,
    Ambiguous,
    Unsupported,
    Invalid,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FrontendResult {
    pub status: FrontendStatus,
    pub request: Option<SimplicialComplexRequest>,
    pub reasons: Vec<String>,
    pub provenance: Vec<String>,
    pub replay_hash: String,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn payload(result: &FrontendResult) -> impl Serialize + '_ {
    (
        result.status,
        &result.request,
        &result.reasons,
        &result.provenance,
    )
}

fn output(
    status: FrontendStatus,
    request: Option<SimplicialComplexRequest>,
    reasons: Vec<String>,
    provenance: Vec<String>,
) -> FrontendResult {
    let mut result = FrontendResult {
        status,
        request,
        reasons,
        provenance,
        replay_hash: String::new(),
    };
    let replay_hash = digest(&payload(&result));
    result.replay_hash = replay_hash;
    result
}

fn marker_after<'a>(lower: &'a str, original: &'a str, markers: &[&str]) -> Option<&'a str> {
    markers.iter().find_map(|marker| {
        lower
            .find(marker)
            .map(|index| &original[index + marker.len()..])
    })
}

/// Return the innermost balanced `[]` or `{}` groups.  This is enough for the
/// explicit vector/list grammar and avoids interpreting arbitrary prose as a
/// mathematical object.
fn innermost_groups(text: &str) -> Vec<String> {
    let mut stack: Vec<(char, usize)> = Vec::new();
    let mut groups = Vec::new();
    for (index, character) in text.char_indices() {
        match character {
            '[' | '{' => stack.push((character, index + character.len_utf8())),
            ']' | '}' => {
                let Some((opening, start)) = stack.pop() else {
                    return Vec::new();
                };
                let matches =
                    (opening == '[' && character == ']') || (opening == '{' && character == '}');
                if !matches {
                    return Vec::new();
                }
                let content = &text[start..index];
                if !content.contains(['[', '{', ']', '}']) {
                    groups.push(content.trim().to_string());
                }
            }
            _ => {}
        }
    }
    if !stack.is_empty() {
        return Vec::new();
    }
    groups
}

fn names(content: &str) -> Option<Vec<String>> {
    let values = content
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .collect::<Vec<_>>();
    if values.is_empty()
        || values.iter().any(|value| {
            value
                .chars()
                .any(|character| !(character.is_ascii_alphanumeric() || character == '_'))
        })
    {
        None
    } else {
        Some(values)
    }
}

fn operation(lower: &str) -> Result<HomologyOperation, FrontendStatus> {
    let candidates = [
        ("betti numbers", HomologyOperation::BettiNumbers),
        (
            "euler characteristic",
            HomologyOperation::EulerCharacteristic,
        ),
        ("boundary matrices", HomologyOperation::BoundaryMatrices),
        ("validate the complex", HomologyOperation::ValidateComplex),
        ("validate complex", HomologyOperation::ValidateComplex),
    ];
    let matches = candidates
        .iter()
        .filter(|(phrase, _)| lower.contains(phrase))
        .map(|(_, operation)| *operation)
        .collect::<Vec<_>>();
    match matches.as_slice() {
        [operation] => Ok(*operation),
        [] => Err(FrontendStatus::Ambiguous),
        _ => Err(FrontendStatus::Ambiguous),
    }
}

fn field(lower: &str) -> Result<Option<u32>, FrontendStatus> {
    if lower.contains("over the integers") || lower.contains("reduced homology") {
        return Err(FrontendStatus::Unsupported);
    }
    let f2 = lower.contains("f2") || lower.contains("f_2") || lower.contains("f₂");
    let f3 = lower.contains("f3") || lower.contains("f_3") || lower.contains("f₃");
    if f2 && f3 {
        return Err(FrontendStatus::Ambiguous);
    }
    Ok(if f2 {
        Some(2)
    } else if f3 {
        Some(3)
    } else {
        None
    })
}

/// Parse an explicitly enumerated technical problem into a typed request.
pub fn formalize(text: &str) -> FrontendResult {
    let lower = text.to_ascii_lowercase();
    let unsupported_terms = [
        "infinite complex",
        "continuous complex",
        "numerical approximation",
        "torsion subgroup",
        "persistent homology",
        "over the integers",
    ];
    if unsupported_terms.iter().any(|term| lower.contains(term)) {
        return output(
            FrontendStatus::Unsupported,
            None,
            vec!["the request lies outside the bounded simplicial contract".into()],
            vec!["frontend:unsupported-boundary".into()],
        );
    }
    let operation = match operation(&lower) {
        Ok(operation) => operation,
        Err(status) => {
            return output(
                status,
                None,
                vec!["the requested homology operation is missing or non-unique".into()],
                vec!["frontend:operation".into()],
            )
        }
    };
    let Some(vertex_tail) =
        marker_after(&lower, text, &["vertices:", "vertex set:", "on vertices "])
    else {
        return output(
            FrontendStatus::Ambiguous,
            None,
            vec!["vertices must be explicitly enumerated".into()],
            vec!["frontend:vertices".into()],
        );
    };
    let Some(simplex_tail) = marker_after(&lower, text, &["simplices:", "simplex list:", "faces:"])
    else {
        return output(
            FrontendStatus::Ambiguous,
            None,
            vec!["simplices must be explicitly enumerated".into()],
            vec!["frontend:simplices".into()],
        );
    };
    let vertex_content = innermost_groups(vertex_tail)
        .into_iter()
        .next()
        .and_then(|content| names(&content));
    let Some(vertices) = vertex_content else {
        return output(
            FrontendStatus::Invalid,
            None,
            vec!["vertex enumeration is malformed".into()],
            vec!["frontend:vertices".into()],
        );
    };
    let simplex_groups = innermost_groups(simplex_tail);
    if simplex_groups.is_empty() {
        return output(
            FrontendStatus::Invalid,
            None,
            vec!["simplex enumeration is malformed".into()],
            vec!["frontend:simplices".into()],
        );
    }
    let mut index = std::collections::BTreeMap::new();
    for (position, vertex) in vertices.iter().enumerate() {
        if index.insert(vertex.clone(), position).is_some() {
            return output(
                FrontendStatus::Invalid,
                None,
                vec!["vertex identities are duplicated".into()],
                vec!["frontend:vertices".into()],
            );
        }
    }
    let mut simplices = Vec::new();
    for group in simplex_groups {
        let Some(simplex_names) = names(&group) else {
            return output(
                FrontendStatus::Invalid,
                None,
                vec!["a simplex contains malformed symbols".into()],
                vec!["frontend:simplices".into()],
            );
        };
        let Some(simplex) = simplex_names
            .iter()
            .map(|name| index.get(name).copied())
            .collect::<Option<Vec<_>>>()
        else {
            return output(
                FrontendStatus::Ambiguous,
                None,
                vec!["a simplex references an undeclared vertex".into()],
                vec!["frontend:symbol-binding".into()],
            );
        };
        simplices.push(simplex);
    }
    let coefficient_field = match field(&lower) {
        Ok(field) => field,
        Err(status) => {
            return output(
                status,
                None,
                vec!["coefficient semantics are unsupported or non-unique".into()],
                vec!["frontend:coefficient-field".into()],
            )
        }
    };
    let status = if coefficient_field == Some(2) {
        FrontendStatus::Complete
    } else {
        FrontendStatus::Ambiguous
    };
    let request = SimplicialComplexRequest {
        operation,
        domain: "finite_simplicial_complex".into(),
        vertices,
        simplices,
        coefficient_field,
        provenance: vec![
            "frontend:operation".into(),
            "frontend:vertices".into(),
            "frontend:simplices".into(),
            "frontend:coefficient-field".into(),
        ],
        ambiguity: None,
    };
    output(
        status,
        Some(request),
        if status == FrontendStatus::Complete {
            Vec::new()
        } else {
            vec!["the coefficient field must be explicitly F_2".into()]
        },
        vec!["frontend:explicit-simplicial-grammar".into()],
    )
}

impl FrontendResult {
    pub fn replay_verified(&self) -> bool {
        self.replay_hash == digest(&payload(self)) && !self.provenance.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEXT: &str = "Compute Betti numbers for the finite simplicial complex. Vertices: [a,b,c]. Simplices: [[a],[b],[c],[a,b],[a,c],[b,c],[a,b,c]]. Coefficients: F_2.";

    #[test]
    fn explicit_problem_formalizes_and_replays() {
        let result = formalize(TEXT);
        assert_eq!(result.status, FrontendStatus::Complete);
        assert!(result.request.is_some());
        assert!(result.replay_verified());
    }

    #[test]
    fn missing_field_is_ambiguous() {
        let result =
            formalize("Find the Euler characteristic. Vertex set: {a,b}. Faces: [[a],[b],[a,b]].");
        assert_eq!(result.status, FrontendStatus::Ambiguous);
    }

    #[test]
    fn unsupported_domain_is_closed() {
        let result =
            formalize("Compute persistent homology for an infinite complex on vertices [a,b].");
        assert_eq!(result.status, FrontendStatus::Unsupported);
        assert!(result.replay_verified());
    }
}
