//! Shared route-blind evaluation for the Goal 6 external portfolio.
//!
//! Every question is offered to every validated shadow route. A candidate is
//! executable only when both its frontend and typed evaluator complete and
//! their receipts replay. This module does not read answer keys, select by
//! lexical hints, authorize production, or mutate a registry.

use crate::parameter_linear_system_frontend::{
    execute as execute_parameter_system, execution_replay_verified as parameter_execution_replay,
    formalize as formalize_parameter_system, replay_verified as parameter_frontend_replay,
};
use crate::source_base_conversion_frontend::{
    formalize_base_conversion_text, replay_verified as base_conversion_frontend_replay,
};
use crate::source_base_conversion_pack::{
    evaluate_base_conversion, replay_verified as base_conversion_replay, BaseConversionStatus,
};
use crate::source_category_selection_frontend::{
    formalize_category_selection_text, replay_verified as category_selection_frontend_replay,
};
use crate::source_category_selection_pack::{
    evaluate_category_selection, replay_verified as category_selection_replay,
    CategorySelectionStatus,
};
use crate::source_combination_frontend::{
    formalize_combination_text, replay_verified as combination_frontend_replay,
};
use crate::source_combination_pack::evaluate_combination;
use crate::source_counting_frontend::formalize_counting_text;
use crate::source_counting_pack::{
    evaluate as evaluate_counting, CountingArtifact, CountingStatus,
};
use crate::source_finite_experiment_frontend::{
    execute as execute_finite_experiment,
    execution_replay_verified as finite_experiment_execution_replay,
    formalize as formalize_finite_experiment, replay_verified as finite_experiment_frontend_replay,
    FrontendStatus as FiniteExperimentStatus,
};
use crate::source_formula_frontend::formalize_formula_text;
use crate::source_formula_pack::{
    evaluate_formula_records, extract_formula_records, source_formula_records, FormulaStatus,
};
use crate::source_mean_update_frontend::{
    formalize_mean_update_text, replay_verified as mean_update_frontend_replay,
};
use crate::source_mean_update_pack::evaluate as evaluate_mean_update;
use crate::source_progression_mean_frontend::formalize_progression_mean_text;
use crate::source_progression_mean_pack::evaluate as evaluate_progression_mean;
use crate::source_regression_pack::source_regression_frontend::formalize_regression_text;
use crate::source_sequence_frontend::{
    formalize_sequence_terms_text, replay_verified as sequence_frontend_replay,
};
use crate::source_statistics_frontend::{
    formalize_finite_list_mean_text, formalize_statistics_text,
};
use crate::source_statistics_pack::evaluate_statistics;
use crate::source_statistics_pack::records as statistics_records;
use crate::source_unit_frontend::{formalize_unit_text, replay_verified as unit_frontend_replay};
use crate::source_word_system_frontend::{
    execute_word_system, execution_replay_verified as word_system_execution_replay,
    formalize_two_number_system_v3, replay_verified as word_system_frontend_replay,
};
use serde::Serialize;

pub const SEQUENCE_DOMAIN: &str = "external-source-sequence-shadow";
pub const UNIT_DOMAIN: &str = "source_catalog_unit_conversion";
pub const UNIT_SOURCE: &str =
    include_str!("../docs/sources/openstax_unit_conversion_goal6_catalog.txt");
pub const GEOMETRY_DOMAIN: &str = "source_derived_bounded_geometry";
pub const GEOMETRY_SOURCE: &str =
    include_str!("../docs/sources/openstax_bounded_geometry_source.txt");
