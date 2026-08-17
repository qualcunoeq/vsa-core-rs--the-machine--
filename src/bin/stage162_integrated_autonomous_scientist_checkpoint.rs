//! Stage 162: integrated release checkpoint for the governed scientist stack.
//!
//! This is a lineage-only checkpoint. It reads immutable reports from the
//! curriculum, source reasoning, epistemic integration, self-directed
//! education, and sealed HLE runs; it does not merge registries, promote a
//! capability, or reinterpret any parent result.

use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;

const REPORT_JSON: &str = "docs/stage162_integrated_autonomous_scientist_checkpoint.json";
const REPORT_MD: &str = "docs/stage162_integrated_autonomous_scientist_checkpoint.md";

const PARENTS: &[&str] = &[
    "docs/stage157_integrated_curriculum_checkpoint.json",
    "docs/stage158_hle_checkpoint_after_curriculum.json",
    "docs/stage159_source_reasoning_scale.json",
    "docs/stage160_source_epistemic_integration.json",
    "docs/stage161_self_directed_curriculum_scale.json",
];

fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn number(report: &Value, field: &str) -> usize {
    report
        .get(field)
        .and_then(Value::as_u64)
        .unwrap_or_else(|| panic!("missing numeric field {field}")) as usize
}

fn zero(report: &Value, field: &str) -> bool {
    number(report, field) == 0
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    release: &'static str,
    parent_report_sha256: BTreeMap<String, String>,
    non_hle_evaluation_cases: usize,
    integrated_curriculum_cases: usize,
    source_reasoning_cases: usize,
    source_epistemic_cases: usize,
    self_directed_cases: usize,
    hle_cases: usize,
    hle_correct_authorized: usize,
    hle_false_authorizations: usize,
    curriculum_authorized: usize,
    source_claim_uses: usize,
    source_epistemic_resolutions: usize,
    self_directed_sealed_authorized: usize,
    replay_receipts_verified: usize,
    tamper_receipts_rejected: usize,
    false_authorizations: usize,
    false_denials: usize,
    live_registry_mutations: usize,
    live_fact_mutations: usize,
    manifest_mutations: usize,
    source_memory_records: usize,
    source_memory_reconstructed: bool,
    hle_route_receipts: usize,
    hle_transfer_delta: isize,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut reports = BTreeMap::new();
    for path in PARENTS {
        reports.insert(
            (*path).to_owned(),
            serde_json::from_slice::<Value>(&fs::read(path)?)?,
        );
    }
    let curriculum = &reports[PARENTS[0]];
    let hle = &reports[PARENTS[1]];
    let source = &reports[PARENTS[2]];
    let epistemic = &reports[PARENTS[3]];
    let self_directed = &reports[PARENTS[4]];
    assert_eq!(
        curriculum["schema"],
        "stage157-integrated-curriculum-checkpoint-v1"
    );
    assert_eq!(hle["schema"], "stage158-hle-checkpoint-after-curriculum-v1");
    assert_eq!(source["schema"], "stage159-source-reasoning-scale-v1");
    assert_eq!(
        epistemic["schema"],
        "stage160-source-epistemic-integration-v1"
    );
    assert_eq!(
        self_directed["schema"],
        "stage161-self-directed-curriculum-scale-v1"
    );
    let mut parent_hashes = BTreeMap::new();
    for path in PARENTS {
        parent_hashes.insert((*path).to_owned(), digest(&fs::read(path)?));
    }
    let integrated_curriculum_cases = number(curriculum, "cases");
    let source_reasoning_cases = number(source, "cases");
    let source_epistemic_cases = number(epistemic, "cases");
    let self_directed_cases = number(self_directed, "cases");
    let non_hle_evaluation_cases = integrated_curriculum_cases
        + source_reasoning_cases
        + source_epistemic_cases
        + self_directed_cases;
    let hle_cases = number(hle, "cases");
    let hle_correct_authorized = number(hle, "correct_authorized");
    let hle_prior = number(hle, "prior_correct_authorized");
    let replay_receipts_verified = number(curriculum, "replay_verified")
        + number(source, "replay_verified")
        + number(epistemic, "epistemic_replay_verified")
        + number(self_directed, "sealed_replay_verified");
    let tamper_receipts_rejected = number(curriculum, "tamper_rejected")
        + number(source, "tamper_rejected")
        + number(epistemic, "epistemic_tamper_rejected")
        + number(self_directed, "sealed_tamper_rejected");
    let false_authorizations = number(curriculum, "false_authorizations")
        + number(source, "false_authorizations")
        + number(epistemic, "false_resolutions")
        + number(self_directed, "false_authorizations")
        + number(hle, "false_authorizations");
    let false_denials = number(curriculum, "false_denials")
        + number(source, "false_denials")
        + number(self_directed, "false_denials")
        + number(hle, "incorrect_authorized");
    let live_registry_mutations = number(curriculum, "production_mutations")
        + number(source, "production_registry_mutations")
        + number(epistemic, "production_registry_mutations")
        + number(self_directed, "production_registry_mutations");
    let live_fact_mutations =
        number(source, "live_fact_mutations") + number(epistemic, "live_fact_mutations");
    let manifest_mutations = number(self_directed, "manifest_mutations");
    assert!(zero(curriculum, "false_authorizations") && zero(curriculum, "false_denials"));
    assert!(zero(source, "false_authorizations") && zero(source, "false_denials"));
    assert!(zero(epistemic, "false_resolutions"));
    assert!(zero(self_directed, "false_authorizations") && zero(self_directed, "false_denials"));
    assert!(zero(hle, "false_authorizations") && zero(hle, "incorrect_authorized"));
    assert_eq!(false_authorizations, 0);
    assert_eq!(false_denials, 0);
    assert_eq!(live_registry_mutations, 0);
    assert_eq!(live_fact_mutations, 0);
    assert_eq!(manifest_mutations, 0);
    assert_eq!(number(hle, "route_receipts"), 0);
    assert!(number(curriculum, "source_memory_records") >= 100_000);
    assert!(curriculum["source_memory_reconstructed"]
        .as_bool()
        .unwrap_or(false));
    assert!(
        number(self_directed, "sealed_exact_decisions") == number(self_directed, "sealed_cases")
    );
    assert!(self_directed["campaign_replay_verified"]
        .as_bool()
        .unwrap_or(false));
    let report = Report {
        schema: "stage162-integrated-autonomous-scientist-checkpoint-v1",
        release: "governed-source-epistemic-self-directed-curriculum",
        parent_report_sha256: parent_hashes,
        non_hle_evaluation_cases,
        integrated_curriculum_cases,
        source_reasoning_cases,
        source_epistemic_cases,
        self_directed_cases,
        hle_cases,
        hle_correct_authorized,
        hle_false_authorizations: number(hle, "false_authorizations"),
        curriculum_authorized: number(curriculum, "authorized"),
        source_claim_uses: number(source, "source_use_authorized"),
        source_epistemic_resolutions: number(epistemic, "supported_resolutions"),
        self_directed_sealed_authorized: number(self_directed, "sealed_authorized"),
        replay_receipts_verified,
        tamper_receipts_rejected,
        false_authorizations,
        false_denials,
        live_registry_mutations,
        live_fact_mutations,
        manifest_mutations,
        source_memory_records: number(curriculum, "source_memory_records"),
        source_memory_reconstructed: curriculum["source_memory_reconstructed"]
            .as_bool()
            .unwrap_or(false),
        hle_route_receipts: number(hle, "route_receipts"),
        hle_transfer_delta: hle_correct_authorized as isize - hle_prior as isize,
    };
    fs::write(
        REPORT_JSON,
        format!("{}\n", serde_json::to_string_pretty(&report)?),
    )?;
    fs::write(
        REPORT_MD,
        format!(
            "# Stage 162 — integrated autonomous scientist checkpoint\n\nThis lineage checkpoint combines the immutable curriculum, source reasoning, epistemic integration, self-directed education, and sealed HLE reports. It does not merge registries or reinterpret parent receipts.\n\n| Measure | Result |\n|---|---:|\n| Non-HLE evaluation cases | {} |\n| Integrated curriculum cases | {} |\n| Source reasoning / epistemic cases | {} / {} |\n| Self-directed campaign cases | {} |\n| Verified replay receipts | {} |\n| Tamper rejections | {} |\n| False authorizations / denials | 0 / 0 |\n| Live registry/fact/manifest mutations | 0 / 0 / 0 |\n| Source-memory records | {} |\n| HLE authorized answers | {}/{} (delta {}) |\n| HLE route receipts | 0 |\n\nAll parent report hashes are recorded in the JSON manifest. This is a release checkpoint, not a completion claim for the full Machine vision.\n",
            non_hle_evaluation_cases,
            integrated_curriculum_cases,
            source_reasoning_cases,
            source_epistemic_cases,
            self_directed_cases,
            replay_receipts_verified,
            tamper_receipts_rejected,
            number(curriculum, "source_memory_records"),
            hle_correct_authorized,
            hle_cases,
            hle_correct_authorized as isize - hle_prior as isize,
        ),
    )?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
