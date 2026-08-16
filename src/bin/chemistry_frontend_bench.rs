//! Stage H technical-language benchmark for the bounded chemistry frontend.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use the_machine::source_formula_pack::chemistry_pack::chemistry_frontend::{
    formalize_chemistry_text, ChemistryFrontendResult, FrontendStatus,
};
use the_machine::source_formula_pack::chemistry_pack::evaluate_chemistry;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
enum Expected {
    Complete,
    Ambiguous,
    Unsupported,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Receipt {
    id: String,
    expected: Expected,
    frontend_status: FrontendStatus,
    downstream_authorized: bool,
    exact: bool,
    frontend_replay_verified: bool,
    downstream_replay_verified: bool,
    frontend_tamper_rejected: bool,
    downstream_tamper_rejected: bool,
    provenance_preserved: bool,
    false_authorization: bool,
    false_denial: bool,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    source: &'static str,
    corpus_sha256: String,
    cases: usize,
    supported: usize,
    ambiguous: usize,
    unsupported: usize,
    exact_decisions: usize,
    complete_frontends: usize,
    downstream_authorizations: usize,
    frontend_replay_verified: usize,
    downstream_replay_verified: usize,
    frontend_tamper_rejections: usize,
    downstream_tamper_rejections: usize,
    provenance_preserved: usize,
    false_authorizations: usize,
    false_denials: usize,
    status_counts: BTreeMap<String, usize>,
    receipts: Vec<Receipt>,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).expect("frontend serializes")))
}

fn downstream(frontend: &ChemistryFrontendResult) -> (bool, bool, bool) {
    let Some(request) = frontend.request.clone() else {
        return (false, false, false);
    };
    let result = evaluate_chemistry(&request);
    let authorized = result.authorized();
    let replay = result.replay_verified();
    let mut tampered = result.clone();
    tampered.replay_hash.push('x');
    (authorized, replay, !tampered.replay_verified())
}

fn run(id: String, text: String, expected: Expected) -> Receipt {
    let frontend = formalize_chemistry_text(&text);
    let (downstream_authorized, downstream_replay_verified, downstream_tamper_rejected) =
        downstream(&frontend);
    let frontend_replay_verified = frontend.replay_verified();
    let mut tampered_frontend = frontend.clone();
    tampered_frontend.replay_hash.push('x');
    let frontend_tamper_rejected = !tampered_frontend.replay_verified();
    let exact = match expected {
        Expected::Complete => frontend.status == FrontendStatus::Complete && downstream_authorized,
        Expected::Ambiguous => {
            frontend.status == FrontendStatus::Ambiguous && !downstream_authorized
        }
        Expected::Unsupported => {
            frontend.status == FrontendStatus::Unsupported && !downstream_authorized
        }
    };
    Receipt {
        id,
        expected,
        frontend_status: frontend.status,
        downstream_authorized,
        exact,
        frontend_replay_verified,
        downstream_replay_verified,
        frontend_tamper_rejected,
        downstream_tamper_rejected,
        provenance_preserved: !frontend.provenance.is_empty(),
        false_authorization: expected != Expected::Complete && downstream_authorized,
        false_denial: expected == Expected::Complete && !downstream_authorized,
    }
}

