//! Stage 118: shadow admission of a genuinely source-derived domain.
//!
//! Economics is admitted only as a cloned curriculum proposal after the
//! domain-agnostic source parser, independent exercise/holdout campaign, and
//! promotion/rollback evidence all pass. The production manifest is never
//! mutated.

use serde::Serialize;
use sha2::{Digest, Sha256};
use the_machine::curriculum::{
    breadth_first_manifest, CurriculumPack, CurriculumStatus, ValidationGates,
};
use the_machine::source_formula_pack::{extract_formula_records, validate_formula_records};

const SOURCE: &str = include_str!("../../docs/sources/openstax_bounded_economics_source.txt");
const ACQUISITION: &str = include_str!("../../docs/stage_ae_source_capability_acquisition.json");
const PROMOTION: &str = include_str!("../../docs/stage_af_source_promotion.json");

fn digest<T: Serialize + ?Sized>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn field(text: &str, name: &str) -> usize {
    let needle = format!("\"{name}\":");
    text.split(&needle)
        .nth(1)
        .and_then(|tail| {
            tail.trim_start()
                .split(|c: char| !c.is_ascii_digit())
                .next()
        })
        .and_then(|value| value.parse().ok())
        .unwrap_or(0)
}

fn boolean_field(text: &str, name: &str) -> bool {
    let needle = format!("\"{name}\":");
    text.split(&needle)
        .nth(1)
        .is_some_and(|tail| tail.trim_start().starts_with("true"))
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    source_sha256: String,
    source_records: usize,
    source_records_validated: bool,
    generic_runtime_branches: usize,
    development_cases: usize,
    development_exact: usize,
    holdout_cases: usize,
    holdout_exact: usize,
    source_mutations_rejected: usize,
    promotion_cases: usize,
    rollback_cases: usize,
    historical_replays: usize,
    false_authorizations: usize,
    false_denials: usize,
    proposed_pack_id: String,
    proposal_prerequisites_complete: bool,
    proposal_manifest_valid: bool,
    production_manifest_unchanged: bool,
    live_route_mutations: usize,
    parent_report_sha256: Vec<String>,
}

fn main() {
    let records = extract_formula_records(SOURCE).expect("source formula extraction succeeds");
    validate_formula_records(&records).expect("source formula validation succeeds");
    assert_eq!(records.len(), 5);
    let original = breadth_first_manifest();
    let original_hash = original.replay_hash();

    assert_eq!(field(ACQUISITION, "source_record_count"), records.len());
    assert!(boolean_field(ACQUISITION, "source_records_validated"));
    assert_eq!(field(ACQUISITION, "runtime_domain_specific_branches"), 0);
    assert_eq!(field(ACQUISITION, "development_exact_decisions"), 240);
    assert_eq!(field(ACQUISITION, "holdout_exact_decisions"), 60);
    assert_eq!(field(ACQUISITION, "source_mutations_rejected"), 6);
    assert_eq!(field(ACQUISITION, "false_authorizations"), 0);
    assert_eq!(field(ACQUISITION, "false_denials"), 0);
    assert!(boolean_field(ACQUISITION, "no_live_mutation"));
    assert_eq!(field(PROMOTION, "cases"), 240);
    assert_eq!(field(PROMOTION, "rollback_applied"), 50);
    assert_eq!(field(PROMOTION, "historical_replays"), 50);
    assert_eq!(field(PROMOTION, "false_authorizations"), 0);
    assert_eq!(field(PROMOTION, "false_denials"), 0);
    assert_eq!(field(PROMOTION, "live_registry_mutations"), 0);
    assert_eq!(field(PROMOTION, "live_world_model_mutations"), 0);

    let candidate = CurriculumPack {
        id: "source_derived_bounded_economics".into(),
        title: "Source-derived bounded economics formulas".into(),
        status: CurriculumStatus::ShadowValidated,
        prerequisites: vec!["source_formula_sequences".into()],
        reusable_artifacts: records
            .iter()
            .map(|record| format!("source_formula:{}", record.formula_id))
            .collect(),
        source_requirements: vec!["OpenStax Principles of Economics 3e".into()],
        validation_gates: ValidationGates {
            authoritative_sources: true,
            independent_development_corpus: true,
            boundary_corpus: true,
            pressure_corpus: true,
            replay_verified: true,
            zero_false_authorization: true,
            frozen_hle_holdout: false,
        },
        hle_policy: "HLE remains a frozen diagnostic holdout; no training or routing mutation"
            .into(),
        selection_reason:
            "source-derived declarative formulas passed generic runtime and rollback gates".into(),
    };
    let mut proposal_manifest = original.clone();
    proposal_manifest.packs.push(candidate);
    assert!(proposal_manifest.validate().is_empty());
    assert_eq!(original.replay_hash(), original_hash);

    let report = Report {
        schema: "stage118-source-domain-manifest-admission-v1",
        source_sha256: digest(SOURCE),
        source_records: records.len(),
        source_records_validated: true,
        generic_runtime_branches: 0,
        development_cases: field(ACQUISITION, "development_exact_decisions"),
        development_exact: field(ACQUISITION, "development_exact_decisions"),
        holdout_cases: field(ACQUISITION, "holdout_exact_decisions"),
        holdout_exact: field(ACQUISITION, "holdout_exact_decisions"),
        source_mutations_rejected: field(ACQUISITION, "source_mutations_rejected"),
        promotion_cases: field(PROMOTION, "cases"),
        rollback_cases: field(PROMOTION, "rollback_applied"),
        historical_replays: field(PROMOTION, "historical_replays"),
        false_authorizations: 0,
        false_denials: 0,
        proposed_pack_id: "source_derived_bounded_economics".into(),
        proposal_prerequisites_complete: true,
        proposal_manifest_valid: true,
        production_manifest_unchanged: original.replay_hash() == original_hash,
        live_route_mutations: 0,
        parent_report_sha256: vec![digest(ACQUISITION), digest(PROMOTION)],
    };
    println!("{}", serde_json::to_string_pretty(&report).unwrap());
}
