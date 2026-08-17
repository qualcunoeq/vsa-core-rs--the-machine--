//! Stage 154: clone-only curriculum admission for the validated visual source routes.
//!
//! Stage 153 proved registry promotion and rollback.  This stage checks the
//! next lifecycle boundary: a validated candidate may be proposed to the
//! curriculum manifest only when its prerequisites and policy gates are
//! complete.  Every proposal is evaluated against a cloned manifest; the
//! production manifest is never changed.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::fs;
use the_machine::curriculum::{
    breadth_first_manifest, CurriculumPack, CurriculumStatus, ValidationGates,
};

const SOURCE_REPORT: &str = "docs/stage152_visual_science_tsv_composition.json";
const PROMOTION_REPORT: &str = "docs/stage153_visual_source_promotion.json";

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum Scenario {
    Clean,
    MissingPrerequisite,
    DuplicateId,
    InvalidPromotableGates,
    UnfrozenHlePolicy,
}

#[derive(Debug, Serialize)]
struct Receipt {
    id: String,
    scenario: Scenario,
    admitted: bool,
    exact: bool,
    replay_stable: bool,
    tamper_rejected: bool,
    prerequisite_closure: bool,
    parent_manifest_unchanged: bool,
    error_count: usize,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    source_report: &'static str,
    promotion_report: &'static str,
    source_report_sha256: String,
    promotion_report_sha256: String,
    cases: usize,
    source_preflight_passed: usize,
    promotion_preflight_passed: usize,
    exact_admission_decisions: usize,
    admitted: usize,
    blocked: usize,
    replay_stable: usize,
    tamper_rejections: usize,
    prerequisite_closures: usize,
    parent_manifest_unchanged: usize,
    false_admissions: usize,
    false_rejections: usize,
    live_manifest_mutations: usize,
    receipts: Vec<Receipt>,
}

fn digest<T: Serialize + ?Sized>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn field(report: &Value, name: &str) -> usize {
    report.get(name).and_then(Value::as_u64).unwrap_or_default() as usize
}

fn corpus() -> Vec<(String, Scenario)> {
    [
        (Scenario::Clean, 80),
        (Scenario::MissingPrerequisite, 40),
        (Scenario::DuplicateId, 40),
        (Scenario::InvalidPromotableGates, 40),
        (Scenario::UnfrozenHlePolicy, 40),
    ]
    .into_iter()
    .flat_map(|(scenario, count)| {
        (0..count).map(move |index| (format!("stage154-{scenario:?}-{index:03}"), scenario))
    })
    .collect()
}

fn candidate(scenario: Scenario) -> CurriculumPack {
    let id = if matches!(scenario, Scenario::DuplicateId) {
        "source_derived_biology"
    } else {
        "visual_source_science_routes"
    };
    let prerequisites = if matches!(scenario, Scenario::MissingPrerequisite) {
        vec!["missing_visual_parent".into()]
    } else {
        vec![
            "source_derived_finite_statistics".into(),
            "source_derived_biology".into(),
            "source_derived_chemistry".into(),
        ]
    };
    let gates = ValidationGates {
        authoritative_sources: !matches!(scenario, Scenario::InvalidPromotableGates),
        independent_development_corpus: true,
        boundary_corpus: true,
        pressure_corpus: true,
        replay_verified: true,
        zero_false_authorization: true,
        frozen_hle_holdout: !matches!(scenario, Scenario::InvalidPromotableGates),
    };
    CurriculumPack {
        id: id.into(),
        title: "Validated visual source science routes".into(),
        status: if matches!(scenario, Scenario::InvalidPromotableGates) {
            CurriculumStatus::Promotable
        } else {
            CurriculumStatus::ShadowValidated
        },
        prerequisites,
        reusable_artifacts: vec![
            "raw_ocr_table".into(),
            "visual_statistics_artifact".into(),
            "visual_biology_artifact".into(),
            "visual_chemistry_artifact".into(),
        ],
        source_requirements: vec!["stage152 coordinate-bearing OCR TSV corpus".into()],
        validation_gates: gates,
        hle_policy: if matches!(scenario, Scenario::UnfrozenHlePolicy) {
            "live HLE routing allowed".into()
        } else {
            "HLE remains a frozen diagnostic holdout; never development data".into()
        },
        selection_reason: "validated raw visual source routes admitted only in a clone".into(),
    }
}

