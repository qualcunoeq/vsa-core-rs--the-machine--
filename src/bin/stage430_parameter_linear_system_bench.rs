//! Stage 430: independent validation of the parameterized linear-system frontend.

use serde::Serialize;
use sha2::{Digest, Sha256};
use the_machine::curriculum::breadth_first_manifest;
use the_machine::parameter_linear_system_frontend::{
    execute, execution_replay_verified as execution_replay_ok, formalize, replay_verified,
    FrontendStatus,
};
use the_machine::probability_pack::Rational;

#[derive(Clone, Copy)]
enum Expected {
    Supported,
    Missing,
    Unsupported,
}

struct Case {
    text: String,
    expected: Expected,
    value: Option<Rational>,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    cases: usize,
    supported_cases: usize,
    missing_cases: usize,
    unsupported_cases: usize,
    exact_statuses: usize,
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

fn supported_cases() -> Vec<Case> {
    (0..120u32)
        .map(|index| {
            let x = (1 + index % 5) as i128;
            let y = (2 + index % 7) as i128;
            let a = (3 + index % 11) as i128;
            let p = (1 + index % 4) as i128;
            let c = p * x + 2 * y - a;
            let d = 2 * x + y - a;
            let body = format!(
                "{}*x+2*y=1*a{}; 2*x+y=1*a{}",
                p,
                if c >= 0 {
                    format!("+{c}")
                } else {
                    c.to_string()
                },
                if d >= 0 {
                    format!("+{d}")
                } else {
                    d.to_string()
                },
            );
            let text = if index % 3 == 0 {
                format!(
                    r"\begin{{align*}}{}\\\end{{align*}} when x={}, compute a",
                    body.replace(';', "\\\\"),
                    x
                )
            } else {
                format!("The system is {body}, given x={x}, compute a.")
            };
            Case {
                text,
                expected: Expected::Supported,
                value: Rational::new(a, 1),
            }
        })
        .collect()
}

fn missing_cases() -> Vec<Case> {
    (0..40)
        .map(|index| {
            let text = match index % 4 {
                0 => "The system is x+y=a; 2*x+y=2*a, compute a.".into(),
                1 => "The system is x+y=a; 2*x+y=2*a, given x=2.".into(),
                2 => "A pair satisfies x+y=5 and x-y=1. Find a.".into(),
                _ => "Given 3*x+y=a and 2*x+5*y=2*a, determine the parameter.".into(),
            };
            Case {
                text,
                expected: Expected::Missing,
                value: None,
            }
        })
        .collect()
}

fn unsupported_cases() -> Vec<Case> {
    (0..80)
        .map(|index| {
            let text = match index % 5 {
                0 => "The system is x^2+y=3; x-y=1, given x=2, compute a.".into(),
                1 => "The system is x*y=3; x+y=4, given x=2, compute a.".into(),
                2 => "Three equations x+y=3; y+z=4; z+x=5, given x=2, compute a.".into(),
                3 => "The system is x+y=a; 2*x+y=2*a, given x=2, compute a and b.".into(),
                _ => "The system is x+y<=3; x-y=1, given x=2, compute a.".into(),
            };
            Case {
                text,
                expected: Expected::Unsupported,
                value: None,
            }
        })
        .collect()
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut cases = supported_cases();
    cases.extend(missing_cases());
    cases.extend(unsupported_cases());
    assert_eq!(cases.len(), 240);
    let corpus_sha256 = digest(&cases.iter().map(|case| &case.text).collect::<Vec<_>>());
    let before = breadth_first_manifest().replay_hash();
    let mut exact = 0;
    let mut supported_values = 0;
    let mut incorrect_values = 0;
    let mut frontend_replay_verified = 0;
    let mut frontend_tamper_rejected = 0;
    let mut execution_replay_verified = 0;
    let mut execution_tamper_rejected = 0;
    let mut false_authorizations = 0;
    let mut false_denials = 0;
    for (index, case) in cases.iter().enumerate() {
        let frontend = formalize(&case.text, &format!("stage430-{index:03}"));
        frontend_replay_verified += usize::from(replay_verified(&frontend));
        let mut tampered = frontend.clone();
        tampered.replay_hash.push('x');
        frontend_tamper_rejected += usize::from(!replay_verified(&tampered));
        let expected_status = match case.expected {
            Expected::Supported => FrontendStatus::Complete,
            Expected::Missing => FrontendStatus::Missing,
            Expected::Unsupported => FrontendStatus::Unsupported,
        };
        if frontend.status == expected_status {
            exact += 1;
        } else if matches!(case.expected, Expected::Supported) {
            false_denials += 1;
        } else {
            false_authorizations += 1;
        }
        if matches!(case.expected, Expected::Supported) {
            let Some(request) = frontend.request.as_ref() else {
                incorrect_values += 1;
                continue;
            };
            let execution = execute(request);
            execution_replay_verified += usize::from(execution_replay_ok(&execution));
            let mut tampered_execution = execution.clone();
            tampered_execution.replay_hash.push('x');
            execution_tamper_rejected += usize::from(!execution_replay_ok(&tampered_execution));
            if execution.status == FrontendStatus::Complete && execution.value == case.value {
                supported_values += 1;
            } else {
                incorrect_values += 1;
            }
        }
    }
    let after = breadth_first_manifest().replay_hash();
    assert_eq!(before, after);
    let report = Report {
        schema: "stage430-parameter-linear-system-bench-v1",
        cases: cases.len(),
        supported_cases: 120,
        missing_cases: 40,
        unsupported_cases: 80,
        exact_statuses: exact,
        supported_values,
        incorrect_values,
        frontend_replay_verified,
        frontend_tamper_rejected,
        execution_replay_verified,
        execution_tamper_rejected,
        false_authorizations,
        false_denials,
        manifest_unchanged: before == after,
        corpus_sha256,
    };
    std::fs::write(
        "docs/stage430_parameter_linear_system_bench.json",
        serde_json::to_vec_pretty(&report)?,
    )?;
    std::fs::write(
        "docs/stage430_parameter_linear_system_bench.md",
        format!(
            "# Stage 430 — parameterized linear-system frontend\n\n- cases / supported / missing / unsupported: 240 / 120 / 40 / 80\n- exact statuses: {}/240\n- supported values / incorrect values: {}/120 / {}\n- frontend replay / tamper: {}/240 / {}/240\n- execution replay / tamper: {}/120 / {}/120\n- false authorizations / false denials: {} / {}\n- manifest unchanged: {}\n- corpus SHA-256: `{}`\n",
            report.exact_statuses,
            report.supported_values,
            report.incorrect_values,
            report.frontend_replay_verified,
            report.frontend_tamper_rejected,
            report.execution_replay_verified,
            report.execution_tamper_rejected,
            report.false_authorizations,
            report.false_denials,
            report.manifest_unchanged,
            report.corpus_sha256,
        ),
    )?;
    println!(
        "Stage 430 — exact={}/240 values={}/120 replay={}/240 execution_replay={}/120 false_auth={}",
        report.exact_statuses,
        report.supported_values,
        report.frontend_replay_verified,
        report.execution_replay_verified,
        report.false_authorizations,
    );
    Ok(())
}
