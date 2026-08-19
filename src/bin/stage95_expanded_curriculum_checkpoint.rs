//! Stage 95: expanded sealed-curriculum checkpoint.
//!
//! This report is a lineage-preserving aggregate, not a replacement for either
//! source corpus.  The original 5,000-case sealed curriculum exam remains an
//! immutable broad checkpoint; the 480-case mixed source-domain transfer set is
//! independently hashed and added as a separate extension.

use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use the_machine::curriculum::breadth_first_manifest;

const PRIOR: &str = include_str!("../../docs/stage_k_sealed_curriculum_exam_5000.json");
const EXTENSION: &str = include_str!("../../docs/stage94_source_domain_router.json");

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    prior_schema: String,
    extension_schema: String,
    prior_report_sha256: String,
    extension_report_sha256: String,
    manifest_sha256: String,
    cases: usize,
    supported: usize,
    ambiguous: usize,
    unsupported: usize,
    supported_authorized: usize,
    ambiguities_preserved: usize,
    unsupported_refused: usize,
    replay_verified: usize,
    tamper_rejections: usize,
    provenance_preserved: usize,
    false_authorizations: usize,
    false_denials: usize,
    route_leakage: usize,
    sealed_cases: usize,
    sealed_supported: usize,
    sealed_ambiguous: usize,
    sealed_unsupported: usize,
    sealed_authorized: usize,
    sealed_replay_verified: usize,
    sealed_tamper_rejections: usize,
    sealed_false_authorizations: usize,
    sealed_false_denials: usize,
}

fn digest(value: &str) -> String {
    format!("{:x}", Sha256::digest(value.as_bytes()))
}

fn number(value: &Value, key: &str) -> usize {
    value[key].as_u64().expect("checkpoint metric is numeric") as usize
}

fn main() {
    let prior: Value = serde_json::from_str(PRIOR).expect("prior sealed report is valid JSON");
    let extension: Value =
        serde_json::from_str(EXTENSION).expect("source routing report is valid JSON");
    let prior_sealed = &prior["partitions"]["sealed"];
    let extension_metrics = &extension["metrics"];
    let extension_sealed = &extension["partitions"]["sealed"];
    let report = Report {
        schema: "stage95-expanded-curriculum-checkpoint-v1",
        prior_schema: prior["schema"].as_str().unwrap().into(),
        extension_schema: extension["schema"].as_str().unwrap().into(),
        prior_report_sha256: digest(PRIOR),
        extension_report_sha256: digest(EXTENSION),
        manifest_sha256: breadth_first_manifest().replay_hash(),
        cases: number(&prior, "cases") + number(extension_metrics, "cases"),
        supported: number(&prior, "supported")
            + number(extension_metrics, "interpolation")
            + number(extension_metrics, "bayes"),
        ambiguous: number(&prior, "ambiguous") + number(extension_metrics, "ambiguous"),
        unsupported: number(&prior, "unsupported") + number(extension_metrics, "unsupported"),
        supported_authorized: number(&prior, "supported_authorized")
            + number(extension_metrics, "authorized"),
        ambiguities_preserved: number(&prior, "ambiguities_preserved")
            + number(extension_metrics, "ambiguity_preserved"),
        unsupported_refused: number(&prior, "unsupported_refused")
            + number(extension_metrics, "unsupported_refused"),
        replay_verified: number(&prior, "replay_verified")
            + number(extension_metrics, "replay_verified"),
        tamper_rejections: number(&prior, "tamper_rejections")
            + number(extension_metrics, "tamper_rejections"),
        provenance_preserved: number(&prior, "provenance_preserved")
            + number(extension_metrics, "provenance_preserved"),
        false_authorizations: number(&prior, "false_authorizations")
            + number(extension_metrics, "false_authorizations"),
        false_denials: number(&prior, "false_denials") + number(extension_metrics, "false_denials"),
        route_leakage: number(extension_metrics, "route_leakage"),
        sealed_cases: number(prior_sealed, "cases") + number(extension_sealed, "cases"),
        sealed_supported: number(prior_sealed, "supported")
            + number(extension_sealed, "interpolation")
            + number(extension_sealed, "bayes"),
        sealed_ambiguous: number(prior_sealed, "ambiguous") + number(extension_sealed, "ambiguous"),
        sealed_unsupported: number(prior_sealed, "unsupported")
            + number(extension_sealed, "unsupported"),
        sealed_authorized: number(prior_sealed, "supported_authorized")
            + number(extension_sealed, "authorized"),
        sealed_replay_verified: number(prior_sealed, "replay_verified")
            + number(extension_sealed, "replay_verified"),
        sealed_tamper_rejections: number(prior_sealed, "tamper_rejections")
            + number(extension_sealed, "tamper_rejections"),
        sealed_false_authorizations: number(prior_sealed, "false_authorizations")
            + number(extension_sealed, "false_authorizations"),
        sealed_false_denials: number(prior_sealed, "false_denials")
            + number(extension_sealed, "false_denials"),
    };
    assert_eq!(
        (
            report.cases,
            report.supported,
            report.ambiguous,
            report.unsupported
        ),
        (5480, 3240, 1120, 1120)
    );
    assert_eq!(report.supported_authorized, 3240);
    assert_eq!(report.ambiguities_preserved, 1120);
    assert_eq!(report.unsupported_refused, 1120);
    assert_eq!(report.replay_verified, 5480);
    assert_eq!(report.tamper_rejections, 5480);
    assert_eq!(report.provenance_preserved, 5480);
    assert_eq!(report.false_authorizations, 0);
    assert_eq!(report.false_denials, 0);
    assert_eq!(
        (
            report.sealed_cases,
            report.sealed_supported,
            report.sealed_ambiguous,
            report.sealed_unsupported
        ),
        (1096, 648, 224, 224)
    );
    assert_eq!(report.sealed_authorized, 648);
    assert_eq!(report.sealed_replay_verified, 1096);
    assert_eq!(report.sealed_tamper_rejections, 1096);
    assert_eq!(report.sealed_false_authorizations, 0);
    assert_eq!(report.sealed_false_denials, 0);
    println!("{}", serde_json::to_string_pretty(&report).unwrap());
}
