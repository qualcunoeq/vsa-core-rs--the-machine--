//! Stage 168: clone-only admission of the source-derived geometry capability.
//!
//! Admission is possible only after the complete source, language-transfer,
//! composition, holdout, replay, and tamper evidence is preflighted.  The
//! production curriculum manifest is never mutated.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::fs;
use the_machine::curriculum::{
    breadth_first_manifest, CurriculumPack, CurriculumStatus, ValidationGates,
};

const SOURCE_REPORTS: [&str; 5] = [
    "docs/stage163_source_geometry_acquisition.json",
    "docs/stage164_source_geometry_language_transfer.json",
    "docs/stage165_geometry_measurement_composition.json",
    "docs/stage166_route_blind_measurement_composition.json",
    "docs/stage167_geometry_technical_language_scale.json",
];
const REPORT_JSON: &str = "docs/stage168_geometry_curriculum_admission.json";
const REPORT_MD: &str = "docs/stage168_geometry_curriculum_admission.md";

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum Scenario {
    Clean,
    MissingPrerequisite,
    DuplicateId,
    InvalidGates,
    UnfrozenHlePolicy,
}

#[derive(Debug, Serialize)]
struct Receipt {
    id: String,
    scenario: Scenario,
    preflight_passed: bool,
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
    source_report_sha256: Vec<String>,
    cases: usize,
    preflight_passed: usize,
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

fn preflight() -> Result<bool, Box<dyn std::error::Error>> {
    let reports = SOURCE_REPORTS
        .iter()
        .map(|path| -> Result<Value, Box<dyn std::error::Error>> {
            Ok(serde_json::from_slice(&fs::read(path)?)?)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let s163 = &reports[0];
    let s164 = &reports[1];
    let s165 = &reports[2];
    let s166 = &reports[3];
    let s167 = &reports[4];
    Ok(field(s163, "independent_development_cases") == 240
        && field(s163, "development_exact_decisions") == 240
        && field(s163, "holdout_exact_decisions") == 60
        && field(s163, "development_replay_verified") == 240
        && field(s163, "holdout_replay_verified") == 60
        && field(s164, "development_exact_decisions") == 500
        && field(s164, "holdout_exact_decisions") == 100
        && field(s165, "development_exact") == 300
        && field(s165, "holdout_exact") == 100
        && field(s166, "development_exact") == 800
        && field(s166, "holdout_exact") == 200
        && field(s167, "development_exact") == 1600
        && field(s167, "holdout_exact") == 400
        && reports.iter().all(|report| {
            field(report, "false_authorizations") == 0 && field(report, "false_denials") == 0
        }))
}

fn scenarios() -> Vec<(String, Scenario)> {
    [
        (Scenario::Clean, 80),
        (Scenario::MissingPrerequisite, 40),
        (Scenario::DuplicateId, 40),
        (Scenario::InvalidGates, 40),
        (Scenario::UnfrozenHlePolicy, 40),
    ]
    .into_iter()
    .flat_map(|(scenario, count)| {
        (0..count).map(move |index| (format!("stage168-{scenario:?}-{index:03}"), scenario))
    })
    .collect()
}

fn candidate(scenario: Scenario) -> CurriculumPack {
    let id = if scenario == Scenario::DuplicateId {
        "source_derived_finite_metric"
    } else {
        "source_derived_bounded_geometry"
    };
    let prerequisites = if scenario == Scenario::MissingPrerequisite {
        vec!["missing_measurement_parent".into()]
    } else {
        vec!["bounded_calculus".into()]
    };
    let gates = ValidationGates {
        authoritative_sources: scenario != Scenario::InvalidGates,
        independent_development_corpus: scenario != Scenario::InvalidGates,
        boundary_corpus: scenario != Scenario::InvalidGates,
        pressure_corpus: scenario != Scenario::InvalidGates,
        replay_verified: scenario != Scenario::InvalidGates,
        zero_false_authorization: scenario != Scenario::InvalidGates,
        frozen_hle_holdout: scenario != Scenario::InvalidGates,
    };
    CurriculumPack {
        id: id.into(),
        title: "Source-derived bounded geometry and measurement".into(),
        status: CurriculumStatus::Promotable,
        prerequisites,
        reusable_artifacts: vec![
            "source_formula_catalog".into(),
            "typed_measurement_conversion".into(),
            "dimensional_expression".into(),
            "geometry_measurement_composition".into(),
        ],
        source_requirements: SOURCE_REPORTS.iter().map(|path| (*path).into()).collect(),
        validation_gates: gates,
        hle_policy: if scenario == Scenario::UnfrozenHlePolicy {
            "live HLE routing allowed".into()
        } else {
            "HLE remains a frozen diagnostic holdout; never development data".into()
        },
        selection_reason: "source and language evidence passed independent composition gates"
            .into(),
    }
}

fn expected_admitted(scenario: Scenario) -> bool {
    scenario == Scenario::Clean
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let preflight_passed = preflight()?;
    assert!(preflight_passed);
    let source_report_sha256 = SOURCE_REPORTS
        .iter()
        .map(|path| digest(&fs::read(path).expect("source report")))
        .collect::<Vec<_>>();
    let mut receipts = Vec::new();
    for (id, scenario) in scenarios() {
        let parent = breadth_first_manifest();
        let parent_hash = parent.replay_hash();
        let pack = candidate(scenario);
        let expected = expected_admitted(scenario);
        let prerequisite_closure = pack
            .prerequisites
            .iter()
            .all(|prerequisite| parent.packs.iter().any(|item| &item.id == prerequisite));
        let mut clone = parent.clone();
        clone.packs.push(pack);
        let errors = clone.validate();
        let admitted = errors.is_empty();
        let replay_stable = errors == clone.clone().validate();
        let mut tampered = clone.clone();
        tampered.policy.push_str(";tampered");
        let tamper_rejected = tampered.replay_hash() != clone.replay_hash();
        receipts.push(Receipt {
            id,
            scenario,
            preflight_passed,
            admitted,
            exact: admitted == expected,
            replay_stable,
            tamper_rejected,
            prerequisite_closure,
            parent_manifest_unchanged: parent.replay_hash() == parent_hash,
            error_count: errors.len(),
        });
    }
    let cases = receipts.len();
    let exact = receipts.iter().filter(|receipt| receipt.exact).count();
    let admitted = receipts.iter().filter(|receipt| receipt.admitted).count();
    let blocked = cases - admitted;
    let replay_stable = receipts
        .iter()
        .filter(|receipt| receipt.replay_stable)
        .count();
    let tamper_rejections = receipts
        .iter()
        .filter(|receipt| receipt.tamper_rejected)
        .count();
    let prerequisite_closures = receipts
        .iter()
        .filter(|receipt| receipt.prerequisite_closure)
        .count();
    let unchanged = receipts
        .iter()
        .filter(|receipt| receipt.parent_manifest_unchanged)
        .count();
    let false_admissions = receipts
        .iter()
        .filter(|receipt| receipt.admitted && !expected_admitted(receipt.scenario))
        .count();
    let false_rejections = receipts
        .iter()
        .filter(|receipt| !receipt.admitted && expected_admitted(receipt.scenario))
        .count();
    assert_eq!(cases, 240);
    assert_eq!(exact, 240);
    assert_eq!(admitted, 80);
    assert_eq!(blocked, 160);
    assert_eq!(replay_stable, 240);
    assert_eq!(tamper_rejections, 240);
    assert_eq!(prerequisite_closures, 200);
    assert_eq!(unchanged, 240);
    assert_eq!(false_admissions, 0);
    assert_eq!(false_rejections, 0);
    let report = Report {
        schema: "stage168-geometry-curriculum-admission-v1",
        source_report_sha256,
        cases,
        preflight_passed: usize::from(preflight_passed) * cases,
        exact_admission_decisions: exact,
        admitted,
        blocked,
        replay_stable,
        tamper_rejections,
        prerequisite_closures,
        parent_manifest_unchanged: unchanged,
        false_admissions,
        false_rejections,
        live_manifest_mutations: 0,
        receipts,
    };
    fs::write(
        REPORT_JSON,
        format!("{}\n", serde_json::to_string_pretty(&report)?),
    )?;
    fs::write(
        REPORT_MD,
        "# Stage 168 — clone-only geometry curriculum admission\n\nThe source-derived geometry/measurement capability was preflighted against all five immutable acquisition, language, composition, and holdout reports. Clean candidates admit only into a cloned curriculum manifest; missing prerequisites, duplicate identifiers, invalid gates, and unfrozen HLE policy are blocked.\n\n| Measure | Result |\n|---|---:|\n| Cases | 240 |\n| Source preflight | 240/240 |\n| Exact admission decisions | 240/240 |\n| Admitted / blocked | 80 / 160 |\n| Replay stable / tamper rejected | 240/240 / 240/240 |\n| Prerequisite closures | 200/240 |\n| Parent manifest unchanged | 240/240 |\n| False admissions / rejections | 0 / 0 |\n| Live manifest mutations | 0 |\n\nThe production curriculum manifest remains unchanged.\n",
    )?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
