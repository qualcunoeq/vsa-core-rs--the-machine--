//! Stage 78: route-blind language access across spectral and mechanics packs.
//!
//! Every report is offered to both independently validated frontends.  A
//! route is authorized only when exactly one frontend produces a complete
//! typed artifact and its shadow executor succeeds.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::fs;
use the_machine::classical_mechanics_pack::MechanicsStatus;
use the_machine::mechanics_situation::{
    execute_mechanics_situation, formalize_mechanics_situation, replay_execution, replay_situation,
    SituationStatus,
};
use the_machine::spectral_frontend::{formalize_spectral_text, SpectralFrontendStatus};
use the_machine::spectral_linear_algebra_pack::{evaluate_spectral, SpectralStatus};

const REPORT_JSON: &str = "docs/stage78_curriculum_frontend_route_blind.json";
const REPORT_MD: &str = "docs/stage78_curriculum_frontend_route_blind.md";

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum Expected {
    Supported,
    Ambiguous,
    Unsupported,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum Module {
    Spectral,
    Mechanics,
}

#[derive(Debug, Clone, Serialize)]
struct Case {
    id: String,
    family: String,
    expected: Expected,
    text: String,
}

#[derive(Debug, Serialize)]
struct Receipt {
    id: String,
    expected: Expected,
    selected_module: Option<Module>,
    authorized_modules: Vec<Module>,
    ambiguous_modules: Vec<Module>,
    exact: bool,
    frontend_replay: bool,
    frontend_tamper_rejected: bool,
    execution_replay: bool,
    execution_tamper_rejected: bool,
    false_authorization: bool,
    provenance_complete: bool,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    corpus_sha256: String,
    cases: usize,
    supported: usize,
    ambiguous: usize,
    unsupported: usize,
    exact_decisions: usize,
    authorized_supported: usize,
    ambiguities_preserved: usize,
    unsupported_refusals: usize,
    frontend_replays: usize,
    frontend_tamper_rejections: usize,
    execution_replays: usize,
    execution_tamper_rejections: usize,
    false_authorizations: usize,
    false_denials: usize,
    route_leakage: usize,
    receipts: Vec<Receipt>,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn spectral_supported(index: usize) -> String {
    match index % 6 {
        0 => "Find the characteristic polynomial of [[2,1],[1,2]].".into(),
        1 => "Find the eigenvalues of [[2,0],[0,5]].".into(),
        2 => "Find the eigenspace for eigenvalue=3 of [[2,1],[1,2]].".into(),
        3 => "Determine whether [[1,0],[0,1]] is diagonalizable.".into(),
        4 => "Compute the matrix power power=2 of [[2,1],[1,2]].".into(),
        _ => "Give the spectral decomposition of [[2,0],[0,5]].".into(),
    }
}

fn mechanics_supported(index: usize) -> String {
    match index % 5 {
        0 => format!(
            "In an inertial frame, a body has mass {} kg and net force {} N. Find acceleration.",
            2 + index % 7,
            (2 + index % 7) * (3 + index % 5)
        ),
        1 => format!(
            "In a non-relativistic regime, a body of mass {} kg moves at velocity {} m/s. Find kinetic energy.",
            2 + index % 7,
            3 + index % 5
        ),
        2 => format!(
            "In one dimension, a body of mass {} kg moves at velocity {} m/s. Find momentum.",
            2 + index % 7,
            3 + index % 5
        ),
        3 => format!(
            "An ideal linear spring has spring constant {} N/m and displacement {} m. Find spring force.",
            4 + index % 7,
            1 + index % 3
        ),
        _ => format!(
            "An ideal linear spring has spring constant {} N/m and displacement {} m. Find elastic potential energy.",
            4 + index % 7,
            1 + index % 3
        ),
    }
}

fn corpus() -> Vec<Case> {
    let mut cases = Vec::with_capacity(1000);
    for index in 0..400 {
        cases.push(Case {
            id: format!("spectral-supported-{index:03}"),
            family: "spectral".into(),
            expected: Expected::Supported,
            text: spectral_supported(index),
        });
    }
    for index in 0..100 {
        cases.push(Case {
            id: format!("spectral-ambiguous-{index:03}"),
            family: "spectral".into(),
            expected: Expected::Ambiguous,
            text: "Find the eigenvalues and characteristic polynomial of [[2,1],[1,2]].".into(),
        });
    }
    for index in 0..100 {
        let text = match index % 4 {
            0 => "Give a numerical approximate spectrum of [[2,1],[1,2]].",
            1 => "Analyze the infinite-dimensional spectral gap of [[2,1],[1,2]].",
            2 => "Find the eigenvalues of matrix A.",
            _ => "Compute the matrix power of [[2,1],[1,2]].",
        };
        cases.push(Case {
            id: format!("spectral-unsupported-{index:03}"),
            family: "spectral".into(),
            expected: Expected::Unsupported,
            text: text.into(),
        });
    }
    for index in 0..200 {
        cases.push(Case {
            id: format!("mechanics-supported-{index:03}"),
            family: "mechanics".into(),
            expected: Expected::Supported,
            text: mechanics_supported(index),
        });
    }
    for index in 0..100 {
        cases.push(Case { id: format!("mechanics-ambiguous-{index:03}"), family: "mechanics".into(), expected: Expected::Ambiguous, text: format!("In a non-relativistic regime, a body of mass 3 kg moves at velocity 4 m/s. Find momentum and kinetic energy.") });
    }
    for index in 0..100 {
        let text = match index % 3 {
            0 => "A relativistic body has mass 3 kg and velocity 4 m/s. Find kinetic energy.",
            1 => "Find the acceleration of a body with only an unspecified force.",
            _ => "A two-body fluid system has mass 3 kg and velocity 4 m/s. Find momentum.",
        };
        cases.push(Case {
            id: format!("mechanics-unsupported-{index:03}"),
            family: "mechanics".into(),
            expected: Expected::Unsupported,
            text: text.into(),
        });
    }
    cases
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cases = corpus();
    assert_eq!(cases.len(), 1000);
    let corpus_sha256 = digest(&cases);
    let mut receipts = Vec::with_capacity(cases.len());
    for case in cases {
        let spectral = formalize_spectral_text(&case.text);
        let mechanics = formalize_mechanics_situation(&case.text);
        let mut authorized = Vec::new();
        let mut ambiguous = Vec::new();
        let mut frontend_replay = spectral.replay_verified() && replay_situation(&mechanics);
        let mut frontend_tamper_rejected = {
            let mut spectral_tampered = spectral.clone();
            spectral_tampered.replay_hash.push('x');
            let mut mechanics_tampered = mechanics.clone();
            mechanics_tampered.replay_hash.push('x');
            !spectral_tampered.replay_verified() && !replay_situation(&mechanics_tampered)
        };
        let mut execution_replay = false;
        let mut execution_tamper_rejected = false;
        let spectral_execution = spectral.request.as_ref().map(evaluate_spectral);
        if spectral.status == SpectralFrontendStatus::Complete
            && spectral_execution
                .as_ref()
                .is_some_and(|r| r.status == SpectralStatus::Complete && r.artifact.is_some())
        {
            authorized.push(Module::Spectral);
            let result = spectral_execution.as_ref().unwrap();
            execution_replay = result.replay_verified();
            let mut tampered = result.clone();
            tampered.replay_hash.push('x');
            execution_tamper_rejected = !tampered.replay_verified();
        } else if spectral.status == SpectralFrontendStatus::Ambiguous {
            ambiguous.push(Module::Spectral);
        }
        let mechanics_execution = execute_mechanics_situation(&mechanics);
        if mechanics.status == SituationStatus::Unique
            && mechanics_execution.mechanics_status == Some(MechanicsStatus::Complete)
            && mechanics_execution.value.is_some()
        {
            authorized.push(Module::Mechanics);
            execution_replay = replay_execution(&mechanics_execution);
            let mut tampered = mechanics_execution.clone();
            tampered.replay_hash.push('x');
            execution_tamper_rejected = !replay_execution(&tampered);
        } else if mechanics.status == SituationStatus::Ambiguous {
            ambiguous.push(Module::Mechanics);
        }
        let selected_module = if authorized.len() == 1 {
            Some(authorized[0])
        } else {
            None
        };
        let exact = match case.expected {
            Expected::Supported => {
                authorized.len() == 1
                    && selected_module
                        == Some(if case.family == "spectral" {
                            Module::Spectral
                        } else {
                            Module::Mechanics
                        })
            }
            Expected::Ambiguous => authorized.is_empty() && !ambiguous.is_empty(),
            Expected::Unsupported => authorized.is_empty() && ambiguous.is_empty(),
        };
        let false_authorization = case.expected != Expected::Supported && !authorized.is_empty();
        if !exact {
            frontend_replay = false;
            frontend_tamper_rejected = false;
        }
        receipts.push(Receipt {
            id: case.id,
            expected: case.expected,
            selected_module,
            authorized_modules: authorized,
            ambiguous_modules: ambiguous,
            exact,
            frontend_replay,
            frontend_tamper_rejected,
            execution_replay,
            execution_tamper_rejected,
            false_authorization,
            provenance_complete: !spectral.provenance_spans.is_empty()
                || !mechanics.provenance.is_empty(),
        });
    }
    let cases = receipts.len();
    let supported = receipts
        .iter()
        .filter(|r| r.expected == Expected::Supported)
        .count();
    let ambiguous = receipts
        .iter()
        .filter(|r| r.expected == Expected::Ambiguous)
        .count();
    let unsupported = receipts
        .iter()
        .filter(|r| r.expected == Expected::Unsupported)
        .count();
    let exact_decisions = receipts.iter().filter(|r| r.exact).count();
    let authorized_supported = receipts
        .iter()
        .filter(|r| r.expected == Expected::Supported && r.selected_module.is_some())
        .count();
    let ambiguities_preserved = receipts
        .iter()
        .filter(|r| r.expected == Expected::Ambiguous && r.exact)
        .count();
    let unsupported_refusals = receipts
        .iter()
        .filter(|r| r.expected == Expected::Unsupported && r.exact)
        .count();
    let frontend_replays = receipts.iter().filter(|r| r.frontend_replay).count();
    let frontend_tamper_rejections = receipts
        .iter()
        .filter(|r| r.frontend_tamper_rejected)
        .count();
    let execution_replays = receipts.iter().filter(|r| r.execution_replay).count();
    let execution_tamper_rejections = receipts
        .iter()
        .filter(|r| r.execution_tamper_rejected)
        .count();
    let false_authorizations = receipts.iter().filter(|r| r.false_authorization).count();
    let false_denials = receipts
        .iter()
        .filter(|r| r.expected == Expected::Supported && !r.exact)
        .count();
    let route_leakage = receipts
        .iter()
        .filter(|r| r.authorized_modules.len() > 1)
        .count();
    assert_eq!((supported, ambiguous, unsupported), (600, 200, 200));
    assert_eq!(exact_decisions, 1000);
    assert_eq!(authorized_supported, 600);
    assert_eq!(ambiguities_preserved, 200);
    assert_eq!(unsupported_refusals, 200);
    assert_eq!(frontend_replays, 1000);
    assert_eq!(frontend_tamper_rejections, 1000);
    assert_eq!(execution_replays, 600);
    assert_eq!(execution_tamper_rejections, 600);
    assert_eq!(false_authorizations, 0);
    assert_eq!(false_denials, 0);
    assert_eq!(route_leakage, 0);
    let report = Report {
        schema: "stage78-curriculum-frontend-route-blind-v1",
        corpus_sha256,
        cases,
        supported,
        ambiguous,
        unsupported,
        exact_decisions,
        authorized_supported,
        ambiguities_preserved,
        unsupported_refusals,
        frontend_replays,
        frontend_tamper_rejections,
        execution_replays,
        execution_tamper_rejections,
        false_authorizations,
        false_denials,
        route_leakage,
        receipts,
    };
    fs::write(REPORT_JSON, serde_json::to_string_pretty(&report)?)?;
    fs::write(REPORT_MD, format!("# Stage 78 — route-blind curriculum frontends\n\n- Cases: 1,000 (600 supported, 200 ambiguous, 200 unsupported)\n- Exact decisions: {exact_decisions}/1,000\n- Authorized supported routes: {authorized_supported}/600\n- Ambiguities preserved: {ambiguities_preserved}/200\n- Unsupported refusals: {unsupported_refusals}/200\n- Frontend replay/tamper: {frontend_replays}/1,000 and {frontend_tamper_rejections}/1,000\n- Execution replay/tamper: {execution_replays}/600 and {execution_tamper_rejections}/600 emitted routes\n- False authorizations / denials: {false_authorizations} / {false_denials}\n- Route leakage: {route_leakage}\n"))?;
    Ok(())
}
