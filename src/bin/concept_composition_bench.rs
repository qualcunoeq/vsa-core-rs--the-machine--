//! Run the bounded validated-concept composition benchmark.

use std::io::Write;
use the_machine::concept_composition_benchmark::evaluate;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let max_depth = args
        .first()
        .map(|value| value.parse::<usize>())
        .transpose()?
        .unwrap_or(5);
    let output = args
        .get(1)
        .cloned()
        .unwrap_or_else(|| "/tmp/concept_composition_bench.json".into());
    let report = evaluate(max_depth);
    let mut file = std::fs::File::create(&output)?;
    writeln!(file, "{}", serde_json::to_string_pretty(&report)?)?;
    for depth in &report.depths {
        eprintln!(
            "depth={}: proposals={} rejections={} routes={:?} bound={} budget={} budgeted_proposals={} nodes={} pruned={} frontier_preserved={}",
            depth.max_concepts,
            depth.proposals,
            depth.rejections,
            depth.route_lengths,
            depth.theoretical_path_bound,
            depth.candidate_budget,
            depth.budgeted_proposals,
            depth.budgeted_nodes_visited,
            depth.budgeted_candidates_pruned,
            depth.full_budget_frontier_preserved
        );
    }
    eprintln!(
        "concepts={} max_depth={} deterministic={} diagnostic_only={} output={}",
        report.graph_concepts,
        report.requested_max_depth,
        report.deterministic,
        report.diagnostic_only,
        output
    );
    Ok(())
}
