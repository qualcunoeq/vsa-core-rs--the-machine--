//! Run the independent prose-recurrence benchmark.

use std::fs;
use the_machine::prose_recurrence_benchmark::{evaluate, ProseRecurrenceCorpus};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let input = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "data/recurrence_ood_v1.json".into());
    let output = std::env::args()
        .nth(2)
        .unwrap_or_else(|| "/tmp/recurrence_ood_v1_report.json".into());
    let corpus: ProseRecurrenceCorpus = serde_json::from_str(&fs::read_to_string(input)?)?;
    let report = evaluate(&corpus);
    println!(
        "recurrence-ood: cases={} authorized={} correct_answers={} replay={} tampered_rejected={} false_auth={} false_denials={}",
        report.cases,
        report.metrics.authorized,
        report.metrics.correct_answers,
        report.metrics.replay_verified,
        report.metrics.tampered_receipts_rejected,
        report.metrics.false_authorizations,
        report.metrics.false_denials,
    );
    println!(
        "rewrites: pairs={} decision_stable={} answer_stable={} regressions={}",
        report.rewrites.pairs,
        report.rewrites.decision_stable,
        report.rewrites.answer_stable,
        report.rewrites.regressions,
    );
    fs::write(output, serde_json::to_vec_pretty(&report)?)?;
    Ok(())
}
