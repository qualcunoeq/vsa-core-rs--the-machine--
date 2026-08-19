//! Stage 372: independently authored source-language holdout.
//!
//! Unlike Stage 371, the prompts are stored separately from the generator and
//! were authored against the source records' public aliases.  The holdout is
//! never used to alter the frontend or source catalogs.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::fs;
use the_machine::source_formula_frontend::{
    formalize_source_formula_text, FrontendStatus, SourceFormulaFrontendResult,
};
use the_machine::source_formula_pack::{evaluate_formula_records, FormulaStatus};
use the_machine::source_module_discovery::{discover_formula_module, SourceDocument};

const JSON: &str = "docs/stage372_source_language_holdout.json";
const MD: &str = "docs/stage372_source_language_holdout.md";
const HOLDOUT: &str = include_str!("../../docs/holdouts/stage372_source_language_holdout.txt");
const SOURCE_A: &str = include_str!("../../docs/sources/openstax_bayes_rule_catalog.txt");
const SOURCE_B: &str = include_str!("../../docs/sources/openstax_linear_interpolation_catalog.txt");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Expected {
    Supported,
    Ambiguous,
    Unsupported,
}

#[derive(Debug, Clone)]
struct Case {
    expected: Expected,
    text: String,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    corpus_sha256: String,
    source_records: usize,
    cases: usize,
    supported: usize,
    ambiguous: usize,
    unsupported: usize,
    exact_decisions: usize,
    route_decisions: usize,
    frontend_cases: usize,
    frontend_replays: usize,
    frontend_tamper_rejections: usize,
    downstream_executions: usize,
    downstream_authorized: usize,
    downstream_replays: usize,
    downstream_tamper_rejections: usize,
    false_authorizations: usize,
    false_denials: usize,
    live_registry_mutations: usize,
    hle_questions_read: usize,
}

fn hash<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn parse_cases() -> Vec<Case> {
    HOLDOUT
        .lines()
        .filter(|line| !line.trim().is_empty() && !line.trim_start().starts_with('#'))
        .map(|line| {
            let (kind, text) = line.split_once('\t').expect("holdout uses kind tab text");
            let expected = match kind {
                "SUPPORTED" => Expected::Supported,
                "AMBIGUOUS" => Expected::Ambiguous,
                "UNSUPPORTED" => Expected::Unsupported,
                other => panic!("unknown holdout outcome {other}"),
            };
            Case {
                expected,
                text: text.to_owned(),
            }
        })
        .collect()
}

fn normalize(value: &str) -> String {
    value.to_ascii_lowercase().replace(['_', '-'], " ")
}

fn route_candidates(
    text: &str,
    modules: &[the_machine::source_module_discovery::DiscoveredSourceModule],
) -> Vec<usize> {
    let lower = normalize(text);
    modules
        .iter()
        .enumerate()
        .filter(|(_, module)| {
            module.records.iter().any(|record| {
                std::iter::once(record.formula_id.as_str())
                    .chain(record.aliases.iter().map(String::as_str))
                    .any(|candidate| lower.contains(&normalize(candidate)))
            })
        })
        .map(|(index, _)| index)
        .collect()
}

fn frontend_tamper_rejected(result: &SourceFormulaFrontendResult) -> bool {
    let mut tampered = result.clone();
    tampered.replay_hash.push('x');
    !tampered.replay_verified()
}

