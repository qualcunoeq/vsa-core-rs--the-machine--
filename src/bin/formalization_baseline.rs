//! Run the current report-only formalizer against the seed curriculum.
//!
//! This is intentionally a baseline evaluator, not an answer route.  It
//! calls `assess_prompt` and `assess_direct_instantiation`, records field-level
//! extraction scores, and writes a deterministic JSON report.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, env, fs};
use the_machine::formalization::{
    assess_direct_instantiation, assess_prompt, score_formalization, AuthorizationDenialTrace,
    FieldScore, FormalizationCorpus, FormalizationGoldCase, FormalizationScore, OperationStatus,
    TargetCompletion, TargetFieldStatus,
};

#[derive(Debug, Serialize)]
struct Aggregate {
    cases: usize,
    exact_target: usize,
    structural_target: usize,
    target_kind_matches: usize,
    target_subject_overlap: usize,
    target_semantic_equivalent: usize,
    target_operation_detected: usize,
    target_subject_complete: usize,
    target_variable_complete: usize,
    target_arguments_complete: usize,
    target_domain_complete: usize,
    target_requested_form_complete: usize,
    target_provenance_complete: usize,
    target_complete: usize,
    target_operation_supported: usize,
    target_verifier_available: usize,
    target_incomplete_reasons: BTreeMap<String, usize>,
    operation_confusion: BTreeMap<String, usize>,
    operation_metrics: BTreeMap<String, OperationMetrics>,
    authorization_correct: usize,
    definitions: Counts,
    facts: Counts,
    entities: Counts,
    assumptions: Counts,
    constraints: Counts,
    obligations: Counts,
    invented_definitions: usize,
    invented_facts: usize,
    invented_entities: usize,
    invented_assumptions: usize,
    invented_constraints: usize,
    invented_obligations: usize,
    false_authorizations: usize,
    false_denials: usize,
    denial_funnel: BTreeMap<String, usize>,
    all_denial_blockers: BTreeMap<String, usize>,
    denial_cases: Vec<AuthorizationDenialTrace>,
    failures: BTreeMap<String, Vec<String>>,
}

#[derive(Debug, Default, Serialize)]
struct OperationMetrics {
    cases: usize,
    target_complete: usize,
    operation_supported: usize,
    verifier_available: usize,
    authorized: usize,
}

#[derive(Debug, Default, Serialize)]
struct Counts {
    matched: usize,
    expected: usize,
    predicted: usize,
    precision: f64,
    recall: f64,
}

impl Counts {
    fn add(&mut self, score: FieldScore) {
        self.matched += score.matched;
        self.expected += score.expected;
        self.predicted += score.predicted;
        self.precision = if self.predicted == 0 {
            if self.expected == 0 {
                1.0
            } else {
                0.0
            }
        } else {
            self.matched as f64 / self.predicted as f64
        };
        self.recall = if self.expected == 0 {
            if self.predicted == 0 {
                1.0
            } else {
                0.0
            }
        } else {
            self.matched as f64 / self.expected as f64
        };
    }
}

impl Aggregate {
    fn new() -> Self {
        Self {
            cases: 0,
            exact_target: 0,
            structural_target: 0,
            target_kind_matches: 0,
            target_subject_overlap: 0,
            target_semantic_equivalent: 0,
            target_operation_detected: 0,
            target_subject_complete: 0,
            target_variable_complete: 0,
            target_arguments_complete: 0,
            target_domain_complete: 0,
            target_requested_form_complete: 0,
            target_provenance_complete: 0,
            target_complete: 0,
            target_operation_supported: 0,
            target_verifier_available: 0,
            target_incomplete_reasons: BTreeMap::new(),
            operation_confusion: BTreeMap::new(),
            operation_metrics: BTreeMap::new(),
            authorization_correct: 0,
            definitions: Counts::default(),
            facts: Counts::default(),
            entities: Counts::default(),
            assumptions: Counts::default(),
            constraints: Counts::default(),
            obligations: Counts::default(),
            invented_definitions: 0,
            invented_facts: 0,
            invented_entities: 0,
            invented_assumptions: 0,
            invented_constraints: 0,
            invented_obligations: 0,
            false_authorizations: 0,
            false_denials: 0,
            denial_funnel: BTreeMap::new(),
            all_denial_blockers: BTreeMap::new(),
            denial_cases: Vec::new(),
            failures: BTreeMap::new(),
        }
    }

