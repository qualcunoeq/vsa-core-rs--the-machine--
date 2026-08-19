//! Route-blind dispatcher for validated technical-language frontends.
//!
//! Every candidate frontend receives the same text.  A route is authorized
//! only when exactly one frontend produces a complete, replayable downstream
//! artifact.  A completed parse alone is never sufficient.

use crate::bounded_complex_analysis_frontend::{
    formalize as formalize_complex, replay_verified as complex_frontend_replay,
    FrontendStatus as ComplexFrontendStatus,
};
use crate::bounded_complex_analysis_pack::{
    evaluate_complex_analysis, replay_verified as complex_analysis_replay, ComplexAnalysisStatus,
};
use crate::combinatorics_frontend::{
    formalize as formalize_combinatorics, replay_verified as combinatorics_frontend_replay,
    CombinatoricsFrontendStatus,
};
use crate::combinatorics_pack::{evaluate_combinatorics, CombinatoricsStatus};
use crate::electromagnetism_frontend::{
    downstream_replay as em_downstream_replay, formalize_em_text,
    replay_verified as em_frontend_replay, EmFrontendStatus,
};
use crate::electromagnetism_pack::{evaluate as evaluate_em, EmStatus};
use crate::finite_markov_frontend::{
    formalize as formalize_markov, replay_verified as markov_frontend_replay,
    MarkovFrontendRequest, MarkovFrontendStatus,
};
use crate::finite_markov_hitting_pack::{evaluate as evaluate_hitting, HittingStatus};
use crate::finite_markov_stationary_pack::{evaluate as evaluate_stationary, StationaryStatus};
use crate::finite_state_contract::{formalize_technical as formalize_state, StateDecision};
use crate::mobius_frontend::{formalize_mobius_text, MobiusFrontendStatus};
use crate::mobius_inversion_pack::{evaluate as evaluate_mobius, MobiusStatus};
use crate::number_theory_frontend::{
    formalize_number_theory_text, replay_verified as number_frontend_replay,
    NumberTheoryFrontendStatus,
};
use crate::number_theory_pack::{evaluate_number_theory, NumberTheoryStatus};
use crate::ode_frontend::{
    downstream_replay as ode_downstream_replay, formalize_ode_text,
    replay_verified as ode_frontend_replay, OdeFrontendStatus,
};
use crate::ode_pack::{evaluate_ode, OdeStatus};
use crate::polynomial_frontend::{
    downstream_replay as polynomial_downstream_replay, formalize_polynomial_text,
    replay_verified as polynomial_frontend_replay, PolynomialFrontendStatus,
};
use crate::polynomial_pack::evaluate_polynomial;
use crate::source_formula_pack::biology_pack::biology_frontend::{
    formalize_biology_text, BiologyFrontendStatus,
};
use crate::source_formula_pack::biology_pack::{evaluate_biology, BiologyStatus};
use crate::source_formula_pack::chemistry_pack::chemistry_frontend::{
    formalize_chemistry_text, FrontendStatus as ChemistryFrontendStatus,
};
use crate::source_formula_pack::chemistry_pack::{evaluate_chemistry, ChemistryStatus};
use crate::source_metric_pack::source_metric_frontend::{
    formalize_metric_text, FrontendStatus as MetricFrontendStatus,
};
use crate::source_metric_pack::{evaluate_metric, MetricDefinitionRecord, MetricStatus};
use crate::source_topology_frontend::{formalize_topology_text, TopologyFrontendStatus};
use crate::source_topology_pack::{evaluate_topology, TopologyDefinitionRecord, TopologyStatus};
use crate::spectral_frontend::{formalize_spectral_text, SpectralFrontendStatus};
use crate::spectral_linear_algebra_pack::{evaluate_spectral, SpectralStatus};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::sync::OnceLock;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum RouteDomain {
    ComplexAnalysis,
    Combinatorics,
    FiniteStateTransition,
    MarkovHitting,
    MarkovStationary,
    Mobius,
    NumberTheory,
    FiniteMetric,
    FiniteTopology,
    Chemistry,
    Biology,
    ODE,
    Polynomial,
    Spectral,
    Electromagnetism,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RouteStatus {
    Authorized,
    Ambiguous,
    Unsupported,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RouteDecision {
    pub status: RouteStatus,
    pub selected: Option<RouteDomain>,
    pub authorized_candidates: Vec<RouteDomain>,
    pub ambiguous_candidates: Vec<RouteDomain>,
    pub provenance: Vec<String>,
    pub reasons: Vec<String>,
    pub replay_hash: String,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(value).expect("route serializes"))
    )
}