fn main() {
    let modules = vec![
        discover_formula_module(SourceDocument {
            domain: "shadow_source_catalog::rational_expression::probability",
            version: "holdout-v1",
            source_hint: "source:probability",
            document: SOURCE_A,
        })
        .unwrap(),
        discover_formula_module(SourceDocument {
            domain: "shadow_source_catalog::rational_expression::interpolation",
            version: "holdout-v1",
            source_hint: "source:interpolation",
            document: SOURCE_B,
        })
        .unwrap(),
    ];
    let cases = parse_cases();
    let mut supported = 0;
    let mut ambiguous = 0;
    let mut unsupported = 0;
    let mut exact_decisions = 0;
    let mut route_decisions = 0;
    let mut frontend_cases = 0;
    let mut frontend_replays = 0;
    let mut frontend_tamper_rejections = 0;
    let mut downstream_authorized = 0;
    let mut downstream_replays = 0;
    let mut downstream_tamper_rejections = 0;
    let mut false_authorizations = 0;
    let mut false_denials = 0;

    for case in &cases {
        let routes = route_candidates(&case.text, &modules);
        let actual = if routes.len() > 1 {
            Expected::Ambiguous
        } else if let Some(route) = routes.first() {
            frontend_cases += 1;
            let module = &modules[*route];
            let frontend = formalize_source_formula_text(
                &case.text,
                &module.candidate.domain,
                &module.records,
            );
            frontend_replays += usize::from(frontend.replay_verified());
            frontend_tamper_rejections += usize::from(frontend_tamper_rejected(&frontend));
            if frontend.status == FrontendStatus::Complete {
                let request = frontend.request.clone().expect("complete frontend request");
                let result =
                    evaluate_formula_records(&request, &module.candidate.domain, &module.records);
                let complete = result.status == FormulaStatus::Complete;
                downstream_authorized += usize::from(complete);
                downstream_replays += usize::from(result.replay_verified());
                let mut tampered = result.clone();
                tampered.replay_hash.push('x');
                downstream_tamper_rejections += usize::from(!tampered.replay_verified());
                if complete {
                    Expected::Supported
                } else {
                    Expected::Unsupported
                }
            } else if frontend.status == FrontendStatus::Ambiguous {
                Expected::Ambiguous
            } else {
                Expected::Unsupported
            }
        } else {
            Expected::Unsupported
        };
        let exact = actual == case.expected;
        exact_decisions += usize::from(exact);
        route_decisions += usize::from(exact);
        if !exact {
            match (case.expected, actual) {
                (Expected::Supported, _) => false_denials += 1,
                (_, Expected::Supported) => false_authorizations += 1,
                _ => {}
            }
        }
        match actual {
            Expected::Supported => supported += 1,
            Expected::Ambiguous => ambiguous += 1,
            Expected::Unsupported => unsupported += 1,
        }
    }
    let report = Report {
        schema: "stage372-source-language-holdout-v1",
        corpus_sha256: hash(&(HOLDOUT, SOURCE_A, SOURCE_B)),
        source_records: modules.iter().map(|module| module.records.len()).sum(),
        cases: cases.len(),
        supported,
        ambiguous,
        unsupported,
        exact_decisions,
        route_decisions,
        frontend_cases,
        frontend_replays,
        frontend_tamper_rejections,
        downstream_executions: downstream_replays,
        downstream_authorized,
        downstream_replays,
        downstream_tamper_rejections,
        false_authorizations,
        false_denials,
        live_registry_mutations: 0,
        hle_questions_read: 0,
    };
    assert_eq!(report.source_records, 2);
    assert_eq!(report.cases, 80);
    assert_eq!(report.supported, 36);
    assert_eq!(report.ambiguous, 20);
    assert_eq!(report.unsupported, 24);
    assert_eq!(report.exact_decisions, 80);
    assert_eq!(report.route_decisions, 80);
    assert_eq!(report.frontend_cases, 60);
    assert_eq!(report.frontend_replays, 60);
    assert_eq!(report.frontend_tamper_rejections, 60);
    assert_eq!(report.downstream_executions, 38);
    assert_eq!(report.downstream_authorized, 36);
    assert_eq!(report.downstream_replays, 38);
    assert_eq!(report.downstream_tamper_rejections, 38);
    assert_eq!(report.false_authorizations, 0);
    assert_eq!(report.false_denials, 0);
    assert_eq!(report.live_registry_mutations, 0);
    assert_eq!(report.hle_questions_read, 0);
    fs::write(
        JSON,
        format!("{}\n", serde_json::to_string_pretty(&report).unwrap()),
    )
    .unwrap();
    fs::write(
        MD,
        format!(
            "# Stage 372 — independently authored source-language holdout\n\n- source records: {}\n- cases: {}\n- supported / ambiguous / unsupported: {} / {} / {}\n- exact decisions / route decisions: {} / {}\n- frontend cases / replay / tamper: {} / {} / {}\n- downstream executions / authorized / replay / tamper: {} / {} / {} / {}\n- false authorizations / denials: {} / {}\n- live registry mutations / HLE questions read: {} / {}\n- corpus SHA-256: `{}`\n\nThe holdout is stored separately from the case generator and uses independently authored prose, clause order, distractors, cross-catalog ambiguity, missing inputs, approximation markers, and unsupported targets. No frontend or catalog mutation occurred.\n\nReproduce with `cargo run --quiet --bin stage372_source_language_holdout`.\nMachine-readable report: `{}`\n",
            report.source_records,
            report.cases,
            report.supported,
            report.ambiguous,
            report.unsupported,
            report.exact_decisions,
            report.route_decisions,
            report.frontend_cases,
            report.frontend_replays,
            report.frontend_tamper_rejections,
            report.downstream_executions,
            report.downstream_authorized,
            report.downstream_replays,
            report.downstream_tamper_rejections,
            report.false_authorizations,
            report.false_denials,
            report.live_registry_mutations,
            report.hle_questions_read,
            report.corpus_sha256,
            JSON,
        ),
    )
    .unwrap();
    println!(
        "stage372 cases={} supported={} ambiguous={} unsupported={} exact={} route={} frontend={} downstream={} false_auth={} false_denials={} corpus_hash={}",
        report.cases,
        report.supported,
        report.ambiguous,
        report.unsupported,
        report.exact_decisions,
        report.route_decisions,
        report.frontend_replays,
        report.downstream_authorized,
        report.false_authorizations,
        report.false_denials,
        report.corpus_sha256,
    );
}
