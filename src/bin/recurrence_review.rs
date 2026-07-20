//! Emit the manual review of the four recurrence-shaped HLE rows.
//! This is evidence/reporting only; it never registers or executes a method.

use std::fs;
use the_machine::recurrence::{reviewed_hle_candidates, RecurrenceCandidateReview};

fn markdown(rows: &[RecurrenceCandidateReview]) -> String {
    let mut out = String::from(
        "# Recurrence candidate review\n\nThis report is a manual review of the four rows grouped by the heuristic recurrence miner. It is not an execution authorization.\n\n| Question ID | Actual task | Recurrence supplied | Initial conditions | Target | Order | Linearity | One-step | Verifier | Eligible | Missing representation | Review note |\n|---|---|---:|---:|---|---:|---|---:|---:|---:|---|---|\n",
    );
    for row in rows {
        let target = format!("{:?}", row.requested_index_or_property);
        let gap = row
            .smallest_missing_representation
            .map(|value| format!("{:?}", value))
            .unwrap_or_else(|| "none".to_string());
        out.push_str(&format!(
            "| {} | {:?} | {} | {} | {} | {} | {:?} | {} | {} | {} | {} | {} |\n",
            row.question_id,
            row.actual_task,
            row.recurrence_supplied_explicitly,
            row.initial_conditions_supplied,
            target,
            row.recurrence_order,
            row.linearity,
            row.one_step_sufficient,
            row.deterministic_verifier_available,
            row.eligible,
            gap,
            row.review_note
        ));
    }
    out.push_str(
        "\nConclusion: no row is eligible for the bounded first-order explicit-affine recurrence contract. The runtime recurrence registry remains empty.\n",
    );
    out
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rows = reviewed_hle_candidates();
    fs::write(
        "docs/recurrence_candidate_reviews_20260720.json",
        serde_json::to_string_pretty(&rows)?,
    )?;
    fs::write(
        "docs/recurrence_candidate_reviews_20260720.md",
        markdown(&rows),
    )?;
    println!(
        "reviewed={} eligible={} recurrence_supplied={}",
        rows.len(),
        rows.iter().filter(|row| row.eligible).count(),
        rows.iter()
            .filter(|row| row.recurrence_supplied_explicitly)
            .count()
    );
    Ok(())
}
