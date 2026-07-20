//! Run the current report-only formalizer against the seed curriculum.
//!
//! This is intentionally a baseline evaluator, not an answer route.  It
//! calls `assess_prompt` and `assess_direct_instantiation`, records field-level
//! extraction scores, and writes a deterministic JSON report.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, env, fs};
use the_machine::formalization::{
    assess_direct_instantiation, assess_prompt, infer_answer_form, score_formalization,
    AuthorizationDenialTrace, FieldScore, FormalizationCorpus, FormalizationGoldCase,
    FormalizationScore, OperationKind, OperationStatus, TargetCompletion, TargetFieldStatus,
    TargetStatus,
};
use the_machine::function_application::execute_function_application;
use the_machine::expression_evaluation::execute_expression_evaluation;
use the_machine::capabilities::{CapabilityRegistry, CapabilitySelection};

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
    target_constructed: usize,
    target_ambiguous: usize,
    target_incomplete: usize,
    binding_complete: usize,
    binding_missing: usize,
    binding_ambiguous: usize,
    binding_conflicting: usize,
    answer_form_present: usize,
    answer_form_correct: usize,
    operation_recognition_correct: usize,
    object_candidates_found: usize,
    object_inventory_nonempty: usize,
    object_candidates_expected: usize,
    object_type_metrics: BTreeMap<String, ObjectTypeMetrics>,
    function_shadow: FunctionShadowMetrics,
    expression_shadow: FunctionShadowMetrics,
    capability_reachability: BTreeMap<String, CapabilityReachabilityMetrics>,
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
    target_gaps: Vec<TargetGapRecord>,
    failures: BTreeMap<String, Vec<String>>,
}

#[derive(Debug, Serialize)]
struct TargetGapRecord {
    case_id: String,
    operation: String,
    status: String,
    missing_fields: Vec<String>,
    blocking_reasons: Vec<String>,
}

#[derive(Debug, Default, Serialize)]
struct ObjectTypeMetrics {
    found: usize,
    grounded: usize,
}

#[derive(Debug, Default, Serialize)]
struct FunctionShadowMetrics {
    candidates: usize,
    authorized: usize,
    executed: usize,
    replay_verified: usize,
    failures: BTreeMap<String, usize>,
}

#[derive(Debug, Default, Serialize)]
struct CapabilityReachabilityMetrics {
    considered: usize,
    eligible: usize,
    uniquely_selected: usize,
    ambiguous_selection: usize,
    no_selection: usize,
    rejections: BTreeMap<String, usize>,
}

