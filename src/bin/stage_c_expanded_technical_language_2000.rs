//! Expanded technical-language gate over source-derived curriculum domains.
//!
//! The corpus is independently authored from the source records.  Each report
//! crosses a real domain frontend, then (only for complete frontends) the
//! source-derived evaluator.  Ambiguous and unsupported language never enters
//! a downstream executor.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use the_machine::source_complex_pack::evaluate_complex;
use the_machine::source_complex_pack::source_complex_frontend::{
    formalize_complex_text, FrontendStatus as ComplexFrontendStatus,
};
use the_machine::source_formula_pack::biology_pack::biology_frontend::{
    formalize_biology_text, BiologyFrontendStatus,
};
use the_machine::source_formula_pack::biology_pack::evaluate_biology;
use the_machine::source_formula_pack::biology_pack::BiologyStatus;
use the_machine::source_formula_pack::chemistry_pack::chemistry_frontend::{
    formalize_chemistry_text, FrontendStatus as ChemistryFrontendStatus,
};
use the_machine::source_formula_pack::chemistry_pack::evaluate_chemistry;
use the_machine::source_formula_pack::chemistry_pack::ChemistryStatus;
use the_machine::source_statistics_frontend::{
    formalize_statistics_text, FrontendStatus as StatisticsFrontendStatus,
};
use the_machine::source_statistics_pack::evaluate_statistics;
use the_machine::source_topology_frontend::{formalize_topology_text, TopologyFrontendStatus};
use the_machine::source_topology_pack::{
    evaluate_topology, extract_topology_definitions, TopologyStatus,
};

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
enum Expected {
    Supported,
    Ambiguous,
    Unsupported,
}

#[derive(Debug, Clone, Serialize)]
struct Receipt {
    id: String,
    route: String,
    expected: Expected,
    frontend_status: String,
    downstream_status: Option<String>,
    target_grounded: bool,
    provenance_preserved: bool,
    replay_verified: bool,
    tamper_rejected: bool,
    authorized: bool,
    exact: bool,
    false_authorization: bool,
    false_denial: bool,
    text_sha256: String,
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
    target_grounded: usize,
    ambiguity_preserved: usize,
    unsupported_refused: usize,
    authorized_supported: usize,
    replay_verified: usize,
    tamper_rejections: usize,
    provenance_preserved: usize,
    false_authorizations: usize,
    false_denials: usize,
    route_counts: BTreeMap<String, usize>,
    receipts: Vec<Receipt>,
}

struct Outcome {
    frontend_status: String,
    downstream_status: Option<String>,
    target_grounded: bool,
    provenance_preserved: bool,
    replay_verified: bool,
    tamper_rejected: bool,
    authorized: bool,
    exact: bool,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(value).expect("language corpus serializes"))
    )
}

fn topology(text: &str, expected: Expected) -> Outcome {
    let frontend = formalize_topology_text(text);
    let records = extract_topology_definitions(include_str!(
        "../../docs/sources/topology_without_tears_finite_definition.txt"
    ))
    .expect("topology source record");
    let downstream = frontend
        .request
        .as_ref()
        .map(|request| evaluate_topology(request, &records));
    let authorized = expected == Expected::Supported
        && frontend.status == TopologyFrontendStatus::Complete
        && downstream.as_ref().is_some_and(|result| {
            result.status == TopologyStatus::Complete
                && result.artifact.is_some()
                && result.replay_verified()
        });
    let replay = frontend.replay_verified()
        && downstream
            .as_ref()
            .is_none_or(|result| result.replay_verified());
    let provenance = !frontend.provenance.is_empty()
        && downstream
            .as_ref()
            .is_none_or(|result| !result.provenance.is_empty());
    let mut tf = frontend.clone();
    tf.replay_hash.push('x');
    let downstream_tamper = downstream.as_ref().is_none_or(|result| {
        let mut tampered = result.clone();
        tampered.replay_hash.push('x');
        !tampered.replay_verified()
    });
    let exact = match expected {
        Expected::Supported => authorized,
        Expected::Ambiguous => frontend.status == TopologyFrontendStatus::Ambiguous && !authorized,
        Expected::Unsupported => {
            frontend.status == TopologyFrontendStatus::Unsupported && !authorized
        }
    };
    Outcome {
        frontend_status: format!("{:?}", frontend.status),
        downstream_status: downstream.map(|r| format!("{:?}", r.status)),
        target_grounded: frontend.status != TopologyFrontendStatus::Missing,
        provenance_preserved: provenance,
        replay_verified: replay,
        tamper_rejected: !tf.replay_verified() && downstream_tamper,
        authorized,
        exact,
    }
}

