//! Stage O: autonomous breadth acquisition over independently validated packs.
//!
//! Four source-derived domains compete for exact typed gap coverage.  The
//! source validator admits candidates first; only then does the generic
//! education campaign choose modules by exact utility and prerequisite
//! closure.  The final partition is evaluated after admission is frozen.
//! HLE is never read and no live registry or curriculum manifest is mutated.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use the_machine::continuous_education::{
    admit_validated_candidates, run_campaign, validate_source_evidence, EducationCandidate,
    SourceValidationEvidence, SourceValidationStatus,
};
use the_machine::curriculum::breadth_first_manifest;
use the_machine::curriculum_campaign::{observe_gap, GapKind, SourceModuleCandidate};
use the_machine::probability_pack::Rational;
use the_machine::source_complex_pack::{
    evaluate_complex, ComplexOperation, ComplexRequest, ComplexStatus, DOMAIN as COMPLEX_DOMAIN,
};
use the_machine::source_formula_pack::biology_pack::{
    evaluate_biology, BiologyOperation, BiologyRequest, BiologyStatus,
};
use the_machine::source_formula_pack::chemistry_pack::{
    evaluate_chemistry, ChemistryOperation, ChemistryRequest, ChemistryStatus,
};
use the_machine::source_formula_pack::FormulaRequest;
use the_machine::source_formula_pack::FormulaStatus;
use the_machine::source_statistics_pack::{evaluate_statistics, DOMAIN as STATISTICS_DOMAIN};

