//! Stage V: independent source-derived cross-domain composition.
//!
//! This is a new corpus rather than a mutation of the earlier Stage-B report.
//! Five routes compose independently validated source-derived domains.  Each
//! route is exercised with supported, ambiguous, and refused inputs; no route
//! is selected from a question label and no production registry is changed.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use the_machine::linear_algebra_pack::{
    evaluate_linear_algebra, LinearAlgebraOperation, LinearAlgebraRequest, LinearAlgebraStatus,
};
use the_machine::metric_topology_bridge::{
    metric_result_to_topology, BridgeStatus as MetricTopologyBridgeStatus,
};
use the_machine::probability_pack::Rational;
use the_machine::source_complex_pack::{
    complex_linear_bridge::{
        bridge_complex_to_real_matrix, BridgeStatus as ComplexBridgeStatus,
        ComplexMatrixBridgeRequest,
    },
    evaluate_complex, ComplexOperation, ComplexRequest, ComplexStatus,
};
use the_machine::source_formula_pack::biology_pack::biology_probability_bridge::{
    bridge_base_composition, BiologyProbabilityBridgeStatus,
};
use the_machine::source_formula_pack::biology_pack::{
    evaluate_biology, BiologyOperation, BiologyRequest,
};
use the_machine::source_formula_pack::chemistry_pack::chemistry_linear_bridge::{
    bridge_chemistry_to_linear, ChemistryLinearBridgeStatus,
};
use the_machine::source_formula_pack::chemistry_pack::{
    evaluate_chemistry, ChemistryOperation, ChemistryRequest,
};
use the_machine::source_formula_pack::{FormulaRequest, FormulaStatus};
use the_machine::source_metric_pack::{
    evaluate_metric, extract_metric_definitions, DistanceEntry, MetricOperation, MetricRequest,
    MetricStatus,
};
use the_machine::source_regression_pack::evaluate_regression;
use the_machine::source_statistics_pack::evaluate_statistics;
use the_machine::source_topology_pack::{evaluate_topology, extract_topology_definitions};

const REPORT_JSON: &str = "docs/stage_v_source_composition_1000.json";
const REPORT_MD: &str = "docs/stage_v_source_composition_1000.md";

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum Expected {
    Supported,
    Ambiguous,
    Refused,
}

#[derive(Debug, Clone, Serialize)]
struct Receipt {
    id: String,
    route: String,
    expected: Expected,
    authorized: bool,
    exact: bool,
    intermediate_entries: usize,
    replay_verified: bool,
    tamper_rejected: bool,
    failure_gate: Option<String>,
    false_authorization: bool,
    false_denial: bool,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    source: &'static str,
    corpus_sha256: String,
    cases: usize,
    supported: usize,
    ambiguous: usize,
    refused: usize,
    exact_decisions: usize,
    supported_routes: usize,
    replay_verified: usize,
    emitted_intermediate_entries: usize,
    tamper_rejections: usize,
    failure_localized: usize,
    false_authorizations: usize,
    false_denials: usize,
    route_counts: BTreeMap<String, usize>,
    hle_questions_read: usize,
    production_registry_mutations: usize,
    receipts: Vec<Receipt>,
}

