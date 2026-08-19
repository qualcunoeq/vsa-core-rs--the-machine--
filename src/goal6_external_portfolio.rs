//! Shared route-blind evaluation for the Goal 6 external portfolio.
//!
//! Every question is offered to every validated shadow route. A candidate is
//! executable only when both its frontend and typed evaluator complete and
//! their receipts replay. This module does not read answer keys, select by
//! lexical hints, authorize production, or mutate a registry.

use crate::source_counting_frontend::formalize_counting_text;
use crate::source_counting_pack::{evaluate as evaluate_counting, CountingArtifact, CountingStatus};
use crate::source_formula_pack::{
    evaluate_formula_records, extract_formula_records, source_formula_records, FormulaStatus,
};
use crate::source_formula_frontend::formalize_formula_text;
use crate::source_sequence_frontend::{
    formalize_sequence_terms_text, replay_verified as sequence_frontend_replay,
};
use crate::source_statistics_frontend::formalize_finite_list_mean_text;
use crate::source_statistics_pack::records as statistics_records;
use crate::source_unit_frontend::{
    formalize_unit_text, replay_verified as unit_frontend_replay,
};
use serde::Serialize;

pub const SEQUENCE_DOMAIN: &str = "external-source-sequence-shadow";
pub const UNIT_DOMAIN: &str = "source_catalog_unit_conversion";
pub const UNIT_SOURCE: &str =
    include_str!("../docs/sources/openstax_unit_conversion_goal6_catalog.txt");