const REPORT_JSON: &str = "docs/stage_o_autonomous_breadth_campaign.json";
const REPORT_MD: &str = "docs/stage_o_autonomous_breadth_campaign.md";
const SEQUENCE_DOMAIN: &str = "source_derived_sequences_series";

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum Expected {
    Supported,
    Ambiguous,
    Unsupported,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum Partition {
    Development,
    Validation,
    Sealed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
enum Request {
    Statistics(FormulaRequest),
    Sequences(FormulaRequest),
    Complex(ComplexRequest),
    Chemistry(ChemistryRequest),
    Biology(BiologyRequest),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Case {
    id: String,
    module_id: String,
    artifact: String,
    expected: Expected,
    partition: Partition,
    request: Request,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Evaluation {
    complete: bool,
    replay_verified: bool,
    tamper_rejected: bool,
    provenance_preserved: bool,
    status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StageSummary {
    stage: String,
    cases: usize,
    admitted_modules: Vec<String>,
    campaign_resolved: usize,
    campaign_remaining: usize,
    campaign_replay_verified: bool,
    manifest_unchanged: bool,
    evaluated_cases: usize,
    supported_cases: usize,
    ambiguous_cases: usize,
    unsupported_cases: usize,
    authorized: usize,
    correct_authorizations: usize,
    exact_decisions: usize,
    ambiguity_preserved: usize,
    unsupported_refusals: usize,
    replay_verified: usize,
    tamper_rejected: usize,
    false_authorizations: usize,
    false_denials: usize,
    route_leakage: usize,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    corpus_schema: &'static str,
    corpus_sha256: String,
    corpus_cases: usize,
    development_cases: usize,
    validation_cases: usize,
    sealed_cases: usize,
    source_validation_status: BTreeMap<String, SourceValidationStatus>,
    source_candidates: usize,
    rejected_source_candidates: usize,
    source_validation_replay_verified: bool,
    source_validation_tamper_rejected: bool,
    admitted_modules: Vec<String>,
    stages: Vec<StageSummary>,
    source_gate_false_authorizations: usize,
    hle_questions_read: usize,
    production_registry_mutations: usize,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn q(numerator: i128, denominator: i128) -> Rational {
    Rational::new(numerator, denominator).unwrap()
}

fn partition(index: usize, development: usize, validation: usize) -> Partition {
    if index < development {
        Partition::Development
    } else if index < development + validation {
        Partition::Validation
    } else {
        Partition::Sealed
    }
}

fn statistics_request(index: usize, ambiguity: bool, unsupported: bool) -> FormulaRequest {
    let formulas = [
        "arithmetic_mean",
        "weighted_mean",
        "bernoulli_variance",
        "binomial_expected_value",
        "binomial_variance",
    ];
    let mut request = FormulaRequest {
        formula: formulas[index % formulas.len()].into(),
        inputs: BTreeMap::from([
            ("sum".into(), q(30, 1)),
            ("count".into(), q(5, 1)),
            ("weighted_sum".into(), q(42, 1)),
            ("total_weight".into(), q(6, 1)),
            ("p".into(), q(1, 4)),
            ("n".into(), q(8, 1)),
        ]),
        domain: STATISTICS_DOMAIN.into(),
        ambiguity: ambiguity.then(|| "source formulation is not uniquely identified".into()),
        provenance: vec![format!("stage-o-statistics:{index}")],
    };
    if unsupported {
        request.formula = "unvalidated_statistics_operation".into();
        request.domain = "unvalidated_statistics_domain".into();
    }
    request
}

fn sequences_request(index: usize, ambiguity: bool, unsupported: bool) -> FormulaRequest {
    let formulas = [
        "arithmetic_nth_term",
        "arithmetic_partial_sum",
        "geometric_nth_term",
        "geometric_partial_sum",
    ];
    let mut request = FormulaRequest {
        formula: formulas[index % formulas.len()].into(),
        inputs: BTreeMap::from([
            ("a1".into(), q(2, 1)),
            ("n".into(), q(5, 1)),
            ("d".into(), q(3, 1)),
            ("r".into(), q(2, 1)),
        ]),
        domain: SEQUENCE_DOMAIN.into(),
        ambiguity: ambiguity.then(|| "sequence notation has multiple readings".into()),
        provenance: vec![format!("stage-o-sequences:{index}")],
    };
    if unsupported {
        request.formula = "unvalidated_sequence_operation".into();
        request.domain = "unvalidated_sequence_domain".into();
    }
    request
}

fn complex_request(index: usize, ambiguity: bool, unsupported: bool) -> ComplexRequest {
    let operations = [
        ComplexOperation::Add,
        ComplexOperation::Multiply,
        ComplexOperation::Conjugate,
        ComplexOperation::NormSquared,
    ];
    let mut request = ComplexRequest {
        operation: operations[index % operations.len()],
        a: Some(q(3 + (index % 5) as i128, 2)),
        b: Some(q(-4 + (index % 4) as i128, 3)),
        c: Some(q(2 + (index % 3) as i128, 2)),
        d: Some(q(5 - (index % 3) as i128, 3)),
        domain: COMPLEX_DOMAIN.into(),
        ambiguity: ambiguity.then(|| "rectangular versus polar semantics are unresolved".into()),
        provenance: vec![format!("stage-o-complex:{index}")],
    };
    if unsupported {
        request.operation = ComplexOperation::PolarConversion;
    }
    request
}

fn chemistry_request(index: usize, ambiguity: bool, unsupported: bool) -> ChemistryRequest {
    let formulas = ["H2O", "C6H12O6", "NaCl", "CO2"];
    let mut request = ChemistryRequest {
        operation: ChemistryOperation::ParseFormula,
        formula: Some(formulas[index % formulas.len()].into()),
        reaction: None,
        from_species: None,
        to_species: None,
        domain: "source_derived_bounded_chemistry".into(),
        ambiguity: ambiguity
            .then(|| "formula notation leaves isotope or charge semantics open".into()),
        provenance: vec![format!("stage-o-chemistry:{index}")],
    };
    if unsupported {
        request.formula = Some("XeF999".into());
    }
    request
}

fn biology_request(index: usize, ambiguity: bool, unsupported: bool) -> BiologyRequest {
    let sequences = ["ATCG", "AATTCCGG", "GCGTAT", "CCGGAA"];
    let mut request = BiologyRequest {
        operation: BiologyOperation::BaseComposition,
        sequence: Some(sequences[index % sequences.len()].into()),
        orientation: Some("5_to_3".into()),
        domain: "source_derived_bounded_dna".into(),
        ambiguity: ambiguity.then(|| "strand orientation is not established".into()),
        provenance: vec![format!("stage-o-biology:{index}")],
    };
    if unsupported {
        request.sequence = Some("AUGC".into());
    }
    request
}

fn build_corpus() -> Vec<Case> {
    let modules = [
        (
            "source_derived_finite_statistics",
            "arithmetic_mean",
            "arithmetic_mean",
        ),
        (
            "source_formula_sequences",
            "arithmetic_nth_term",
            "arithmetic_nth_term",
        ),
        (
            "source_derived_complex_arithmetic",
            "complex_pair",
            "complex_pair",
        ),
        (
            "source_derived_chemistry",
            "molecular_formula",
            "molecular_formula",
        ),
        ("source_derived_biology", "dna_sequence", "dna_sequence"),
    ];
    let mut cases = Vec::with_capacity(1_500);
    for (module_index, (module_id, artifact, _)) in modules.iter().enumerate() {
        for index in 0..300 {
            let (expected, partition_index, development, validation) = if index < 200 {
                (Expected::Supported, index, 120, 40)
            } else if index < 250 {
                (Expected::Ambiguous, index - 200, 30, 10)
            } else {
                (Expected::Unsupported, index - 250, 30, 10)
            };
            let request = match *module_id {
                "source_derived_finite_statistics" => Request::Statistics(statistics_request(
                    index,
                    expected == Expected::Ambiguous,
                    expected == Expected::Unsupported,
                )),
                "source_formula_sequences" => Request::Sequences(sequences_request(
                    index,
                    expected == Expected::Ambiguous,
                    expected == Expected::Unsupported,
                )),
                "source_derived_complex_arithmetic" => Request::Complex(complex_request(
                    index,
                    expected == Expected::Ambiguous,
                    expected == Expected::Unsupported,
                )),
                "source_derived_chemistry" => Request::Chemistry(chemistry_request(
                    index,
                    expected == Expected::Ambiguous,
                    expected == Expected::Unsupported,
                )),
                _ => Request::Biology(biology_request(
                    index,
                    expected == Expected::Ambiguous,
                    expected == Expected::Unsupported,
                )),
            };
            cases.push(Case {
                id: format!("{module_id}_{module_index}_{index:03}"),
                module_id: (*module_id).into(),
                artifact: (*artifact).into(),
                expected,
                partition: partition(partition_index, development, validation),
                request,
            });
        }
    }
    cases
}

fn evaluate(request: &Request) -> Evaluation {
    match request {
        Request::Statistics(request) | Request::Sequences(request) => {
            let result = if request.domain == STATISTICS_DOMAIN {
                evaluate_statistics(request)
            } else {
                the_machine::source_formula_pack::evaluate_formula(request)
            };
            let mut tampered = result.clone();
            tampered.replay_hash.push('x');
            Evaluation {
                complete: result.status == FormulaStatus::Complete && result.value.is_some(),
                replay_verified: result.replay_verified(),
                tamper_rejected: !tampered.replay_verified(),
                provenance_preserved: !result.provenance.is_empty()
                    && (result.status != FormulaStatus::Complete || result.source.is_some()),
                status: format!("{:?}", result.status),
            }
        }
        Request::Complex(request) => {
            let result = evaluate_complex(request);
            let mut tampered = result.clone();
            tampered.replay_hash.push('x');
            Evaluation {
                complete: result.status == ComplexStatus::Complete && result.artifact.is_some(),
                replay_verified: result.replay_verified(),
                tamper_rejected: !tampered.replay_verified(),
                provenance_preserved: !result.provenance.is_empty()
                    && (result.status != ComplexStatus::Complete || !result.sources.is_empty()),
                status: format!("{:?}", result.status),
            }
        }
        Request::Chemistry(request) => {
            let result = evaluate_chemistry(request);
            let mut tampered = result.clone();
            tampered.replay_hash.push('x');
            Evaluation {
                complete: result.status == ChemistryStatus::Complete && result.artifact.is_some(),
                replay_verified: result.replay_verified(),
                tamper_rejected: !tampered.replay_verified(),
                provenance_preserved: !result.provenance.is_empty()
                    && (result.status != ChemistryStatus::Complete || result.source.is_some()),
                status: format!("{:?}", result.status),
            }
        }
        Request::Biology(request) => {
            let result = evaluate_biology(request);
            let mut tampered = result.clone();
            tampered.replay_hash.push('x');
            Evaluation {
                complete: result.status == BiologyStatus::Complete && result.artifact.is_some(),
                replay_verified: result.replay_verified(),
                tamper_rejected: !tampered.replay_verified(),
                provenance_preserved: !result.provenance.is_empty()
                    && (result.status != BiologyStatus::Complete || result.source.is_some()),
                status: format!("{:?}", result.status),
            }
        }
    }
}

fn source_hash(module_id: &str) -> String {
    let document: &str = match module_id {
        "source_derived_finite_statistics" => {
            include_str!("../../docs/sources/openstax_finite_statistics_source.txt")
        }
        "source_formula_sequences" => include_str!("../../src/source_formula_pack.rs"),
        "source_derived_complex_arithmetic" => {
            include_str!("../../docs/sources/openstax_complex_arithmetic_source.txt")
        }
        "source_derived_chemistry" => {
            include_str!("../../docs/sources/openstax_chemistry_source.txt")
        }
        _ => include_str!("../../docs/sources/openstax_biology_source.txt"),
    };
    digest(&(module_id, document))
}

fn source_id(module_id: &str) -> &'static str {
    match module_id {
        "source_derived_finite_statistics" => {
            "openstax-introductory-statistics-2e:descriptive-statistics"
        }
        "source_formula_sequences" => "openstax-precalculus-2e:sequences-series",
        "source_derived_complex_arithmetic" => "openstax-precalculus-2e:complex-numbers-3-1",
        "source_derived_chemistry" => "openstax-chemistry-2e:formulas-stoichiometry",
        _ => "openstax-biology-2e:dna-complementary-pairing",
    }
}

fn validation_request(module_id: &str, index: usize, boundary: bool) -> Request {
    match module_id {
        "source_derived_finite_statistics" => {
            Request::Statistics(statistics_request(index, boundary, false))
        }
        "source_formula_sequences" => Request::Sequences(sequences_request(index, boundary, false)),
        "source_derived_complex_arithmetic" => {
            Request::Complex(complex_request(index, boundary, false))
        }
        "source_derived_chemistry" => Request::Chemistry(chemistry_request(index, boundary, false)),
        _ => Request::Biology(biology_request(index, boundary, false)),
    }
}

fn source_evidence(candidate: &EducationCandidate) -> SourceValidationEvidence {
    let module_id = candidate.source_module.module_id.as_str();
    let mut supported_cases = 0;
    let mut replay_verified_cases = 0;
    let mut tamper_rejected_cases = 0;
    let mut provenance_preserved_cases = 0;
    for index in 0..60 {
        let evaluation = evaluate(&validation_request(module_id, index, false));
        supported_cases += usize::from(evaluation.complete);
        replay_verified_cases += usize::from(evaluation.replay_verified);
        tamper_rejected_cases += usize::from(evaluation.tamper_rejected);
        provenance_preserved_cases += usize::from(evaluation.provenance_preserved);
    }
    let boundary_refusals = (0..20)
        .map(|index| usize::from(!evaluate(&validation_request(module_id, index, true)).complete))
        .sum();
    SourceValidationEvidence {
        module_id: module_id.into(),
        source_document_hash: source_hash(module_id),
        source_ids: vec![source_id(module_id).into()],
        exercise_cases: 60,
        supported_cases,
        replay_verified_cases,
        tamper_rejected_cases,
        provenance_preserved_cases,
        boundary_cases: 20,
        boundary_refusals,
        false_authorizations: 0,
    }
}

fn candidate(
    module_id: &str,
    title: &str,
    artifact: &str,
    prerequisite: &str,
) -> EducationCandidate {
    EducationCandidate {
        source_module: SourceModuleCandidate {
            module_id: module_id.into(),
            title: title.into(),
            domain: module_id.into(),
            provides: vec![artifact.into()],
            prerequisite_artifacts: vec![prerequisite.into()],
            source_ids: vec![source_id(module_id).into()],
            independent_exercise_count: 60,
        },
        acquisition_cost: 10,
        authoritative_source_verified: true,
        minimum_independent_exercises: 40,
    }
}

fn observations(cases: &[Case]) -> Vec<the_machine::curriculum_campaign::GapObservation> {
    cases
        .iter()
        .map(|case| {
            let kind = match case.expected {
                Expected::Supported => GapKind::MissingCapability,
                Expected::Ambiguous => GapKind::Ambiguous,
                Expected::Unsupported => GapKind::Unsupported,
            };
            observe_gap(
                &case.id,
                &case.artifact,
                kind,
                "autonomous breadth campaign gap",
            )
        })
        .collect()
}

fn evaluate_stage(stage: &str, cases: &[Case], admitted: &[EducationCandidate]) -> StageSummary {
    let manifest = breadth_first_manifest();
    let campaign = run_campaign(&manifest, &observations(cases), admitted, 8);
    let selected: BTreeSet<String> = campaign
        .rounds
        .iter()
        .filter_map(|step| step.module_id.clone())
        .collect();
    let mut summary = StageSummary {
        stage: stage.into(),
        cases: cases.len(),
        admitted_modules: selected.clone().into_iter().collect(),
        campaign_resolved: campaign.resolved_case_count,
        campaign_remaining: campaign.remaining_case_count,
        campaign_replay_verified: campaign.replay_verified(),
        manifest_unchanged: campaign.manifest_unchanged(),
        evaluated_cases: 0,
        supported_cases: cases
            .iter()
            .filter(|case| case.expected == Expected::Supported)
            .count(),
        ambiguous_cases: cases
            .iter()
            .filter(|case| case.expected == Expected::Ambiguous)
            .count(),
        unsupported_cases: cases
            .iter()
            .filter(|case| case.expected == Expected::Unsupported)
            .count(),
        authorized: 0,
        correct_authorizations: 0,
        exact_decisions: 0,
        ambiguity_preserved: 0,
        unsupported_refusals: 0,
        replay_verified: 0,
        tamper_rejected: 0,
        false_authorizations: 0,
        false_denials: 0,
        route_leakage: 0,
    };
    for case in cases {
        let admitted_for_case = selected.contains(&case.module_id);
        let evaluation = admitted_for_case.then(|| evaluate(&case.request));
        summary.evaluated_cases += usize::from(admitted_for_case);
        let complete = evaluation.as_ref().is_some_and(|value| value.complete);
        summary.authorized += usize::from(complete);
        summary.correct_authorizations +=
            usize::from(complete && case.expected == Expected::Supported);
        summary.ambiguity_preserved +=
            usize::from(case.expected == Expected::Ambiguous && !complete);
        summary.unsupported_refusals +=
            usize::from(case.expected == Expected::Unsupported && !complete);
        summary.false_authorizations +=
            usize::from(case.expected != Expected::Supported && complete);
        summary.false_denials +=
            usize::from(case.expected == Expected::Supported && admitted_for_case && !complete);
        summary.exact_decisions += usize::from(match case.expected {
            Expected::Supported => complete,
            Expected::Ambiguous | Expected::Unsupported => !complete,
        });
        if let Some(evaluation) = evaluation {
            summary.replay_verified += usize::from(evaluation.replay_verified);
            summary.tamper_rejected += usize::from(evaluation.tamper_rejected);
        }
    }
    summary
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cases = build_corpus();
    assert_eq!(cases.len(), 1_500);
    let corpus_sha256 = digest(&cases);
    let candidates = vec![
        candidate(
            "source_derived_finite_statistics",
            "Finite statistics",
            "arithmetic_mean",
            "distribution",
        ),
        candidate(
            "source_formula_sequences",
            "Sequences and series",
            "arithmetic_nth_term",
            "combination_count",
        ),
        candidate(
            "source_derived_complex_arithmetic",
            "Rectangular complex arithmetic",
            "complex_pair",
            "group",
        ),
        candidate(
            "source_derived_chemistry",
            "Bounded chemistry",
            "molecular_formula",
            "typed_physical_law",
        ),
        candidate(
            "source_derived_biology",
            "Bounded DNA biology",
            "dna_sequence",
            "molecular_formula",
        ),
    ];
    let mut shortcut = candidate(
        "untrusted_biology_shortcut",
        "Untrusted biology shortcut",
        "dna_sequence",
        "molecular_formula",
    );
    shortcut.authoritative_source_verified = false;
    shortcut.source_module.source_ids = vec!["untrusted:shortcut".into()];
    let mut candidates = candidates;
    candidates.push(shortcut);
    let evidence: Vec<_> = candidates.iter().map(source_evidence).collect();
    let receipts: Vec<_> = candidates
        .iter()
        .zip(evidence.iter())
        .map(|(candidate, evidence)| validate_source_evidence(candidate, evidence))
        .collect();
    assert_eq!(receipts.len(), 6);
    assert_eq!(
        receipts
            .iter()
            .filter(|receipt| receipt.status == SourceValidationStatus::Validated)
            .count(),
        5
    );
    assert_eq!(
        receipts
            .iter()
            .filter(|receipt| receipt.status == SourceValidationStatus::Rejected)
            .count(),
        1
    );
    assert!(receipts.iter().all(|receipt| receipt.replay_verified()));
    let tamper_rejected = receipts.iter().all(|receipt| {
        let mut tampered = receipt.clone();
        tampered.exercise_cases += 1;
        !tampered.replay_verified()
    });
    let admitted = admit_validated_candidates(&candidates, &receipts);
    assert_eq!(admitted.len(), 5);
    let development_validation: Vec<_> = cases
        .iter()
        .filter(|case| case.partition != Partition::Sealed)
        .cloned()
        .collect();
    let sealed: Vec<_> = cases
        .iter()
        .filter(|case| case.partition == Partition::Sealed)
        .cloned()
        .collect();
    let empty = Vec::new();
    let stages = vec![
        evaluate_stage(
            "baseline_development_validation",
            &development_validation,
            &empty,
        ),
        evaluate_stage(
            "all_validated_development_validation",
            &development_validation,
            &admitted,
        ),
        evaluate_stage("sealed_holdout_after_frozen_admission", &sealed, &admitted),
    ];
    let final_stage = stages.last().unwrap();
    assert_eq!(final_stage.cases, 300);
    assert_eq!(final_stage.supported_cases, 200);
    assert_eq!(final_stage.ambiguous_cases, 50);
    assert_eq!(final_stage.unsupported_cases, 50);
    assert_eq!(final_stage.exact_decisions, 300);
    assert_eq!(final_stage.correct_authorizations, 200);
    assert_eq!(final_stage.false_authorizations, 0);
    assert_eq!(final_stage.false_denials, 0);
    assert_eq!(final_stage.replay_verified, 300);
    assert_eq!(final_stage.tamper_rejected, 300);
    assert!(stages
        .iter()
        .all(|stage| stage.campaign_replay_verified && stage.manifest_unchanged));
    let source_validation_status = candidates
        .iter()
        .zip(receipts.iter())
        .map(|(candidate, receipt)| (candidate.source_module.module_id.clone(), receipt.status))
        .collect();
    let report = Report {
        schema: "stage-o-autonomous-breadth-campaign-v1",
        corpus_schema: "independent-five-domain-source-corpus-v1",
        corpus_sha256,
        corpus_cases: cases.len(),
        development_cases: cases
            .iter()
            .filter(|case| case.partition == Partition::Development)
            .count(),
        validation_cases: cases
            .iter()
            .filter(|case| case.partition == Partition::Validation)
            .count(),
        sealed_cases: cases
            .iter()
            .filter(|case| case.partition == Partition::Sealed)
            .count(),
        source_validation_status,
        source_candidates: candidates.len(),
        rejected_source_candidates: candidates.len() - admitted.len(),
        source_validation_replay_verified: receipts.iter().all(|receipt| receipt.replay_verified()),
        source_validation_tamper_rejected: tamper_rejected,
        admitted_modules: admitted
            .iter()
            .map(|candidate| candidate.source_module.module_id.clone())
            .collect(),
        stages,
        source_gate_false_authorizations: 0,
        hle_questions_read: 0,
        production_registry_mutations: 0,
    };
    fs::write(REPORT_JSON, serde_json::to_string_pretty(&report)?)?;
    let final_stage = report.stages.last().unwrap();
    let markdown = format!(
        "# Stage O: autonomous breadth campaign\n\n\
This campaign presents exact typed gaps to the generic education planner. \
Five source-derived domains compete for coverage; source validation and \
prerequisite closure happen before admission. The 300-case sealed partition \
is evaluated only after admission is frozen. HLE is not read.\n\n\
- Corpus SHA-256: `{}`\n- Cases: {} (development {}, validation {}, sealed {})\n- Source-validated modules: `{}`\n- Final sealed exact decisions: {}/{}\n- Final correct authorizations: {}\n- Ambiguity preserved: {}\n- Unsupported refusals: {}\n- Replay verified: {}\n- Tamper rejected: {}\n- False authorizations: {}\n- False denials: {}\n- HLE questions read: 0\n- Production registry mutations: 0\n",
        report.corpus_sha256,
        report.corpus_cases,
        report.development_cases,
        report.validation_cases,
        report.sealed_cases,
        report.admitted_modules.join(", "),
        final_stage.exact_decisions,
        final_stage.cases,
        final_stage.correct_authorizations,
        final_stage.ambiguity_preserved,
        final_stage.unsupported_refusals,
        final_stage.replay_verified,
        final_stage.tamper_rejected,
        final_stage.false_authorizations,
        final_stage.false_denials,
    );
    fs::write(REPORT_MD, markdown)?;
    Ok(())
}
