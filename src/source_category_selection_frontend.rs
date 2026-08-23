//! Fail-closed frontend for explicit distinct-category selection problems.

use crate::source_category_selection_pack::{CategorySelectionRequest, DOMAIN};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CategorySelectionFrontendStatus {
    Complete,
    Ambiguous,
    Missing,
    Unsupported,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CategorySelectionFrontendResult {
    pub status: CategorySelectionFrontendStatus,
    pub request: Option<CategorySelectionRequest>,
    pub reasons: Vec<String>,
    pub provenance_spans: Vec<String>,
    pub replay_hash: String,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn payload(result: &CategorySelectionFrontendResult) -> impl Serialize + '_ {
    (
        result.status,
        &result.request,
        &result.reasons,
        &result.provenance_spans,
    )
}

fn finish(
    status: CategorySelectionFrontendStatus,
    request: Option<CategorySelectionRequest>,
    reasons: Vec<String>,
    provenance_spans: Vec<String>,
) -> CategorySelectionFrontendResult {
    let mut result = CategorySelectionFrontendResult {
        status,
        request,
        reasons,
        provenance_spans,
        replay_hash: String::new(),
    };
    let replay_hash = digest(&payload(&result));
    result.replay_hash = replay_hash;
    result
}

fn number_after(text: &str, markers: &[&str]) -> Option<u64> {
    let lower = text.to_ascii_lowercase();
    markers.iter().find_map(|marker| {
        let start = lower.find(marker)? + marker.len();
        let digits: String = lower[start..]
            .chars()
            .skip_while(|character| !character.is_ascii_digit())
            .take_while(|character| character.is_ascii_digit())
            .collect();
        (!digits.is_empty()).then(|| digits.parse().ok()).flatten()
    })
}

pub fn formalize_category_selection_text(
    text: &str,
    case_id: &str,
) -> CategorySelectionFrontendResult {
    let lower = text.to_ascii_lowercase();
    let provenance = vec![
        format!("source-category-selection-frontend:{case_id}"),
        "explicit-equal-category-structure".into(),
    ];
    if [
        "at least",
        "at most",
        "replacement",
        "with replacement",
        "more than one",
        "multiple from",
        "unequal",
        "different numbers per",
        "infinite",
        "probability",
    ]
    .iter()
    .any(|marker| lower.contains(marker))
    {
        return finish(
            CategorySelectionFrontendStatus::Unsupported,
            None,
            vec!["request is outside the exact equal-category rule".into()],
            provenance,
        );
    }
    if lower.contains(" or ") || lower.contains("either") {
        return finish(
            CategorySelectionFrontendStatus::Ambiguous,
            None,
            vec!["multiple category-selection interpretations remain".into()],
            provenance,
        );
    }
    let selected = number_after(&lower, &["choose ", "select ", "pick ", "selecting "]);
    let categories = number_after(&lower, &["from ", "of "]);
    let choices_per_category = number_after(
        &lower,
        &[
            "each category has ",
            "each group has ",
            "each type has ",
            "each suit has ",
            "with ",
            " choices per category",
            " items per category",
            " options per group",
        ],
    );
    if selected.is_none() || categories.is_none() || choices_per_category.is_none() {
        return finish(
            CategorySelectionFrontendStatus::Missing,
            None,
            vec![
                "explicit selected-category count, category count, and per-category choices are required"
                    .into(),
            ],
            provenance,
        );
    }
    if !lower.contains("one from each")
        || !(lower.contains("order does not matter") || lower.contains("unordered"))
    {
        return finish(
            CategorySelectionFrontendStatus::Ambiguous,
            None,
            vec!["the one-per-category and unordered-selection semantics must be explicit".into()],
            provenance,
        );
    }
    let request = CategorySelectionRequest {
        category_count: categories.unwrap(),
        choices_per_category: choices_per_category.unwrap(),
        selected_categories: selected.unwrap(),
        order_matters: Some(false),
        domain: DOMAIN.into(),
        ambiguity: None,
        provenance: provenance.clone(),
    };
    finish(
        CategorySelectionFrontendStatus::Complete,
        Some(request),
        Vec::new(),
        provenance,
    )
}

pub fn replay_verified(result: &CategorySelectionFrontendResult) -> bool {
    result.replay_hash == digest(&payload(result))
        && !result.provenance_spans.is_empty()
        && (result.status != CategorySelectionFrontendStatus::Complete || result.request.is_some())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn binds_explicit_equal_category_structure() {
        let result = formalize_category_selection_text(
            "Choose 3 categories from 4 categories, with 13 choices per category, one from each selected category; order does not matter.",
            "supported",
        );
        assert_eq!(result.status, CategorySelectionFrontendStatus::Complete);
        assert_eq!(result.request.as_ref().unwrap().category_count, 4);
        assert_eq!(result.request.as_ref().unwrap().selected_categories, 3);
        assert!(replay_verified(&result));
    }

    #[test]
    fn refuses_unstated_cardinality_and_preserves_boundaries() {
        let missing = formalize_category_selection_text(
            "Choose 3 cards from 4 suits, one from each; order does not matter.",
            "missing-size",
        );
        assert_eq!(missing.status, CategorySelectionFrontendStatus::Missing);
        let unsupported = formalize_category_selection_text(
            "Choose at least 3 items from 4 equal groups, with 5 choices per group.",
            "at-least",
        );
        assert_eq!(
            unsupported.status,
            CategorySelectionFrontendStatus::Unsupported
        );
    }
}
