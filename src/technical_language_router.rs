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
use crate::finite_markov_frontend::{
    formalize as formalize_markov, replay_verified as markov_frontend_replay,
    MarkovFrontendRequest, MarkovFrontendStatus,
};
use crate::finite_markov_hitting_pack::{evaluate as evaluate_hitting, HittingStatus};
use crate::finite_markov_stationary_pack::{evaluate as evaluate_stationary, StationaryStatus};
use crate::number_theory_frontend::{
    formalize_number_theory_text, replay_verified as number_frontend_replay,
    NumberTheoryFrontendStatus,
};
use crate::number_theory_pack::{evaluate_number_theory, NumberTheoryStatus};
use crate::mobius_frontend::{formalize_mobius_text, MobiusFrontendStatus};
use crate::mobius_inversion_pack::{evaluate as evaluate_mobius, MobiusStatus};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum RouteDomain {
    ComplexAnalysis,
    Combinatorics,
    MarkovHitting,
    MarkovStationary,
    Mobius,
    NumberTheory,
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
}
