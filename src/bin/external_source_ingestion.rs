//! Answer-key-blind sandbox ingestion for source-selection proposals.
//!
//! This binary deliberately stops before curriculum promotion.  It accepts
//! only proposals produced by `external_source_selection`, verifies the
//! document hash and generic source-formula schema, exercises the resulting
//! records through the generic interpreter, and injects structural mutations.
//! It never reads an answer key and never mutates the live manifest or router.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use the_machine::curriculum::breadth_first_manifest;
use the_machine::probability_pack::Rational;
use the_machine::source_formula_pack::{
    evaluate_formula_records, extract_formula_records, validate_formula_records, Expr,
    FormulaRecord, FormulaRequest, FormulaStatus, InputConstraint,
};

const SELECTION_REPORT: &str = "docs/goal6_external_source_selection.json";
const OUTPUT_JSON: &str = "docs/goal6_external_source_ingestion.json";
const OUTPUT_MD: &str = "docs/goal6_external_source_ingestion.md";

#[derive(Debug, Deserialize)]
struct SelectionReport {
    release_id: String,
    answer_keys_read: usize,
    manifest_sha256: String,
    proposals: Vec<SelectionProposal>,
}

#[derive(Debug, Deserialize)]
struct SelectionProposal {
    cluster_key: String,
    source_path: String,
    source_sha256: String,
    status: String,
}

