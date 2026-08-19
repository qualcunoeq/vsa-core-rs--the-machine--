//! Stage 326: route-blind source-science language routes.
//!
//! Chemistry and bounded DNA biology are exposed through the shared technical
//! router only when their local artifacts are explicit. The corpus exercises
//! formula/reaction parsing, DNA orientation, ambiguity, and unsupported
//! scientific semantics without reading HLE or mutating production state.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::fs;
use the_machine::technical_language_router::{replay_verified, route, RouteDomain, RouteStatus};

const REPORT_JSON: &str = "docs/stage326_source_science_language_routes.json";
const REPORT_MD: &str = "docs/stage326_source_science_language_routes.md";

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum Expected {
    Authorized,
    Ambiguous,
    Unsupported,
}

#[derive(Debug, Clone, Serialize)]
struct Case {
    id: String,
    family: String,
    text: String,
    expected: Expected,
}

#[derive(Debug, Serialize)]
struct Receipt {
    id: String,
    family: String,
    expected: Expected,
    actual: RouteStatus,
    selected: Option<RouteDomain>,
    exact: bool,
    replay_verified: bool,
    tamper_rejected: bool,
    false_authorization: bool,
    false_denial: bool,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    corpus_sha256: String,
    cases: usize,
    supported: usize,
    ambiguous: usize,
    refused: usize,
    exact_decisions: usize,
    chemistry_routes: usize,
    biology_routes: usize,
    replay_verified: usize,
    tamper_rejected: usize,
    false_authorizations: usize,
    false_denials: usize,
    route_leakage: usize,
    hle_questions_read: usize,
    production_mutations: usize,
    receipts: Vec<Receipt>,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn chemistry_supported(index: usize) -> String {
    match index % 3 {
        0 => "Parse the molecular formula: Al2(SO4)3.".into(),
        1 => "Validate reaction: N2 + 3H2 -> 2NH3.".into(),
        _ => "Find the stoichiometric ratio from H2 to NH3 in the balanced reaction: N2 + 3H2 -> 2NH3.".into(),
    }
}

fn biology_supported(index: usize) -> String {
    match index % 3 {
        0 => "Validate DNA sequence: AATTGGCC.".into(),
        1 => "Compute base composition of DNA sequence: AATTGGCC.".into(),
        _ => {
            "Compute the reverse complement of DNA sequence: AATTGGCC, given 5' to 3' orientation."
                .into()
        }
    }
}

fn ambiguous(index: usize) -> (String, String) {
    match index % 4 {
        0 => (
            "chemistry_multiple_formula_spans".into(),
            "Formula: H2O; formula: CO2.".into(),
        ),
        1 => (
            "chemistry_competing_operations".into(),
            "Validate either reaction: N2 + 3H2 -> 2NH3; or reaction: H2 -> H2.".into(),
        ),
        2 => (
            "biology_missing_orientation".into(),
            "Find the complement of DNA sequence: AATTGGCC.".into(),
        ),
        _ => (
            "biology_multiple_sequences".into(),
            "Compute base composition of DNA sequence: AATTGGCC and DNA sequence: GGCC.".into(),
        ),
    }
}

fn unsupported(index: usize) -> (String, String) {
    match index % 5 {
        0 => (
            "chemistry_molar_mass".into(),
            "Compute the molar mass of H2O.".into(),
        ),
        1 => (
            "chemistry_product_inference".into(),
            "Predict the product of an unspecified reaction involving oxygen.".into(),
        ),
        2 => (
            "biology_translation".into(),
            "Translate the codon sequence: AUGGCU.".into(),
        ),
        3 => (
            "biology_rna".into(),
            "Infer protein expression from an RNA sequence.".into(),
        ),
        _ => (
            "biology_mutation".into(),
            "Determine the phenotype caused by a DNA mutation.".into(),
        ),
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut cases = Vec::with_capacity(240);
    for index in 0..60 {
        cases.push(Case {
            id: format!("stage326-chemistry-supported-{index:03}"),
            family: "chemistry_supported".into(),
            text: chemistry_supported(index),
            expected: Expected::Authorized,
        });
        cases.push(Case {
            id: format!("stage326-biology-supported-{index:03}"),
            family: "biology_supported".into(),
            text: biology_supported(index),
            expected: Expected::Authorized,
        });
    }
    for index in 0..40 {
        let (family, text) = ambiguous(index);
        cases.push(Case {
            id: format!("stage326-ambiguous-{index:03}"),
            family,
            text,
            expected: Expected::Ambiguous,
        });
    }
    for index in 0..80 {
        let (family, text) = unsupported(index);
        cases.push(Case {
            id: format!("stage326-unsupported-{index:03}"),
            family,
            text,
            expected: Expected::Unsupported,
        });
    }
    assert_eq!(cases.len(), 240);
    let mut receipts = Vec::with_capacity(cases.len());
    let mut supported = 0;
    let mut ambiguous_count = 0;
    let mut refused = 0;
    let mut exact_decisions = 0;
    let mut chemistry_routes = 0;
    let mut biology_routes = 0;
    let mut replay_count = 0;
    let mut tamper_count = 0;
    let mut false_auth = 0;
    let mut false_denial = 0;
    let mut route_leakage = 0;
    for case in &cases {
        let decision = route(&case.text, &case.id);
        let expected = match case.expected {
            Expected::Authorized => RouteStatus::Authorized,
            Expected::Ambiguous => RouteStatus::Ambiguous,
            Expected::Unsupported => RouteStatus::Unsupported,
        };
        let exact = decision.status == expected;
        let replay = replay_verified(&decision);
        let mut tampered = decision.clone();
        tampered.replay_hash.push('x');
        let tamper = !replay_verified(&tampered);
        let false_authorization =
            case.expected != Expected::Authorized && decision.status == RouteStatus::Authorized;
        let false_denial_case =
            case.expected == Expected::Authorized && decision.status != RouteStatus::Authorized;
        exact_decisions += usize::from(exact);
        match decision.status {
            RouteStatus::Authorized => supported += 1,
            RouteStatus::Ambiguous => ambiguous_count += 1,
            RouteStatus::Unsupported => refused += 1,
        }
        chemistry_routes += usize::from(decision.selected == Some(RouteDomain::Chemistry));
        biology_routes += usize::from(decision.selected == Some(RouteDomain::Biology));
        replay_count += usize::from(replay);
        tamper_count += usize::from(tamper);
        false_auth += usize::from(false_authorization);
        false_denial += usize::from(false_denial_case);
        route_leakage += usize::from(
            decision.status == RouteStatus::Authorized
                && (decision.authorized_candidates.len() != 1 || decision.selected.is_none()),
        );
        receipts.push(Receipt {
            id: case.id.clone(),
            family: case.family.clone(),
            expected: case.expected,
            actual: decision.status,
            selected: decision.selected,
            exact,
            replay_verified: replay,
            tamper_rejected: tamper,
            false_authorization,
            false_denial: false_denial_case,
        });
    }
    let report = Report {
        schema: "stage326-source-science-language-routes-v1",
        corpus_sha256: digest(&cases),
        cases: cases.len(),
        supported,
        ambiguous: ambiguous_count,
        refused,
        exact_decisions,
        chemistry_routes,
        biology_routes,
        replay_verified: replay_count,
        tamper_rejected: tamper_count,
        false_authorizations: false_auth,
        false_denials: false_denial,
        route_leakage,
        hle_questions_read: 0,
        production_mutations: 0,
        receipts,
    };
    assert_eq!(report.cases, 240);
    assert_eq!(report.supported, 120);
    assert_eq!(report.ambiguous, 40);
    assert_eq!(report.refused, 80);
    assert_eq!(report.exact_decisions, 240);
    assert_eq!(report.chemistry_routes, 60);
    assert_eq!(report.biology_routes, 60);
    assert_eq!(report.replay_verified, 240);
    assert_eq!(report.tamper_rejected, 240);
    assert_eq!(report.false_authorizations, 0);
    assert_eq!(report.false_denials, 0);
    assert_eq!(report.route_leakage, 0);
    fs::write(
        REPORT_JSON,
        format!("{}\n", serde_json::to_string_pretty(&report)?),
    )?;
    fs::write(REPORT_MD, format!("# Stage 326 — source-science technical-language routes\n\n- Cases: {} ({} supported, {} ambiguous, {} refused)\n- Exact decisions: {}/{}\n- Chemistry/DNA routes: {} / {}\n- Replay / tamper: {} / {}\n- False authorizations / denials: {} / {}\n- Route leakage: {}\n- HLE questions read / production mutations: {} / {}\n", report.cases, report.supported, report.ambiguous, report.refused, report.exact_decisions, report.cases, report.chemistry_routes, report.biology_routes, report.replay_verified, report.tamper_rejected, report.false_authorizations, report.false_denials, report.route_leakage, report.hle_questions_read, report.production_mutations))?;
    println!("stage326 cases={} exact={} supported={} ambiguous={} refused={} chemistry={} biology={} replay={} tamper={}", report.cases, report.exact_decisions, report.supported, report.ambiguous, report.refused, report.chemistry_routes, report.biology_routes, report.replay_verified, report.tamper_rejected);
    Ok(())
}