#[derive(Debug, Default, Serialize)]
struct OperationMetrics {
    cases: usize,
    target_complete: usize,
    target_ambiguous: usize,
    target_incomplete: usize,
    binding_complete: usize,
    binding_missing: usize,
    binding_ambiguous: usize,
    binding_conflicting: usize,
    answer_form_present: usize,
    answer_form_correct: usize,
    operation_recognition_correct: usize,
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
            target_constructed: 0,
            target_ambiguous: 0,
            target_incomplete: 0,
            binding_complete: 0,
            binding_missing: 0,
            binding_ambiguous: 0,
            binding_conflicting: 0,
            answer_form_present: 0,
            answer_form_correct: 0,
            operation_recognition_correct: 0,
            object_candidates_found: 0,
            object_inventory_nonempty: 0,
            object_candidates_expected: 0,
            object_type_metrics: BTreeMap::new(),
            function_shadow: FunctionShadowMetrics::default(),
            expression_shadow: FunctionShadowMetrics::default(),
            capability_reachability: BTreeMap::new(),
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
            target_gaps: Vec::new(),
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
        let discovery = CapabilityRegistry::production().discover(&target_completion.target);
        for candidate in &discovery.candidates {
            let metrics = self
                .capability_reachability
                .entry(candidate.id.clone())
                .or_default();
            metrics.considered += 1;
            if candidate.eligible {
                metrics.eligible += 1;
            }
            for rejection in &candidate.rejections {
                *metrics.rejections.entry(format!("{rejection:?}")).or_default() += 1;
            }
        }
        match &discovery.selection {
            CapabilitySelection::Unique(id) => {
                if let Some(metrics) = self.capability_reachability.get_mut(id) {
                    metrics.uniquely_selected += 1;
                }
            }
            CapabilitySelection::Ambiguous(ids) => {
                for id in ids {
                    if let Some(metrics) = self.capability_reachability.get_mut(id) {
                        metrics.ambiguous_selection += 1;
                    }
                }
            }
            CapabilitySelection::None => {
                for metrics in self.capability_reachability.values_mut() {
                    metrics.no_selection += 1;
                }
            }
        }
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
        let inventory = &target_completion.target.object_inventory;
        self.object_candidates_found += inventory.objects.len();
        self.object_inventory_nonempty += usize::from(!inventory.objects.is_empty());
        self.object_candidates_expected +=
            score.definitions.expected + score.facts.expected + score.entities.expected;
        for object in &inventory.objects {
            let kind = format!("{:?}", object.kind).to_ascii_lowercase();
            let metrics = self.object_type_metrics.entry(kind).or_default();
            metrics.found += 1;
            if target_completion
                .target
                .subject_resolution
                .selected
                .as_ref()
                .map(|selected| selected.object_id == object.id)
                .unwrap_or(false)
            {
                metrics.grounded += 1;
            }
        }
        let function_candidate = target_completion.target.operation == OperationKind::Evaluate
            && target_completion
                .target
                .subject_resolution
                .selected
                .as_ref()
                .map(|subject| {
                    subject.object_type == the_machine::formalization::SubjectObjectType::Function
                })
                .unwrap_or(false);
        if function_candidate {
            self.function_shadow.candidates += 1;
            match execute_function_application(&target_completion.target) {
                Ok(receipt) => {
                    self.function_shadow.authorized += 1;
                    self.function_shadow.executed += 1;
                    self.function_shadow.replay_verified += usize::from(receipt.replay_verified);
                }
                Err(error) => {
                    *self
                        .function_shadow
                        .failures
                        .entry(format!("{error:?}"))
                        .or_default() += 1;
                }
            }
        }
        let expression_candidate = target_completion.target.operation == OperationKind::Evaluate
            && target_completion
                .target
                .subject_resolution
                .selected
                .as_ref()
                .map(|subject| {
                    subject.object_type
                        == the_machine::formalization::SubjectObjectType::Expression
                })
                .unwrap_or(false);
        if expression_candidate {
            self.expression_shadow.candidates += 1;
            match execute_expression_evaluation(&target_completion.target) {
                Ok(receipt) => {
                    self.expression_shadow.authorized += 1;
                    self.expression_shadow.executed += 1;
                    self.expression_shadow.replay_verified +=
                        usize::from(receipt.replay_verified);
                }
                Err(error) => {
                    *self
                        .expression_shadow
                        .failures
                        .entry(format!("{error:?}"))
                        .or_default() += 1;
                }
            }
        }
        match &target_completion.build_trace.binding_status {
            the_machine::formalization::BindingStatus::Complete => self.binding_complete += 1,
            the_machine::formalization::BindingStatus::Missing(_) => self.binding_missing += 1,
            the_machine::formalization::BindingStatus::Ambiguous(_) => self.binding_ambiguous += 1,
            the_machine::formalization::BindingStatus::Conflicting(_) => {
                self.binding_conflicting += 1
            }
        }
        match target_completion.build_trace.final_status {
            TargetStatus::Complete => self.target_constructed += 1,
            TargetStatus::Ambiguous(_) => self.target_ambiguous += 1,
            TargetStatus::Incomplete(_) => self.target_incomplete += 1,
        }
        let expected_form = expected_answer_form(gold_operation, &target_completion.target);
        let form_present = target_completion.target.answer_form.is_some();
        self.answer_form_present += usize::from(form_present);
        self.answer_form_correct += usize::from(
            form_present
                && expected_form.is_some()
                && target_completion.target.answer_form == expected_form,
        );
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
        let target_status = match &target_completion.build_trace.final_status {
            TargetStatus::Complete => "complete",
            TargetStatus::Ambiguous(_) => "ambiguous",
            TargetStatus::Incomplete(_) => "incomplete",
        };
        if target_status != "complete" {
            let mut missing_fields = target_completion.reasons.clone();
            match &target_completion.build_trace.binding_status {
                the_machine::formalization::BindingStatus::Missing(fields)
                | the_machine::formalization::BindingStatus::Ambiguous(fields)
                | the_machine::formalization::BindingStatus::Conflicting(fields) => {
                    missing_fields.extend(fields.iter().cloned());
                }
                the_machine::formalization::BindingStatus::Complete => {}
            }
            missing_fields.sort();
            missing_fields.dedup();
            self.target_gaps.push(TargetGapRecord {
                case_id: id.into(),
                operation: predicted_operation.clone(),
                status: target_status.into(),
                missing_fields,
                blocking_reasons: target_completion.reasons.clone(),
            });
        }
        let recognition_correct =
            operation_recognition_correct(gold_operation, &predicted_operation);
        self.operation_recognition_correct += usize::from(recognition_correct);
        *self
            .operation_confusion
            .entry(format!("{gold_operation}->{predicted_operation}"))
            .or_default() += 1;
        let operation_entry = self
            .operation_metrics
            .entry(predicted_operation.clone())
            .or_default();
        operation_entry.cases += 1;
        operation_entry.target_ambiguous += usize::from(matches!(
            target_completion.build_trace.final_status,
            TargetStatus::Ambiguous(_)
        ));
        operation_entry.target_incomplete += usize::from(matches!(
            target_completion.build_trace.final_status,
            TargetStatus::Incomplete(_)
        ));
        match &target_completion.build_trace.binding_status {
            the_machine::formalization::BindingStatus::Complete => {
                operation_entry.binding_complete += 1
            }
            the_machine::formalization::BindingStatus::Missing(_) => {
                operation_entry.binding_missing += 1
            }
            the_machine::formalization::BindingStatus::Ambiguous(_) => {
                operation_entry.binding_ambiguous += 1
            }
            the_machine::formalization::BindingStatus::Conflicting(_) => {
                operation_entry.binding_conflicting += 1
            }
        }
        operation_entry.answer_form_present += usize::from(form_present);
        operation_entry.answer_form_correct += usize::from(
            form_present
                && expected_form.is_some()
                && target_completion.target.answer_form == expected_form,
        );
        operation_entry.operation_recognition_correct += usize::from(recognition_correct);
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

fn gold_operation_kind(operation: &str) -> Option<OperationKind> {
    match operation {
        "evaluate" | "find_or_evaluate" => Some(OperationKind::Evaluate),
        "solve" => Some(OperationKind::Solve),
        "simplify" => Some(OperationKind::Simplify),
        "substitute" => Some(OperationKind::Substitute),
        "compare" => Some(OperationKind::Compare),
        "verify" => Some(OperationKind::Verify),
        "prove" => Some(OperationKind::Prove),
        "count" => Some(OperationKind::Count),
        _ => None,
    }
}

fn operation_recognition_correct(gold: &str, predicted: &str) -> bool {
    match gold_operation_kind(gold) {
        Some(operation) => {
            predicted == operation.label()
                || predicted == format!("unsupported:{}", operation.label())
        }
        None => predicted == "not_identified",
    }
}

fn expected_answer_form(
    gold_operation: &str,
    target: &the_machine::formalization::FormalizedTarget,
) -> Option<the_machine::formalization::AnswerForm> {
    let operation = gold_operation_kind(gold_operation)?;
    infer_answer_form(
        target
            .provenance
            .as_ref()
            .and_then(|provenance| provenance.operation_span.as_ref())
            .map(|span| span.source_fragment.as_str())
            .unwrap_or_default(),
        operation,
    )
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
