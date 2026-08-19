//! Controlled technical-language frontend for the bounded chemistry pack.
//!
//! The frontend accepts only explicit, local forms: a formula after a formula
//! cue, a reaction containing one arrow, or a ratio request naming both
//! species. It preserves alternatives and refuses broader chemistry language.

use super::{ChemistryOperation, ChemistryRequest};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FrontendStatus {
    Complete,
    Ambiguous,
    Unsupported,
    Missing,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChemistryFrontendResult {
    pub status: FrontendStatus,
    pub request: Option<ChemistryRequest>,
    pub candidate_spans: Vec<String>,
    pub unresolved_alternatives: Vec<String>,
    pub reasons: Vec<String>,
    pub provenance: Vec<String>,
    pub replay_hash: String,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(value).expect("chemistry frontend serializes"))
    )
}

fn payload(result: &ChemistryFrontendResult) -> impl Serialize + '_ {
    (
        result.status,
        &result.request,
        &result.candidate_spans,
        &result.unresolved_alternatives,
        &result.reasons,
        &result.provenance,
    )
}

fn output(
    status: FrontendStatus,
    request: Option<ChemistryRequest>,
    candidate_spans: Vec<String>,
    unresolved_alternatives: Vec<String>,
    reasons: Vec<String>,
    text: &str,
) -> ChemistryFrontendResult {
    let mut result = ChemistryFrontendResult {
        status,
        request,
        candidate_spans,
        unresolved_alternatives,
        reasons,
        provenance: vec![format!("text-sha256:{:x}", Sha256::digest(text.as_bytes()))],
        replay_hash: String::new(),
    };
    let replay_hash = digest(&payload(&result));
    result.replay_hash = replay_hash;
    result
}

fn token_chars(value: char) -> bool {
    value.is_ascii_alphanumeric() || matches!(value, '(' | ')')
}

fn formula_candidates(text: &str) -> Vec<String> {
    let lower = text.to_ascii_lowercase();
    let mut candidates = Vec::new();
    for marker in ["formula:", "formula is", "formula "] {
        let mut start = 0;
        while let Some(offset) = lower[start..].find(marker) {
            let index = start + offset + marker.len();
            let token: String = text[index..]
                .chars()
                .skip_while(|value| value.is_whitespace())
                .take_while(|value| token_chars(*value))
                .collect();
            if !token.is_empty()
                && !matches!(
                    token.to_ascii_lowercase().as_str(),
                    "is" | "are" | "the" | "of"
                )
                && !candidates.contains(&token)
            {
                candidates.push(token);
            }
            start = index;
        }
    }
    candidates
}

fn reaction_candidate(text: &str) -> Option<String> {
    let arrow = text.find("->").or_else(|| text.find('→'))?;
    let before = &text[..arrow];
    let start = before
        .rfind(|value| matches!(value, '.' | ':' | '\n'))
        .map(|index| index + 1)
        .unwrap_or(0);
    let arrow_end = if text[arrow..].starts_with("->") {
        arrow + 2
    } else {
        arrow + '→'.len_utf8()
    };
    let after = &text[arrow_end..];
    let end = after
        .find(|value| matches!(value, '.' | '\n'))
        .map(|index| arrow_end + index)
        .unwrap_or(text.len());
    let candidate = text[start..end].trim();
    if candidate.is_empty() {
        None
    } else {
        Some(candidate.to_string())
    }
}

fn species_after(text: &str, marker: &str) -> Option<String> {
    let lower = text.to_ascii_lowercase();
    let index = lower.find(marker)? + marker.len();
    let value: String = text[index..]
        .chars()
        .skip_while(|value| value.is_whitespace())
        .take_while(|value| token_chars(*value))
        .collect();
    (!value.is_empty()).then_some(value)
}

fn request(operation: ChemistryOperation, text: &str) -> ChemistryRequest {
    ChemistryRequest {
        operation,
        formula: None,
        reaction: None,
        from_species: None,
        to_species: None,
        domain: "source_derived_bounded_chemistry".into(),
        ambiguity: None,
        provenance: vec![format!(
            "chemistry-frontend-text:{:x}",
            Sha256::digest(text.as_bytes())
        )],
    }
}

