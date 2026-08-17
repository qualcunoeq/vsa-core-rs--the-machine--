//! Stage 160: source retrieval integrated with bounded epistemic reasoning.
//!
//! Retrieved claims are converted into evidence only when an exact query has
//! one object and the required independent lineages are present. Conflicting
//! and missing retrievals remain unresolved evidence states; they never become
//! a fact or a hypothesis promotion.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use the_machine::epistemic::{
    analyze, replay_beliefs, EpistemicInvestigation, EvidenceQuery, EvidenceRecord, Hypothesis,
    HypothesisId, Recommendation,
};
use the_machine::source_retrieval::{
    retrieve_claim, ClaimQuery, ClaimSource, RetrievalStatus, SourceClaim,
};

const REPORT_JSON: &str = "docs/stage160_source_epistemic_integration.json";
const REPORT_MD: &str = "docs/stage160_source_epistemic_integration.md";
const PARENT_REPORT: &str = "docs/stage159_source_reasoning_scale.json";

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
enum Expected {
    SupportedAndResolved,
    ConflictingAndUnresolved,
    MissingAndUnresolved,
}

#[derive(Debug, Serialize)]
struct Receipt {
    index: usize,
    expected: Expected,
    retrieval_status: RetrievalStatus,
    retrieval_exact: bool,
    source_replay_verified: bool,
    source_tamper_rejected: bool,
    independent_lineages: usize,
    evidence_ingested: bool,
    final_plausible: Vec<String>,
    expected_resolution: bool,
    epistemic_replay_verified: bool,
    epistemic_tamper_rejected: bool,
    provenance_preserved: bool,
    false_resolution: bool,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    corpus_schema: &'static str,
    parent_report_sha256: String,
    corpus_sha256: String,
    cases: usize,
    supported_resolutions: usize,
    conflicting_unresolved: usize,
    missing_unresolved: usize,
    exact_retrieval_decisions: usize,
    source_replay_verified: usize,
    source_tamper_rejected: usize,
    evidence_ingested: usize,
    epistemic_replay_verified: usize,
    epistemic_tamper_rejected: usize,
    provenance_preserved: usize,
    false_resolutions: usize,
    ambiguity_preserved: usize,
    recommendations_for_unresolved: usize,
    production_registry_mutations: usize,
    live_fact_mutations: usize,
    receipts: Vec<Receipt>,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn source(id: &str, lineage: &str) -> ClaimSource {
    ClaimSource {
        source_id: id.into(),
        title: format!("Stage 160 reference {id}"),
        locator: format!("https://example.invalid/stage160/{id}"),
        retrieved_utc: "2026-08-17".into(),
        lineage_id: lineage.into(),
    }
}

fn claim(id: &str, subject: &str, object: &str, source_id: &str, lineage: &str) -> SourceClaim {
    SourceClaim {
        claim_id: id.into(),
        subject: subject.into(),
        predicate: "state".into(),
        object: object.into(),
        domain: "bounded_investigation".into(),
        scope: "case_exact".into(),
        validity: "timestamp-one".into(),
        assumptions: vec!["claim is scoped to this investigation case".into()],
        source: source(source_id, lineage),
    }
}

fn query(subject: &str, index: usize) -> ClaimQuery {
    ClaimQuery {
        subject: subject.into(),
        predicate: "state".into(),
        domain: "bounded_investigation".into(),
        scope: "case_exact".into(),
        provenance: vec![format!("stage160-query:{index}")],
    }
}

fn investigation(evidence: Vec<EvidenceRecord>, index: usize) -> EpistemicInvestigation {
    let h1 = Hypothesis {
        id: HypothesisId("h1".into()),
        description: "state is present".into(),
        predictions: BTreeMap::from([(String::from("q1"), String::from("present"))]),
        causal_paths: BTreeMap::from([(String::from("q1"), vec![String::from("source_claim")])]),
    };
    let h2 = Hypothesis {
        id: HypothesisId("h2".into()),
        description: "state is absent".into(),
        predictions: BTreeMap::from([(String::from("q1"), String::from("absent"))]),
        causal_paths: BTreeMap::from([(String::from("q1"), vec![String::from("source_claim")])]),
    };
    EpistemicInvestigation {
        id: format!("stage160-case-{index}"),
        hypotheses: vec![h1, h2],
        queries: vec![EvidenceQuery {
            id: "q1".into(),
            description: "which state is reported by the source?".into(),
            cost: 1,
        }],
        evidence,
        ground_truth: None,
        expected_recommendation: Recommendation::Recommend {
            query_id: "q1".into(),
        },
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let parent = fs::read(PARENT_REPORT)?;
    let mut receipts = Vec::with_capacity(1_000);
    let mut supported_resolutions = 0;
    let mut conflicting_unresolved = 0;
    let mut missing_unresolved = 0;
    let mut exact_retrieval_decisions = 0;
    let mut source_replay_verified = 0;
    let mut source_tamper_rejected = 0;
    let mut evidence_ingested = 0;
    let mut epistemic_replay_verified = 0;
    let mut epistemic_tamper_rejected = 0;
    let mut provenance_preserved = 0;
    let mut false_resolutions = 0;
    let mut ambiguity_preserved = 0;
    let mut recommendations_for_unresolved = 0;

    for index in 0..1_000 {
        let subject = format!("entity_{}", index % 149);
        let (expected, corpus) = if index < 400 {
            (
                Expected::SupportedAndResolved,
                vec![
                    claim("primary", &subject, "present", "primary", "lineage-a"),
                    claim(
                        "independent",
                        &subject,
                        "present",
                        "independent",
                        "lineage-b",
                    ),
                    claim("copied", &subject, "present", "copy", "lineage-a"),
                ],
            )
        } else if index < 700 {
            (
                Expected::ConflictingAndUnresolved,
                vec![
                    claim("claim-a", &subject, "present", "source-a", "lineage-a"),
                    claim("claim-b", &subject, "absent", "source-b", "lineage-b"),
                ],
            )
        } else {
            (
                Expected::MissingAndUnresolved,
                vec![claim(
                    "wrong-case",
                    "other-entity",
                    "present",
                    "unrelated",
                    "lineage-other",
                )],
            )
        };
        let query = query(&subject, index);
        let result = retrieve_claim(&query, &corpus);
        let expected_status = match expected {
            Expected::SupportedAndResolved => RetrievalStatus::Supported,
            Expected::ConflictingAndUnresolved => RetrievalStatus::Conflicting,
            Expected::MissingAndUnresolved => RetrievalStatus::Missing,
        };
        let exact = result.status == expected_status;
        exact_retrieval_decisions += usize::from(exact);
        let mut tampered_source = result.clone();
        tampered_source.replay_hash.push('x');
        source_replay_verified += usize::from(result.replay_verified());
        source_tamper_rejected += usize::from(!tampered_source.replay_verified());
        let source_provenance = result
            .claims
            .iter()
            .all(|claim| !claim.source.source_id.is_empty() && !claim.source.lineage_id.is_empty());
        let evidence = if result.eligible_for_shadow_use() && result.has_independent_lineages(2) {
            evidence_ingested += 1;
            let claim = result.claims.first().expect("supported result has claim");
            vec![EvidenceRecord {
                id: format!("source-evidence-{index}"),
                query_id: "q1".into(),
                outcome: claim.object.clone(),
                timestamp: 1,
                valid_until: Some(1),
                source: claim.source.source_id.clone(),
                reliability: 100,
                confidence: 100,
                ancestry: result.independent_lineages.clone(),
                correlation_group: Some(result.independent_lineages[0].clone()),
                failure_mode: None,
                causal_path: vec!["source_claim".into(), "bounded_investigation".into()],
            }]
        } else {
            Vec::new()
        };
        let investigation = investigation(evidence, index);
        let analysis = analyze(&investigation);
        let replay = replay_beliefs(&investigation);
        let final_plausible: Vec<String> = replay
            .final_plausible
            .iter()
            .map(|id| id.0.clone())
            .collect();
        let resolved = final_plausible == vec!["h1".to_string()];
        let expected_resolution = expected == Expected::SupportedAndResolved;
        let false_resolution = resolved != expected_resolution;
        false_resolutions += usize::from(false_resolution);
        if expected_resolution {
            supported_resolutions += usize::from(resolved);
        } else {
            ambiguity_preserved += usize::from(final_plausible.len() == 2);
            recommendations_for_unresolved += usize::from(matches!(
                analysis.recommendation,
                Recommendation::Recommend { .. } | Recommendation::Ambiguous { .. }
            ));
            if expected == Expected::ConflictingAndUnresolved {
                conflicting_unresolved += usize::from(final_plausible.len() == 2);
            } else {
                missing_unresolved += usize::from(final_plausible.len() == 2);
            }
        }
        let mut tampered_epistemic = replay.clone();
        tampered_epistemic
            .final_plausible
            .push(HypothesisId("forged".into()));
        epistemic_replay_verified += usize::from(replay.replay_verified());
        epistemic_tamper_rejected += usize::from(!tampered_epistemic.replay_verified());
        provenance_preserved += usize::from(source_provenance && replay.replay_verified());
        receipts.push(Receipt {
            index,
            expected,
            retrieval_status: result.status,
            retrieval_exact: exact,
            source_replay_verified: result.replay_verified(),
            source_tamper_rejected: !tampered_source.replay_verified(),
            independent_lineages: result.independent_lineages.len(),
            evidence_ingested: !investigation.evidence.is_empty(),
            final_plausible,
            expected_resolution,
            epistemic_replay_verified: replay.replay_verified(),
            epistemic_tamper_rejected: !tampered_epistemic.replay_verified(),
            provenance_preserved: source_provenance && replay.replay_verified(),
            false_resolution,
        });
    }
    assert_eq!(exact_retrieval_decisions, 1_000);
    assert_eq!(source_replay_verified, 1_000);
    assert_eq!(source_tamper_rejected, 1_000);
    assert_eq!(evidence_ingested, 400);
    assert_eq!(supported_resolutions, 400);
    assert_eq!(conflicting_unresolved, 300);
    assert_eq!(missing_unresolved, 300);
    assert_eq!(ambiguity_preserved, 600);
    assert_eq!(epistemic_replay_verified, 1_000);
    assert_eq!(epistemic_tamper_rejected, 1_000);
    assert_eq!(provenance_preserved, 1_000);
    assert_eq!(false_resolutions, 0);
    let report = Report {
        schema: "stage160-source-epistemic-integration-v1",
        corpus_schema: "independent-source-claim-investigation-corpus-v1",
        parent_report_sha256: digest(&parent),
        corpus_sha256: digest(&receipts),
        cases: 1_000,
        supported_resolutions,
        conflicting_unresolved,
        missing_unresolved,
        exact_retrieval_decisions,
        source_replay_verified,
        source_tamper_rejected,
        evidence_ingested,
        epistemic_replay_verified,
        epistemic_tamper_rejected,
        provenance_preserved,
        false_resolutions,
        ambiguity_preserved,
        recommendations_for_unresolved,
        production_registry_mutations: 0,
        live_fact_mutations: 0,
        receipts,
    };
    fs::write(
        REPORT_JSON,
        format!("{}\n", serde_json::to_string_pretty(&report)?),
    )?;
    fs::write(
        REPORT_MD,
        format!(
            "# Stage 160 — source retrieval inside epistemic reasoning\n\nThis 1,000-case independent campaign converts retrieved claims into evidence only after unique-object and two-lineage checks. Conflicting and missing claims remain unresolved hypotheses.\n\n| Measure | Result |\n|---|---:|\n| Cases | 1,000 |\n| Resolved supported claims | 400/400 |\n| Conflicting unresolved | 300/300 |\n| Missing unresolved | 300/300 |\n| Exact retrieval / source replay | 1,000/1,000 |\n| Source tamper rejection | 1,000/1,000 |\n| Evidence ingested | 400 |\n| Epistemic replay / tamper | 1,000/1,000 |\n| Provenance preserved | 1,000/1,000 |\n| False resolutions | 0 |\n| Live mutations | 0 |\n\nParent provenance is hash-bound to Stage 159 in the JSON report. HLE was not read.\n"
        ),
    )?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