    fn add(
        &mut self,
        id: &str,
        score: &FormalizationScore,
        should_authorize: bool,
        authorized: bool,
        denial: Option<AuthorizationDenialTrace>,
        target_completion: &TargetCompletion,
        gold_operation: &str,
    ) {
        self.cases += 1;
        self.exact_target += usize::from(score.target_exact);
        self.structural_target += usize::from(score.target_structural);
        self.target_kind_matches += usize::from(score.target_comparison.kind_matches);
        self.target_subject_overlap += usize::from(score.target_comparison.subject_overlap);
        self.target_semantic_equivalent +=
            usize::from(score.target_comparison.semantically_equivalent);
        let completeness = &target_completion.target.completeness;
        self.target_operation_detected +=
            usize::from(completeness.operation_kind == TargetFieldStatus::Complete);
        self.target_subject_complete +=
            usize::from(completeness.subject == TargetFieldStatus::Complete);
        self.target_variable_complete +=
            usize::from(completeness.target_variable == TargetFieldStatus::Complete);
        self.target_arguments_complete +=
            usize::from(completeness.arguments != TargetFieldStatus::Missing);
        self.target_domain_complete +=
            usize::from(completeness.domain != TargetFieldStatus::Missing);
        self.target_requested_form_complete +=
            usize::from(completeness.requested_form != TargetFieldStatus::Missing);
        self.target_provenance_complete +=
            usize::from(completeness.provenance == TargetFieldStatus::Complete);
        self.target_complete += usize::from(target_completion.complete);
        self.target_operation_supported += usize::from(target_completion.operation_supported);
        self.target_verifier_available += usize::from(target_completion.verifier_available);
        for reason in &target_completion.reasons {
            *self
                .target_incomplete_reasons
                .entry(reason.clone())
                .or_default() += 1;
        }
        let predicted_operation = match &target_completion.target.operation_status {
            OperationStatus::Recognized(operation) => operation.label().to_string(),
            OperationStatus::Unsupported(name) => format!("unsupported:{name}"),
            OperationStatus::Ambiguous(_) => "ambiguous".into(),
            OperationStatus::NotIdentified => "not_identified".into(),
        };
        *self
            .operation_confusion
            .entry(format!("{gold_operation}->{predicted_operation}"))
            .or_default() += 1;
        let operation_entry = self
            .operation_metrics
            .entry(predicted_operation.clone())
            .or_default();
        operation_entry.cases += 1;
        operation_entry.target_complete += usize::from(matches!(
            target_completion.build_trace.final_status,
            the_machine::formalization::TargetStatus::Complete
        ));
        operation_entry.operation_supported += usize::from(target_completion.operation_supported);
        operation_entry.verifier_available += usize::from(target_completion.verifier_available);
        operation_entry.authorized += usize::from(authorized);
        self.authorization_correct += usize::from(score.authorization_correct);
        self.definitions.add(score.definitions);
        self.facts.add(score.facts);
        self.entities.add(score.entities);
        self.assumptions.add(score.assumptions);
        self.constraints.add(score.constraints);
        self.obligations.add(score.obligations);
        self.invented_definitions += score
            .definitions
            .predicted
            .saturating_sub(score.definitions.matched);
        self.invented_facts += score.facts.predicted.saturating_sub(score.facts.matched);
        self.invented_entities += score
            .entities
            .predicted
            .saturating_sub(score.entities.matched);
        self.invented_assumptions += score
            .assumptions
            .predicted
            .saturating_sub(score.assumptions.matched);
        self.invented_constraints += score
            .constraints
            .predicted
            .saturating_sub(score.constraints.matched);
        self.invented_obligations += score
            .obligations
            .predicted
            .saturating_sub(score.obligations.matched);
        if authorized && !should_authorize {
            self.false_authorizations += 1;
            self.failures
                .entry("false_authorization".into())
                .or_default()
                .push(id.into());
        }
        if !authorized && should_authorize {
            self.false_denials += 1;
            if let Some(denial) = denial {
                *self
                    .denial_funnel
                    .entry(denial.first_blocker.clone())
                    .or_default() += 1;
                for blocker in &denial.all_blockers {
                    *self.all_denial_blockers.entry(blocker.clone()).or_default() += 1;
                }
                self.denial_cases.push(denial);
            }
            self.failures
                .entry("false_denial".into())
                .or_default()
                .push(id.into());
        }
        if !score.target_structural {
            self.failures
                .entry("target".into())
                .or_default()
                .push(id.into());
        }
        for (name, field) in [
            ("definitions", score.definitions),
            ("facts", score.facts),
            ("entities", score.entities),
            ("assumptions", score.assumptions),
            ("constraints", score.constraints),
            ("obligations", score.obligations),
        ] {
            if field.recall < 1.0 {
                self.failures
                    .entry(format!("{name}_recall"))
                    .or_default()
                    .push(id.into());
            }
        }
    }
}