/// Formalize a deliberately narrow chemistry report into a typed request.
pub fn formalize_chemistry_text(text: &str) -> ChemistryFrontendResult {
    let lower = text.to_ascii_lowercase();
    let unsupported_terms = [
        "molar mass",
        "molecular weight",
        "oxidation state",
        "reaction mechanism",
        "equilibrium",
        "aqueous",
        "(aq)",
        "gas law",
        "in solution",
        "chemical process",
    ];
    if unsupported_terms.iter().any(|term| lower.contains(term)) {
        return output(
            FrontendStatus::Unsupported,
            None,
            Vec::new(),
            Vec::new(),
            vec!["requested chemistry semantics exceed the bounded frontend".into()],
            text,
        );
    }

    let arrows = text.matches("->").count() + text.matches('→').count();
    let asks_ratio = lower.contains("stoichiometric ratio") || lower.contains("ratio from");
    if arrows > 1 {
        return output(
            FrontendStatus::Ambiguous,
            None,
            Vec::new(),
            vec!["multiple reaction arrows require an explicit target reaction".into()],
            vec!["more than one reaction candidate".into()],
            text,
        );
    }
    if arrows == 1 {
        let Some(reaction) = reaction_candidate(text) else {
            return output(
                FrontendStatus::Missing,
                None,
                Vec::new(),
                Vec::new(),
                vec!["reaction arrow is present but the reaction span is incomplete".into()],
                text,
            );
        };
        let mut request = request(
            if asks_ratio {
                ChemistryOperation::StoichiometricRatio
            } else {
                ChemistryOperation::ValidateReaction
            },
            text,
        );
        request.reaction = Some(reaction.clone());
        if asks_ratio {
            let Some(from) =
                species_after(text, "ratio from").or_else(|| species_after(text, "from"))
            else {
                return output(
                    FrontendStatus::Missing,
                    None,
                    vec![reaction],
                    Vec::new(),
                    vec!["ratio source species is missing".into()],
                    text,
                );
            };
            let Some(to) = species_after(text, " to ") else {
                return output(
                    FrontendStatus::Missing,
                    None,
                    vec![reaction],
                    Vec::new(),
                    vec!["ratio target species is missing".into()],
                    text,
                );
            };
            request.from_species = Some(from);
            request.to_species = Some(to);
        }
        return output(
            FrontendStatus::Complete,
            Some(request),
            vec![reaction],
            Vec::new(),
            Vec::new(),
            text,
        );
    }

    if lower.contains("formula") {
        let candidates = formula_candidates(text);
        if candidates.len() == 1 {
            let mut request = request(ChemistryOperation::ParseFormula, text);
            request.formula = candidates.first().cloned();
            return output(
                FrontendStatus::Complete,
                Some(request),
                candidates,
                Vec::new(),
                Vec::new(),
                text,
            );
        }
        if candidates.len() > 1 {
            return output(
                FrontendStatus::Ambiguous,
                None,
                candidates.clone(),
                candidates,
                vec!["multiple formula spans are plausible targets".into()],
                text,
            );
        }
        return output(
            FrontendStatus::Missing,
            None,
            Vec::new(),
            Vec::new(),
            vec!["formula cue has no locally recoverable formula".into()],
            text,
        );
    }
    output(
        FrontendStatus::Missing,
        None,
        Vec::new(),
        Vec::new(),
        vec!["no supported chemistry operation cue".into()],
        text,
    )
}

impl ChemistryFrontendResult {
    pub fn replay_verified(&self) -> bool {
        self.replay_hash == digest(&payload(self))
            && !self.provenance.is_empty()
            && (self.status != FrontendStatus::Complete || self.request.is_some())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_formula_and_reaction_are_typed() {
        let formula = formalize_chemistry_text("Parse the molecular formula: Al2(SO4)3.");
        assert_eq!(formula.status, FrontendStatus::Complete);
        assert!(formula.replay_verified());
        let reaction = formalize_chemistry_text("Validate reaction: N2 + 3H2 -> 2NH3.");
        assert_eq!(reaction.status, FrontendStatus::Complete);
        assert!(reaction.replay_verified());
    }

    #[test]
    fn multiple_targets_and_unsupported_requests_fail_closed() {
        let ambiguous = formalize_chemistry_text("Formula: H2O; formula: CO2.");
        assert_eq!(ambiguous.status, FrontendStatus::Ambiguous);
        let unsupported = formalize_chemistry_text("Compute the molar mass of H2O.");
        assert_eq!(unsupported.status, FrontendStatus::Unsupported);
    }
}
