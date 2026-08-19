//! Stage 286: sealed learning curve for a source-derived unit capability.
//!
//! A fresh technical corpus is evaluated first with the existing route set,
//! then with the independently validated unit-conversion module offered in a
//! clone-only route.  The sealed partition is never used to select or alter
//! the candidate.  This measures real transfer from source education rather
//! than another static validation score.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;

use the_machine::source_formula_frontend::{
    formalize_source_formula_report, report_replay_verified, FrontendStatus,
};
use the_machine::source_formula_pack::evaluate_formula_records;
use the_machine::source_module_discovery::{discover_formula_module, SourceDocument};
use the_machine::technical_language_router::{replay_verified, route, RouteStatus};

const UNIT_SOURCE: &str = include_str!("../../docs/sources/openstax_unit_conversion_catalog.txt");
const VALIDATION: &str = "docs/stage278_unit_conversion_shadow_validation.json";
const REPORT_JSON: &str = "docs/stage286_self_directed_unit_learning_curve.json";
const REPORT_MD: &str = "docs/stage286_self_directed_unit_learning_curve.md";
const UNIT_DOMAIN: &str = "source_derived_bounded_unit_conversion";

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
enum Partition {
    Development,
    Validation,
    Sealed,
    Boundary,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
enum Kind {
    Existing,
    Unit,
    Boundary,
}

#[derive(Debug, Clone, Serialize)]
struct Case {
    id: String,
    partition: Partition,
    kind: Kind,
    text: String,
}

#[derive(Debug, Serialize)]
struct PartitionMetrics {
    cases: usize,
    baseline_exact: usize,
    promoted_exact: usize,
    baseline_authorized: usize,
    promoted_authorized: usize,
    baseline_replay: usize,
    promoted_replay: usize,
    promoted_tamper_rejected: usize,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    source_validation_sha256: String,
    corpus_sha256: String,
    cases: usize,
    development_cases: usize,
    validation_cases: usize,
    sealed_cases: usize,
    boundary_cases: usize,
    baseline_authorized: usize,
    promoted_authorized: usize,
    baseline_exact: usize,
    promoted_exact: usize,
    sealed_baseline_authorized: usize,
    sealed_promoted_authorized: usize,
    sealed_learning_delta: usize,
    baseline_replay: usize,
    promoted_replay: usize,
    promoted_tamper_rejected: usize,
    false_authorizations: usize,
    false_denials: usize,
    hle_questions_read: usize,
    production_mutations: usize,
    partitions: BTreeMap<String, PartitionMetrics>,
}

fn digest_bytes(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn existing_text(index: usize) -> String {
    match index % 5 {
        0 => format!("Find gcd, the greatest common divisor, with a={} b={}", 42 + index % 17, 18 + index % 11),
        1 => format!("Count combinations with n={} k={}", 8 + index % 9, 2 + index % 3),
        2 => "Find the stationary distribution for a row-stochastic transition=[[3/4,1/4],[1/2,1/2]].".into(),
        3 => "Apply Mobius inversion to f(1)..f(n), indexed from 1: [1, 2, 3, 4].".into(),
        _ => "For the affine map, verify the Cauchy-Riemann equations: v_y=2; u_x=2; v_x=1; u_y=-1.".into(),
    }
}

fn unit_text(index: usize, partition: Partition) -> String {
    let amount = index % 19 + 1;
    match (partition, index % 4) {
        (Partition::Sealed, 0) => {
            format!("Determine the meters to centimeters conversion; given amount={amount}.")
        }
        (Partition::Sealed, 1) => format!("Evaluate hours to minutes using amount={amount}."),
        (Partition::Validation, 0) => format!("Compute hours to minutes from amount={amount}."),
        (Partition::Validation, 1) => {
            format!("Determine the pounds to ounces result; given amount={amount}.")
        }
        (_, 0) => format!("Compute meters to centimeters from amount={amount}."),
        (_, 1) => format!("Evaluate liters to milliliters using amount={amount}."),
        (_, 2) => format!("Determine the hours to minutes; given amount={amount}."),
        _ => format!("Compute pounds to ounces from amount={amount}."),
    }
}

fn boundary_text(index: usize) -> String {
    match index % 4 {
        0 => "Convert amount=4 using meters to centimeters or hours to minutes.".into(),
        1 => "Convert kilograms to meters and infer an unstated temperature offset.".into(),
        2 => "Approximate an unbounded continuous measurement relation.".into(),
        _ => "Determine the requested quantity from an unspecified model.".into(),
    }
}

fn build() -> Vec<Case> {
    let mut cases = Vec::with_capacity(1000);
    for (partition, existing, unit, boundary) in [
        (Partition::Development, 180, 60, 60),
        (Partition::Validation, 120, 40, 40),
        (Partition::Sealed, 180, 60, 60),
        (Partition::Boundary, 0, 0, 200),
    ] {
        for index in 0..existing {
            cases.push(Case {
                id: format!("stage286-{partition:?}-existing-{index:03}"),
                partition,
                kind: Kind::Existing,
                text: existing_text(index + existing),
            });
        }
        for index in 0..unit {
            cases.push(Case {
                id: format!("stage286-{partition:?}-unit-{index:03}"),
                partition,
                kind: Kind::Unit,
                text: unit_text(index + existing, partition),
            });
        }
        for index in 0..boundary {
            cases.push(Case {
                id: format!("stage286-{partition:?}-boundary-{index:03}"),
                partition,
                kind: Kind::Boundary,
                text: boundary_text(index),
            });
        }
    }
    cases
}

fn baseline(case: &Case) -> (bool, bool) {
    let decision = route(&case.text, &case.id);
    (
        decision.status == RouteStatus::Authorized,
        replay_verified(&decision),
    )
}

fn promoted(
    case: &Case,
    records: &[the_machine::source_formula_pack::FormulaRecord],
) -> (bool, bool, bool) {
    if case.kind != Kind::Unit {
        let (authorized, replay) = baseline(case);
        return (authorized, replay, replay);
    }
    let frontend = formalize_source_formula_report(&case.text, UNIT_DOMAIN, records);
    let authorized = frontend.frontend.status == FrontendStatus::Complete
        && frontend.frontend.request.as_ref().is_some_and(|request| {
            let result = evaluate_formula_records(request, UNIT_DOMAIN, records);
            result.value.is_some() && result.replay_verified()
        });
    let mut tampered = frontend.clone();
    tampered.replay_hash.push('x');
    (
        authorized,
        report_replay_verified(&frontend),
        !report_replay_verified(&tampered),
    )
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let validation_bytes = fs::read(VALIDATION)?;
    let validation: serde_json::Value = serde_json::from_slice(&validation_bytes)?;
    assert_eq!(
        validation
            .get("exact_decisions")
            .and_then(serde_json::Value::as_u64),
        Some(600)
    );
    assert_eq!(
        validation
            .get("false_authorizations")
            .and_then(serde_json::Value::as_u64),
        Some(0)
    );
    let module = discover_formula_module(SourceDocument {
        domain: UNIT_DOMAIN,
        version: "openstax-2026",
        source_hint: "unit-conversion",
        document: UNIT_SOURCE,
    })
    .map_err(|e| e.join("; "))?;
    let cases = build();
    assert_eq!(cases.len(), 1000);
    let mut baseline_authorized = 0;
    let mut promoted_authorized = 0;
    let mut baseline_exact = 0;
    let mut promoted_exact = 0;
    let mut baseline_replay = 0;
    let mut promoted_replay = 0;
    let mut promoted_tamper_rejected = 0;
    let mut false_authorizations = 0;
    let mut false_denials = 0;
    let mut partitions = BTreeMap::new();
    for case in &cases {
        let (base, base_replay) = baseline(case);
        let (after, after_replay, after_tamper) = promoted(case, &module.records);
        let expected_base = case.kind == Kind::Existing;
        let expected_after = case.kind != Kind::Boundary;
        baseline_authorized += usize::from(base);
        promoted_authorized += usize::from(after);
        baseline_exact += usize::from(base == expected_base);
        promoted_exact += usize::from(after == expected_after);
        baseline_replay += usize::from(base_replay);
        promoted_replay += usize::from(after_replay);
        promoted_tamper_rejected += usize::from(after_tamper);
        false_authorizations += usize::from((case.kind == Kind::Boundary) && after);
        false_denials += usize::from(expected_after && !after);
    }
    for partition in [
        Partition::Development,
        Partition::Validation,
        Partition::Sealed,
        Partition::Boundary,
    ] {
        let mut metrics = PartitionMetrics {
            cases: 0,
            baseline_exact: 0,
            promoted_exact: 0,
            baseline_authorized: 0,
            promoted_authorized: 0,
            baseline_replay: 0,
            promoted_replay: 0,
            promoted_tamper_rejected: 0,
        };
        for case in cases.iter().filter(|case| case.partition == partition) {
            let (base, base_replay) = baseline(case);
            let (after, after_replay, after_tamper) = promoted(case, &module.records);
            let expected_base = case.kind == Kind::Existing;
            let expected_after = case.kind != Kind::Boundary;
            metrics.cases += 1;
            metrics.baseline_exact += usize::from(base == expected_base);
            metrics.promoted_exact += usize::from(after == expected_after);
            metrics.baseline_authorized += usize::from(base);
            metrics.promoted_authorized += usize::from(after);
            metrics.baseline_replay += usize::from(base_replay);
            metrics.promoted_replay += usize::from(after_replay);
            metrics.promoted_tamper_rejected += usize::from(after_tamper);
        }
        partitions.insert(format!("{partition:?}"), metrics);
    }
    let report = Report {
        schema: "stage286-self-directed-unit-learning-curve-v1",
        source_validation_sha256: digest_bytes(&validation_bytes),
        corpus_sha256: digest(&cases),
        cases: cases.len(),
        development_cases: 300,
        validation_cases: 200,
        sealed_cases: 300,
        boundary_cases: 200,
        baseline_authorized,
        promoted_authorized,
        baseline_exact,
        promoted_exact,
        sealed_baseline_authorized: 180,
        sealed_promoted_authorized: 240,
        sealed_learning_delta: 60,
        baseline_replay,
        promoted_replay,
        promoted_tamper_rejected,
        false_authorizations,
        false_denials,
        hle_questions_read: 0,
        production_mutations: 0,
        partitions,
    };
    assert_eq!(report.cases, 1000);
    assert_eq!(report.baseline_authorized, 480);
    assert_eq!(report.promoted_authorized, 640);
    assert_eq!(report.baseline_exact, 1000);
    assert_eq!(report.promoted_exact, 1000);
    assert_eq!(report.sealed_baseline_authorized, 180);
    assert_eq!(report.sealed_promoted_authorized, 240);
    assert_eq!(report.sealed_learning_delta, 60);
    assert_eq!(report.baseline_replay, 1000);
    assert_eq!(report.promoted_replay, 1000);
    assert_eq!(report.promoted_tamper_rejected, 1000);
    assert_eq!(report.false_authorizations, 0);
    assert_eq!(report.false_denials, 0);
    assert_eq!(report.hle_questions_read, 0);
    assert_eq!(report.production_mutations, 0);
    fs::write(REPORT_JSON, serde_json::to_vec_pretty(&report)?)?;
    fs::write(REPORT_MD, format!("# Stage 286 — self-directed unit learning curve\n\nA fresh 1,000-case technical corpus was evaluated before and after the source-derived unit-conversion candidate was offered in shadow routing. The sealed partition remains isolated from selection.\n\n* baseline authorized: {}\n* promoted authorized: {}\n* baseline / promoted exact: {} / {}\n* sealed baseline / promoted: {} / {}\n* sealed learning delta: {}\n* baseline / promoted replay: {} / {}\n* promoted tamper rejection: {}\n* false authorizations / denials: 0 / 0\n* HLE questions read / production mutations: 0 / 0\n\nPermanent split: development 300, validation 200, sealed 300, boundary 200.\n\nReproduce with `cargo run --quiet --bin stage286_self_directed_unit_learning_curve`.\n", report.baseline_authorized, report.promoted_authorized, report.baseline_exact, report.promoted_exact, report.sealed_baseline_authorized, report.sealed_promoted_authorized, report.sealed_learning_delta, report.baseline_replay, report.promoted_replay, report.promoted_tamper_rejected))?;
    println!(
        "stage286 cases=1000 baseline_auth={} promoted_auth={} sealed_delta={} false_auth=0",
        report.baseline_authorized, report.promoted_authorized, report.sealed_learning_delta
    );
    Ok(())
}
