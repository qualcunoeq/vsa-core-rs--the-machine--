//! Shifted technical-language frontend for bounded elementary number theory.
//!
//! The frontend only constructs a typed request.  It never authorizes an
//! answer: downstream execution must still validate arithmetic prerequisites.

use crate::number_theory_pack::{NumberTheoryOperation, NumberTheoryRequest};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum NumberTheoryFrontendStatus {
    Complete,
    Ambiguous,
    Missing,
    Unsupported,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NumberTheoryFrontendResult {
    pub status: NumberTheoryFrontendStatus,
    pub request: Option<NumberTheoryRequest>,
    pub unresolved: Vec<String>,
    pub provenance: Vec<String>,
    pub replay_hash: String,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(value).expect("frontend serializes"))
    )
}

fn finish(mut result: NumberTheoryFrontendResult) -> NumberTheoryFrontendResult {
    result.replay_hash.clear();
    result.replay_hash = digest(&result);
    result
}

pub fn replay_verified(result: &NumberTheoryFrontendResult) -> bool {
    let mut copy = result.clone();
    let hash = copy.replay_hash.clone();
    copy.replay_hash.clear();
    hash == digest(&copy) && !result.provenance.is_empty()
}

fn integer_after(text: &str, labels: &[&str]) -> Option<i64> {
    labels.iter().find_map(|label| {
        let start = text.find(label)? + label.len();
        let token = text[start..]
            .trim_start_matches(|c: char| c == '=' || c == ':' || c.is_whitespace())
            .strip_prefix('−')
            .map(|rest| format!("-{rest}"))
            .unwrap_or_else(|| text[start..].trim_start().to_string());
        let token = token
            .trim_start_matches(|c: char| c == '=' || c == ':' || c.is_whitespace())
            .chars()
            .take_while(|c| c.is_ascii_digit() || *c == '-')
            .collect::<String>();
        (!token.is_empty()).then(|| token.parse().ok()).flatten()
    })
}

fn marker_count(text: &str, marker: &str) -> usize {
    text.match_indices(marker).count()
}

fn request(
    operation: NumberTheoryOperation,
    a: Option<i64>,
    b: Option<i64>,
    c: Option<i64>,
    modulus: Option<u64>,
    second_modulus: Option<u64>,
    provenance: Vec<String>,
) -> NumberTheoryRequest {
    NumberTheoryRequest {
        operation,
        a,
        b,
        c,
        modulus,
        second_modulus,
        domain: "bounded_exact_elementary_number_theory".into(),
        ambiguity: None,
        provenance,
    }
}

