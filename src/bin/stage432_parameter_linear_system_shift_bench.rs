//! Stage 432: shifted-language pressure testing for the parameter-system
//! frontend.  The corpus is generated independently of the frontend and
//! includes supported, incomplete, ambiguous, and out-of-scope forms.

use serde::Serialize;
use sha2::{Digest, Sha256};
use the_machine::curriculum::breadth_first_manifest;
use the_machine::parameter_linear_system_frontend::{
    execute, execution_replay_verified, formalize, replay_verified, FrontendStatus,
};
use the_machine::probability_pack::Rational;

#[derive(Clone)]
enum Expected {
    Supported(Rational),
    Missing,
    AmbiguousFrontend,
    AmbiguousExecution,
    Unsupported,
}

struct Case {
    text: String,
    expected: Expected,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    cases: usize,
    supported_cases: usize,
    missing_cases: usize,
    ambiguous_cases: usize,
    unsupported_cases: usize,
    exact_frontend_statuses: usize,
    exact_execution_statuses: usize,
    execution_emitted: usize,
    supported_values: usize,
    incorrect_values: usize,
    frontend_replay_verified: usize,
    frontend_tamper_rejected: usize,
    execution_replay_verified: usize,
    execution_tamper_rejected: usize,
    false_authorizations: usize,
    false_denials: usize,
    manifest_unchanged: bool,
    corpus_sha256: String,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn form(
    index: usize,
) -> (
    i128,
    i128,
    i128,
    i128,
    i128,
    i128,
    i128,
    i128,
    i128,
    Rational,
) {
    let x = (1 + (index % 7)) as i128;
    let y = (2 + (index % 5)) as i128;
    let a = (3 + (index % 11)) as i128;
    let p = (1 + (index % 4)) as i128;
    let q = (2 + (index % 3)) as i128;
    let r = 1i128;
    let s = (2 + (index % 4)) as i128;
    let t = 1i128;
    let u = 1i128;
    (x, y, a, p, q, r, s, t, u, Rational::new(a, 1).unwrap())
}

fn supported_cases() -> Vec<Case> {
    (0..120usize)
        .map(|index| {
            let (x, _y, _a, p, q, r, s, t, u, value) = form(index);
            let y = (2 + (index % 5)) as i128;
            let a = (3 + (index % 11)) as i128;
            let c = p * x + q * y - r * a;
            let d = s * x + t * y - u * a;
            let body = format!(
                "{p}*x+{q}*y={r}*a{c_sign}{c_abs}; {s}*x+{t}*y={u}*a{d_sign}{d_abs}",
                c_sign = if c >= 0 { "+" } else { "" },
                c_abs = c,
                d_sign = if d >= 0 { "+" } else { "" },
                d_abs = d,
            );
            let text = match index % 4 {
                0 => format!("The system is {body}, given x={x}, compute a."),
                1 => format!("The equations are {body}. Assuming x={x}, calculate a."),
                2 => format!("\\begin{{align*}}{body}\\end{{align*}} when x={x}, determine a"),
                _ => format!("The equations {body}; where x={x}, find a."),
            };
            Case {
                text,
                expected: Expected::Supported(value),
            }
        })
        .collect()
}

fn missing_cases() -> Vec<Case> {
    (0..40)
        .map(|index| {
            let text = match index % 4 {
                0 => "The equations are x+y=a; 2*x+y=2*a, compute a.".to_string(),
                1 => "The equations are x+y=a; 2*x+y=2*a, given x=2.".to_string(),
                2 => "A pair satisfies x+y=5 and x-y=1. Find a.".to_string(),
                _ => "Given 3*x+y=a and 2*x+5*y=2*a, determine the parameter.".to_string(),
            };
            Case {
                text,
                expected: Expected::Missing,
            }
        })
        .collect()
}

fn ambiguous_cases() -> Vec<Case> {
    (0..40)
        .map(|index| {
            let text = match index % 4 {
                0 => "The equations are x+y=a; 2*x+2*y=2*a, given x=2, compute a.".to_string(),
                1 => "The equations are x+y=a; 2*x+y=2*a, given z=2, compute a.".to_string(),
                2 => "The equations are x+y=a; 2*x+y=2*a, given x=2, compute c.".to_string(),
                _ => "The equations are x+y=a; 2*x+y=2*a, given z=2, compute a.".to_string(),
            };
            Case {
                text,
                expected: if index % 4 == 0 {
                    Expected::AmbiguousExecution
                } else {
                    Expected::AmbiguousFrontend
                },
            }
        })
        .collect()
}

fn unsupported_cases() -> Vec<Case> {
    (0..80)
        .map(|index| {
            let text = match index % 5 {
                0 => "The system is x^2+y=3; x-y=1, given x=2, compute a.".to_string(),
                1 => "The system is x*y=3; x+y=4, given x=2, compute a.".to_string(),
                2 => "Three equations x+y=3; y+z=4; z+x=5, given x=2, compute a.".to_string(),
                3 => "The system is x+y=a; 2*x+y=2*a, given x=2, compute a and b.".to_string(),
                _ => "The system is x+y<=3; x-y=1, given x=2, compute a.".to_string(),
            };
            Case {
                text,
                expected: Expected::Unsupported,
            }
        })
        .collect()
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut cases = supported_cases();
    cases.extend(missing_cases());
    cases.extend(ambiguous_cases());
    cases.extend(unsupported_cases());
    assert_eq!(cases.len(), 280);
    let corpus_sha256 = digest(&cases.iter().map(|case| &case.text).collect::<Vec<_>>());
    let before = breadth_first_manifest().replay_hash();
    let mut exact_frontend_statuses = 0;
    let mut exact_execution_statuses = 0;
    let mut execution_emitted = 0;
    let mut supported_values = 0;
    let mut incorrect_values = 0;
    let mut frontend_replay_verified_count = 0;
    let mut frontend_tamper_rejected_count = 0;
    let mut execution_replay_verified_count = 0;
    let mut execution_tamper_rejected_count = 0;
    let mut false_authorizations = 0;
    let mut false_denials = 0;
    for (index, case) in cases.iter().enumerate() {
        let frontend = formalize(&case.text, &format!("stage432-{index:03}"));
        frontend_replay_verified_count += usize::from(replay_verified(&frontend));
        let mut frontend_tampered = frontend.clone();
        frontend_tampered.replay_hash.push('x');
        frontend_tamper_rejected_count += usize::from(!replay_verified(&frontend_tampered));
        let (expected_frontend, expected_execution, expected_value) = match case.expected.clone() {
            Expected::Supported(value) => (
                FrontendStatus::Complete,
                Some(FrontendStatus::Complete),
                Some(value),
            ),
            Expected::Missing => (FrontendStatus::Missing, None, None),
            Expected::AmbiguousFrontend => (FrontendStatus::Ambiguous, None, None),
            Expected::AmbiguousExecution => (
                FrontendStatus::Complete,
                Some(FrontendStatus::Ambiguous),
                None,
            ),
            Expected::Unsupported => (FrontendStatus::Unsupported, None, None),
        };
        if frontend.status == expected_frontend {
            exact_frontend_statuses += 1;
        } else if matches!(case.expected, Expected::Supported(_)) {
            false_denials += 1;
        } else {
            false_authorizations += 1;
        }
        let Some(request) = frontend.request.as_ref() else {
            continue;
        };
        execution_emitted += 1;
        let execution = execute(request);
        execution_replay_verified_count += usize::from(execution_replay_verified(&execution));
        let mut execution_tampered = execution.clone();
        execution_tampered.replay_hash.push('x');
        execution_tamper_rejected_count +=
            usize::from(!execution_replay_verified(&execution_tampered));
        if expected_execution.is_some_and(|status| execution.status == status) {
            exact_execution_statuses += 1;
        } else if matches!(case.expected, Expected::Supported(_)) {
            false_denials += 1;
        } else if execution.status == FrontendStatus::Complete {
            false_authorizations += 1;
        }
        if let Some(expected_value) = expected_value {
            if execution.status == FrontendStatus::Complete
                && execution.value == Some(expected_value.clone())
            {
                supported_values += 1;
            } else {
                incorrect_values += 1;
            }
        }
    }
    let after = breadth_first_manifest().replay_hash();
    assert_eq!(before, after);
    let report = Report {
        schema: "stage432-parameter-linear-system-shift-bench-v1",
        cases: cases.len(),
        supported_cases: 120,
        missing_cases: 40,
        ambiguous_cases: 40,
        unsupported_cases: 80,
        exact_frontend_statuses,
        exact_execution_statuses,
        execution_emitted,
        supported_values,
        incorrect_values,
        frontend_replay_verified: frontend_replay_verified_count,
        frontend_tamper_rejected: frontend_tamper_rejected_count,
        execution_replay_verified: execution_replay_verified_count,
        execution_tamper_rejected: execution_tamper_rejected_count,
        false_authorizations,
        false_denials,
        manifest_unchanged: before == after,
        corpus_sha256,
    };
    std::fs::write(
        "docs/stage432_parameter_linear_system_shift_bench.json",
        serde_json::to_vec_pretty(&report)?,
    )?;
    std::fs::write(
        "docs/stage432_parameter_linear_system_shift_bench.md",
        format!(
            "# Stage 432 — shifted parameterized linear-system benchmark\n\n- cases / supported / missing / ambiguous / unsupported: {} / 120 / 40 / 40 / 80\n- exact frontend statuses: {}/{}\n- execution emitted / exact statuses: {} / {}/{}\n- supported values / incorrect values: {}/120 / {}\n- frontend replay / tamper: {}/{} / {}/{}\n- execution replay / tamper: {}/{} / {}/{}\n- false authorizations / false denials: {} / {}\n- manifest unchanged: {}\n- corpus SHA-256: `{}`\n",
            report.cases,
            report.exact_frontend_statuses,
            report.cases,
            report.execution_emitted,
            report.exact_execution_statuses,
            report.execution_emitted,
            report.supported_values,
            report.incorrect_values,
            report.frontend_replay_verified,
            report.cases,
            report.frontend_tamper_rejected,
            report.cases,
            report.execution_replay_verified,
            report.execution_emitted,
            report.execution_tamper_rejected,
            report.execution_emitted,
            report.false_authorizations,
            report.false_denials,
            report.manifest_unchanged,
            report.corpus_sha256,
        ),
    )?;
    println!(
        "Stage 432 — exact_frontend={}/{} values={}/120 false_auth={} false_denial={}",
        report.exact_frontend_statuses,
        report.cases,
        report.supported_values,
        report.false_authorizations,
        report.false_denials,
    );
    Ok(())
}