#[derive(Debug, Clone, Serialize)]
struct ProposalReceipt {
    cluster_key: String,
    source_path: String,
    source_sha256: String,
    hash_verified: bool,
    format: String,
    records: usize,
    valid_catalog: bool,
    provenance_records: usize,
    generated_exercises: usize,
    generated_complete: usize,
    generated_replays: usize,
    mutation_cases: usize,
    mutation_rejections: usize,
    independent_exercise_evidence: bool,
    promotion_status: String,
    reasons: Vec<String>,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    release_id: String,
    answer_keys_read: usize,
    proposals_considered: usize,
    source_documents_read: usize,
    hash_verified: usize,
    valid_catalogs: usize,
    generated_exercises: usize,
    generated_complete: usize,
    generated_replays: usize,
    mutation_cases: usize,
    mutation_rejections: usize,
    independent_exercise_evidence: usize,
    promotion_allowed: usize,
    manifest_sha256_before: String,
    selection_manifest_matches: bool,
    manifest_sha256_after: String,
    manifest_unchanged: bool,
    false_authorizations: usize,
    replay_verified: bool,
    receipts: Vec<ProposalReceipt>,
    report_sha256: String,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn digest_bytes(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn generated_inputs(record: &FormulaRecord) -> BTreeMap<String, Rational> {
    let mut inputs = BTreeMap::new();
    for name in &record.required_inputs {
        inputs.insert(name.clone(), Rational::new(2, 1).unwrap());
    }
    for constraint in &record.constraints {
        let (name, value) = match constraint {
            InputConstraint::Positive(name) => (name, Rational::new(3, 1).unwrap()),
            InputConstraint::PositiveInteger(name) => (name, Rational::new(4, 1).unwrap()),
            InputConstraint::NonnegativeInteger(name) => (name, Rational::new(4, 1).unwrap()),
            InputConstraint::Probability(name) => (name, Rational::new(1, 4).unwrap()),
            InputConstraint::NotEqualInteger(name, forbidden) => {
                (name, Rational::new(forbidden + 1, 1).unwrap())
            }
        };
        inputs.insert(name.clone(), value);
    }
    inputs
}

fn mutate_catalog(records: &[FormulaRecord]) -> Vec<Vec<FormulaRecord>> {
    if records.is_empty() {
        return Vec::new();
    }
    let mut mutations = Vec::new();

    let mut duplicate_id = records.to_vec();
    if duplicate_id.len() > 1 {
        duplicate_id[1].formula_id = duplicate_id[0].formula_id.clone();
        mutations.push(duplicate_id);
    }

    let mut undeclared_input = records.to_vec();
    undeclared_input[0].expression = Expr::Input("__undeclared_input".into());
    mutations.push(undeclared_input);

    let mut bad_source = records.to_vec();
    bad_source[0].source.url.clear();
    mutations.push(bad_source);
    mutations
}

fn parse_records(path: &str, document: &str) -> Result<(String, Vec<FormulaRecord>), String> {
    if document.contains("BEGIN FORMULA ") {
        return extract_formula_records(document)
            .map(|records| ("formula_text".into(), records))
            .map_err(|errors| errors.join("; "));
    }
    if Path::new(path)
        .extension()
        .and_then(|extension| extension.to_str())
        == Some("json")
    {
        let records: Vec<FormulaRecord> = serde_json::from_str(document)
            .map_err(|error| format!("JSON catalog is not a generic formula catalog: {error}"))?;
        return Ok(("formula_json".into(), records));
    }
    Err("source format has no generic formula blocks".into())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let selection: SelectionReport = serde_json::from_slice(&fs::read(SELECTION_REPORT)?)?;
    let manifest_before = breadth_first_manifest().replay_hash();
    let mut receipts = Vec::new();

    for proposal in selection
        .proposals
        .iter()
        .filter(|proposal| proposal.status == "shadow_candidate")
    {
        let bytes = match fs::read(&proposal.source_path) {
            Ok(bytes) => bytes,
            Err(error) => {
                receipts.push(ProposalReceipt {
                    cluster_key: proposal.cluster_key.clone(),
                    source_path: proposal.source_path.clone(),
                    source_sha256: proposal.source_sha256.clone(),
                    hash_verified: false,
                    format: "unreadable".into(),
                    records: 0,
                    valid_catalog: false,
                    provenance_records: 0,
                    generated_exercises: 0,
                    generated_complete: 0,
                    generated_replays: 0,
                    mutation_cases: 0,
                    mutation_rejections: 0,
                    independent_exercise_evidence: false,
                    promotion_status: "blocked".into(),
                    reasons: vec![format!("source read failed: {error}")],
                });
                continue;
            }
        };
        let hash_verified = digest_bytes(&bytes) == proposal.source_sha256;
        let document = String::from_utf8_lossy(&bytes);
        let parsed = if hash_verified {
            parse_records(&proposal.source_path, &document)
        } else {
            Err("source hash does not match the selection receipt".into())
        };
        let (format, records, parse_reason) = match parsed {
            Ok((format, records)) => (format, records, None),
            Err(reason) => ("unsupported".into(), Vec::new(), Some(reason)),
        };
        let valid_catalog = !records.is_empty() && validate_formula_records(&records).is_ok();
        let provenance_records = records
            .iter()
            .filter(|record| {
                !record.source.source_id.is_empty()
                    && !record.source.evidence_span.is_empty()
                    && record.source.url.starts_with("https://")
            })
            .count();
        let mut generated_complete = 0;
        let mut generated_replays = 0;
        if valid_catalog {
            let domain = format!("external-source-shadow:{}", proposal.source_sha256);
            for record in &records {
                let request = FormulaRequest {
                    formula: record.formula_id.clone(),
                    inputs: generated_inputs(record),
                    domain: domain.clone(),
                    ambiguity: None,
                    provenance: vec![
                        format!("source-document:{}", proposal.source_sha256),
                        format!("source-record:{}", record.source.source_id),
                    ],
                };
                let result = evaluate_formula_records(&request, &domain, &records);
                if result.status == FormulaStatus::Complete && result.value.is_some() {
                    generated_complete += 1;
                }
                if result.replay_verified() {
                    generated_replays += 1;
                }
            }
        }
        let mutations = mutate_catalog(&records);
        let mutation_rejections = mutations
            .iter()
            .filter(|candidate| validate_formula_records(candidate).is_err())
            .count();
        let mut reasons = Vec::new();
        if let Some(reason) = parse_reason {
            reasons.push(reason);
        }
        if !valid_catalog {
            reasons.push("generic catalog validation did not pass".into());
        }
        if provenance_records != records.len() {
            reasons.push("one or more records lack complete source provenance".into());
        }
        reasons.push(
            "generated exercises are derived from source records, not independent external exercise evidence".into(),
        );
        let promotion_status = "blocked";
        receipts.push(ProposalReceipt {
            cluster_key: proposal.cluster_key.clone(),
            source_path: proposal.source_path.clone(),
            source_sha256: proposal.source_sha256.clone(),
            hash_verified,
            format,
            records: records.len(),
            valid_catalog,
            provenance_records,
            generated_exercises: records.len(),
            generated_complete,
            generated_replays,
            mutation_cases: mutations.len(),
            mutation_rejections,
            independent_exercise_evidence: false,
            promotion_status: promotion_status.into(),
            reasons,
        });
    }

    let selection_manifest_matches = selection.manifest_sha256 == manifest_before;
    let manifest_after = breadth_first_manifest().replay_hash();
    let generated_exercises = receipts.iter().map(|r| r.generated_exercises).sum();
    let generated_complete = receipts.iter().map(|r| r.generated_complete).sum();
    let generated_replays = receipts.iter().map(|r| r.generated_replays).sum();
    let mutation_cases = receipts.iter().map(|r| r.mutation_cases).sum();
    let mutation_rejections = receipts.iter().map(|r| r.mutation_rejections).sum();
    let mut report = Report {
        schema: "external-source-ingestion-v1",
        release_id: selection.release_id,
        answer_keys_read: selection.answer_keys_read,
        proposals_considered: selection
            .proposals
            .iter()
            .filter(|proposal| proposal.status == "shadow_candidate")
            .count(),
        source_documents_read: receipts.len(),
        hash_verified: receipts.iter().filter(|r| r.hash_verified).count(),
        valid_catalogs: receipts.iter().filter(|r| r.valid_catalog).count(),
        generated_exercises,
        generated_complete,
        generated_replays,
        mutation_cases,
        mutation_rejections,
        independent_exercise_evidence: receipts
            .iter()
            .filter(|r| r.independent_exercise_evidence)
            .count(),
        promotion_allowed: receipts
            .iter()
            .filter(|r| r.promotion_status == "allowed")
            .count(),
        manifest_sha256_before: manifest_before.clone(),
        selection_manifest_matches,
        manifest_sha256_after: manifest_after.clone(),
        manifest_unchanged: manifest_before == manifest_after,
        false_authorizations: 0,
        replay_verified: receipts.iter().all(|r| {
            (!r.valid_catalog || r.generated_replays == r.generated_exercises)
                && r.mutation_rejections == r.mutation_cases
        }),
        receipts,
        report_sha256: String::new(),
    };
    let mut unsigned = report.clone_for_hash();
    unsigned.report_sha256.clear();
    report.report_sha256 = digest(&unsigned);
    assert_eq!(selection.answer_keys_read, 0);
    assert!(report.selection_manifest_matches);
    assert_eq!(report.false_authorizations, 0);
    assert!(report.manifest_unchanged);
    assert!(report.replay_verified);

    let serialized = serde_json::to_string_pretty(&report)?;
    fs::write(OUTPUT_JSON, format!("{serialized}\n"))?;
    fs::write(
        OUTPUT_MD,
        format!(
            "# Goal 6 — sandbox source ingestion\n\n\
             - Shadow proposals considered: {}\n\
             - Source documents read / hash verified: {} / {}\n\
             - Valid generic catalogs: {}\n\
             - Derived exercises complete / replayed: {} / {}\n\
             - Mutation rejection: {} / {}\n\
             - Independent exercise evidence: {}\n\
             - Promotion allowed: {}\n\
             - Answer keys read: {}\n\
             - Manifest unchanged: {}\n\
             - Selection manifest matches current manifest: {}\n\
             - False authorizations: {}\n\n\
             All proposals remain blocked because source-derived exercises are\
             not independent exercise evidence.\n",
            report.proposals_considered,
            report.source_documents_read,
            report.hash_verified,
            report.valid_catalogs,
            report.generated_complete,
            report.generated_replays,
            report.mutation_rejections,
            report.mutation_cases,
            report.independent_exercise_evidence,
            report.promotion_allowed,
            report.answer_keys_read,
            report.manifest_unchanged,
            report.selection_manifest_matches,
            report.false_authorizations,
        ),
    )?;
    println!("{serialized}");
    Ok(())
}

impl Report {
    fn clone_for_hash(&self) -> Self {
        Self {
            schema: self.schema,
            release_id: self.release_id.clone(),
            answer_keys_read: self.answer_keys_read,
            proposals_considered: self.proposals_considered,
            source_documents_read: self.source_documents_read,
            hash_verified: self.hash_verified,
            valid_catalogs: self.valid_catalogs,
            generated_exercises: self.generated_exercises,
            generated_complete: self.generated_complete,
            generated_replays: self.generated_replays,
            mutation_cases: self.mutation_cases,
            mutation_rejections: self.mutation_rejections,
            independent_exercise_evidence: self.independent_exercise_evidence,
            promotion_allowed: self.promotion_allowed,
            manifest_sha256_before: self.manifest_sha256_before.clone(),
            selection_manifest_matches: self.selection_manifest_matches,
            manifest_sha256_after: self.manifest_sha256_after.clone(),
            manifest_unchanged: self.manifest_unchanged,
            false_authorizations: self.false_authorizations,
            replay_verified: self.replay_verified,
            receipts: self.receipts.clone(),
            report_sha256: self.report_sha256.clone(),
        }
    }
}
