//! Independent benchmark for the bounded LaTeX frequency-table frontend.

use serde::Serialize;
use sha2::{Digest, Sha256};
use the_machine::source_frequency_table_frontend::{
    formalize_frequency_table_text, FrequencyTableResult, FrequencyTableStatus,
};

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum Expected {
    Supported,
    Refused,
}

#[derive(Debug, Serialize)]
struct Receipt {
    id: String,
    expected: Expected,
    actual: FrequencyTableStatus,
    exact_decision: bool,
    value: Option<String>,
    replay_verified: bool,
    tamper_rejected: bool,
    false_authorization: bool,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    corpus_sha256: String,
    cases: usize,
    supported: usize,
    refused: usize,
    exact_decisions: usize,
    supported_values: usize,
    frontend_replay_verified: usize,
    tamper_rejections: usize,
    false_authorizations: usize,
    false_denials: usize,
    receipts: Vec<Receipt>,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn supported_table(index: usize, reverse_headers: bool) -> String {
    let v1 = 10 + (index % 7) as i32;
    let v2 = 20 + (index % 11) as i32;
    let v3 = 30 + (index % 13) as i32;
    let f1 = 1 + (index % 5) as i32;
    let f2 = 2 + (index % 4) as i32;
    let f3 = 1 + (index % 3) as i32;
    if reverse_headers {
        format!(
            r#"Find the weighted average.
\begin{{tabular}}{{|c|c|}}
\hline
Number of observations & Value \\
\hline
{f1} & {v1} \\
{f2} & {v2} \\
{f3} & {v3} \\
\hline
\end{{tabular}}"#
        )
    } else {
        format!(
            r#"Find the average value.
\begin{{tabular}}{{|c|c|}}
\hline
Value & Frequency \\
\hline
{v1} & {f1} \\
{v2} & {f2} \\
{v3} & {f3} \\
\hline
\end{{tabular}}"#
        )
    }
}

fn run(id: String, text: String, expected: Expected) -> Receipt {
    let frontend: FrequencyTableResult = formalize_frequency_table_text(&text);
    let exact_decision = match expected {
        Expected::Supported => frontend.status == FrequencyTableStatus::Complete,
        Expected::Refused => frontend.status != FrequencyTableStatus::Complete,
    };
    let replay_verified = frontend.replay_verified();
    let mut tampered = frontend.clone();
    tampered.replay_hash.push('x');
    let tamper_rejected = !tampered.replay_verified();
    let authorized = frontend.status == FrequencyTableStatus::Complete
        && frontend.statistics.as_ref().is_some_and(|result| {
            result.status == the_machine::source_formula_pack::FormulaStatus::Complete
                && result.value.is_some()
                && result.replay_verified()
        });
    let value = frontend
        .statistics
        .as_ref()
        .and_then(|result| result.value.as_ref())
        .map(|value| format!("{}/{}", value.numerator, value.denominator));
    Receipt {
        id,
        expected,
        actual: frontend.status,
        exact_decision,
        value,
        replay_verified,
        tamper_rejected,
        false_authorization: expected == Expected::Refused && authorized,
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut receipts = Vec::with_capacity(240);
    for index in 0..120 {
        receipts.push(run(
            format!("supported_{index:03}"),
            supported_table(index, index % 2 == 1),
            Expected::Supported,
        ));
    }
    for index in 0..20 {
        receipts.push(run(
            format!("ambiguous_headers_{index:03}"),
            "Find the average. \\begin{tabular}{|c|c|} Value & Score \\\\ 1 & 2 \\\\ 3 & 4 \\\\ \\end{tabular}".into(),
            Expected::Refused,
        ));
        receipts.push(run(
            format!("ambiguous_rows_{index:03}"),
            "Find the mean. \\begin{tabular}{|c|c|} Value & Frequency \\\\ 1 & \\\\ 3 & 2 \\\\ \\end{tabular}".into(),
            Expected::Refused,
        ));
    }
    for index in 0..20 {
        receipts.push(run(
            format!("unsupported_no_target_{index:03}"),
            "The table lists values and frequencies. \\begin{tabular}{|c|c|} Value & Frequency \\\\ 1 & 2 \\\\ 3 & 4 \\\\ \\end{tabular}".into(),
            Expected::Refused,
        ));
        receipts.push(run(
            format!("unsupported_frequency_{index:03}"),
            "Find the average. \\begin{tabular}{|c|c|} Value & Frequency \\\\ 1 & 0 \\\\ 3 & 2 \\\\ \\end{tabular}".into(),
            Expected::Refused,
        ));
        receipts.push(run(
            format!("unsupported_non_table_{index:03}"),
            "Find the average of a continuous distribution.".into(),
            Expected::Refused,
        ));
        receipts.push(run(
            format!("unsupported_three_columns_{index:03}"),
            "Find the average. \\begin{tabular}{|c|c|c|} Value & Frequency & Unit \\\\ 1 & 2 & kg \\\\ 3 & 4 & kg \\\\ \\end{tabular}".into(),
            Expected::Refused,
        ));
    }
    assert_eq!(receipts.len(), 240);
    let supported = receipts
        .iter()
        .filter(|receipt| receipt.expected == Expected::Supported)
        .count();
    let refused = receipts.len() - supported;
    let exact_decisions = receipts
        .iter()
        .filter(|receipt| receipt.exact_decision)
        .count();
    let supported_values = receipts
        .iter()
        .filter(|receipt| receipt.expected == Expected::Supported && receipt.value.is_some())
        .count();
    let frontend_replay_verified = receipts
        .iter()
        .filter(|receipt| receipt.replay_verified)
        .count();
    let tamper_rejections = receipts
        .iter()
        .filter(|receipt| receipt.tamper_rejected)
        .count();
    let false_authorizations = receipts
        .iter()
        .filter(|receipt| receipt.false_authorization)
        .count();
    let false_denials = receipts
        .iter()
        .filter(|receipt| receipt.expected == Expected::Supported && !receipt.exact_decision)
        .count();
    let mut report = Report {
        schema: "stage448-frequency-table-frontend-bench-v1",
        corpus_sha256: String::new(),
        cases: receipts.len(),
        supported,
        refused,
        exact_decisions,
        supported_values,
        frontend_replay_verified,
        tamper_rejections,
        false_authorizations,
        false_denials,
        receipts,
    };
    report.corpus_sha256 = digest(&report.receipts);
    assert_eq!(report.exact_decisions, 240);
    assert_eq!(report.supported_values, 120);
    assert_eq!(report.frontend_replay_verified, 240);
    assert_eq!(report.tamper_rejections, 240);
    assert_eq!(report.false_authorizations, 0);
    assert_eq!(report.false_denials, 0);
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
