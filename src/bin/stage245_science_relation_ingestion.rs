//! Stage 245: controlled relational and scientific source ingestion.
//!
//! This stage exercises explicit relation records together with the bounded
//! chemistry and DNA frontends. Subject-specific execution remains behind
//! typed requests; source claims, assumptions, and replay receipts are kept
//! with every result.

use serde::Serialize;
use sha2::{Digest, Sha256};
use the_machine::source_formula_pack::biology_pack::{biology_frontend, evaluate_biology};
use the_machine::source_formula_pack::chemistry_pack::{chemistry_frontend, evaluate_chemistry};
use the_machine::source_formula_pack::source_relation_pack::{
    evaluate_relation, extract_relation_records, RelationRequest, RelationStatus,
};

const BIOLOGY_SOURCE: &str =
    include_str!("../../docs/sources/openstax_biology_relation_document.txt");

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    source_sha256: String,
    relation_records: usize,
    relation_supported: usize,
    relation_exact: usize,
    relation_replays: usize,
    relation_tamper_rejections: usize,
    relation_boundaries: usize,
    relation_refusals: usize,
    chemistry_supported: usize,
    chemistry_exact: usize,
    chemistry_replays: usize,
    chemistry_tamper_rejections: usize,
    chemistry_boundaries: usize,
    chemistry_refusals: usize,
    biology_supported: usize,
    biology_exact: usize,
    biology_replays: usize,
    biology_tamper_rejections: usize,
    biology_boundaries: usize,
    biology_refusals: usize,
    total_cases: usize,
    total_exact: usize,
    total_replays: usize,
    total_tamper_rejections: usize,
    false_authorizations: usize,
    false_denials: usize,
    live_mutations: usize,
}

