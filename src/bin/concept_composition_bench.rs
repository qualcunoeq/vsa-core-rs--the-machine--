//! Run the bounded validated-concept composition benchmark.

use std::io::Write;
use the_machine::concept_composition_benchmark::{
    evaluate, evaluate_budget_sweep_with_stages,
};

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
    let budget_output = args
        .get(2)
        .cloned()
        .unwrap_or_else(|| format!("{output}.budget.json"));
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
    let budget_report = evaluate_budget_sweep_with_stages(4, 5, 5, &[1, 16, 64, 256, 1024]);
    let mut budget_file = std::fs::File::create(&budget_output)?;
    writeln!(
        budget_file,
        "{}",
        serde_json::to_string_pretty(&budget_report)?
    )?;
    for budget in &budget_report.budgets {
        eprintln!(
            "larger branches={} stages={}: budget={} proposals={}/{} nodes={} pruned={} subset={} nested={}",
            budget_report.branches_per_stage,
            budget_report.stage_count,
            budget.budget,
            budget.budgeted_proposals,
            budget.full_proposals,
            budget.nodes_visited,
            budget.candidates_pruned,
            budget.frontier_subset,
            budget.nested_with_previous
        );
    }
    eprintln!(
        "larger concepts={} stages={} max_concepts={} full_proposals={} deterministic={} diagnostic_only={} output={}",
        budget_report.graph_concepts,
        budget_report.stage_count,
        budget_report.max_concepts,
        budget_report.full_proposals,
        budget_report.deterministic,
        budget_report.diagnostic_only,
        budget_output
    );
    Ok(())
}