/// Parse a shifted natural-language request into an exact bounded typed request.
/// The parser refuses semantic overlap and missing bindings rather than guessing.
pub fn formalize_number_theory_text(text: &str, case_id: &str) -> NumberTheoryFrontendResult {
    let lower = text.to_ascii_lowercase();
    let provenance = vec![
        format!("number-theory-frontend:{case_id}"),
        "explicit-bounded-integer-parser".into(),
        format!("source-span:0..{}", text.len()),
    ];
    if [
        "cryptograph",
        "asymptotic",
        "infinite",
        "unbounded",
        "analytic number theory",
        "prime factorization",
        "security claim",
    ]
    .iter()
    .any(|term| lower.contains(term))
    {
        return finish(NumberTheoryFrontendResult {
            status: NumberTheoryFrontendStatus::Unsupported,
            request: None,
            unresolved: vec!["request exceeds bounded elementary number theory".into()],
            provenance,
            replay_hash: String::new(),
        });
    }
    let operation_families: &[&[&str]] = &[
        &["gcd", "greatest common divisor", "bezout", "bézout"],
        &["modular inverse", "inverse modulo"],
        &[
            "chinese remainder",
            "simultaneous congruence",
            "simultaneous congruences",
        ],
        &["linear congruence", "congruence", "≡"],
        &["totient", "phi(", "φ("],
        &["diophantine"],
        // A visible arithmetic-function formula is not a number-theory
        // operation merely because it contains a familiar symbol.  Keep it
        // as a competing semantic scope when it occurs beside a supported
        // number-theory request.
        &[
            "mu(",
            "μ(",
            "möbius",
            "mobius",
            "divisor sum",
            "divisor count",
            "sigma(",
            "σ(",
        ],
    ];
    if operation_families
        .iter()
        .filter(|family| family.iter().any(|marker| lower.contains(marker)))
        .count()
        > 1
    {
        return finish(NumberTheoryFrontendResult {
            status: NumberTheoryFrontendStatus::Ambiguous,
            request: None,
            unresolved: vec![
                "multiple number-theory operations or scoped formulas are present".into(),
            ],
            provenance,
            replay_hash: String::new(),
        });
    }
    let binding_markers = [
        "a=", "a =", "b=", "b =", "c=", "c =", "m=", "m =", "n=", "n =",
    ];
    if binding_markers
        .iter()
        .any(|marker| marker_count(&lower, marker) > 1)
    {
        return finish(NumberTheoryFrontendResult {
            status: NumberTheoryFrontendStatus::Ambiguous,
            request: None,
            unresolved: vec![
                "a required number-theory binding appears in multiple local scopes".into(),
            ],
            provenance,
            replay_hash: String::new(),
        });
    }
    if lower.contains(" or ")
        || lower.contains("either")
        || (lower.contains("gcd") && (lower.contains("inverse") || lower.contains("congruence")))
    {
        return finish(NumberTheoryFrontendResult {
            status: NumberTheoryFrontendStatus::Ambiguous,
            request: None,
            unresolved: vec!["more than one number-theory operation is plausible".into()],
            provenance,
            replay_hash: String::new(),
        });
    }

    let operation = if lower.contains("bezout")
        || lower.contains("bézout")
        || (lower.contains("gcd") && lower.contains("greatest common divisor"))
    {
        NumberTheoryOperation::GcdBezout
    } else if lower.contains("modular inverse") || lower.contains("inverse modulo") {
        NumberTheoryOperation::ModularInverse
    } else if lower.contains("chinese remainder")
        || lower.contains("simultaneous congruence")
        || lower.contains("simultaneous congruences")
    {
        NumberTheoryOperation::ChineseRemainder
    } else if lower.contains("linear congruence")
        || lower.contains("congruence")
        || lower.contains('≡')
    {
        NumberTheoryOperation::LinearCongruence
    } else if lower.contains("totient") || lower.contains("phi(") || lower.contains("φ(") {
        NumberTheoryOperation::EulerTotient
    } else if lower.contains("diophantine") {
        NumberTheoryOperation::LinearDiophantine
    } else {
        return finish(NumberTheoryFrontendResult {
            status: NumberTheoryFrontendStatus::Unsupported,
            request: None,
            unresolved: vec!["no bounded elementary number-theory operation identified".into()],
            provenance,
            replay_hash: String::new(),
        });
    };

    let a = integer_after(&lower, &["a=", "a =", "a:", "value="]);
    let b = integer_after(&lower, &["b=", "b =", "b:", "right side="]);
    let c = integer_after(&lower, &["c=", "c =", "c:", "constant="]);
    let modulus = integer_after(&lower, &["m=", "m =", "modulus=", "modulus ", "n=", "n ="])
        .and_then(|value| u64::try_from(value).ok());
    let second_modulus =
        integer_after(&lower, &["n=", "n =", "second modulus=", "second modulus "])
            .and_then(|value| u64::try_from(value).ok());

    let missing = match operation {
        NumberTheoryOperation::GcdBezout => [a.is_none(), b.is_none()].iter().any(|v| *v),
        NumberTheoryOperation::ModularInverse => a.is_none() || modulus.is_none(),
        NumberTheoryOperation::LinearCongruence => a.is_none() || b.is_none() || modulus.is_none(),
        NumberTheoryOperation::ChineseRemainder => {
            a.is_none() || b.is_none() || modulus.is_none() || second_modulus.is_none()
        }
        NumberTheoryOperation::EulerTotient => modulus.is_none(),
        NumberTheoryOperation::LinearDiophantine => a.is_none() || b.is_none() || c.is_none(),
    };
    if missing {
        return finish(NumberTheoryFrontendResult {
            status: NumberTheoryFrontendStatus::Missing,
            request: None,
            unresolved: vec!["required integer bindings are not explicit".into()],
            provenance,
            replay_hash: String::new(),
        });
    }
    let typed = request(
        operation,
        a,
        b,
        c,
        modulus,
        second_modulus,
        provenance.clone(),
    );
    finish(NumberTheoryFrontendResult {
        status: NumberTheoryFrontendStatus::Complete,
        request: Some(typed),
        unresolved: Vec::new(),
        provenance,
        replay_hash: String::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shifted_inverse_and_unicode_congruence_bind() {
        let inverse = formalize_number_theory_text(
            "Find the least nonnegative modular inverse of a=7 modulo m=20.",
            "inverse",
        );
        assert_eq!(inverse.status, NumberTheoryFrontendStatus::Complete);
        assert!(replay_verified(&inverse));
        let congruence =
            formalize_number_theory_text("Solve a=6 x ≡ b=9 (mod m=15).", "congruence");
        assert_eq!(congruence.status, NumberTheoryFrontendStatus::Complete);
    }

    #[test]
    fn ambiguity_and_unsupported_are_fail_closed() {
        let ambiguous = formalize_number_theory_text(
            "Find the gcd or modular inverse for a=7 and b=20.",
            "ambiguous",
        );
        assert_eq!(ambiguous.status, NumberTheoryFrontendStatus::Ambiguous);
        let unsupported = formalize_number_theory_text(
            "Give the cryptographic security consequence of this modulus.",
            "unsupported",
        );
        assert_eq!(unsupported.status, NumberTheoryFrontendStatus::Unsupported);
        let scoped = formalize_number_theory_text(
            "A quoted example uses a=7 modulo m=20, while another scope uses a=11 modulo m=20; choose neither inverse.",
            "scoped",
        );
        assert_eq!(scoped.status, NumberTheoryFrontendStatus::Ambiguous);
    }
}
