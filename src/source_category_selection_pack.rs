//! Source-derived bounded selection from distinct equal-sized categories.
//!
//! The rule is an explicit composition of the cited combination and
//! multiplication primitives. It is intentionally narrower than general
//! occupancy or constrained-card selection.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const DOMAIN: &str = "source_derived_distinct_category_selection";
pub const SOURCE: &str = include_str!("../docs/sources/openstax_category_selection_source.txt");

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CategorySelectionStatus {
    Complete,
    Missing,
    Ambiguous,
    Unsupported,
    InvalidRange,
    Overflow,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CategorySelectionRequest {
    pub category_count: u64,
    pub choices_per_category: u64,
    pub selected_categories: u64,
    pub order_matters: Option<bool>,
    pub domain: String,
    pub ambiguity: Option<String>,
    pub provenance: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CategorySelectionResult {
    pub status: CategorySelectionStatus,
    pub count: Option<u128>,
    pub category_count: u64,
    pub choices_per_category: u64,
    pub selected_categories: u64,
    pub assumptions: Vec<String>,
    pub source_provenance: Vec<String>,
    pub reasons: Vec<String>,
    pub provenance: Vec<String>,
    pub replay_hash: String,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn payload(result: &CategorySelectionResult) -> impl Serialize + '_ {
    (
        result.status,
        result.count,
        result.category_count,
        result.choices_per_category,
        result.selected_categories,
        &result.assumptions,
        &result.source_provenance,
        &result.reasons,
        &result.provenance,
    )
}

fn source_provenance() -> Vec<String> {
    SOURCE
        .lines()
        .filter_map(|line| line.strip_prefix("SOURCE_ID: ").map(str::to_owned))
        .chain(
            SOURCE
                .lines()
                .filter_map(|line| line.strip_prefix("DERIVED_RULE: ").map(str::to_owned)),
        )
        .collect()
}

pub fn source_valid() -> bool {
    [
        "SOURCE_ID:",
        "URL:",
        "EVIDENCE:",
        "DERIVED_RULE:",
        "SCOPE:",
        "UNSUPPORTED:",
    ]
    .iter()
    .all(|field| SOURCE.contains(field))
}

fn finish(
    request: &CategorySelectionRequest,
    status: CategorySelectionStatus,
    count: Option<u128>,
    reasons: Vec<String>,
) -> CategorySelectionResult {
    let mut result = CategorySelectionResult {
        status,
        count,
        category_count: request.category_count,
        choices_per_category: request.choices_per_category,
        selected_categories: request.selected_categories,
        assumptions: vec![
            "finite exact counting model".into(),
            "categories have equal known cardinality".into(),
            "exactly one item is selected from each selected category".into(),
            "category order does not matter".into(),
            "category_count <= 20 and choices_per_category <= 100".into(),
        ],
        source_provenance: source_provenance(),
        reasons,
        provenance: request.provenance.clone(),
        replay_hash: String::new(),
    };
    let replay_hash = digest(&payload(&result));
    result.replay_hash = replay_hash;
    result
}

fn choose(n: u64, r: u64) -> Option<u128> {
    if r > n {
        return None;
    }
    let r = r.min(n - r);
    (0..r).try_fold(1_u128, |value, index| {
        value
            .checked_mul((n - index) as u128)?
            .checked_div((index + 1) as u128)
    })
}

pub fn evaluate_category_selection(request: &CategorySelectionRequest) -> CategorySelectionResult {
    if request.domain != DOMAIN {
        return finish(
            request,
            CategorySelectionStatus::Unsupported,
            None,
            vec!["request domain is outside the source-derived category rule".into()],
        );
    }
    if request.provenance.is_empty() || !source_valid() {
        return finish(
            request,
            CategorySelectionStatus::Missing,
            None,
            vec!["source record or request provenance is incomplete".into()],
        );
    }
    if let Some(reason) = &request.ambiguity {
        return finish(
            request,
            CategorySelectionStatus::Ambiguous,
            None,
            vec![reason.clone()],
        );
    }
    match request.order_matters {
        None => {
            return finish(
                request,
                CategorySelectionStatus::Ambiguous,
                None,
                vec!["category order must be explicitly irrelevant".into()],
            )
        }
        Some(true) => {
            return finish(
                request,
                CategorySelectionStatus::Unsupported,
                None,
                vec!["ordered category selections are outside this rule".into()],
            )
        }
        Some(false) => {}
    }
    if request.category_count > 20
        || request.choices_per_category > 100
        || request.selected_categories > request.category_count
    {
        return finish(
            request,
            CategorySelectionStatus::InvalidRange,
            None,
            vec![
                "requires selected_categories <= category_count <= 20 and choices_per_category <= 100"
                    .into(),
            ],
        );
    }
    let Some(category_choices) = choose(request.category_count, request.selected_categories) else {
        return finish(
            request,
            CategorySelectionStatus::InvalidRange,
            None,
            vec!["selected category count exceeds category count".into()],
        );
    };
    let item_choices = (0..request.selected_categories).try_fold(1_u128, |value, _| {
        value.checked_mul(request.choices_per_category as u128)
    });
    let Some(count) = item_choices.and_then(|items| category_choices.checked_mul(items)) else {
        return finish(
            request,
            CategorySelectionStatus::Overflow,
            None,
            vec!["exact category selection count overflowed".into()],
        );
    };
    finish(
        request,
        CategorySelectionStatus::Complete,
        Some(count),
        Vec::new(),
    )
}

pub fn replay_verified(result: &CategorySelectionResult) -> bool {
    result.replay_hash == digest(&payload(result)) && !result.provenance.is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> CategorySelectionRequest {
        CategorySelectionRequest {
            category_count: 4,
            choices_per_category: 13,
            selected_categories: 3,
            order_matters: Some(false),
            domain: DOMAIN.into(),
            ambiguity: None,
            provenance: vec!["unit-test:category-selection".into()],
        }
    }

    #[test]
    fn counts_distinct_equal_sized_category_selection() {
        let result = evaluate_category_selection(&request());
        assert_eq!(result.status, CategorySelectionStatus::Complete);
        assert_eq!(result.count, Some(8788));
        assert!(replay_verified(&result));
    }

    #[test]
    fn preserves_order_ambiguity_and_rejects_ordered_requests() {
        let mut ambiguous = request();
        ambiguous.order_matters = None;
        assert_eq!(
            evaluate_category_selection(&ambiguous).status,
            CategorySelectionStatus::Ambiguous
        );
        let mut ordered = request();
        ordered.order_matters = Some(true);
        assert_eq!(
            evaluate_category_selection(&ordered).status,
            CategorySelectionStatus::Unsupported
        );
    }

    #[test]
    fn rejects_out_of_scope_ranges_and_tampering() {
        let mut invalid = request();
        invalid.selected_categories = 5;
        assert_eq!(
            evaluate_category_selection(&invalid).status,
            CategorySelectionStatus::InvalidRange
        );
        let result = evaluate_category_selection(&request());
        let mut tampered = result.clone();
        tampered.replay_hash.push('x');
        assert!(!replay_verified(&tampered));
    }
}