fn chemistry(text: &str, expected: Expected) -> Outcome {
    let frontend = formalize_chemistry_text(text);
    let downstream = frontend.request.as_ref().map(evaluate_chemistry);
    let authorized = expected == Expected::Supported
        && frontend.status == ChemistryFrontendStatus::Complete
        && downstream.as_ref().is_some_and(|result| {
            result.status == ChemistryStatus::Complete
                && result.artifact.is_some()
                && result.replay_verified()
        });
    let replay = frontend.replay_verified()
        && downstream
            .as_ref()
            .is_none_or(|result| result.replay_verified());
    let provenance = !frontend.provenance.is_empty()
        && downstream
            .as_ref()
            .is_none_or(|result| !result.provenance.is_empty());
    let mut tf = frontend.clone();
    tf.replay_hash.push('x');
    let downstream_tamper = downstream.as_ref().is_none_or(|result| {
        let mut t = result.clone();
        t.replay_hash.push('x');
        !t.replay_verified()
    });
    let exact = match expected {
        Expected::Supported => authorized,
        Expected::Ambiguous => frontend.status == ChemistryFrontendStatus::Ambiguous && !authorized,
        Expected::Unsupported => {
            frontend.status == ChemistryFrontendStatus::Unsupported && !authorized
        }
    };
    Outcome {
        frontend_status: format!("{:?}", frontend.status),
        downstream_status: downstream.map(|r| format!("{:?}", r.status)),
        target_grounded: frontend.status != ChemistryFrontendStatus::Missing,
        provenance_preserved: provenance,
        replay_verified: replay,
        tamper_rejected: !tf.replay_verified() && downstream_tamper,
        authorized,
        exact,
    }
}

fn biology(text: &str, expected: Expected) -> Outcome {
    let frontend = formalize_biology_text(text);
    let downstream = frontend.request.as_ref().map(evaluate_biology);
    let authorized = expected == Expected::Supported
        && frontend.status == BiologyFrontendStatus::Complete
        && downstream.as_ref().is_some_and(|result| {
            result.status == BiologyStatus::Complete
                && result.artifact.is_some()
                && result.replay_verified()
        });
    let replay = frontend.replay_verified()
        && downstream
            .as_ref()
            .is_none_or(|result| result.replay_verified());
    let provenance = !frontend.provenance.is_empty()
        && downstream
            .as_ref()
            .is_none_or(|result| !result.provenance.is_empty());
    let mut tf = frontend.clone();
    tf.replay_hash.push('x');
    let downstream_tamper = downstream.as_ref().is_none_or(|result| {
        let mut t = result.clone();
        t.replay_hash.push('x');
        !t.replay_verified()
    });
    let exact = match expected {
        Expected::Supported => authorized,
        Expected::Ambiguous => frontend.status == BiologyFrontendStatus::Ambiguous && !authorized,
        Expected::Unsupported => {
            frontend.status == BiologyFrontendStatus::Unsupported && !authorized
        }
    };
    Outcome {
        frontend_status: format!("{:?}", frontend.status),
        downstream_status: downstream.map(|r| format!("{:?}", r.status)),
        target_grounded: frontend.status != BiologyFrontendStatus::Missing,
        provenance_preserved: provenance,
        replay_verified: replay,
        tamper_rejected: !tf.replay_verified() && downstream_tamper,
        authorized,
        exact,
    }
}

fn complex(text: &str, expected: Expected) -> Outcome {
    let frontend = formalize_complex_text(text);
    let downstream = frontend.request.as_ref().map(evaluate_complex);
    let authorized = expected == Expected::Supported
        && frontend.status == ComplexFrontendStatus::Complete
        && downstream.as_ref().is_some_and(|result| {
            result.status == the_machine::source_complex_pack::ComplexStatus::Complete
                && result.artifact.is_some()
                && result.replay_verified()
        });
    let replay = frontend.replay_verified()
        && downstream
            .as_ref()
            .is_none_or(|result| result.replay_verified());
    let provenance = !frontend.provenance_spans.is_empty()
        && downstream
            .as_ref()
            .is_none_or(|result| !result.provenance.is_empty());
    let mut tf = frontend.clone();
    tf.replay_hash.push('x');
    let downstream_tamper = downstream.as_ref().is_none_or(|result| {
        let mut t = result.clone();
        t.replay_hash.push('x');
        !t.replay_verified()
    });
    let exact = match expected {
        Expected::Supported => authorized,
        Expected::Ambiguous => frontend.status == ComplexFrontendStatus::Ambiguous && !authorized,
        Expected::Unsupported => {
            frontend.status == ComplexFrontendStatus::Unsupported && !authorized
        }
    };
    Outcome {
        frontend_status: format!("{:?}", frontend.status),
        downstream_status: downstream.map(|r| format!("{:?}", r.status)),
        target_grounded: frontend.status != ComplexFrontendStatus::Missing,
        provenance_preserved: provenance,
        replay_verified: replay,
        tamper_rejected: !tf.replay_verified() && downstream_tamper,
        authorized,
        exact,
    }
}