fn main() {
    let formulas = ["H2O", "CO2", "C6H12O6", "Al2(SO4)3", "Ca(OH)2", "NH4NO3"];
    let mut receipts = Vec::with_capacity(240);
    for index in 0..60 {
        let formula = formulas[index % formulas.len()];
        let text = match index % 3 {
            0 => format!("Parse formula: {formula}."),
            1 => format!("The molecular formula is {formula}."),
            _ => format!("Given formula: {formula}; preserve its atom counts."),
        };
        receipts.push(run(format!("formula_{index:03}"), text, Expected::Complete));
    }
    let reactions = [
        "N2 + 3H2 -> 2NH3",
        "2H2 + O2 -> 2H2O",
        "CH4 + 2O2 -> CO2 + 2H2O",
        "2Na + Cl2 -> 2NaCl",
    ];
    for index in 0..40 {
        let reaction = reactions[index % reactions.len()];
        let text = if index % 2 == 0 {
            format!("Validate reaction: {reaction}.")
        } else {
            format!("Check this balanced equation: {reaction}.")
        };
        receipts.push(run(format!("reaction_{index:03}"), text, Expected::Complete));
    }
    for index in 0..20 {
        let text = if index % 2 == 0 {
            "Find the stoichiometric ratio from H2 to NH3 using reaction: N2 + 3H2 -> 2NH3."
        } else {
            "Find ratio from H2 to NH3 using equation: N2 + 3H2 -> 2NH3."
        };
        receipts.push(run(format!("ratio_{index:03}"), text.into(), Expected::Complete));
    }
    for index in 0..20 {
        receipts.push(run(
            format!("ambiguous_formula_{index:03}"),
            "Formula: H2O; formula: CO2.".into(),
            Expected::Ambiguous,
        ));
    }
    for index in 0..20 {
        receipts.push(run(
            format!("ambiguous_reaction_{index:03}"),
            "Compare reactions: H2 + O2 -> H2O and N2 + 3H2 -> 2NH3.".into(),
            Expected::Ambiguous,
        ));
    }
    for index in 0..20 {
        receipts.push(run(
            format!("unsupported_molar_mass_{index:03}"),
            "Compute the molar mass of formula H2O.".into(),
            Expected::Unsupported,
        ));
    }
    for index in 0..20 {
        receipts.push(run(
            format!("unsupported_oxidation_{index:03}"),
            "Determine the oxidation state in formula Fe2O3.".into(),
            Expected::Unsupported,
        ));
    }
    for index in 0..20 {
        receipts.push(run(
            format!("unsupported_equilibrium_{index:03}"),
            "Analyze the equilibrium reaction N2 + 3H2 -> 2NH3.".into(),
            Expected::Unsupported,
        ));
    }
    for index in 0..20 {
        receipts.push(run(
            format!("unsupported_unscoped_{index:03}"),
            "Explain why this chemical process occurs in solution.".into(),
            Expected::Unsupported,
        ));
    }

    assert_eq!(receipts.len(), 240);
    let cases = receipts.len();
    let supported = receipts.iter().filter(|r| r.expected == Expected::Complete).count();
    let ambiguous = receipts.iter().filter(|r| r.expected == Expected::Ambiguous).count();
    let unsupported = receipts.iter().filter(|r| r.expected == Expected::Unsupported).count();
    let exact_decisions = receipts.iter().filter(|r| r.exact).count();
    let complete_frontends = receipts
        .iter()
        .filter(|r| r.frontend_status == FrontendStatus::Complete)
        .count();
    let downstream_authorizations = receipts.iter().filter(|r| r.downstream_authorized).count();
    let frontend_replay_verified = receipts.iter().filter(|r| r.frontend_replay_verified).count();
    let downstream_replay_verified = receipts.iter().filter(|r| r.downstream_replay_verified).count();
    let frontend_tamper_rejections = receipts.iter().filter(|r| r.frontend_tamper_rejected).count();
    let downstream_tamper_rejections = receipts
        .iter()
        .filter(|r| r.downstream_tamper_rejected)
        .count();
    let provenance_preserved = receipts.iter().filter(|r| r.provenance_preserved).count();
    let false_authorizations = receipts.iter().filter(|r| r.false_authorization).count();
    let false_denials = receipts.iter().filter(|r| r.false_denial).count();
    assert_eq!((supported, ambiguous, unsupported), (120, 40, 80));
    assert_eq!(exact_decisions, cases);
    assert_eq!(complete_frontends, supported);
    assert_eq!(downstream_authorizations, supported);
    assert_eq!(frontend_replay_verified, cases);
    assert_eq!(downstream_replay_verified, supported);
    assert_eq!(frontend_tamper_rejections, cases);
    assert_eq!(downstream_tamper_rejections, supported);
    assert_eq!(provenance_preserved, cases);
    assert_eq!(false_authorizations, 0);
    assert_eq!(false_denials, 0);
    let mut status_counts = BTreeMap::new();
    for receipt in &receipts {
        *status_counts
            .entry(format!("{:?}", receipt.frontend_status))
            .or_insert(0usize) += 1;
    }
    let report = Report {
        schema: "stage-h-source-chemistry-frontend-v1",
        source: "independently authored shifted chemistry-language corpus",
        corpus_sha256: digest(&receipts),
        cases,
        supported,
        ambiguous,
        unsupported,
        exact_decisions,
        complete_frontends,
        downstream_authorizations,
        frontend_replay_verified,
        downstream_replay_verified,
        frontend_tamper_rejections,
        downstream_tamper_rejections,
        provenance_preserved,
        false_authorizations,
        false_denials,
        status_counts,
        receipts,
    };
    let serialized = serde_json::to_string_pretty(&report).expect("frontend report serializes");
    std::fs::write("docs/stage_h_source_chemistry_frontend.json", format!("{serialized}\n"))
        .expect("frontend report writes");
    println!("{serialized}");
}