fn metric_records() -> &'static [MetricDefinitionRecord] {
    static RECORDS: OnceLock<Vec<MetricDefinitionRecord>> = OnceLock::new();
    RECORDS.get_or_init(|| {
        let source =
            include_str!("../docs/sources/topology_without_tears_finite_metric_definition.txt");
        let records = crate::source_metric_pack::extract_metric_definitions(source)
            .expect("validated finite-metric source record");
        crate::source_metric_pack::validate_metric_definitions(&records)
            .expect("finite-metric source record validates");
        records
    })
}

fn topology_records() -> &'static [TopologyDefinitionRecord] {
    static RECORDS: OnceLock<Vec<TopologyDefinitionRecord>> = OnceLock::new();
    RECORDS.get_or_init(|| {
        let source = include_str!("../docs/sources/topology_without_tears_finite_definition.txt");
        let records = crate::source_topology_pack::extract_topology_definitions(source)
            .expect("validated finite-topology source record");
        crate::source_topology_pack::validate_topology_definitions(&records)
            .expect("finite-topology source record validates");
        records
    })
}

fn payload(decision: &RouteDecision) -> impl Serialize + '_ {
    (
        decision.status,
        decision.selected,
        &decision.authorized_candidates,
        &decision.ambiguous_candidates,
        &decision.provenance,
        &decision.reasons,
    )
}

fn finish(
    status: RouteStatus,
    authorized_candidates: Vec<RouteDomain>,
    ambiguous_candidates: Vec<RouteDomain>,
    text: &str,
    reasons: Vec<String>,
) -> RouteDecision {
    let selected = (status == RouteStatus::Authorized)
        .then(|| authorized_candidates.first().copied())
        .flatten();
    let mut decision = RouteDecision {
        status,
        selected,
        authorized_candidates,
        ambiguous_candidates,
        provenance: vec![format!("route-source-span:0..{}", text.len())],
        reasons,
        replay_hash: String::new(),
    };
    let replay_hash = digest(&(
        decision.status,
        decision.selected,
        decision.authorized_candidates.clone(),
        decision.ambiguous_candidates.clone(),
        decision.provenance.clone(),
        decision.reasons.clone(),
    ));
    decision.replay_hash = replay_hash;
    decision
}