fn statistics(text: &str, expected: Expected) -> Outcome {
    let frontend = formalize_statistics_text(text);
    let downstream = frontend.request.as_ref().map(evaluate_statistics);
    let authorized = expected == Expected::Supported
        && frontend.status == StatisticsFrontendStatus::Complete
        && downstream.as_ref().is_some_and(|result| {
            result.status == the_machine::source_formula_pack::FormulaStatus::Complete
                && result.value.is_some()
                && result.replay_verified()
        });
    let replay = frontend.replay_verified()
        && downstream
            .as_ref()
            .is_none_or(|result| result.replay_verified());
    let provenance = !frontend.provenance_spans.is_empty()
        && downstream
            .as_ref()
            .is_none_or(|result| !result.provenance.is_empty());
    let mut tf = frontend.clone();
    tf.replay_hash.push('x');
    let downstream_tamper = downstream.as_ref().is_none_or(|result| {
        let mut t = result.clone();
        t.replay_hash.push('x');
        !t.replay_verified()
    });
    let exact = match expected {
        Expected::Supported => authorized,
        Expected::Ambiguous => {
            frontend.status == StatisticsFrontendStatus::Ambiguous && !authorized
        }
        Expected::Unsupported => {
            frontend.status == StatisticsFrontendStatus::Unsupported && !authorized
        }
    };
    Outcome {
        frontend_status: format!("{:?}", frontend.status),
        downstream_status: downstream.map(|r| format!("{:?}", r.status)),
        target_grounded: frontend.status != StatisticsFrontendStatus::Missing,
        provenance_preserved: provenance,
        replay_verified: replay,
        tamper_rejected: !tf.replay_verified() && downstream_tamper,
        authorized,
        exact,
    }
}

