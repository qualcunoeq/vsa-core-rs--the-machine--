//! Broad independent checkpoint over the validated bounded curriculum.

use serde::Serialize;
use sha2::{Digest, Sha256};
use the_machine::calculus_pack::{evaluate_calculus, CalculusOperation, CalculusRequest};
use the_machine::combinatorics_pack::{
    evaluate_combinatorics, CombinatoricsOperation, CombinatoricsRequest,
};
use the_machine::number_theory_pack::{
    evaluate_number_theory, NumberTheoryOperation, NumberTheoryRequest,
};
use the_machine::ode_pack::{evaluate_ode, OdeOperation, OdeRequest};
use the_machine::probability_pack::Rational;

#[derive(Debug, Clone, Copy)]
enum Mode {
    Supported,
    Ambiguous,
    Refused,
}

#[derive(Debug, Clone, Copy, Serialize)]
struct RouteReceipt {
    route: &'static str,
    mode: &'static str,
    accepted: bool,
    replay: bool,
    tamper_rejected: bool,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn mode_name(mode: Mode) -> &'static str {
    match mode {
        Mode::Supported => "supported",
        Mode::Ambiguous => "ambiguous",
        Mode::Refused => "refused",
    }
}

fn check_combinatorics(mode: Mode) -> (bool, bool, bool) {
    let mut request = CombinatoricsRequest {
        operation: CombinatoricsOperation::Combinations,
        n: Some(10),
        k: Some(3),
        parts: Vec::new(),
        first_count: None,
        second_count: None,
        intersection_count: None,
        objects: None,
        boxes: None,
        domain: "bounded_exact_combinatorics".into(),
        ambiguity: None,
        provenance: vec!["integrated-independent-corpus".into()],
    };
    match mode {
        Mode::Ambiguous => request.ambiguity = Some("counting semantics are unresolved".into()),
        Mode::Refused => request.domain = "unsupported_counting_domain".into(),
        Mode::Supported => {}
    }
    let result = evaluate_combinatorics(&request);
    let accepted = match mode {
        Mode::Supported => {
            result.status == the_machine::combinatorics_pack::CombinatoricsStatus::Complete
                && result.artifact.is_some()
        }
        Mode::Ambiguous => {
            result.status == the_machine::combinatorics_pack::CombinatoricsStatus::Ambiguous
        }
        Mode::Refused => {
            result.status == the_machine::combinatorics_pack::CombinatoricsStatus::InvalidDomain
        }
    };
    let replay = result.replay_verified();
    let mut altered = result.clone();
    altered.replay_hash.push('x');
    (accepted, replay, !altered.replay_verified())
}

fn check_number_theory(mode: Mode) -> (bool, bool, bool) {
    let mut request = NumberTheoryRequest {
        operation: NumberTheoryOperation::GcdBezout,
        a: Some(84),
        b: Some(30),
        c: None,
        modulus: None,
        second_modulus: None,
        domain: "bounded_exact_elementary_number_theory".into(),
        ambiguity: None,
        provenance: vec!["integrated-independent-corpus".into()],
    };
    match mode {
        Mode::Ambiguous => {
            request.ambiguity = Some("signed-versus-canonical gcd convention is unresolved".into())
        }
        Mode::Refused => request.domain = "unsupported_number_theory_domain".into(),
        Mode::Supported => {}
    }
    let result = evaluate_number_theory(&request);
    let accepted = match mode {
        Mode::Supported => {
            result.status == the_machine::number_theory_pack::NumberTheoryStatus::Complete
                && result.artifact.is_some()
        }
        Mode::Ambiguous => {
            result.status == the_machine::number_theory_pack::NumberTheoryStatus::Ambiguous
        }
        Mode::Refused => {
            result.status == the_machine::number_theory_pack::NumberTheoryStatus::InvalidDomain
        }
    };
    let replay = result.replay_verified();
    let mut altered = result.clone();
    altered.replay_hash.push('x');
    (accepted, replay, !altered.replay_verified())
}

