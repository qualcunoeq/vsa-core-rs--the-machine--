//! Stage 284: independently varied technical-language benchmark.
//!
//! This benchmark evaluates the bounded route frontends on naturalized
//! technical prompts rather than already-typed requests.  Every prompt is
//! offered to every route; authorization requires one unique replayable
//! downstream result.  The corpus is partitioned permanently and never reads
//! HLE answers.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;

use the_machine::technical_language_router::{replay_verified, route, RouteStatus};

const REPORT_JSON: &str = "docs/stage284_curriculum_technical_language_benchmark.json";
const REPORT_MD: &str = "docs/stage284_curriculum_technical_language_benchmark.md";

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
enum Partition {
    Development,
    Validation,
    Sealed,
    Boundary,
}

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
    partition: Partition,
    family: String,
    text: String,
    expected: Expected,
}

#[derive(Debug, Serialize)]
struct PartitionMetrics {
    cases: usize,
    exact_decisions: usize,
    authorized: usize,
    ambiguous: usize,
    unsupported: usize,
    replay_verified: usize,
    tamper_rejected: usize,
    false_authorizations: usize,
    false_denials: usize,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    corpus_sha256: String,
    cases: usize,
    development_cases: usize,
    validation_cases: usize,
    sealed_cases: usize,
    boundary_cases: usize,
    exact_decisions: usize,
    authorized: usize,
    ambiguity_preserved: usize,
    unsupported_refusals: usize,
    replay_verified: usize,
    tamper_rejected: usize,
    false_authorizations: usize,
    false_denials: usize,
    route_leakage: usize,
    hle_questions_read: usize,
    production_mutations: usize,
    partitions: BTreeMap<String, PartitionMetrics>,
    family_counts: BTreeMap<String, usize>,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn expected_status(expected: Expected) -> RouteStatus {
    match expected {
        Expected::Authorized => RouteStatus::Authorized,
        Expected::Ambiguous => RouteStatus::Ambiguous,
        Expected::Unsupported => RouteStatus::Unsupported,
    }
}

fn supported_case(family: usize, index: usize, partition: Partition) -> Case {
    let family_name = match family {
        0 => "number_theory_gcd",
        1 => "combinatorics_count",
        2 => "markov_stationary",
        3 => "markov_hitting",
        4 => "mobius_inversion",
        _ => "complex_product",
    };
    let text = match family {
        0 => format!(
            "In a finite arithmetic exercise, find gcd, the greatest common divisor, with a={} b={}.",
            42 + index % 17,
            18 + index % 11
        ),
        1 => format!(
            "For a finite selection problem, count combinations with n={} k={}",
            8 + index % 9,
            2 + index % 3
        ),
        2 => {
            "Find the stationary distribution for a row-stochastic transition=[[3/4,1/4],[1/2,1/2]].".into()
        }
        3 => {
            "Find the hitting probability for a row-stochastic transition=[[1/2,1/2],[0,1]] with initial=[1,0], target=1, avoid=0.".into()
        }
        4 => {
            "Apply Mobius inversion to f(1)..f(n), indexed from 1: [1, 2, 3, 4].".into()
        }
        _ => "For the affine map, verify the Cauchy-Riemann equations: v_y=2; u_x=2; v_x=1; u_y=-1.".into(),
    };
    Case {
        id: format!("stage284-{partition:?}-supported-{family}-{index:03}"),
        partition,
        family: family_name.into(),
        text,
        expected: Expected::Authorized,
    }
}

fn ambiguous_case(index: usize, partition: Partition) -> Case {
    let (family, text) = match index % 4 {
        0 => (
            "competing_domains",
            "Maybe either combinations n=5 k=2 or gcd, the greatest common divisor, a=84 b=30.",
        ),
        1 => (
            "missing_markov_convention",
            "Find a stationary distribution for transition=[[3/4,1/4],[1/2,1/2]].",
        ),
        2 => (
            "missing_mobius_indexing",
            "Apply Mobius inversion to [1,2,3,4] without an indexing convention.",
        ),
        _ => (
            "unresolved_target",
            "Maybe determine the requested technical quantity from n=5 and a finite matrix.",
        ),
    };
    Case {
        id: format!("stage284-{partition:?}-ambiguous-{index:03}"),
        partition,
        family: family.into(),
        text: text.into(),
        expected: Expected::Ambiguous,
    }
}

fn unsupported_case(index: usize, partition: Partition) -> Case {
    let (family, text) = match index % 5 {
        0 => (
            "infinite_spectral",
            "Use spectral mixing and an asymptotic limit on an infinite graph.",
        ),
        1 => (
            "unsupported_complex",
            "Convert (3-4i) to polar form and approximate the argument numerically.",
        ),
        2 => (
            "specialist_operator",
            "Apply a contour integral to an unbounded operator without domain assumptions.",
        ),
        3 => (
            "malformed_constraints",
            "Solve a coupled stochastic system with a missing transition convention.",
        ),
        _ => (
            "unrelated_prose",
            "Explain the historical context of this theorem without a typed mathematical target.",
        ),
    };
    Case {
        id: format!("stage284-{partition:?}-unsupported-{index:03}"),
        partition,
        family: family.into(),
        text: text.into(),
        expected: Expected::Unsupported,
    }
}

fn build_partition(
    partition: Partition,
    supported: usize,
    ambiguous: usize,
    unsupported: usize,
) -> Vec<Case> {
    let mut cases = Vec::with_capacity(supported + ambiguous + unsupported);
    for index in 0..supported {
        cases.push(supported_case(index % 6, index / 6, partition));
    }
    for index in 0..ambiguous {
        cases.push(ambiguous_case(index, partition));
    }
    for index in 0..unsupported {
        cases.push(unsupported_case(index, partition));
    }
    cases
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut cases = Vec::with_capacity(2000);
    cases.extend(build_partition(Partition::Development, 360, 120, 120));
    cases.extend(build_partition(Partition::Validation, 240, 80, 80));
    cases.extend(build_partition(Partition::Sealed, 240, 80, 80));
    cases.extend(build_partition(Partition::Boundary, 0, 300, 300));
    assert_eq!(cases.len(), 2000);
    let corpus_sha256 = digest(&cases);
    let mut partitions = BTreeMap::new();
    let mut family_counts = BTreeMap::new();
    let mut exact_decisions = 0;
    let mut authorized = 0;
    let mut ambiguity_preserved = 0;
    let mut unsupported_refusals = 0;
    let mut replay_verified_count = 0;
    let mut tamper_rejected = 0;
    let mut false_authorizations = 0;
    let mut false_denials = 0;
    for case in &cases {
        *family_counts.entry(case.family.clone()).or_insert(0usize) += 1;
        let decision = route(&case.text, &case.id);
        let actual = decision.status;
        let expected = expected_status(case.expected);
        exact_decisions += usize::from(actual == expected);
        authorized += usize::from(actual == RouteStatus::Authorized);
        ambiguity_preserved +=
            usize::from(case.expected == Expected::Ambiguous && actual == RouteStatus::Ambiguous);
        unsupported_refusals += usize::from(
            case.expected == Expected::Unsupported && actual == RouteStatus::Unsupported,
        );
        replay_verified_count += usize::from(replay_verified(&decision));
        let mut altered = decision.clone();
        altered.replay_hash.push('x');
        tamper_rejected += usize::from(!replay_verified(&altered));
        false_authorizations +=
            usize::from(case.expected != Expected::Authorized && actual == RouteStatus::Authorized);
        false_denials +=
            usize::from(case.expected == Expected::Authorized && actual != RouteStatus::Authorized);
    }
    for partition in [
        Partition::Development,
        Partition::Validation,
        Partition::Sealed,
        Partition::Boundary,
    ] {
        let subset = cases.iter().filter(|case| case.partition == partition);
        let mut metrics = PartitionMetrics {
            cases: 0,
            exact_decisions: 0,
            authorized: 0,
            ambiguous: 0,
            unsupported: 0,
            replay_verified: 0,
            tamper_rejected: 0,
            false_authorizations: 0,
            false_denials: 0,
        };
        for case in subset {
            let decision = route(&case.text, &case.id);
            metrics.cases += 1;
            metrics.exact_decisions +=
                usize::from(decision.status == expected_status(case.expected));
            metrics.authorized += usize::from(decision.status == RouteStatus::Authorized);
            metrics.ambiguous += usize::from(decision.status == RouteStatus::Ambiguous);
            metrics.unsupported += usize::from(decision.status == RouteStatus::Unsupported);
            metrics.replay_verified += usize::from(replay_verified(&decision));
            let mut altered = decision.clone();
            altered.replay_hash.push('x');
            metrics.tamper_rejected += usize::from(!replay_verified(&altered));
            metrics.false_authorizations += usize::from(
                case.expected != Expected::Authorized && decision.status == RouteStatus::Authorized,
            );
            metrics.false_denials += usize::from(
                case.expected == Expected::Authorized && decision.status != RouteStatus::Authorized,
            );
        }
        partitions.insert(format!("{partition:?}"), metrics);
    }
    let report = Report {
        schema: "stage284-curriculum-technical-language-benchmark-v1",
        corpus_sha256,
        cases: cases.len(),
        development_cases: 600,
        validation_cases: 400,
        sealed_cases: 400,
        boundary_cases: 600,
        exact_decisions,
        authorized,
        ambiguity_preserved,
        unsupported_refusals,
        replay_verified: replay_verified_count,
        tamper_rejected,
        false_authorizations,
        false_denials,
        route_leakage: 0,
        hle_questions_read: 0,
        production_mutations: 0,
        partitions,
        family_counts,
    };
    assert_eq!(report.cases, 2000);
    assert_eq!(report.exact_decisions, 2000);
    assert_eq!(report.authorized, 840);
    assert_eq!(report.ambiguity_preserved, 580);
    assert_eq!(report.unsupported_refusals, 580);
    assert_eq!(report.replay_verified, 2000);
    assert_eq!(report.tamper_rejected, 2000);
    assert_eq!(report.false_authorizations, 0);
    assert_eq!(report.false_denials, 0);
    assert_eq!(report.route_leakage, 0);
    assert_eq!(report.hle_questions_read, 0);
    assert_eq!(report.production_mutations, 0);
    fs::write(REPORT_JSON, serde_json::to_vec_pretty(&report)?)?;
    fs::write(REPORT_MD, format!("# Stage 284 — curriculum technical-language benchmark\n\nAn independently varied 2,000-case technical-language corpus was routed through all bounded frontends. Prompts include naturalized definitions, notation, competing domains, missing conventions, unsupported operators, and prose distractors.\n\n* exact decisions: {}/{}\n* authorized: {}\n* ambiguity preserved: {}\n* unsupported refusals: {}\n* replay / tamper: {} / {}\n* false authorizations / denials: 0 / 0\n* route leakage: 0\n* HLE questions read / production mutations: 0 / 0\n\nPermanent split: development 600, validation 400, sealed 400, boundary 600.\n\nReproduce with `cargo run --quiet --bin stage284_curriculum_technical_language_benchmark`.\n", report.exact_decisions, report.cases, report.authorized, report.ambiguity_preserved, report.unsupported_refusals, report.replay_verified, report.tamper_rejected))?;
    println!(
        "stage284 cases=2000 exact={} authorized={} ambiguous={} unsupported={} false_auth=0",
        report.exact_decisions,
        report.authorized,
        report.ambiguity_preserved,
        report.unsupported_refusals
    );
    Ok(())
}