fn text(route: usize, expected: Expected, index: usize) -> String {
    let variant = index % 4;
    match (route, expected, variant) {
        (0, Expected::Supported, 0) => "Validate topology: points: {a,b,c}; open sets: {}; open sets: {a}; open sets: {a,b,c}.".into(),
        (0, Expected::Supported, 1) => "Context says x^2 is incidental. Is open: points: {a,b,c}; target: {a}; open sets: {}; open sets: {a}; open sets: {a,b,c}.".into(),
        (0, Expected::Supported, 2) => "Using the following declaration, find the closure. Points: {a,b,c}; target: {a}; open sets: {}; open sets: {a}; open sets: {a,b,c}.".into(),
        (0, Expected::Supported, _) => "Find the interior. Points: {a,b,c}; target: {a}; open sets: {}; open sets: {a}; open sets: {a,b,c}.".into(),
        (0, Expected::Ambiguous, _) => "Determine the interior; points: {a,b,c}; points: {a,b}; target: {a}; open sets: {}; open sets: {a}; open sets: {a,b,c}.".into(),
        (0, Expected::Unsupported, _) => "Determine whether this metric space is compact and Hausdorff.".into(),
        (1, Expected::Supported, _) => format!("For the molecular formula: {}, parse the formula; an unrelated citation follows.", ["H2O", "CO2", "NH4NO3", "Ca(OH)2"][index % 4]),
        (1, Expected::Ambiguous, _) => "Two candidates are present: formula: H2O and formula: CO2; select the requested formula.".into(),
        (1, Expected::Unsupported, _) => "Compute the molar mass of formula: H2O.".into(),
        (2, Expected::Supported, 0) => "Report the base composition for DNA sequence: AATTGGCC.".into(),
        (2, Expected::Supported, 1) => "Given sequence: ATCGATCG, compute its base composition; prose after it is incidental.".into(),
        (2, Expected::Supported, _) => "For strand: GCGCGCAA, report base composition.".into(),
        (2, Expected::Ambiguous, _) => "Find the complement of sequence: AATTGGCC, but strand orientation is not stated.".into(),
        (2, Expected::Unsupported, _) => "Translate the RNA sequence: AUGGCC into a protein.".into(),
        (3, Expected::Supported, 0) => "Compute the product of (3-4i) and (2+5i).".into(),
        (3, Expected::Supported, 1) => "Find the conjugate of (7/2+1/3i); the quoted title is incidental.".into(),
        (3, Expected::Supported, _) => "Compute the squared magnitude of (5-2i).".into(),
        (3, Expected::Ambiguous, _) => "Add and multiply (3-4i) and (2+5i); the requested operation is not unique.".into(),
        (3, Expected::Unsupported, _) => "Convert (3-4i) to polar form and report its argument.".into(),
        (4, Expected::Supported, 0) => "Find the mean from sum=30 and count=5.".into(),
        (4, Expected::Supported, 1) => "Using count : 5, compute the average from sum = 30.".into(),
        (4, Expected::Supported, _) => "For a Bernoulli variable with p=1/2, find the variance.".into(),
        (4, Expected::Ambiguous, _) => "Find the average from total=30 and count=5; the weighted sum is not identified.".into(),
        (4, Expected::Unsupported, _) => "Fit a regression model and report a confidence interval.".into(),
        _ => unreachable!(),
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let routes = [
        "finite_topology",
        "chemistry",
        "dna_biology",
        "complex_arithmetic",
        "finite_statistics",
    ];
    let mut receipts = Vec::with_capacity(2000);
    for (route, route_name) in routes.iter().enumerate() {
        for index in 0..400 {
            let expected = match index % 10 {
                0..=5 => Expected::Supported,
                6..=7 => Expected::Ambiguous,
                _ => Expected::Unsupported,
            };
            let source_text = text(route, expected, index);
            let outcome = match route {
                0 => topology(&source_text, expected),
                1 => chemistry(&source_text, expected),
                2 => biology(&source_text, expected),
                3 => complex(&source_text, expected),
                4 => statistics(&source_text, expected),
                _ => unreachable!(),
            };
            receipts.push(Receipt {
                id: format!("{route_name}_{index:04}"),
                route: (*route_name).into(),
                expected,
                frontend_status: outcome.frontend_status,
                downstream_status: outcome.downstream_status,
                target_grounded: outcome.target_grounded,
                provenance_preserved: outcome.provenance_preserved,
                replay_verified: outcome.replay_verified,
                tamper_rejected: outcome.tamper_rejected,
                authorized: outcome.authorized,
                exact: outcome.exact,
                false_authorization: expected != Expected::Supported && outcome.authorized,
                false_denial: expected == Expected::Supported && !outcome.authorized,
                text_sha256: digest(&source_text),
            });
        }
    }
    let cases = receipts.len();
    let supported = receipts
        .iter()
        .filter(|r| r.expected == Expected::Supported)
        .count();
    let ambiguous = receipts
        .iter()
        .filter(|r| r.expected == Expected::Ambiguous)
        .count();
    let unsupported = receipts
        .iter()
        .filter(|r| r.expected == Expected::Unsupported)
        .count();
    let target_grounded = receipts.iter().filter(|r| r.target_grounded).count();
    let ambiguity_preserved = receipts
        .iter()
        .filter(|r| r.expected == Expected::Ambiguous && r.exact)
        .count();
    let unsupported_refused = receipts
        .iter()
        .filter(|r| r.expected == Expected::Unsupported && r.exact)
        .count();
    let authorized_supported = receipts
        .iter()
        .filter(|r| r.expected == Expected::Supported && r.authorized)
        .count();
    let replay_verified = receipts.iter().filter(|r| r.replay_verified).count();
    let tamper_rejections = receipts.iter().filter(|r| r.tamper_rejected).count();
    let provenance_preserved = receipts.iter().filter(|r| r.provenance_preserved).count();
    let false_authorizations = receipts.iter().filter(|r| r.false_authorization).count();
    let false_denials = receipts.iter().filter(|r| r.false_denial).count();
    assert_eq!(
        (cases, supported, ambiguous, unsupported),
        (2000, 1200, 400, 400)
    );
    assert_eq!(target_grounded, cases);
    assert_eq!(ambiguity_preserved, ambiguous);
    assert_eq!(unsupported_refused, unsupported);
    assert_eq!(authorized_supported, supported);
    assert_eq!(replay_verified, cases);
    assert_eq!(tamper_rejections, cases);
    assert_eq!(provenance_preserved, cases);
    assert_eq!(false_authorizations, 0);
    assert_eq!(false_denials, 0);
    let mut route_counts = BTreeMap::new();
    for receipt in &receipts {
        *route_counts.entry(receipt.route.clone()).or_insert(0) += 1;
    }
    let report = Report {
        schema: "stage-c-expanded-technical-language-2000-v1",
        source: "independently authored shifted source-domain reports",
        corpus_sha256: digest(&receipts),
        cases,
        supported,
        ambiguous,
        unsupported,
        target_grounded,
        ambiguity_preserved,
        unsupported_refused,
        authorized_supported,
        replay_verified,
        tamper_rejections,
        provenance_preserved,
        false_authorizations,
        false_denials,
        route_counts,
        receipts,
    };
    let serialized = serde_json::to_string_pretty(&report)?;
    std::fs::write(
        "docs/stage_c_expanded_technical_language_2000.json",
        format!("{serialized}\n"),
    )?;
    println!("{serialized}");
    Ok(())
}