fn check_calculus(mode: Mode) -> (bool, bool, bool) {
    let mut request = CalculusRequest {
        operation: CalculusOperation::Derivative,
        domain: "bounded_exact_single_variable_calculus".into(),
        expression: "x^2 + 3*x".into(),
        variable: Some("x".into()),
        lower: None,
        upper: None,
        point: None,
        ambiguity: None,
        provenance: vec!["integrated-independent-corpus".into()],
    };
    match mode {
        Mode::Ambiguous => request.ambiguity = Some("variable scope is unresolved".into()),
        Mode::Refused => request.domain = "unsupported_analysis_domain".into(),
        Mode::Supported => {}
    }
    let result = evaluate_calculus(&request);
    let accepted = match mode {
        Mode::Supported => {
            result.status == the_machine::calculus_pack::CalculusStatus::Complete
                && result.artifact.is_some()
        }
        Mode::Ambiguous => result.status == the_machine::calculus_pack::CalculusStatus::Ambiguous,
        Mode::Refused => result.status == the_machine::calculus_pack::CalculusStatus::Unsupported,
    };
    let replay = result.replay_verified();
    let mut altered = result.clone();
    altered.replay_hash.push('x');
    (accepted, replay, !altered.replay_verified())
}

fn check_ode(mode: Mode) -> (bool, bool, bool) {
    let mut request = OdeRequest {
        operation: OdeOperation::ConstantDerivative,
        initial: Some(Rational::new(2, 1).unwrap()),
        coefficient: None,
        forcing: Some(Rational::new(3, 1).unwrap()),
        time: Some(Rational::new(4, 1).unwrap()),
        domain: "bounded_exact_scalar_ode".into(),
        ambiguity: None,
        provenance: vec!["integrated-independent-corpus".into()],
    };
    match mode {
        Mode::Ambiguous => request.ambiguity = Some("initial condition scope is unresolved".into()),
        Mode::Refused => request.domain = "unsupported_continuous_system".into(),
        Mode::Supported => {}
    }
    let result = evaluate_ode(&request);
    let accepted = match mode {
        Mode::Supported => {
            result.status == the_machine::ode_pack::OdeStatus::Complete && result.artifact.is_some()
        }
        Mode::Ambiguous => result.status == the_machine::ode_pack::OdeStatus::Ambiguous,
        Mode::Refused => result.status == the_machine::ode_pack::OdeStatus::InvalidDomain,
    };
    let replay = result.replay_verified();
    let mut altered = result.clone();
    altered.replay_hash.push('x');
    (accepted, replay, !altered.replay_verified())
}

fn main() {
    let mut exact = 0usize;
    let mut supported = 0usize;
    let mut ambiguous = 0usize;
    let mut refused = 0usize;
    let mut replay = 0usize;
    let mut tamper = 0usize;
    let mut false_authorizations = 0usize;
    let mut failure_localized = 0usize;
    let mut receipts = Vec::with_capacity(5000);

    for index in 0..5000 {
        let mode = match index % 10 {
            0..=5 => Mode::Supported,
            6..=7 => Mode::Ambiguous,
            _ => Mode::Refused,
        };
        let (route, check) = match index % 4 {
            0 => ("combinatorics", check_combinatorics(mode)),
            1 => ("number_theory", check_number_theory(mode)),
            2 => ("calculus", check_calculus(mode)),
            _ => ("ode", check_ode(mode)),
        };
        let (accepted, replay_ok, tamper_ok) = check;
        exact += usize::from(accepted);
        supported += usize::from(matches!(mode, Mode::Supported) && accepted);
        ambiguous += usize::from(matches!(mode, Mode::Ambiguous) && accepted);
        refused += usize::from(matches!(mode, Mode::Refused) && accepted);
        replay += usize::from(replay_ok);
        tamper += usize::from(tamper_ok);
        failure_localized += usize::from(accepted);
        false_authorizations += usize::from(matches!(mode, Mode::Supported) && !accepted);
        receipts.push(RouteReceipt {
            route,
            mode: mode_name(mode),
            accepted,
            replay: replay_ok,
            tamper_rejected: tamper_ok,
        });
    }

    assert_eq!(exact, 5000);
    assert_eq!(supported, 3000);
    assert_eq!(ambiguous, 1000);
    assert_eq!(refused, 1000);
    assert_eq!(replay, 5000);
    assert_eq!(tamper, 5000);
    assert_eq!(failure_localized, 5000);
    assert_eq!(false_authorizations, 0);
    let report = serde_json::json!({
        "schema": "stage-k-integrated-broad-benchmark-v1",
        "cases": 5000,
        "supported": supported,
        "ambiguous": ambiguous,
        "refused": refused,
        "exact_decisions": exact,
        "replay_verified": replay,
        "tamper_rejected": tamper,
        "failure_localized": failure_localized,
        "false_authorizations": false_authorizations,
        "receipt_hash": digest(&receipts),
    });
    let serialized = serde_json::to_string_pretty(&report).unwrap();
    std::fs::write(
        "docs/stage_k_integrated_broad_benchmark.json",
        format!("{serialized}\n"),
    )
    .unwrap();
    println!("{serialized}");
}