/// Offer one text to all validated frontends and select only a unique route.
pub fn route(text: &str, case_id: &str) -> RouteDecision {
    let complex = formalize_complex(text, case_id);
    let combinatorics = formalize_combinatorics(text, case_id);
    let number = formalize_number_theory_text(text, case_id);
    let markov = formalize_markov(text, case_id);
    let mut authorized = Vec::new();
    let mut ambiguous = Vec::new();
    if complex.status == ComplexFrontendStatus::Complete
        && complex_frontend_replay(&complex)
        && complex.request.as_ref().is_some_and(|request| {
            let result = evaluate_complex_analysis(request);
            result.status == ComplexAnalysisStatus::Complete && complex_analysis_replay(&result)
        })
    {
        authorized.push(RouteDomain::ComplexAnalysis);
    } else if complex.status == ComplexFrontendStatus::Ambiguous {
        ambiguous.push(RouteDomain::ComplexAnalysis);
    }
    if combinatorics.status == CombinatoricsFrontendStatus::Complete
        && combinatorics_frontend_replay(&combinatorics)
        && combinatorics.request.as_ref().is_some_and(|request| {
            let result = evaluate_combinatorics(request);
            result.status == CombinatoricsStatus::Complete && result.replay_verified()
        })
    {
        authorized.push(RouteDomain::Combinatorics);
    } else if combinatorics.status == CombinatoricsFrontendStatus::Ambiguous {
        ambiguous.push(RouteDomain::Combinatorics);
    }
    if number.status == NumberTheoryFrontendStatus::Complete
        && number_frontend_replay(&number)
        && number.request.as_ref().is_some_and(|request| {
            let result = evaluate_number_theory(request);
            result.status == NumberTheoryStatus::Complete && result.replay_verified()
        })
    {
        authorized.push(RouteDomain::NumberTheory);
    } else if number.status == NumberTheoryFrontendStatus::Ambiguous {
        ambiguous.push(RouteDomain::NumberTheory);
    }
    let mobius = formalize_mobius_text(text);
    if mobius.status == MobiusFrontendStatus::Complete
        && mobius.replay_verified()
        && mobius.request.as_ref().is_some_and(|request| {
            let result = evaluate_mobius(request);
            result.status == MobiusStatus::Complete && result.replay_verified()
        })
    {
        authorized.push(RouteDomain::Mobius);
    } else if mobius.status == MobiusFrontendStatus::Ambiguous {
        ambiguous.push(RouteDomain::Mobius);
    }
    if markov.status == MarkovFrontendStatus::Complete && markov_frontend_replay(&markov) {
        match markov.request.as_ref() {
            Some(MarkovFrontendRequest::Stationary(request)) => {
                let result = evaluate_stationary(request);
                if result.status == StationaryStatus::Complete && result.replay_verified() {
                    authorized.push(RouteDomain::MarkovStationary);
                }
            }
            Some(MarkovFrontendRequest::Hitting(request)) => {
                let result = evaluate_hitting(request);
                if result.status == HittingStatus::Complete && result.replay_verified() {
                    authorized.push(RouteDomain::MarkovHitting);
                }
            }
            None => {}
        }
    } else if markov.status == MarkovFrontendStatus::Ambiguous {
        ambiguous.extend([RouteDomain::MarkovHitting, RouteDomain::MarkovStationary]);
    }
    let lower_text = text.to_ascii_lowercase();

    let em_signal = [
        "ohm's law",
        "ohms law",
        "electric power",
        "electrical power",
        "charge from constant current",
        "capacitor charge",
    ]
    .iter()
    .any(|marker| lower_text.contains(marker));
    if em_signal {
        let em = formalize_em_text(text, case_id);
        if em.status == EmFrontendStatus::Complete
            && em_frontend_replay(&em)
            && em_downstream_replay(&em)
            && em.request.as_ref().is_some_and(|request| {
                let result = evaluate_em(request);
                result.status == EmStatus::Complete && result.replay_verified()
            })
        {
            authorized.push(RouteDomain::Electromagnetism);
        } else if em.status == EmFrontendStatus::Ambiguous {
            ambiguous.push(RouteDomain::Electromagnetism);
        }
    }

    let ode_signal = lower_text.contains("differential equation") || lower_text.contains("ode");
    if ode_signal {
        let ode = formalize_ode_text(text, case_id);
        if ode.status == OdeFrontendStatus::Complete
            && ode_frontend_replay(&ode)
            && ode_downstream_replay(&ode)
            && ode.request.as_ref().is_some_and(|request| {
                let result = evaluate_ode(request);
                result.status == OdeStatus::Complete && result.replay_verified()
            })
        {
            authorized.push(RouteDomain::ODE);
        } else if ode.status == OdeFrontendStatus::Ambiguous {
            ambiguous.push(RouteDomain::ODE);
        }
    }

    let polynomial_signal = lower_text.contains("polynomial") || lower_text.contains("prime field");
    if polynomial_signal {
        let polynomial = formalize_polynomial_text(text, case_id);
        if polynomial.status == PolynomialFrontendStatus::Complete
            && polynomial_frontend_replay(&polynomial)
            && polynomial_downstream_replay(&polynomial)
            && polynomial.request.as_ref().is_some_and(|request| {
                let result = evaluate_polynomial(request);
                result.status == crate::polynomial_pack::PolynomialStatus::Complete
                    && result.replay_verified()
            })
        {
            authorized.push(RouteDomain::Polynomial);
        } else if polynomial.status == PolynomialFrontendStatus::Ambiguous {
            ambiguous.push(RouteDomain::Polynomial);
        }
    }

    let spectral_signal = lower_text.contains("eigenvalue")
        || lower_text.contains("eigenspace")
        || lower_text.contains("characteristic polynomial")
        || lower_text.contains("diagonaliz")
        || lower_text.contains("spectral decomposition")
        || lower_text.contains("matrix power");
    if spectral_signal {
        let spectral = formalize_spectral_text(text);
        if spectral.status == SpectralFrontendStatus::Complete
            && spectral.replay_verified()
            && spectral.request.as_ref().is_some_and(|request| {
                let result = evaluate_spectral(request);
                result.status == SpectralStatus::Complete && result.replay_verified()
            })
        {
            authorized.push(RouteDomain::Spectral);
        } else if spectral.status == SpectralFrontendStatus::Ambiguous {
            ambiguous.push(RouteDomain::Spectral);
        }
    }

    // The finite-state parser intentionally returns `Ambiguous` when its
    // required fields are absent.  Only expose that ambiguity to the
    // dispatcher when the text actually claims to describe a state machine;
    // otherwise every unrelated technical question would become ambiguous.
    let state_signal = [
        "initial state",
        "transitions:",
        "event sequence",
        "start in state",
        "begin in state",
        "input event",
        "final state",
        "end in state",
    ]
    .iter()
    .any(|marker| lower_text.contains(marker));
    if state_signal {
        let (state_status, state_artifact) = formalize_state(text);
        if state_status == StateDecision::Supported
            && state_artifact
                .as_ref()
                .is_some_and(|artifact| artifact.replay_verified())
        {
            authorized.push(RouteDomain::FiniteStateTransition);
        } else if state_status == StateDecision::Ambiguous {
            ambiguous.push(RouteDomain::FiniteStateTransition);
        }
    }

    let metric_signal = lower_text.contains("metric") || lower_text.contains("distance function");
    if metric_signal {
        let metric = formalize_metric_text(text);
        if metric.status == MetricFrontendStatus::Complete
            && metric.replay_verified()
            && metric.request.as_ref().is_some_and(|request| {
                let result = evaluate_metric(request, metric_records());
                result.status == MetricStatus::Complete && result.replay_verified()
            })
        {
            authorized.push(RouteDomain::FiniteMetric);
        } else if metric.status == MetricFrontendStatus::Ambiguous {
            ambiguous.push(RouteDomain::FiniteMetric);
        }
    }

    let topology_signal = lower_text.contains("topology")
        || lower_text.contains("points:")
        || lower_text.contains("open sets:");
    if topology_signal {
        let topology = formalize_topology_text(text);
        if topology.status == TopologyFrontendStatus::Complete
            && topology.request.as_ref().is_some_and(|request| {
                let result = evaluate_topology(request, topology_records());
                result.status == TopologyStatus::Complete && result.replay_verified()
            })
        {
            authorized.push(RouteDomain::FiniteTopology);
        } else if topology.status == TopologyFrontendStatus::Ambiguous {
            ambiguous.push(RouteDomain::FiniteTopology);
        }
    }

    let chemistry_signal = lower_text.contains("formula:")
        || lower_text.contains("chemical formula")
        || lower_text.contains("balanced reaction")
        || lower_text.contains("validate reaction")
        || lower_text.contains("reaction:")
        || lower_text.contains("stoichiometric");
    if chemistry_signal {
        let chemistry = formalize_chemistry_text(text);
        if chemistry.status == ChemistryFrontendStatus::Complete
            && chemistry.replay_verified()
            && chemistry.request.as_ref().is_some_and(|request| {
                let result = evaluate_chemistry(request);
                result.status == ChemistryStatus::Complete && result.replay_verified()
            })
        {
            authorized.push(RouteDomain::Chemistry);
        } else if chemistry.status == ChemistryFrontendStatus::Ambiguous {
            ambiguous.push(RouteDomain::Chemistry);
        }
    }

    let biology_signal = lower_text.contains("dna")
        || lower_text.contains("base composition")
        || lower_text.contains("reverse complement")
        || lower_text.contains("complementary strand");
    if biology_signal {
        let biology = formalize_biology_text(text);
        if biology.status == BiologyFrontendStatus::Complete
            && biology.replay_verified()
            && biology.request.as_ref().is_some_and(|request| {
                let result = evaluate_biology(request);
                result.status == BiologyStatus::Complete && result.replay_verified()
            })
        {
            authorized.push(RouteDomain::Biology);
        } else if biology.status == BiologyFrontendStatus::Ambiguous {
            ambiguous.push(RouteDomain::Biology);
        }
    }
    authorized.sort();
    ambiguous.sort();
    if authorized.len() == 1 && ambiguous.is_empty() {
        finish(
            RouteStatus::Authorized,
            authorized,
            ambiguous,
            text,
            Vec::new(),
        )
    } else if !authorized.is_empty() || !ambiguous.is_empty() {
        finish(
            RouteStatus::Ambiguous,
            authorized,
            ambiguous,
            text,
            vec!["no unique replayable frontend route was established".into()],
        )
    } else {
        finish(
            RouteStatus::Unsupported,
            authorized,
            ambiguous,
            text,
            vec!["all candidate frontends refused or lacked complete typed inputs".into()],
        )
    }
}