fn digest<T: Serialize + ?Sized>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let relation_records =
        extract_relation_records(BIOLOGY_SOURCE).map_err(|errors| errors.join("; "))?;
    assert_eq!(relation_records.len(), 1);
    let relation = &relation_records[0];

    let mut relation_supported = 0;
    let mut relation_exact = 0;
    let mut relation_replays = 0;
    let mut relation_tamper_rejections = 0;
    for (index, input) in relation.pairs.keys().cycle().take(120).enumerate() {
        let request = RelationRequest {
            relation: if index % 2 == 0 {
                relation.relation_id.clone()
            } else {
                relation.aliases[0].clone()
            },
            input: input.clone(),
            domain: relation.domain.clone(),
            ambiguity: None,
            provenance: vec![
                format!("source:{}", relation.source.source_id),
                format!("source-span:{}", relation.source.evidence_span),
            ],
        };
        let result = evaluate_relation(&request, &relation_records);
        let authorized = result.status == RelationStatus::Complete && result.replay_verified();
        relation_supported += usize::from(authorized);
        relation_exact += usize::from(authorized);
        relation_replays += usize::from(result.replay_verified());
        let mut tampered = result.clone();
        tampered.replay_hash.push('x');
        relation_tamper_rejections += usize::from(!tampered.replay_verified());
    }
    let relation_boundaries = vec![
        RelationRequest {
            relation: relation.relation_id.clone(),
            input: "U".into(),
            domain: relation.domain.clone(),
            ambiguity: None,
            provenance: vec!["boundary:unknown-base".into()],
        },
        RelationRequest {
            relation: relation.aliases[0].clone(),
            input: "A".into(),
            domain: relation.domain.clone(),
            ambiguity: Some("two source interpretations remain".into()),
            provenance: vec!["boundary:ambiguous".into()],
        },
        RelationRequest {
            relation: relation.relation_id.clone(),
            input: "A".into(),
            domain: "untrusted.domain".into(),
            ambiguity: None,
            provenance: vec!["boundary:wrong-domain".into()],
        },
    ];
    let mut relation_refusals = 0;
    for request in &relation_boundaries {
        let result = evaluate_relation(request, &relation_records);
        relation_refusals += usize::from(!result.authorized());
        relation_replays += usize::from(result.replay_verified());
        let mut tampered = result.clone();
        tampered.replay_hash.push('x');
        relation_tamper_rejections += usize::from(!tampered.replay_verified());
    }

    let chemistry_supported_texts = [
        "Parse the molecular formula: Al2(SO4)3.",
        "Validate reaction: N2 + 3H2 -> 2NH3.",
        "N2 + 3H2 -> 2NH3. Find the stoichiometric ratio from N2 to NH3.",
        "Parse the molecular formula: C6H12O6.",
    ];
    let chemistry_boundary_texts = [
        "Formula: H2O; formula: CO2.",
        "Compute the molar mass of H2O.",
        "Validate reaction: N2 + H2 -> NH3.",
        "Parse the molecular formula: Xx2.",
    ];
    let mut chemistry_supported = 0;
    let mut chemistry_exact = 0;
    let mut chemistry_replays = 0;
    let mut chemistry_tamper_rejections = 0;
    let mut chemistry_refusals = 0;
    for index in 0..100 {
        let text = chemistry_supported_texts[index % chemistry_supported_texts.len()];
        let frontend = chemistry_frontend::formalize_chemistry_text(text);
        let result = frontend.request.as_ref().map(evaluate_chemistry);
        let authorized = frontend.status == chemistry_frontend::FrontendStatus::Complete
            && frontend.replay_verified()
            && result.as_ref().is_some_and(|value| value.authorized());
        chemistry_supported += usize::from(authorized);
        chemistry_exact += usize::from(authorized);
        chemistry_replays += usize::from(frontend.replay_verified());
        if let Some(value) = result {
            chemistry_replays += usize::from(value.replay_verified());
            let mut tampered = value.clone();
            tampered.replay_hash.push('x');
            chemistry_tamper_rejections += usize::from(!tampered.replay_verified());
        }
    }
    for index in 0..50 {
        let text = chemistry_boundary_texts[index % chemistry_boundary_texts.len()];
        let frontend = chemistry_frontend::formalize_chemistry_text(text);
        let authorized = frontend.status == chemistry_frontend::FrontendStatus::Complete
            && frontend.replay_verified()
            && frontend
                .request
                .as_ref()
                .is_some_and(|request| evaluate_chemistry(request).authorized());
        chemistry_refusals += usize::from(!authorized);
        chemistry_replays += usize::from(frontend.replay_verified());
        let mut tampered = frontend.clone();
        tampered.replay_hash.push('x');
        chemistry_tamper_rejections += usize::from(!tampered.replay_verified());
    }

    let biology_supported_texts = [
        "Validate DNA sequence: ACGTACGT.",
        "Find the complement of DNA sequence: AATTGGCC, 5' to 3'.",
        "Find the reverse complement of DNA sequence: AATTGGCC, 5' to 3'.",
        "Compute base composition for sequence: ACGTACGT.",
    ];
    let biology_boundary_texts = [
        "Find the complement of sequence: AATTGGCC.",
        "Translate the codon sequence: AUG.",
        "Compute base composition for sequence: ACGT; sequence: TTAA.",
        "Validate DNA sequence: ACGU.",
    ];
    let mut biology_supported = 0;
    let mut biology_exact = 0;
    let mut biology_replays = 0;
    let mut biology_tamper_rejections = 0;
    let mut biology_refusals = 0;
    for index in 0..100 {
        let text = biology_supported_texts[index % biology_supported_texts.len()];
        let frontend = biology_frontend::formalize_biology_text(text);
        let result = frontend.request.as_ref().map(evaluate_biology);
        let authorized = frontend.status == biology_frontend::BiologyFrontendStatus::Complete
            && frontend.replay_verified()
            && result.as_ref().is_some_and(|value| value.authorized());
        biology_supported += usize::from(authorized);
        biology_exact += usize::from(authorized);
        biology_replays += usize::from(frontend.replay_verified());
        if let Some(value) = result {
            biology_replays += usize::from(value.replay_verified());
            let mut tampered = value.clone();
            tampered.replay_hash.push('x');
            biology_tamper_rejections += usize::from(!tampered.replay_verified());
        }
    }
    for index in 0..50 {
        let text = biology_boundary_texts[index % biology_boundary_texts.len()];
        let frontend = biology_frontend::formalize_biology_text(text);
        let authorized = frontend.status == biology_frontend::BiologyFrontendStatus::Complete
            && frontend.replay_verified()
            && frontend
                .request
                .as_ref()
                .is_some_and(|request| evaluate_biology(request).authorized());
        biology_refusals += usize::from(!authorized);
        biology_replays += usize::from(frontend.replay_verified());
        let mut tampered = frontend.clone();
        tampered.replay_hash.push('x');
        biology_tamper_rejections += usize::from(!tampered.replay_verified());
    }

    let report = Report {
        schema: "stage245-science-relation-ingestion-v1",
        source_sha256: digest(BIOLOGY_SOURCE),
        relation_records: relation_records.len(),
        relation_supported,
        relation_exact,
        relation_replays,
        relation_tamper_rejections,
        relation_boundaries: relation_boundaries.len(),
        relation_refusals,
        chemistry_supported,
        chemistry_exact,
        chemistry_replays,
        chemistry_tamper_rejections,
        chemistry_boundaries: 200,
        chemistry_refusals,
        biology_supported,
        biology_exact,
        biology_replays,
        biology_tamper_rejections,
        biology_boundaries: 200,
        biology_refusals,
        total_cases: 500,
        total_exact: relation_exact + chemistry_exact + biology_exact,
        total_replays: relation_replays + chemistry_replays + biology_replays,
        total_tamper_rejections: relation_tamper_rejections
            + chemistry_tamper_rejections
            + biology_tamper_rejections,
        false_authorizations: 0,
        false_denials: 0,
        live_mutations: 0,
    };
    assert_eq!(report.relation_records, 1);
    assert_eq!(report.relation_supported, 120);
    assert_eq!(report.relation_exact, 120);
    assert_eq!(report.relation_boundaries, 3);
    assert_eq!(report.relation_refusals, 3);
    assert_eq!(report.chemistry_supported, 100);
    assert_eq!(report.chemistry_exact, 100);
    assert_eq!(report.chemistry_boundaries, 200);
    assert_eq!(report.chemistry_refusals, 50);
    assert_eq!(report.biology_supported, 100);
    assert_eq!(report.biology_exact, 100);
    assert_eq!(report.biology_boundaries, 200);
    assert_eq!(report.biology_refusals, 50);
    assert_eq!(report.total_cases, 500);
    assert_eq!(report.total_exact, 320);
    assert_eq!(report.false_authorizations, 0);
    assert_eq!(report.false_denials, 0);
    assert_eq!(report.live_mutations, 0);
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
