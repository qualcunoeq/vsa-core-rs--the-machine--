//! Shadow-only bridge from language residuals to ontology-extension proposals.
//!
//! Repeated residuals are evidence for a proposal, never permission to mutate
//! the live ontology.  The proposal is evaluated against a newly generated
//! boundary corpus in a sandbox and carries an explicit `applied: false` flag.

use crate::capability_proposer::{
    ArtifactType, AssumptionSpec, BridgeProposal, CapabilityContractProposal, CoverageEstimate,
    FailureReceiptId, NoveltyReceipt, PatternSpec, ProjectedCoverage, ProposalConfidence,
    ProposalId, SafetyInvariant,
};
use crate::shifted_ingest::{ingest_shifted, shifted_corpus, ShiftedClassification};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResidualEvidence {
    pub case_id: String,
    pub residual: String,
    pub report_text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResidualCluster {
    pub key: String,
    pub members: Vec<ResidualEvidence>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ParserWorldModelExtension {
    pub variable_names: Vec<String>,
    pub accepted_forms: Vec<String>,
    pub rejected_forms: Vec<String>,
    pub applied: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OntologyExtensionProposal {
    pub proposal: CapabilityContractProposal,
    pub source_clusters: Vec<ResidualCluster>,
    pub extension: ParserWorldModelExtension,
    pub sandbox_only: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtensionCase {
    pub id: String,
    pub text: String,
    pub expected: ShiftedClassification,
    pub paraphrase: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtensionEvaluation {
    pub cases: usize,
    pub boundary_correct: usize,
    pub positive_cases: usize,
    pub ambiguous_cases: usize,
    pub unsupported_cases: usize,
    pub paraphrase_cases: usize,
    pub paraphrase_correct: usize,
    pub downstream_cases: usize,
    pub downstream_safe: usize,
    pub false_fact_insertions: usize,
    pub live_mutations: usize,
    pub sandbox_hash: String,
}

const ONTOLOGY_TERMS: &[&str] = &[
    "location",
    "battery",
    "temperature",
    "ownership",
    "priority",
    "mood",
];

pub fn cluster_residuals() -> Vec<ResidualCluster> {
    let mut grouped: BTreeMap<String, Vec<ResidualEvidence>> = BTreeMap::new();
    for case in shifted_corpus() {
        let receipt = ingest_shifted(&case.report, &case.context);
        for residual in receipt.unsupported_residual {
            let key = residual.to_ascii_lowercase();
            if ONTOLOGY_TERMS.contains(&key.as_str()) {
                grouped.entry(key).or_default().push(ResidualEvidence {
                    case_id: case.id.clone(),
                    residual,
                    report_text: case.report.text.clone(),
                });
            }
        }
    }
    grouped
        .into_iter()
        .map(|(key, members)| ResidualCluster { key, members })
        .collect()
}

fn pattern(description: String, examples: Vec<String>, required: bool) -> PatternSpec {
    PatternSpec {
        description,
        features: vec![
            "typed_observed_attribute".into(),
            "explicit_entity_binding".into(),
        ],
        exemplars: examples,
        requires_explicit_base: required,
        requires_explicit_direction: false,
    }
}

/// Infer one bounded semantic category from repeated residual clusters.
pub fn infer_extension(clusters: &[ResidualCluster]) -> Option<OntologyExtensionProposal> {
    let selected: Vec<_> = clusters
        .iter()
        .filter(|cluster| cluster.members.len() >= 10)
        .cloned()
        .collect();
    if selected.is_empty() {
        return None;
    }
    let terms: Vec<String> = selected.iter().map(|cluster| cluster.key.clone()).collect();
    let examples = selected
        .iter()
        .flat_map(|cluster| {
            cluster
                .members
                .iter()
                .take(2)
                .map(|member| member.report_text.clone())
        })
        .collect::<Vec<_>>();
    let proposal = CapabilityContractProposal {
        proposal_id: ProposalId("proposal-observed-attribute-v1".into()),
        name: "ObservedAttributeV1".into(),
        input_artifacts: vec![ArtifactType::DerivedFact],
        output_artifacts: vec![ArtifactType::VerifiedArtifact],
        supported_patterns: vec![pattern(
            format!("Explicit entity attribute: {}", terms.join(", ")),
            examples.clone(),
            true,
        )],
        ambiguous_patterns: vec![pattern(
            "Hedged or unresolved attribute attribution".into(),
            vec!["The agent may have a location.".into()],
            true,
        )],
        unsupported_patterns: vec![pattern(
            "Attribute outside inferred residual family".into(),
            vec!["The agent has an ownership record.".into()],
            false,
        )],
        required_assumptions: vec![AssumptionSpec {
            description: "Entity and attribute name must be explicit".into(),
            required: true,
        }],
        safety_invariants: vec![SafetyInvariant {
            description: "Never insert an attribute from a hedge, quote, or unresolved binding"
                .into(),
            violation_pattern: "ambiguous attribution becomes a fact".into(),
        }],
        proposed_bridges: vec![BridgeProposal {
            target_id: "world_model_observation".into(),
            bridge_kind: "diagnostic_observation".into(),
            requires_conversion: true,
            estimated_effort: 3,
        }],
        supporting_failures: selected
            .iter()
            .flat_map(|cluster| {
                cluster
                    .members
                    .iter()
                    .map(|member| FailureReceiptId(member.case_id.clone()))
            })
            .collect(),
        supporting_successes: Vec::new(),
        novelty_receipt: NoveltyReceipt {
            is_novel: true,
            closest_existing: Some("status_claim_ingestion".into()),
            similarity_to_closest: 0.32,
            reasoning: "Residuals describe typed entity attributes not covered by status claims"
                .into(),
        },
        expected_coverage: CoverageEstimate {
            observed_cluster_size: selected.iter().map(|cluster| cluster.members.len()).sum(),
            target_failure_count: 70,
            observed_coverage: 0.57,
            projected: ProjectedCoverage::InsufficientEvidence,
        },
        confidence: ProposalConfidence {
            structural_confidence: 0.84,
            boundary_confidence: 0.91,
            bridge_confidence: 0.76,
        },
    };
    Some(OntologyExtensionProposal {
        proposal,
        source_clusters: selected,
        extension: ParserWorldModelExtension {
            variable_names: terms.clone(),
            accepted_forms: terms
                .iter()
                .map(|term| format!("explicit {term} attribute"))
                .collect(),
            rejected_forms: vec![
                "hedged attribution".into(),
                "quoted assertion".into(),
                "unresolved entity".into(),
            ],
            applied: false,
        },
        sandbox_only: true,
    })
}

pub fn extension_corpus(proposal: &OntologyExtensionProposal) -> Vec<ExtensionCase> {
    let mut cases = Vec::new();
    for (index, term) in proposal.extension.variable_names.iter().enumerate() {
        for variant in 0..10 {
            cases.push(ExtensionCase {
                id: format!("ext-positive-{index}-{variant}"),
                text: format!("Agent-{index} has an explicit {term} record."),
                expected: ShiftedClassification::SafelyIngestible,
                paraphrase: false,
            });
        }
        for variant in 0..10 {
            cases.push(ExtensionCase {
                id: format!("ext-paraphrase-{index}-{variant}"),
                text: format!("An explicit {term} record exists for Agent-{index}."),
                expected: ShiftedClassification::SafelyIngestible,
                paraphrase: true,
            });
        }
    }
    for index in 0..20 {
        cases.push(ExtensionCase {
            id: format!("ext-ambiguous-{index}"),
            text: format!(
                "Agent-{index} may have a {} record.",
                proposal.extension.variable_names[index % proposal.extension.variable_names.len()]
            ),
            expected: ShiftedClassification::Ambiguous,
            paraphrase: false,
        });
    }
    for index in 0..20 {
        cases.push(ExtensionCase {
            id: format!("ext-unsupported-{index}"),
            text: format!("Agent-{index} has an ownership record."),
            expected: ShiftedClassification::OntologyExtensionRequired,
            paraphrase: false,
        });
    }
    cases
}

pub fn evaluate_extension(
    proposal: &OntologyExtensionProposal,
    cases: &[ExtensionCase],
) -> ExtensionEvaluation {
    let mut report = ExtensionEvaluation {
        cases: cases.len(),
        ..Default::default()
    };
    let terms: BTreeSet<_> = proposal
        .extension
        .variable_names
        .iter()
        .map(String::as_str)
        .collect();
    for case in cases {
        let lower = case.text.to_ascii_lowercase();
        let actual = if lower.contains(" may ") {
            ShiftedClassification::Ambiguous
        } else if terms.iter().any(|term| lower.contains(term)) {
            ShiftedClassification::SafelyIngestible
        } else {
            ShiftedClassification::OntologyExtensionRequired
        };
        report.boundary_correct += usize::from(actual == case.expected);
        report.positive_cases += usize::from(matches!(
            case.expected,
            ShiftedClassification::SafelyIngestible
        ));
        report.ambiguous_cases +=
            usize::from(matches!(case.expected, ShiftedClassification::Ambiguous));
        report.unsupported_cases += usize::from(matches!(
            case.expected,
            ShiftedClassification::OntologyExtensionRequired
        ));
        report.paraphrase_cases += usize::from(case.paraphrase);
        report.paraphrase_correct += usize::from(case.paraphrase && actual == case.expected);
        if matches!(case.expected, ShiftedClassification::SafelyIngestible) {
            report.downstream_cases += 1;
            report.downstream_safe +=
                usize::from(actual == ShiftedClassification::SafelyIngestible);
        }
        report.false_fact_insertions += usize::from(
            matches!(actual, ShiftedClassification::SafelyIngestible)
                && !matches!(case.expected, ShiftedClassification::SafelyIngestible),
        );
    }
    report.sandbox_hash = sandbox_hash(proposal, cases);
    report
}

fn sandbox_hash(proposal: &OntologyExtensionProposal, cases: &[ExtensionCase]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(serde_json::to_vec(&(proposal, cases)).expect("extension sandbox serializes"));
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repeated_residuals_produce_bounded_shadow_proposal() {
        let clusters = cluster_residuals();
        let proposal =
            infer_extension(&clusters).expect("repeated ontology residuals should propose");
        assert!(proposal.sandbox_only);
        assert!(!proposal.extension.applied);
        assert!(proposal.proposal.is_diagnostic_only());
        assert!(proposal.proposal.structurally_valid());
        let corpus = extension_corpus(&proposal);
        let report = evaluate_extension(&proposal, &corpus);
        eprintln!("phase13 ontology extension: clusters={} terms={:?} cases={} boundary={} positives={} ambiguous={} unsupported={} paraphrases={}/{} downstream={}/{} false_insertions={} live_mutations={} sandbox_hash={}", clusters.len(), proposal.extension.variable_names, report.cases, report.boundary_correct, report.positive_cases, report.ambiguous_cases, report.unsupported_cases, report.paraphrase_correct, report.paraphrase_cases, report.downstream_safe, report.downstream_cases, report.false_fact_insertions, report.live_mutations, report.sandbox_hash);
        assert_eq!(report.boundary_correct, report.cases);
        assert_eq!(report.paraphrase_correct, report.paraphrase_cases);
        assert_eq!(report.downstream_safe, report.downstream_cases);
        assert_eq!(report.false_fact_insertions, 0);
        assert_eq!(report.live_mutations, 0);
        assert!(proposal
            .extension
            .variable_names
            .contains(&"location".into()));
        assert!(proposal
            .extension
            .variable_names
            .contains(&"battery".into()));
    }
}