pub fn replay_verified(decision: &RouteDecision) -> bool {
    decision.replay_hash == digest(&payload(decision)) && !decision.provenance.is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unique_routes_are_selected_after_downstream_replay() {
        let number = route(
            "Find gcd, the greatest common divisor, with a=84 b=30.",
            "number",
        );
        assert_eq!(number.status, RouteStatus::Authorized);
        assert_eq!(number.selected, Some(RouteDomain::NumberTheory));
        let counting = route("Count combinations with n=5 k=2.", "count");
        assert_eq!(counting.selected, Some(RouteDomain::Combinatorics));
        assert!(replay_verified(&number));
    }

    #[test]
    fn competing_or_unsupported_routes_do_not_authorize() {
        let ambiguous = route(
            "Maybe either combinations n=5 k=2 or gcd, the greatest common divisor, a=84 b=30.",
            "ambiguous",
        );
        assert_eq!(ambiguous.status, RouteStatus::Ambiguous);
        let unsupported = route(
            "Use a contour integral on an infinite graph.",
            "unsupported",
        );
        assert_eq!(unsupported.status, RouteStatus::Unsupported);
        assert!(replay_verified(&ambiguous));
        assert!(replay_verified(&unsupported));
    }

    #[test]
    fn markov_frontend_requires_unique_explicit_operation() {
        let stationary = route(
            "Find the stationary distribution for a row-stochastic transition=[[3/4,1/4],[1/2,1/2]].",
            "stationary",
        );
        assert_eq!(stationary.status, RouteStatus::Authorized);
        assert_eq!(stationary.selected, Some(RouteDomain::MarkovStationary));
        let ambiguous = route(
            "Find a stationary distribution for transition=[[3/4,1/4],[1/2,1/2]].",
            "missing-convention",
        );
        assert_eq!(ambiguous.status, RouteStatus::Ambiguous);
        assert!(replay_verified(&ambiguous));
    }

    #[test]
    fn mobius_frontend_requires_unique_downstream_replay() {
        let inversion = route(
            "Apply Mobius inversion to f(1)..f(n), indexed from 1: [1, 2, 3, 4].",
            "mobius",
        );
        assert_eq!(inversion.status, RouteStatus::Authorized);
        assert_eq!(inversion.selected, Some(RouteDomain::Mobius));
        let competing = route(
            "Apply Mobius inversion or find the Bezout gcd for a=18 and b=30.",
            "mobius-competing",
        );
        assert_eq!(competing.status, RouteStatus::Ambiguous);
        assert!(replay_verified(&inversion));
        assert!(replay_verified(&competing));
    }

    #[test]
    fn finite_state_route_requires_a_replayable_trace() {
        let supported = route(
            "Initial state: locked. Transitions: locked --open--> open; open --close--> locked. Event sequence: open, close. Expected state: locked.",
            "finite-state-supported",
        );
        assert_eq!(supported.status, RouteStatus::Authorized);
        assert_eq!(supported.selected, Some(RouteDomain::FiniteStateTransition));
        let ambiguous = route(
            "Initial state: locked. Transitions: locked --open [key_ok]--> open. Event sequence: open. Expected state: open.",
            "finite-state-ambiguous",
        );
        assert_eq!(ambiguous.status, RouteStatus::Ambiguous);
        assert!(replay_verified(&supported));
        assert!(replay_verified(&ambiguous));
    }

    #[test]
    fn source_metric_and_topology_routes_require_explicit_carriers() {
        let metric = route(
            "For a finite metric on points: p0,p1,p2; distances: p0-p0=0,p0-p1=1,p0-p2=2,p1-p1=0,p1-p2=1,p2-p2=0; determine the distance from p0 to p2.",
            "metric-route",
        );
        assert_eq!(metric.status, RouteStatus::Authorized);
        assert_eq!(metric.selected, Some(RouteDomain::FiniteMetric));

        let topology = route(
            "Validate topology: points: {a,b,c}; open sets: {}; open sets: {a}; open sets: {a,b,c}.",
            "topology-route",
        );
        assert_eq!(topology.status, RouteStatus::Authorized);
        assert_eq!(topology.selected, Some(RouteDomain::FiniteTopology));

        let missing = route(
            "Determine the topology of a finite carrier without listing its open sets.",
            "topology-missing",
        );
        assert_eq!(missing.status, RouteStatus::Unsupported);
        assert!(replay_verified(&metric));
        assert!(replay_verified(&topology));
    }

    #[test]
    fn source_chemistry_and_biology_routes_require_local_artifacts() {
        let chemistry = route("Parse the molecular formula: Al2(SO4)3.", "chemistry-route");
        assert_eq!(chemistry.status, RouteStatus::Authorized);
        assert_eq!(chemistry.selected, Some(RouteDomain::Chemistry));

        let biology = route(
            "Compute the reverse complement of DNA sequence: AATTGGCC, given 5' to 3' orientation.",
            "biology-route",
        );
        assert_eq!(biology.status, RouteStatus::Authorized);
        assert_eq!(biology.selected, Some(RouteDomain::Biology));

        let unsupported = route("Compute the molar mass of H2O.", "chemistry-unsupported");
        assert_eq!(unsupported.status, RouteStatus::Unsupported);
        assert!(replay_verified(&chemistry));
        assert!(replay_verified(&biology));
    }

    #[test]
    fn bounded_ode_polynomial_and_spectral_routes_require_explicit_inputs() {
        let ode = route(
            "Solve the bounded exact scalar ODE with constant derivative: initial=2 derivative=3 time=2.",
            "ode-route",
        );
        assert_eq!(ode.status, RouteStatus::Authorized);
        assert_eq!(ode.selected, Some(RouteDomain::ODE));

        let polynomial = route(
            "Over a prime field, evaluate polynomial p=[1,2,1] mod=5 at point=2.",
            "polynomial-route",
        );
        assert_eq!(polynomial.status, RouteStatus::Authorized);
        assert_eq!(polynomial.selected, Some(RouteDomain::Polynomial));

        let spectral = route("Find the eigenvalues of [[2,0],[0,5]].", "spectral-route");
        assert_eq!(spectral.status, RouteStatus::Authorized);
        assert_eq!(spectral.selected, Some(RouteDomain::Spectral));

        let missing = route("Find the eigenvalues of A.", "spectral-missing");
        assert_eq!(missing.status, RouteStatus::Unsupported);
        assert!(replay_verified(&ode));
        assert!(replay_verified(&polynomial));
        assert!(replay_verified(&spectral));
        assert!(replay_verified(&missing));
    }

    #[test]
    fn source_electromagnetism_route_requires_law_and_units() {
        let supported = route(
            "Apply Ohm's law with I=2 and R=5 in SI-consistent exact units.",
            "em-supported",
        );
        assert_eq!(supported.status, RouteStatus::Authorized);
        assert_eq!(supported.selected, Some(RouteDomain::Electromagnetism));

        let ambiguous = route("Use electric power with V=3 and I=2.", "em-missing-scope");
        assert_eq!(ambiguous.status, RouteStatus::Ambiguous);
        assert!(replay_verified(&supported));
        assert!(replay_verified(&ambiguous));
    }
}