pub const BASE_CONVERSION_DOMAIN: &str = crate::source_base_conversion_pack::DOMAIN;
pub const CATEGORY_SELECTION_DOMAIN: &str = crate::source_category_selection_pack::DOMAIN;

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum PortfolioRoute {
    FiniteListMean,
    ArithmeticSequence,
    ArithmeticProgressionMean,
    NaturalCombination,
    MeanUpdate,
    BoundedCounting,
    UnitConversion,
    BoundedGeometry,
    FiniteRegression,
    FiniteStatistics,
    BaseConversion,
    CategorySelection,
    ParameterLinearSystem,
    WordSystem,
    FiniteDieExperiment,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case", tag = "kind", content = "value")]
pub enum PortfolioCandidate {
    Rational(crate::probability_pack::Rational),
    ExactCount(u128),
    Text(String),
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
        executable: candidate.is_some() && frontend_replay_verified && execution_replay_verified,
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

fn statistics_route(text: &str) -> RouteObservation {
    let frontend = formalize_statistics_text(text);
    let mut tampered = frontend.clone();
    tampered.replay_hash.push('x');
    let Some(request) = frontend.request.as_ref() else {
        return observation(
            PortfolioRoute::FiniteStatistics,
            format!("{:?}", frontend.status),
            "not_run",
            None,
            frontend.replay_verified(),
            false,
            !tampered.replay_verified(),
            false,
        );
    };
    let execution = evaluate_statistics(request);
    let mut execution_tampered = execution.clone();
    execution_tampered.replay_hash.push('x');
    let candidate = (execution.status == FormulaStatus::Complete)
        .then(|| execution.value.clone())
        .flatten()
        .map(PortfolioCandidate::Rational);
    observation(
        PortfolioRoute::FiniteStatistics,
        format!("{:?}", frontend.status),
        format!("{:?}", execution.status),
        candidate,
        frontend.replay_verified(),
        execution.replay_verified(),
        !tampered.replay_verified(),
        !execution_tampered.replay_verified(),
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

fn progression_mean_route(text: &str) -> RouteObservation {
    let frontend = formalize_progression_mean_text(text);
    let mut tampered = frontend.clone();
    tampered.replay_hash.push('x');
    let frontend_replay = crate::source_progression_mean_frontend::replay_verified(&frontend);
    let frontend_tamper = !crate::source_progression_mean_frontend::replay_verified(&tampered);
    let Some(request) = frontend.frontend.request.as_ref() else {
        return observation(
            PortfolioRoute::ArithmeticProgressionMean,
            format!("{:?}", frontend.frontend.status),
            "not_run",
            None,
            frontend_replay,
            false,
            frontend_tamper,
            false,
        );
    };
    let execution = evaluate_progression_mean(request);
    let mut execution_tampered = execution.clone();
    execution_tampered.replay_hash.push('x');
    let candidate = (execution.status == FormulaStatus::Complete)
        .then(|| execution.value.clone())
        .flatten()
        .map(PortfolioCandidate::Rational);
    observation(
        PortfolioRoute::ArithmeticProgressionMean,
        format!("{:?}", frontend.frontend.status),
        format!("{:?}", execution.status),
        candidate,
        frontend_replay,
        execution.replay_verified(),
        frontend_tamper,
        !execution_tampered.replay_verified(),
    )
}

fn natural_combination_route(text: &str, case_id: &str) -> RouteObservation {
    let frontend = formalize_combination_text(text, case_id);
    let mut tampered = frontend.clone();
    tampered.replay_hash.push('x');
    let frontend_replay = combination_frontend_replay(&frontend);
    let frontend_tamper = !combination_frontend_replay(&tampered);
    let Some(request) = frontend.request.as_ref() else {
        return observation(
            PortfolioRoute::NaturalCombination,
            format!("{:?}", frontend.status),
            "not_run",
            None,
            frontend_replay,
            false,
            frontend_tamper,
            false,
        );
    };
    let execution = evaluate_combination(request);
    let mut execution_tampered = execution.clone();
    execution_tampered.replay_hash.push('x');
    let candidate = (execution.status == CountingStatus::Complete)
        .then(|| execution.artifact.clone())
        .flatten()
        .map(|artifact| match artifact {
            CountingArtifact::ExactCount(value) => PortfolioCandidate::ExactCount(value),
        });
    observation(
        PortfolioRoute::NaturalCombination,
        format!("{:?}", frontend.status),
        format!("{:?}", execution.status),
        candidate,
        frontend_replay,
        crate::source_counting_pack::replay_verified(&execution),
        frontend_tamper,
        !crate::source_counting_pack::replay_verified(&execution_tampered),
    )
}

fn mean_update_route(text: &str) -> RouteObservation {
    let frontend = formalize_mean_update_text(text);
    let mut tampered = frontend.clone();
    tampered.replay_hash.push('x');
    let frontend_replay = mean_update_frontend_replay(&frontend);
    let frontend_tamper = !mean_update_frontend_replay(&tampered);
    let Some(request) = frontend.frontend.request.as_ref() else {
        return observation(
            PortfolioRoute::MeanUpdate,
            format!("{:?}", frontend.frontend.status),
            "not_run",
            None,
            frontend_replay,
            false,
            frontend_tamper,
            false,
        );
    };
    let execution = evaluate_mean_update(request);
    let mut execution_tampered = execution.clone();
    execution_tampered.replay_hash.push('x');
    let candidate = (execution.status == FormulaStatus::Complete)
        .then(|| execution.value.clone())
        .flatten()
        .map(PortfolioCandidate::Rational);
    observation(
        PortfolioRoute::MeanUpdate,
        format!("{:?}", frontend.frontend.status),
        format!("{:?}", execution.status),
        candidate,
        frontend_replay,
        execution.replay_verified(),
        frontend_tamper,
        !execution_tampered.replay_verified(),
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

fn regression_route(text: &str) -> RouteObservation {
    let frontend = formalize_regression_text(text);
    let mut frontend_tampered = frontend.clone();
    frontend_tampered.replay_hash.push('x');
    let frontend_replay = frontend.replay_verified();
    let frontend_tamper = !frontend_tampered.replay_verified();
    let Some(request) = frontend.request.as_ref() else {
        return observation(
            PortfolioRoute::FiniteRegression,
            format!("{:?}", frontend.status),
            "not_run",
            None,
            frontend_replay,
            false,
            frontend_tamper,
            false,
        );
    };
    let execution = crate::source_regression_pack::evaluate_regression(request);
    let mut execution_tampered = execution.clone();
    execution_tampered.replay_hash.push('x');
    let candidate = (execution.status == FormulaStatus::Complete)
        .then(|| execution.value.clone())
        .flatten()
        .map(PortfolioCandidate::Rational);
    observation(
        PortfolioRoute::FiniteRegression,
        format!("{:?}", frontend.status),
        format!("{:?}", execution.status),
        candidate,
        frontend_replay,
        execution.replay_verified(),
        frontend_tamper,
        !execution_tampered.replay_verified(),
    )
}

fn base_conversion_route(text: &str, case_id: &str) -> RouteObservation {
    let frontend = formalize_base_conversion_text(text, case_id);
    let mut frontend_tampered = frontend.clone();
    frontend_tampered.replay_hash.push('x');
    let frontend_replay = base_conversion_frontend_replay(&frontend);
    let frontend_tamper = !base_conversion_frontend_replay(&frontend_tampered);
    let Some(request) = frontend.request.as_ref() else {
        return observation(
            PortfolioRoute::BaseConversion,
            format!("{:?}", frontend.status),
            "not_run",
            None,
            frontend_replay,
            false,
            frontend_tamper,
            false,
        );
    };
    let execution = evaluate_base_conversion(request);
    let mut execution_tampered = execution.clone();
    execution_tampered.replay_hash.push('x');
    let candidate = (execution.status == BaseConversionStatus::Complete)
        .then(|| execution.numeral.clone())
        .flatten()
        .map(PortfolioCandidate::Text);
    observation(
        PortfolioRoute::BaseConversion,
        format!("{:?}", frontend.status),
        format!("{:?}", execution.status),
        candidate,
        frontend_replay,
        base_conversion_replay(&execution),
        frontend_tamper,
        !base_conversion_replay(&execution_tampered),
    )
}

fn category_selection_route(text: &str, case_id: &str) -> RouteObservation {
    let frontend = formalize_category_selection_text(text, case_id);
    let mut frontend_tampered = frontend.clone();
    frontend_tampered.replay_hash.push('x');
    let frontend_replay = category_selection_frontend_replay(&frontend);
    let frontend_tamper = !category_selection_frontend_replay(&frontend_tampered);
    let Some(request) = frontend.request.as_ref() else {
        return observation(
            PortfolioRoute::CategorySelection,
            format!("{:?}", frontend.status),
            "not_run",
            None,
            frontend_replay,
            false,
            frontend_tamper,
            false,
        );
    };
    let execution = evaluate_category_selection(request);
    let mut execution_tampered = execution.clone();
    execution_tampered.replay_hash.push('x');
    let candidate = (execution.status == CategorySelectionStatus::Complete)
        .then_some(execution.count)
        .flatten()
        .map(PortfolioCandidate::ExactCount);
    observation(
        PortfolioRoute::CategorySelection,
        format!("{:?}", frontend.status),
        format!("{:?}", execution.status),
        candidate,
        frontend_replay,
        category_selection_replay(&execution),
        frontend_tamper,
        !category_selection_replay(&execution_tampered),
    )
}

fn parameter_linear_system_route(text: &str, case_id: &str) -> RouteObservation {
    let frontend = formalize_parameter_system(text, case_id);
    let mut frontend_tampered = frontend.clone();
    frontend_tampered.replay_hash.push('x');
    let frontend_replay = parameter_frontend_replay(&frontend);
    let frontend_tamper = !parameter_frontend_replay(&frontend_tampered);
    let Some(request) = frontend.request.as_ref() else {
        return observation(
            PortfolioRoute::ParameterLinearSystem,
            format!("{:?}", frontend.status),
            "not_run",
            None,
            frontend_replay,
            false,
            frontend_tamper,
            false,
        );
    };
    let execution = execute_parameter_system(request);
    let mut execution_tampered = execution.clone();
    execution_tampered.replay_hash.push('x');
    let candidate = (execution.status
        == crate::parameter_linear_system_frontend::FrontendStatus::Complete)
        .then(|| execution.value.clone())
        .flatten()
        .map(PortfolioCandidate::Rational);
    observation(
        PortfolioRoute::ParameterLinearSystem,
        format!("{:?}", frontend.status),
        format!("{:?}", execution.status),
        candidate,
        frontend_replay,
        parameter_execution_replay(&execution),
        frontend_tamper,
        !parameter_execution_replay(&execution_tampered),
    )
}

fn word_system_route(text: &str, case_id: &str) -> RouteObservation {
    let frontend = formalize_two_number_system_v3(text, case_id);
    let mut frontend_tampered = frontend.clone();
    frontend_tampered.replay_hash.push('x');
    let frontend_replay = word_system_frontend_replay(&frontend);
    let frontend_tamper = !word_system_frontend_replay(&frontend_tampered);
    let Some(execution) = execute_word_system(&frontend) else {
        return observation(
            PortfolioRoute::WordSystem,
            format!("{:?}", frontend.status),
            "not_run",
            None,
            frontend_replay,
            false,
            frontend_tamper,
            false,
        );
    };
    let mut execution_tampered = execution.clone();
    execution_tampered.result.push('x');
    let candidate = (frontend.status
        == crate::source_word_system_frontend::WordSystemStatus::Complete)
        .then(|| execution.result.clone())
        .map(PortfolioCandidate::Text);
    observation(
        PortfolioRoute::WordSystem,
        format!("{:?}", frontend.status),
        "complete",
        candidate,
        frontend_replay,
        word_system_execution_replay(&execution),
        frontend_tamper,
        !word_system_execution_replay(&execution_tampered),
    )
}

fn finite_die_experiment_route(text: &str, case_id: &str) -> RouteObservation {
    let frontend = formalize_finite_experiment(text, case_id);
    let mut frontend_tampered = frontend.clone();
    frontend_tampered.replay_hash.push('x');
    let frontend_replay = finite_experiment_frontend_replay(&frontend);
    let frontend_tamper = !finite_experiment_frontend_replay(&frontend_tampered);
    let Some(execution) = execute_finite_experiment(&frontend) else {
        return observation(
            PortfolioRoute::FiniteDieExperiment,
            format!("{:?}", frontend.status),
            "not_run",
            None,
            frontend_replay,
            false,
            frontend_tamper,
            false,
        );
    };
    let mut execution_tampered = execution.clone();
    execution_tampered.replay_hash.push('x');
    let candidate = (execution.status == FiniteExperimentStatus::Complete)
        .then(|| execution.value.clone())
        .flatten()
        .map(PortfolioCandidate::Rational);
    observation(
        PortfolioRoute::FiniteDieExperiment,
        format!("{:?}", frontend.status),
        format!("{:?}", execution.status),
        candidate,
        frontend_replay,
        finite_experiment_execution_replay(&execution),
        frontend_tamper,
        !finite_experiment_execution_replay(&execution_tampered),
    )
}

/// Offer one prompt to every portfolio route, without a lexical pre-dispatch.
pub fn observe_all(text: &str, case_id: &str) -> Vec<RouteObservation> {
    vec![
        mean_route(text),
        statistics_route(text),
        sequence_route(text, case_id),
        progression_mean_route(text),
        natural_combination_route(text, case_id),
        mean_update_route(text),
        counting_route(text, case_id),
        unit_route(text, case_id),
        geometry_route(text),
        regression_route(text),
        base_conversion_route(text, case_id),
        category_selection_route(text, case_id),
        parameter_linear_system_route(text, case_id),
        word_system_route(text, case_id),
        finite_die_experiment_route(text, case_id),
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
        assert_eq!(observations.len(), 15);
        let executable = executable_routes(&observations);
        assert_eq!(executable.len(), 1);
        assert_eq!(executable[0].route, PortfolioRoute::FiniteListMean);
        assert!(observations
            .iter()
            .all(|item| item.frontend_replay_verified));
        assert!(observations
            .iter()
            .all(|item| item.frontend_tamper_rejected));
    }

    #[test]
    fn route_blind_natural_combination_selects_only_combination() {
        let observations = observe_all(
            "In how many ways can a student choose three out of eight classes?",
            "test-combination",
        );
        let executable = executable_routes(&observations);
        assert_eq!(executable.len(), 1);
        assert_eq!(executable[0].route, PortfolioRoute::NaturalCombination);
        assert_eq!(
            executable[0].candidate,
            Some(PortfolioCandidate::ExactCount(56))
        );
        assert!(observations
            .iter()
            .all(|item| item.frontend_replay_verified));
        assert!(observations
            .iter()
            .all(|item| item.frontend_tamper_rejected));
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
    fn route_blind_labeled_statistics_selects_only_statistics_route() {
        let observations = observe_all(
            "Find the expected value of a binomial variable with n=8 and p=1/4.",
            "test-statistics",
        );
        let executable = executable_routes(&observations);
        assert_eq!(executable.len(), 1);
        assert_eq!(executable[0].route, PortfolioRoute::FiniteStatistics);
        assert!(executable[0].execution_replay_verified);
        assert!(executable[0].execution_tamper_rejected);
    }

    #[test]
    fn route_blind_regression_selects_only_regression_route() {
        let observations = observe_all(
            "Find the slope with covariance_sum=12 and x_variance_sum=4.",
            "test-regression",
        );
        let executable = executable_routes(&observations);
        assert_eq!(executable.len(), 1);
        assert_eq!(executable[0].route, PortfolioRoute::FiniteRegression);
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

    #[test]
    fn route_blind_base_conversion_selects_only_base_route() {
        let observations = observe_all("Convert $10101_3$ to a base 10 integer.", "test-base");
        let executable = executable_routes(&observations);
        assert_eq!(executable.len(), 1);
        assert_eq!(executable[0].route, PortfolioRoute::BaseConversion);
        assert_eq!(
            executable[0].candidate,
            Some(PortfolioCandidate::Text("91".into()))
        );
        assert!(executable[0].execution_replay_verified);
        assert!(executable[0].execution_tamper_rejected);
    }

    #[test]
    fn route_blind_category_selection_selects_only_category_route() {
        let observations = observe_all(
            "Choose 3 categories from 4 categories, with 13 choices per category, one from each selected category; order does not matter.",
            "test-category-selection",
        );
        let executable = executable_routes(&observations);
        assert_eq!(executable.len(), 1);
        assert_eq!(executable[0].route, PortfolioRoute::CategorySelection);
        assert_eq!(
            executable[0].candidate,
            Some(PortfolioCandidate::ExactCount(8788))
        );
        assert!(executable[0].execution_replay_verified);
        assert!(executable[0].execution_tamper_rejected);
    }

    #[test]
    fn route_blind_parameter_system_selects_only_parameter_route() {
        let observations = observe_all(
            "The system is 3*x+y=a; 2*x+5*y=2*a. Given x=2, compute a.",
            "test-parameter-system",
        );
        let executable = executable_routes(&observations);
        assert_eq!(executable.len(), 1);
        assert_eq!(executable[0].route, PortfolioRoute::ParameterLinearSystem);
        assert_eq!(
            executable[0].candidate,
            Some(PortfolioCandidate::Rational(
                crate::probability_pack::Rational::new(26, 3).unwrap()
            ))
        );
        assert!(executable[0].execution_replay_verified);
        assert!(executable[0].execution_tamper_rejected);
    }

    #[test]
    fn route_blind_word_system_selects_only_word_system_route() {
        let observations = observe_all(
            "The sum of two numbers is twenty. One number is four less than the other. Find the numbers.",
            "test-word-system",
        );
        let executable = executable_routes(&observations);
        assert_eq!(executable.len(), 1);
        assert_eq!(executable[0].route, PortfolioRoute::WordSystem);
        assert_eq!(
            executable[0].candidate,
            Some(PortfolioCandidate::Text(
                "{\"x\": \"8\", \"y\": \"12\"}".into()
            ))
        );
        assert!(executable[0].execution_replay_verified);
        assert!(executable[0].execution_tamper_rejected);
    }

    #[test]
    fn route_blind_finite_die_experiment_selects_only_die_route() {
        let observations = observe_all(
            "Two fair six-sided dice are rolled. What is the probability that their sum is 7?",
            "test-finite-die",
        );
        let executable = executable_routes(&observations);
        assert_eq!(executable.len(), 1);
        assert_eq!(executable[0].route, PortfolioRoute::FiniteDieExperiment);
        assert_eq!(
            executable[0].candidate,
            Some(PortfolioCandidate::Rational(
                crate::probability_pack::Rational::new(1, 6).unwrap()
            ))
        );
        assert!(executable[0].execution_replay_verified);
        assert!(executable[0].execution_tamper_rejected);
    }
}