#[derive(Debug, Clone)]
struct Outcome {
    authorized: bool,
    exact: bool,
    intermediate_entries: usize,
    replay_verified: bool,
    tamper_rejected: bool,
    failure_gate: Option<String>,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn q(n: i128, d: i128) -> Rational {
    Rational::new(n, d).expect("valid rational")
}

fn tamper<T: Clone + HasReplayHash>(value: &T) -> bool {
    let mut copy = value.clone();
    copy.append_replay_tamper();
    !copy.replay_verified()
}

trait HasReplayHash {
    fn append_replay_tamper(&mut self);
    fn replay_verified(&self) -> bool;
}

impl HasReplayHash for the_machine::source_formula_pack::FormulaResult {
    fn append_replay_tamper(&mut self) {
        self.replay_hash.push('x');
    }
    fn replay_verified(&self) -> bool {
        self.replay_verified()
    }
}

impl HasReplayHash for the_machine::source_formula_pack::biology_pack::BiologyResult {
    fn append_replay_tamper(&mut self) {
        self.replay_hash.push('x');
    }
    fn replay_verified(&self) -> bool {
        self.replay_verified()
    }
}

impl HasReplayHash for the_machine::source_formula_pack::chemistry_pack::ChemistryResult {
    fn append_replay_tamper(&mut self) {
        self.replay_hash.push('x');
    }
    fn replay_verified(&self) -> bool {
        self.replay_verified()
    }
}

impl HasReplayHash for the_machine::source_formula_pack::chemistry_pack::chemistry_linear_bridge::ChemistryLinearBridgeResult {
    fn append_replay_tamper(&mut self) { self.replay_hash.push('x'); }
    fn replay_verified(&self) -> bool { self.replay_verified() }
}

impl HasReplayHash for the_machine::source_complex_pack::ComplexResult {
    fn append_replay_tamper(&mut self) {
        self.replay_hash.push('x');
    }
    fn replay_verified(&self) -> bool {
        self.replay_verified()
    }
}

impl HasReplayHash
    for the_machine::source_complex_pack::complex_linear_bridge::ComplexMatrixBridgeResult
{
    fn append_replay_tamper(&mut self) {
        self.replay_hash.push('x');
    }
    fn replay_verified(&self) -> bool {
        self.replay_verified()
    }
}

impl HasReplayHash for the_machine::source_formula_pack::biology_pack::biology_probability_bridge::BiologyProbabilityBridgeResult {
    fn append_replay_tamper(&mut self) { self.replay_hash.push('x'); }
    fn replay_verified(&self) -> bool { self.replay_verified() }
}

impl HasReplayHash for the_machine::probability_pack::ProbabilityResult {
    fn append_replay_tamper(&mut self) {
        self.replay_hash.push('x');
    }
    fn replay_verified(&self) -> bool {
        self.replay_verified()
    }
}

impl HasReplayHash for the_machine::linear_algebra_pack::LinearAlgebraResult {
    fn append_replay_tamper(&mut self) {
        self.replay_hash.push('x');
    }
    fn replay_verified(&self) -> bool {
        self.replay_verified()
    }
}

impl HasReplayHash for the_machine::source_metric_pack::MetricResult {
    fn append_replay_tamper(&mut self) {
        self.replay_hash.push('x');
    }
    fn replay_verified(&self) -> bool {
        self.replay_verified()
    }
}

impl HasReplayHash for the_machine::metric_topology_bridge::MetricTopologyBridgeResult {
    fn append_replay_tamper(&mut self) {
        self.replay_hash.push('x');
    }
    fn replay_verified(&self) -> bool {
        self.replay_verified()
    }
}

impl HasReplayHash for the_machine::source_topology_pack::TopologyResult {
    fn append_replay_tamper(&mut self) {
        self.replay_hash.push('x');
    }
    fn replay_verified(&self) -> bool {
        self.replay_verified()
    }
}

fn regression_statistics(mode: Expected, index: usize) -> Outcome {
    let count = q((5 + index % 3) as i128, 1);
    let x_mean = q((2 + index % 4) as i128, 1);
    let slope_value = q((3 + index % 5) as i128, 1);
    let y_mean = slope_value.mul(&x_mean).expect("exact mean product");
    let x_variance = q((4 + index % 3) as i128, 1);
    let covariance = slope_value
        .mul(&x_variance)
        .expect("exact covariance product");
    let sum = y_mean.mul(&count).expect("exact sum product");
    let mut stats_request = FormulaRequest {
        formula: "arithmetic_mean".into(),
        inputs: BTreeMap::from([("sum".into(), sum), ("count".into(), count)]),
        domain: "source_derived_finite_statistics".into(),
        ambiguity: (mode == Expected::Ambiguous).then(|| "mean role is unresolved".into()),
        provenance: vec!["stage-v-regression-statistics".into()],
    };
    if mode == Expected::Refused {
        stats_request.domain = "continuous_statistics".into();
    }
    let stats = evaluate_statistics(&stats_request);
    let mean = stats.value.clone().unwrap_or_else(|| q(6, 1));
    let mut slope_request = FormulaRequest {
        formula: "regression_slope".into(),
        inputs: BTreeMap::from([
            ("covariance_sum".into(), covariance),
            ("x_variance_sum".into(), x_variance),
        ]),
        domain: "source_derived_finite_regression".into(),
        ambiguity: (mode == Expected::Ambiguous).then(|| "slope target is unresolved".into()),
        provenance: stats.provenance.clone(),
    };
    if mode == Expected::Refused {
        slope_request
            .inputs
            .insert("x_variance_sum".into(), q(0, 1));
    }
    let slope = evaluate_regression(&slope_request);
    let intercept = evaluate_regression(&FormulaRequest {
        formula: "regression_intercept".into(),
        inputs: BTreeMap::from([
            ("y_mean".into(), mean),
            (
                "slope".into(),
                slope.value.clone().unwrap_or_else(|| q(3, 1)),
            ),
            ("x_mean".into(), x_mean),
        ]),
        domain: "source_derived_finite_regression".into(),
        ambiguity: (mode == Expected::Ambiguous).then(|| "intercept target is unresolved".into()),
        provenance: slope.provenance.clone(),
    });
    let replay = stats.replay_verified() && slope.replay_verified() && intercept.replay_verified();
    let tampered = tamper(&stats) && tamper(&slope) && tamper(&intercept);
    let authorized = mode == Expected::Supported
        && stats.status == FormulaStatus::Complete
        && slope.status == FormulaStatus::Complete
        && intercept.status == FormulaStatus::Complete
        && intercept.value == Some(q(0, 1))
        && replay;
    let exact = match mode {
        Expected::Supported => authorized,
        Expected::Ambiguous => {
            stats.status == FormulaStatus::Ambiguous && slope.status == FormulaStatus::Ambiguous
        }
        Expected::Refused => slope.status == FormulaStatus::Inconsistent,
    };
    Outcome {
        authorized,
        exact,
        intermediate_entries: 3,
        replay_verified: replay,
        tamper_rejected: tampered,
        failure_gate: (!authorized).then(|| match mode {
            Expected::Supported => "regression_statistics_handoff".into(),
            Expected::Ambiguous => "regression_target_ambiguity".into(),
            Expected::Refused => "regression_domain_or_variation_boundary".into(),
        }),
    }
}

fn chemistry_linear(mode: Expected, index: usize) -> Outcome {
    let formula = match index % 3 {
        0 => "H2O",
        1 => "CO2",
        _ => "NH3",
    };
    let chemistry = evaluate_chemistry(&ChemistryRequest {
        operation: ChemistryOperation::ParseFormula,
        formula: Some(formula.into()),
        reaction: None,
        from_species: None,
        to_species: None,
        domain: "source_derived_bounded_chemistry".into(),
        ambiguity: (mode == Expected::Ambiguous).then(|| "formula target is unresolved".into()),
        provenance: vec!["stage-v-chemistry-linear".into()],
    });
    let bridge = bridge_chemistry_to_linear(&chemistry);
    let linear = bridge.artifact.as_ref().map(|vector| {
        evaluate_linear_algebra(&LinearAlgebraRequest {
            operation: LinearAlgebraOperation::VectorConstruction,
            matrix: None,
            vector_a: Some(vector.values.clone()),
            vector_b: None,
            domain: if mode == Expected::Refused {
                "probability_vector"
            } else {
                "finite_exact_integer"
            }
            .into(),
            requested_output: format!("element_count_vector:{}", vector.semantic_kind),
            provenance: bridge.provenance.clone(),
        })
    });
    let replay = chemistry.replay_verified()
        && bridge.replay_verified()
        && linear
            .as_ref()
            .is_none_or(|result| result.replay_verified());
    let tampered = tamper(&chemistry) && tamper(&bridge) && linear.as_ref().is_none_or(tamper);
    let authorized = mode == Expected::Supported
        && bridge.status == ChemistryLinearBridgeStatus::Complete
        && linear
            .as_ref()
            .is_some_and(|result| result.status == LinearAlgebraStatus::Complete)
        && replay;
    let exact = match mode {
        Expected::Supported => authorized,
        Expected::Ambiguous => bridge.status == ChemistryLinearBridgeStatus::Ambiguous,
        Expected::Refused => linear
            .as_ref()
            .is_none_or(|result| result.status != LinearAlgebraStatus::Complete),
    };
    Outcome {
        authorized,
        exact,
        intermediate_entries: 2 + usize::from(linear.is_some()),
        replay_verified: replay,
        tamper_rejected: tampered,
        failure_gate: (!authorized).then(|| "chemistry_linear_boundary".into()),
    }
}

fn biology_probability(mode: Expected, index: usize) -> Outcome {
    let sequence = match index % 3 {
        0 => "AATTGGCC",
        1 => "ATCGATCG",
        _ => "GGCCAATT",
    };
    let biology = evaluate_biology(&BiologyRequest {
        operation: BiologyOperation::BaseComposition,
        sequence: Some(sequence.into()),
        orientation: None,
        domain: "source_derived_bounded_dna".into(),
        ambiguity: (mode == Expected::Ambiguous).then(|| "sampling target is unresolved".into()),
        provenance: vec!["stage-v-biology-probability".into()],
    });
    let policy = match mode {
        Expected::Supported => Some("uniform_position"),
        _ => None,
    };
    let bridge = bridge_base_composition(&biology, policy);
    let probability = bridge
        .handoff
        .as_ref()
        .map(|handoff| the_machine::probability_pack::evaluate_probability(&handoff.request));
    let replay = biology.replay_verified()
        && bridge.replay_verified()
        && probability
            .as_ref()
            .is_none_or(|result| result.replay_verified());
    let tampered = tamper(&biology) && tamper(&bridge) && probability.as_ref().is_none_or(tamper);
    let authorized = mode == Expected::Supported
        && bridge.status == BiologyProbabilityBridgeStatus::Complete
        && probability.as_ref().is_some_and(|result| {
            result.status == the_machine::probability_pack::ProbabilityStatus::Complete
        })
        && replay;
    let exact = match mode {
        Expected::Supported => authorized,
        Expected::Ambiguous => bridge.status == BiologyProbabilityBridgeStatus::Ambiguous,
        Expected::Refused => bridge.status != BiologyProbabilityBridgeStatus::Complete,
    };
    Outcome {
        authorized,
        exact,
        intermediate_entries: 2 + usize::from(probability.is_some()),
        replay_verified: replay,
        tamper_rejected: tampered,
        failure_gate: (!authorized).then(|| "biology_probability_policy".into()),
    }
}

fn complex_linear(mode: Expected, index: usize) -> Outcome {
    let real = q((3 + index % 7) as i128, 1);
    let imag = q((4 + index % 5) as i128, 1);
    let complex = evaluate_complex(&ComplexRequest {
        operation: if mode == Expected::Refused {
            ComplexOperation::PolarConversion
        } else {
            ComplexOperation::Conjugate
        },
        a: Some(real),
        b: Some(imag),
        c: None,
        d: None,
        domain: "source_derived_complex_arithmetic".into(),
        ambiguity: (mode == Expected::Ambiguous).then(|| "complex operation is unresolved".into()),
        provenance: vec!["stage-v-complex-linear".into()],
    });
    let complex_pair = complex.artifact.clone();
    let bridge = bridge_complex_to_real_matrix(&ComplexMatrixBridgeRequest {
        complex: complex_pair,
        domain: "complex_to_real_matrix_bridge".into(),
        ambiguity: (mode == Expected::Ambiguous)
            .then(|| "matrix representation is unresolved".into()),
        provenance: complex.provenance.clone(),
    });
    let replay = complex.replay_verified()
        && bridge.replay_verified()
        && bridge
            .linear_algebra
            .as_ref()
            .is_none_or(|result| result.replay_verified());
    let tampered =
        tamper(&complex) && tamper(&bridge) && bridge.linear_algebra.as_ref().is_none_or(tamper);
    let authorized = mode == Expected::Supported
        && complex.status == ComplexStatus::Complete
        && bridge.status == ComplexBridgeStatus::Complete
        && replay;
    let exact = match mode {
        Expected::Supported => authorized,
        Expected::Ambiguous => bridge.status == ComplexBridgeStatus::Ambiguous,
        Expected::Refused => bridge.status != ComplexBridgeStatus::Complete,
    };
    Outcome {
        authorized,
        exact,
        intermediate_entries: 2 + usize::from(bridge.linear_algebra.is_some()),
        replay_verified: replay,
        tamper_rejected: tampered,
        failure_gate: (!authorized).then(|| "complex_linear_boundary".into()),
    }
}

fn metric_topology(mode: Expected, index: usize) -> Outcome {
    let count = 3 + index % 3;
    let points = (0..count).map(|i| format!("p{i}")).collect::<Vec<_>>();
    let distances = (0..count)
        .flat_map(|left| (left..count).map(move |right| (left, right)))
        .map(|(left, right)| DistanceEntry {
            left: format!("p{left}"),
            right: format!("p{right}"),
            distance: (right - left) as i64,
        })
        .collect::<Vec<_>>();
    let metric_records = extract_metric_definitions(include_str!(
        "../../docs/sources/topology_without_tears_finite_metric_definition.txt"
    ))
    .expect("metric source extracts");
    let metric = evaluate_metric(
        &MetricRequest {
            operation: MetricOperation::ValidateMetric,
            metric: "finite_metric_axioms".into(),
            points,
            distances,
            center: None,
            target: None,
            radius: None,
            domain: if mode == Expected::Refused {
                "infinite_metric_space"
            } else {
                "source_derived_finite_metric"
            }
            .into(),
            ambiguity: (mode == Expected::Ambiguous)
                .then(|| "induced topology policy is unresolved".into()),
            provenance: vec!["stage-v-metric-topology".into()],
        },
        &metric_records,
    );
    let bridge = metric_result_to_topology(&metric);
    let topology = bridge.request.as_ref().map(|request| {
        let records = extract_topology_definitions(include_str!(
            "../../docs/sources/topology_without_tears_finite_definition.txt"
        ))
        .expect("topology source extracts");
        evaluate_topology(request, &records)
    });
    let replay = metric.replay_verified()
        && bridge.replay_verified()
        && topology
            .as_ref()
            .is_none_or(|result| result.replay_verified());
    let tampered = tamper(&metric) && tamper(&bridge) && topology.as_ref().is_none_or(tamper);
    let authorized = mode == Expected::Supported
        && metric.status == MetricStatus::Complete
        && bridge.status == MetricTopologyBridgeStatus::Complete
        && topology.as_ref().is_some_and(|result| result.authorized())
        && replay;
    let exact = match mode {
        Expected::Supported => authorized,
        Expected::Ambiguous => bridge.status == MetricTopologyBridgeStatus::Ambiguous,
        Expected::Refused => bridge.status != MetricTopologyBridgeStatus::Complete,
    };
    Outcome {
        authorized,
        exact,
        intermediate_entries: 2 + usize::from(topology.is_some()),
        replay_verified: replay,
        tamper_rejected: tampered,
        failure_gate: (!authorized).then(|| "metric_topology_boundary".into()),
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let routes = [
        (
            "regression_statistics",
            regression_statistics as fn(Expected, usize) -> Outcome,
        ),
        ("chemistry_linear", chemistry_linear),
        ("biology_probability", biology_probability),
        ("complex_linear", complex_linear),
        ("metric_topology", metric_topology),
    ];
    let mut receipts = Vec::with_capacity(1_000);
    for (route_index, (route, evaluator)) in routes.into_iter().enumerate() {
        for index in 0..120 {
            let outcome = evaluator(Expected::Supported, index);
            receipts.push(Receipt {
                id: format!("supported_{route_index}_{index:03}"),
                route: route.into(),
                expected: Expected::Supported,
                authorized: outcome.authorized,
                exact: outcome.exact,
                intermediate_entries: outcome.intermediate_entries,
                replay_verified: outcome.replay_verified,
                tamper_rejected: outcome.tamper_rejected,
                failure_gate: outcome.failure_gate,
                false_authorization: false,
                false_denial: !outcome.authorized,
            });
        }
        for index in 0..40 {
            let outcome = evaluator(Expected::Ambiguous, index);
            receipts.push(Receipt {
                id: format!("ambiguous_{route_index}_{index:03}"),
                route: route.into(),
                expected: Expected::Ambiguous,
                authorized: outcome.authorized,
                exact: outcome.exact && !outcome.authorized,
                intermediate_entries: outcome.intermediate_entries,
                replay_verified: outcome.replay_verified,
                tamper_rejected: outcome.tamper_rejected,
                failure_gate: outcome.failure_gate,
                false_authorization: outcome.authorized,
                false_denial: false,
            });
        }
        for index in 0..40 {
            let outcome = evaluator(Expected::Refused, index);
            receipts.push(Receipt {
                id: format!("refused_{route_index}_{index:03}"),
                route: route.into(),
                expected: Expected::Refused,
                authorized: outcome.authorized,
                exact: outcome.exact && !outcome.authorized,
                intermediate_entries: outcome.intermediate_entries,
                replay_verified: outcome.replay_verified,
                tamper_rejected: outcome.tamper_rejected,
                failure_gate: outcome.failure_gate,
                false_authorization: outcome.authorized,
                false_denial: false,
            });
        }
    }
    let report = Report {
        schema: "stage-v-source-composition-1000-v1",
        source:
            "independently authored source-derived routes; every route validates typed handoffs",
        corpus_sha256: digest(&receipts),
        cases: receipts.len(),
        supported: receipts
            .iter()
            .filter(|r| r.expected == Expected::Supported)
            .count(),
        ambiguous: receipts
            .iter()
            .filter(|r| r.expected == Expected::Ambiguous)
            .count(),
        refused: receipts
            .iter()
            .filter(|r| r.expected == Expected::Refused)
            .count(),
        exact_decisions: receipts.iter().filter(|r| r.exact).count(),
        supported_routes: receipts
            .iter()
            .filter(|r| r.expected == Expected::Supported && r.authorized)
            .count(),
        replay_verified: receipts.iter().filter(|r| r.replay_verified).count(),
        emitted_intermediate_entries: receipts.iter().map(|r| r.intermediate_entries).sum(),
        tamper_rejections: receipts.iter().filter(|r| r.tamper_rejected).count(),
        failure_localized: receipts.iter().filter(|r| r.failure_gate.is_some()).count(),
        false_authorizations: receipts.iter().filter(|r| r.false_authorization).count(),
        false_denials: receipts.iter().filter(|r| r.false_denial).count(),
        route_counts: receipts
            .iter()
            .fold(BTreeMap::new(), |mut counts, receipt| {
                *counts.entry(receipt.route.clone()).or_insert(0) += 1;
                counts
            }),
        hle_questions_read: 0,
        production_registry_mutations: 0,
        receipts,
    };
    assert_eq!(report.cases, 1_000);
    assert_eq!(
        (report.supported, report.ambiguous, report.refused),
        (600, 200, 200)
    );
    assert_eq!(report.exact_decisions, 1_000);
    assert_eq!(report.supported_routes, 600);
    assert_eq!(report.replay_verified, 1_000);
    assert_eq!(report.tamper_rejections, 1_000);
    assert_eq!(report.failure_localized, 400);
    assert_eq!(report.false_authorizations, 0);
    assert_eq!(report.false_denials, 0);
    fs::write(
        REPORT_JSON,
        format!("{}\n", serde_json::to_string_pretty(&report)?),
    )?;
    fs::write(REPORT_MD, format!("# Stage V: source-derived composition\n\n- Cases: 1,000 (600 supported, 200 ambiguous, 200 refused)\n- Routes: regression+statistics, chemistry+linear algebra, biology+probability, complex+linear algebra, metric+topology\n- Exact decisions: 1,000/1,000\n- Supported routes: 600/600\n- Replay and tamper: 1,000/1,000 each\n- Failure localization: 400/400 non-supported routes\n- False authorizations / denials: 0 / 0\n- HLE questions read: 0\n- Production registry mutations: 0\n- Corpus report: `{}`\n", REPORT_JSON))?;
    Ok(())
}