#[derive(Debug, Serialize)]
struct BaselineReport {
    corpus_schema_version: u32,
    corpus_sha256: String,
    split_rule: String,
    total: Aggregate,
    development: Aggregate,
    holdout: Aggregate,
    by_tier: BTreeMap<String, Aggregate>,
    by_transformation: BTreeMap<String, Aggregate>,
}

fn holdout(id: &str) -> bool {
    id.rsplit('-')
        .next()
        .and_then(|suffix| suffix.parse::<u32>().ok())
        .map(|number| number >= 15)
        .unwrap_or(false)
}

fn gold_operation(statement: &str) -> &'static str {
    let lower = statement.to_ascii_lowercase();
    if lower.contains("prove") || lower.contains("show") {
        "prove"
    } else if lower.contains("how many") || lower.contains("count") {
        "count"
    } else if lower.contains("simplify") {
        "simplify"
    } else if lower.contains("substitute") || lower.contains("plug in") {
        "substitute"
    } else if lower.contains("solve") {
        "solve"
    } else if lower.contains("evaluate") || lower.contains("compute") || lower.contains("calculate")
    {
        "evaluate"
    } else if lower.contains("compare") {
        "compare"
    } else if lower.contains("verify") || lower.contains("check") {
        "verify"
    } else if lower.contains("find") || lower.contains("what is") {
        "find_or_evaluate"
    } else {
        "unknown"
    }
}

fn evaluate_case(
    case: &FormalizationGoldCase,
) -> (
    FormalizationScore,
    bool,
    AuthorizationDenialTrace,
    TargetCompletion,
) {
    let trace = assess_prompt(&case.id, &case.prompt, "Math", false);
    let assessment = assess_direct_instantiation(&trace);
    let authorized = assessment.authorization_safe();
    (
        score_formalization(case, &trace, authorized),
        authorized,
        assessment.denial_trace(case.authorization_expected),
        trace.target_completion.clone(),
    )
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = env::args()
        .nth(1)
        .ok_or("usage: formalization_baseline <corpus.json> [report.json]")?;
    let text = fs::read_to_string(&path)?;
    let corpus: FormalizationCorpus = serde_json::from_str(&text)?;
    if !corpus.is_valid() {
        return Err("formalization corpus validation failed".into());
    }
    let digest = format!("{:x}", Sha256::digest(text.as_bytes()));
    let mut report = BaselineReport {
        corpus_schema_version: corpus.schema_version,
        corpus_sha256: digest,
        split_rule: "case suffix 01-14=development, 15-20=holdout within each tier".into(),
        total: Aggregate::new(),
        development: Aggregate::new(),
        holdout: Aggregate::new(),
        by_tier: BTreeMap::new(),
        by_transformation: BTreeMap::new(),
    };
    for case in &corpus.cases {
        let (score, authorized, denial, target_completion) = evaluate_case(case);
        let expected_operation = gold_operation(&case.target.statement);
        report.total.add(
            &case.id,
            &score,
            case.authorization_expected,
            authorized,
            Some(denial.clone()),
            &target_completion,
            expected_operation,
        );
        let split = if holdout(&case.id) {
            &mut report.holdout
        } else {
            &mut report.development
        };
        split.add(
            &case.id,
            &score,
            case.authorization_expected,
            authorized,
            Some(denial.clone()),
            &target_completion,
            expected_operation,
        );
        report
            .by_tier
            .entry(case.tier.label().into())
            .or_insert_with(Aggregate::new)
            .add(
                &case.id,
                &score,
                case.authorization_expected,
                authorized,
                Some(denial.clone()),
                &target_completion,
                expected_operation,
            );
        report
            .by_transformation
            .entry(case.transformation.label().into())
            .or_insert_with(Aggregate::new)
            .add(
                &case.id,
                &score,
                case.authorization_expected,
                authorized,
                Some(denial),
                &target_completion,
                expected_operation,
            );
    }
    let output = serde_json::to_string_pretty(&report)?;
    println!("{output}");
    if let Some(report_path) = env::args().nth(2) {
        fs::write(report_path, output)?;
    }
    Ok(())
}