pub const GEOMETRY_DOMAIN: &str = "source_derived_bounded_geometry";
pub const GEOMETRY_SOURCE: &str =
    include_str!("../docs/sources/openstax_bounded_geometry_source.txt");

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum PortfolioRoute {
    FiniteListMean,
    ArithmeticSequence,
    BoundedCounting,
    UnitConversion,
    BoundedGeometry,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case", tag = "kind", content = "value")]
pub enum PortfolioCandidate {
    Rational(crate::probability_pack::Rational),
    ExactCount(u128),
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct RouteObservation {
    pub route: PortfolioRoute,
    pub frontend_status: String,
    pub execution_status: String,
    pub executable: bool,
    pub candidate: Option<PortfolioCandidate>,
    pub frontend_replay_verified: bool,
    pub execution_replay_verified: bool,
    pub frontend_tamper_rejected: bool,
    pub execution_tamper_rejected: bool,
}

fn observation(
    route: PortfolioRoute,
    frontend_status: impl Into<String>,
    execution_status: impl Into<String>,
    candidate: Option<PortfolioCandidate>,
    frontend_replay_verified: bool,
    execution_replay_verified: bool,
    frontend_tamper_rejected: bool,
    execution_tamper_rejected: bool,
) -> RouteObservation {
    RouteObservation {
        route,
        frontend_status: frontend_status.into(),
        execution_status: execution_status.into(),
        executable: candidate.is_some()
            && frontend_replay_verified
            && execution_replay_verified,
        candidate,
        frontend_replay_verified,
        execution_replay_verified,
        frontend_tamper_rejected,
        execution_tamper_rejected,
    }
}

fn formula_rational_observation(
    route: PortfolioRoute,
    frontend_status: impl Into<String>,
    frontend_replay_verified: bool,
    frontend_tamper_rejected: bool,
    request: Option<&crate::source_formula_pack::FormulaRequest>,
    domain: &str,
    records: &[crate::source_formula_pack::FormulaRecord],
) -> RouteObservation {
    let Some(request) = request else {
        return observation(
            route,
            frontend_status,
            "not_run",
            None,
            frontend_replay_verified,
            false,
            frontend_tamper_rejected,
            false,
        );
    };
    let execution = evaluate_formula_records(request, domain, records);
    let mut tampered = execution.clone();
    tampered.replay_hash.push('x');
    let execution_replay_verified = execution.replay_verified();
    let candidate = (execution.status == FormulaStatus::Complete)
        .then(|| execution.value.clone())
        .flatten()
        .map(PortfolioCandidate::Rational);
    observation(
        route,
        frontend_status,
        format!("{:?}", execution.status),
        candidate,
        frontend_replay_verified,
        execution_replay_verified,
        frontend_tamper_rejected,
        !tampered.replay_verified(),
    )
}

fn mean_route(text: &str) -> RouteObservation {
    let frontend = formalize_finite_list_mean_text(text);
    let mut tampered = frontend.clone();
    tampered.replay_hash.push('x');
    formula_rational_observation(
        PortfolioRoute::FiniteListMean,
        format!("{:?}", frontend.status),
        frontend.replay_verified(),
        !tampered.replay_verified(),
        frontend.request.as_ref(),
        crate::source_statistics_pack::DOMAIN,
        &statistics_records(),
    )
}

fn sequence_route(text: &str, case_id: &str) -> RouteObservation {
    let frontend = formalize_sequence_terms_text(text, case_id, SEQUENCE_DOMAIN);
    let mut tampered = frontend.clone();
    tampered.replay_hash.push('x');
    formula_rational_observation(
        PortfolioRoute::ArithmeticSequence,
        format!("{:?}", frontend.status),
        sequence_frontend_replay(&frontend),
        !sequence_frontend_replay(&tampered),
        frontend.request.as_ref(),
        SEQUENCE_DOMAIN,
        &source_formula_records(),
    )
}

fn counting_route(text: &str, case_id: &str) -> RouteObservation {
    let frontend = formalize_counting_text(text, case_id);
    let mut frontend_tampered = frontend.clone();
    frontend_tampered.replay_hash.push('x');
    let frontend_replay = crate::source_counting_frontend::replay_verified(&frontend);
    let frontend_tamper = !crate::source_counting_frontend::replay_verified(&frontend_tampered);
    let Some(request) = frontend.request.as_ref() else {
        return observation(
            PortfolioRoute::BoundedCounting,
            format!("{:?}", frontend.status),
            "not_run",
            None,
            frontend_replay,
            false,
            frontend_tamper,
            false,
        );
    };
    let execution = evaluate_counting(request);
    let mut execution_tampered = execution.clone();
    execution_tampered.replay_hash.push('x');
    let candidate = (execution.status == CountingStatus::Complete)
        .then(|| execution.artifact.clone())
        .flatten()
        .map(|artifact| match artifact {
            CountingArtifact::ExactCount(value) => PortfolioCandidate::ExactCount(value),
        });
    observation(
        PortfolioRoute::BoundedCounting,
        format!("{:?}", frontend.status),
        format!("{:?}", execution.status),
        candidate,
        frontend_replay,
        crate::source_counting_pack::replay_verified(&execution),
        frontend_tamper,
        !crate::source_counting_pack::replay_verified(&execution_tampered),
    )
}

fn unit_route(text: &str, case_id: &str) -> RouteObservation {
    let records = extract_formula_records(UNIT_SOURCE).expect("unit source extracts");
    let frontend = formalize_unit_text(text, case_id, &records);
    let mut tampered = frontend.clone();
    tampered.replay_hash.push('x');
    formula_rational_observation(
        PortfolioRoute::UnitConversion,
        format!("{:?}", frontend.status),
        unit_frontend_replay(&frontend),
        !unit_frontend_replay(&tampered),
        frontend.request.as_ref(),
        UNIT_DOMAIN,
        &records,
    )
}

fn geometry_route(text: &str) -> RouteObservation {
    let records = extract_formula_records(GEOMETRY_SOURCE).expect("geometry source extracts");
    let frontend = formalize_formula_text(text, GEOMETRY_DOMAIN, &records);
    let mut tampered = frontend.clone();
    tampered.replay_hash.push('x');
    formula_rational_observation(
        PortfolioRoute::BoundedGeometry,
        format!("{:?}", frontend.status),
        frontend.replay_verified(),
        !tampered.replay_verified(),
        frontend.request.as_ref(),
        GEOMETRY_DOMAIN,
        &records,
    )
}

/// Offer one prompt to every portfolio route, without a lexical pre-dispatch.
pub fn observe_all(text: &str, case_id: &str) -> Vec<RouteObservation> {
    vec![
        mean_route(text),
        sequence_route(text, case_id),
        counting_route(text, case_id),
        unit_route(text, case_id),
        geometry_route(text),
    ]
}

pub fn executable_routes(observations: &[RouteObservation]) -> Vec<&RouteObservation> {
    observations
        .iter()
        .filter(|observation| observation.executable)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn route_blind_mean_selects_only_mean() {
        let observations = observe_all("Find the arithmetic mean of {2, 4, 8}.", "test-mean");
        assert_eq!(observations.len(), 5);
        let executable = executable_routes(&observations);
        assert_eq!(executable.len(), 1);
        assert_eq!(executable[0].route, PortfolioRoute::FiniteListMean);
        assert!(observations.iter().all(|item| item.frontend_replay_verified));
        assert!(observations.iter().all(|item| item.frontend_tamper_rejected));
    }

    #[test]
    fn route_blind_geometry_selects_only_geometry_route() {
        let observations = observe_all(
            "Compute the rectangle area using length=4 and width=3.",
            "test-geometry",
        );
        let executable = executable_routes(&observations);
        assert_eq!(executable.len(), 1);
        assert_eq!(executable[0].route, PortfolioRoute::BoundedGeometry);
        assert!(executable[0].execution_replay_verified);
        assert!(executable[0].execution_tamper_rejected);
    }

    #[test]
    fn route_blind_unit_conversion_selects_only_unit_route() {
        let observations = observe_all(
            "Express 60 inches in centimeters using the conversion 1 inch = 2.54 cm.",
            "test-unit",
        );
        let executable = executable_routes(&observations);
        assert_eq!(executable.len(), 1);
        assert_eq!(executable[0].route, PortfolioRoute::UnitConversion);
        assert!(executable[0].execution_replay_verified);
        assert!(executable[0].execution_tamper_rejected);
    }
}