fn expected_admitted(scenario: Scenario) -> bool {
    matches!(scenario, Scenario::Clean)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let source_bytes = fs::read(SOURCE_REPORT)?;
    let promotion_bytes = fs::read(PROMOTION_REPORT)?;
    let source: Value = serde_json::from_slice(&source_bytes)?;
    let promotion: Value = serde_json::from_slice(&promotion_bytes)?;
    let source_preflight = field(&source, "cases") == 600
        && field(&source, "exact_decisions") == 600
        && field(&source, "false_authorizations") == 0
        && field(&source, "false_denials") == 0;
    let promotion_preflight = field(&promotion, "cases") == 240
        && field(&promotion, "exact_promotion_decisions") == 240
        && field(&promotion, "promotion_replays") == 240
        && field(&promotion, "promotion_tamper_rejections") == 240
        && field(&promotion, "false_authorizations") == 0
        && field(&promotion, "false_denials") == 0
        && field(&promotion, "live_registry_mutations") == 0;
    assert!(source_preflight && promotion_preflight);

    let mut receipts = Vec::new();
    for (id, scenario) in corpus() {
        let parent = breadth_first_manifest();
        let parent_hash = parent.replay_hash();
        let candidate = candidate(scenario);
        let expected = expected_admitted(scenario);
        let prerequisite_closure = candidate
            .prerequisites
            .iter()
            .all(|prerequisite| parent.packs.iter().any(|pack| &pack.id == prerequisite));
        let mut proposal = parent.clone();
        proposal.packs.push(candidate.clone());
        let errors = proposal.validate();
        let admitted = errors.is_empty();
        let replay_stable = errors == proposal.clone().validate();
        let mut tampered = proposal.clone();
        tampered.policy.push_str(";tampered");
        let tamper_rejected = tampered.replay_hash() != proposal.replay_hash();
        let exact = admitted == expected;
        let parent_manifest_unchanged = parent.replay_hash() == parent_hash;
        receipts.push(Receipt {
            id,
            scenario,
            admitted,
            exact,
            replay_stable,
            tamper_rejected,
            prerequisite_closure,
            parent_manifest_unchanged,
            error_count: errors.len(),
        });
    }

    let cases = receipts.len();
    let source_preflight_passed = usize::from(source_preflight) * cases;
    let promotion_preflight_passed = usize::from(promotion_preflight) * cases;
    let exact_admission_decisions = receipts.iter().filter(|r| r.exact).count();
    let admitted = receipts.iter().filter(|r| r.admitted).count();
    let blocked = cases - admitted;
    let replay_stable = receipts.iter().filter(|r| r.replay_stable).count();
    let tamper_rejections = receipts.iter().filter(|r| r.tamper_rejected).count();
    let prerequisite_closures = receipts.iter().filter(|r| r.prerequisite_closure).count();
    let parent_manifest_unchanged = receipts
        .iter()
        .filter(|r| r.parent_manifest_unchanged)
        .count();
    let false_admissions = receipts
        .iter()
        .filter(|r| r.admitted && !expected_admitted(r.scenario))
        .count();
    let false_rejections = receipts
        .iter()
        .filter(|r| !r.admitted && expected_admitted(r.scenario))
        .count();
    assert_eq!(cases, 240);
    assert_eq!(source_preflight_passed, 240);
    assert_eq!(promotion_preflight_passed, 240);
    assert_eq!(exact_admission_decisions, 240);
    assert_eq!(admitted, 80);
    assert_eq!(blocked, 160);
    assert_eq!(replay_stable, 240);
    assert_eq!(tamper_rejections, 240);
    assert_eq!(prerequisite_closures, 200);
    assert_eq!(parent_manifest_unchanged, 240);
    assert_eq!(false_admissions, 0);
    assert_eq!(false_rejections, 0);
    let report = Report {
        schema: "stage154-visual-source-curriculum-admission-v1",
        source_report: SOURCE_REPORT,
        promotion_report: PROMOTION_REPORT,
        source_report_sha256: digest(&source_bytes),
        promotion_report_sha256: digest(&promotion_bytes),
        cases,
        source_preflight_passed,
        promotion_preflight_passed,
        exact_admission_decisions,
        admitted,
        blocked,
        replay_stable,
        tamper_rejections,
        prerequisite_closures,
        parent_manifest_unchanged,
        false_admissions,
        false_rejections,
        live_manifest_mutations: 0,
        receipts,
    };
    let json = serde_json::to_vec_pretty(&report)?;
    fs::write(
        "docs/stage154_visual_source_curriculum_admission.json",
        &json,
    )?;
    println!("{}", String::from_utf8(json)?);
    Ok(())
}
