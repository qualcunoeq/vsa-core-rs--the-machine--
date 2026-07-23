//! Run the independent proposition proof-kernel benchmark.

use std::fs;
use the_machine::proposition_ood_benchmark::{evaluate, PropositionOodCorpus};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let input = args
        .first()
        .cloned()
        .unwrap_or_else(|| "data/propositions_ood_v1.json".into());
    let output = args
        .get(1)
        .cloned()
        .unwrap_or_else(|| "/tmp/propositions_ood_v1_report.json".into());
    let corpus: PropositionOodCorpus = serde_json::from_str(&fs::read_to_string(&input)?)?;
    let errors = corpus.validation_errors();
    if !errors.is_empty() {
        return Err(format!("invalid proposition OOD corpus: {errors:?}").into());
    }
    let report = evaluate(&corpus);
    fs::write(&output, serde_json::to_vec_pretty(&report)?)?;
    let metrics = &report.metrics;
    eprintln!(
        "proposition-ood: cases={} decisions={:.3} accepted={} replay={} false_auth={} false_denials={}",
        report.corpus_cases,
        metrics.correct_decisions as f64 / metrics.cases.max(1) as f64,
        metrics.accepted,
        metrics.replay_verified,
        metrics.false_authorizations,
        metrics.false_denials,
    );
    eprintln!(
        "rewrites: pairs={} stable={} regressions={}",
        report.rewrites.pairs, report.rewrites.decision_stable, report.rewrites.rewrite_regressions,
    );
    eprintln!(
        "ablations: assumption_bypass_false_accepts={}/{} replay_tampered_rejections={}/{} replay_bypass_tampered_accepts={}",
        report.ablation.assumption_bypass_false_accepts,
        report.ablation.assumption_cases,
        report.ablation.replay_tampered_rejections,
        report.ablation.valid_cases,
        report.ablation.replay_bypass_tampered_accepts,
    );
    eprintln!("failure_taxonomy={:?}", metrics.failure_taxonomy);
    eprintln!("wrote {output}");
    Ok(())
}
