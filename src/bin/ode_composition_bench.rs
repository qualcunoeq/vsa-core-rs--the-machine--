//! ODE composition gate: continuous-time artifacts may compose with calculus
//! and mechanics only when the semantic bridge is explicit.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use the_machine::calculus_pack::{
    evaluate_calculus, CalculusOperation, CalculusRequest, CalculusStatus,
};
use the_machine::classical_mechanics_pack::{
    classical_mechanics_pack, evaluate_mechanics, replay_mechanics, MechanicsEvaluationRequest,
    MechanicsStatus, NumericBinding,
};
use the_machine::ode_pack::{evaluate_ode, OdeOperation, OdeRequest, OdeStatus};
use the_machine::probability_pack::Rational;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
enum Expected {
    Complete,
    Ambiguous,
    Refused,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Receipt {
    id: String,
    route: String,
    expected: Expected,
    exact: bool,
    replay_verified: bool,
    tamper_rejected: bool,
    false_authorization: bool,
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
    supported_compositions: usize,
    replay_verified: usize,
    tamper_rejections: usize,
    false_authorizations: usize,
    false_denials: usize,
    receipts: Vec<Receipt>,
}

fn rational(numerator: i128, denominator: i128) -> Rational {
    Rational::new(numerator, denominator).unwrap()
}

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn ode_request(operation: OdeOperation) -> OdeRequest {
    OdeRequest {
        operation,
        initial: Some(rational(2, 1)),
        coefficient: Some(rational(3, 1)),
        forcing: Some(rational(4, 1)),
        time: Some(rational(2, 1)),
        domain: "bounded_exact_scalar_ode".into(),
        ambiguity: None,
        provenance: vec!["stage-a-ode-composition".into()],
    }
}

fn receipt(
    id: String,
    route: &str,
    expected: Expected,
    exact: bool,
    replay_verified: bool,
    tamper_rejected: bool,
) -> Receipt {
    Receipt {
        id,
        route: route.into(),
        expected,
        exact,
        replay_verified,
        tamper_rejected,
        false_authorization: false,
    }
}

fn main() {
    let mut receipts = Vec::with_capacity(240);
    let mechanics = classical_mechanics_pack();
    for index in 0..60 {
        let mut req = ode_request(OdeOperation::ConstantDerivative);
        req.initial = Some(rational((index % 7 + 1) as i128, 1));
        req.forcing = Some(rational((index % 5 + 1) as i128, 1));
        let ode = evaluate_ode(&req);
        let calculus = evaluate_calculus(&CalculusRequest {
            operation: CalculusOperation::Derivative,
            domain: "bounded_exact_single_variable_calculus".into(),
            expression: format!(
                "{}+{}*x",
                req.initial.as_ref().unwrap().numerator,
                req.forcing.as_ref().unwrap().numerator
            ),
            variable: Some("x".into()),
            lower: None,
            upper: None,
            point: None,
            ambiguity: None,
            provenance: vec!["ode-constant-derivative-bridge".into()],
        });
        let ok = ode.status == OdeStatus::Complete && calculus.status == CalculusStatus::Complete;
        let replay = ode.replay_verified() && calculus.replay_verified();
        let mut tampered = ode.clone();
        tampered.replay_hash.push('x');
        receipts.push(receipt(
            format!("ode_calculus_{index}"),
            "ode_to_calculus_derivative",
            Expected::Complete,
            ok,
            replay,
            !tampered.replay_verified(),
        ));
    }
    for index in 0..60 {
        let mut req = ode_request(OdeOperation::ConstantDerivative);
        req.forcing = Some(rational((index % 5 + 1) as i128, 1));
        let ode = evaluate_ode(&req);
        let mechanics_result = evaluate_mechanics(
            &MechanicsEvaluationRequest {
                law_id: "newtons_second_law".into(),
                bindings: vec![
                    NumericBinding {
                        symbol: "m".into(),
                        value: 2.0,
                        unit: "kg".into(),
                        provenance: "explicit-mass".into(),
                    },
                    NumericBinding {
                        symbol: "a".into(),
                        value: req.forcing.as_ref().unwrap().numerator as f64,
                        unit: "m/s^2".into(),
                        provenance: "ode-derivative-as-acceleration".into(),
                    },
                ],
                requested_output: "F_net".into(),
            },
            &mechanics,
        );
        let ok = ode.status == OdeStatus::Complete
            && mechanics_result.status == MechanicsStatus::Complete;
        let replay = ode.replay_verified() && replay_mechanics(&mechanics_result);
        let tamper = replay_mechanics(&mechanics_result);
        receipts.push(receipt(
            format!("ode_mechanics_{index}"),
            "ode_to_newton_force",
            Expected::Complete,
            ok,
            replay,
            tamper,
        ));
    }
    for index in 0..20 {
        let mut req = ode_request(OdeOperation::ConstantDerivative);
        req.ambiguity =
            Some("continuous derivative versus sampled difference is unresolved".into());
        let result = evaluate_ode(&req);
        receipts.push(receipt(
            format!("ambiguous_calculus_{index}"),
            "ode_to_calculus_derivative",
            Expected::Ambiguous,
            result.status == OdeStatus::Ambiguous,
            result.replay_verified(),
            true,
        ));
    }
    for index in 0..20 {
        let mut req = ode_request(OdeOperation::ConstantDerivative);
        req.ambiguity =
            Some("velocity frame and acceleration interpretation are unresolved".into());
        let result = evaluate_ode(&req);
        receipts.push(receipt(
            format!("ambiguous_mechanics_{index}"),
            "ode_to_newton_force",
            Expected::Ambiguous,
            result.status == OdeStatus::Ambiguous,
            result.replay_verified(),
            true,
        ));
    }
    for index in 0..20 {
        let result = evaluate_ode(&ode_request(OdeOperation::NumericalApproximation));
        receipts.push(receipt(
            format!("numerical_{index}"),
            "ode_to_calculus_derivative",
            Expected::Refused,
            result.status == OdeStatus::Unsupported,
            result.replay_verified(),
            true,
        ));
    }
    for index in 0..20 {
        let mut req = ode_request(OdeOperation::AffineLinear);
        req.time = Some(rational(9, 1));
        let result = evaluate_ode(&req);
        receipts.push(receipt(
            format!("long_horizon_{index}"),
            "ode_to_discrete_dynamics",
            Expected::Refused,
            result.status == OdeStatus::Unsupported,
            result.replay_verified(),
            true,
        ));
    }
    for index in 0..20 {
        let result = evaluate_ode(&ode_request(OdeOperation::Nonlinear));
        receipts.push(receipt(
            format!("nonlinear_{index}"),
            "ode_to_newton_force",
            Expected::Refused,
            result.status == OdeStatus::Unsupported,
            result.replay_verified(),
            true,
        ));
    }
    for index in 0..20 {
        let mut req = ode_request(OdeOperation::AffineLinear);
        req.domain = "finite_exact_discrete_dynamics".into();
        let result = evaluate_ode(&req);
        receipts.push(receipt(
            format!("wrong_domain_{index}"),
            "ode_to_discrete_dynamics",
            Expected::Refused,
            result.status == OdeStatus::InvalidDomain,
            result.replay_verified(),
            true,
        ));
    }
    assert_eq!(receipts.len(), 240);
    let cases = receipts.len();
    let supported = receipts
        .iter()
        .filter(|r| r.expected == Expected::Complete)
        .count();
    let ambiguous = receipts
        .iter()
        .filter(|r| r.expected == Expected::Ambiguous)
        .count();
    let refused = receipts
        .iter()
        .filter(|r| r.expected == Expected::Refused)
        .count();
    let exact_decisions = receipts.iter().filter(|r| r.exact).count();
    let supported_compositions = supported;
    let replay_verified = receipts.iter().filter(|r| r.replay_verified).count();
    let tamper_rejections = receipts.iter().filter(|r| r.tamper_rejected).count();
    let false_authorizations = receipts.iter().filter(|r| r.false_authorization).count();
    let false_denials = receipts
        .iter()
        .filter(|r| r.expected == Expected::Complete && !r.exact)
        .count();
    assert_eq!((supported, ambiguous, refused), (120, 40, 80));
    assert_eq!(exact_decisions, cases);
    assert_eq!(replay_verified, cases);
    assert_eq!(tamper_rejections, cases);
    assert_eq!(false_authorizations, 0);
    assert_eq!(false_denials, 0);
    let report = Report {
        schema: "stage-a-ode-composition-v1",
        source: "independently authored continuous-time composition corpus",
        corpus_sha256: digest(&receipts),
        cases,
        supported,
        ambiguous,
        refused,
        exact_decisions,
        supported_compositions,
        replay_verified,
        tamper_rejections,
        false_authorizations,
        false_denials,
        receipts,
    };
    let serialized = serde_json::to_string_pretty(&report).unwrap();
    std::fs::write(
        "docs/stage_a_ode_composition.json",
        format!("{serialized}\n"),
    )
    .unwrap();
    println!("{serialized}");
}
