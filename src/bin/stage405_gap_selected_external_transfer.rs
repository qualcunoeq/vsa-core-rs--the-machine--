//! Stage 405: frozen answer-key-blind transfer against naturally authored data.
//!
//! The source-selected health/economics frontend is evaluated over the frozen
//! external question corpus using only question text.  This measures overlap
//! and safe reachability, not answer accuracy: oracle files are never opened,
//! and no result changes production routing.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use the_machine::source_formula_frontend::{formalize_source_formula_text, FrontendStatus};
use the_machine::source_formula_pack::{evaluate_formula_records, FormulaStatus};
use the_machine::source_module_discovery::discover_formula_corpus;

const CORPUS: &str = include_str!("../../data/external_math_exam_v1/questions.jsonl");
const HEALTH_SOURCE: &str =
    include_str!("../../docs/sources/openstax_bounded_health_ratios_source.txt");
const ECONOMICS_SOURCE: &str =
    include_str!("../../docs/sources/openstax_bounded_economics_source.txt");
const DOMAIN: &str = "source_derived_gap_selected_rational_expression";
const JSON: &str = "docs/stage405_gap_selected_external_transfer.json";
const MD: &str = "docs/stage405_gap_selected_external_transfer.md";

#[derive(Debug, Deserialize)]
struct Question {
    id: String,
    original_prompt: String,
    split: String,
}

#[derive(Debug, Serialize)]
struct SplitReport {
    questions: usize,
    candidate_formula_mentions: usize,
    complete_frontends: usize,
    ambiguous_frontends: usize,
    missing_frontends: usize,
    unsupported_frontends: usize,
    downstream_attempts: usize,
    downstream_complete: usize,
    downstream_replays: usize,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    corpus_sha256: String,
    source_records: usize,
    selected_source_ids: Vec<String>,
    answer_keys_read: usize,
    production_authorizations: usize,
    false_authorizations: usize,
    live_mutations: usize,
    development: SplitReport,
    validation: SplitReport,
    sealed: SplitReport,
    status_totals: BTreeMap<String, usize>,
}

fn hash_bytes(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn empty_split() -> SplitReport {
    SplitReport {
        questions: 0,
        candidate_formula_mentions: 0,
        complete_frontends: 0,
        ambiguous_frontends: 0,
        missing_frontends: 0,
        unsupported_frontends: 0,
        downstream_attempts: 0,
        downstream_complete: 0,
        downstream_replays: 0,
    }
}

fn split_mut<'a>(
    split: &str,
    reports: &'a mut BTreeMap<String, SplitReport>,
) -> &'a mut SplitReport {
    reports.entry(split.to_owned()).or_insert_with(empty_split)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let health = discover_formula_corpus(&[HEALTH_SOURCE], "health")
        .map_err(|errors| errors.join("; "))?
        .into_iter()
        .find(|module| module.candidate.source_ids[0].contains(":health:incidence"))
        .expect("selected health lineage");
    let economics = discover_formula_corpus(&[ECONOMICS_SOURCE], "economics")
        .map_err(|errors| errors.join("; "))?
        .into_iter()
        .find(|module| module.candidate.source_ids[0].ends_with(":revenue"))
        .expect("selected economics lineage");
    let records = health
        .records
        .iter()
        .chain(economics.records.iter())
        .cloned()
        .collect::<Vec<_>>();
    let selected_source_ids = records
        .iter()
        .map(|record| record.source.source_id.clone())
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let mut reports = BTreeMap::new();
    let mut status_totals = BTreeMap::new();
    for line in CORPUS.lines() {
        let question: Question = serde_json::from_str(line)?;
        let report = split_mut(&question.split, &mut reports);
        report.questions += 1;
        let frontend = formalize_source_formula_text(&question.original_prompt, DOMAIN, &records);
        *status_totals
            .entry(format!("{:?}", frontend.status))
            .or_insert(0) += 1;
        if frontend.formula_id.is_some() {
            report.candidate_formula_mentions += 1;
        }
        match frontend.status {
            FrontendStatus::Complete => report.complete_frontends += 1,
            FrontendStatus::Ambiguous => report.ambiguous_frontends += 1,
            FrontendStatus::Missing => report.missing_frontends += 1,
            FrontendStatus::Unsupported => report.unsupported_frontends += 1,
        }
        if let Some(request) = frontend.request {
            report.downstream_attempts += 1;
            let result = evaluate_formula_records(&request, DOMAIN, &records);
            if result.status == FormulaStatus::Complete {
                report.downstream_complete += 1;
            }
            if result.replay_verified() {
                report.downstream_replays += 1;
            }
        }
        let _ = question.id;
    }
    let development = reports.remove("development").unwrap_or_else(empty_split);
    let validation = reports.remove("validation").unwrap_or_else(empty_split);
    let sealed = reports.remove("sealed").unwrap_or_else(empty_split);
    let report = Report {
        schema: "stage405-gap-selected-external-transfer-v1",
        corpus_sha256: hash_bytes(CORPUS.as_bytes()),
        source_records: records.len(),
        selected_source_ids,
        answer_keys_read: 0,
        production_authorizations: 0,
        false_authorizations: 0,
        live_mutations: 0,
        development,
        validation,
        sealed,
        status_totals,
    };
    let serialized = serde_json::to_string_pretty(&report)?;
    fs::write(JSON, format!("{serialized}\n"))?;
    fs::write(
        MD,
        format!(
            "# Stage 405 — gap-selected external transfer\n\n- Corpus SHA-256: `{}`\n- Source records: {}\n- Development questions / candidate mentions / complete frontends: {} / {} / {}\n- Validation questions / candidate mentions / complete frontends: {} / {} / {}\n- Sealed questions / candidate mentions / complete frontends: {} / {} / {}\n- Answer keys read / production authorizations / false authorizations: {} / {} / {}\n- Live mutations: {}\n\nThis is a frozen reachability diagnostic over naturally authored prompts. It reads only `questions.jsonl`; oracle files are not opened. A complete frontend would still require downstream value validation before any authorization, and no authorization is emitted by this stage.\n",
            report.corpus_sha256,
            report.source_records,
            report.development.questions,
            report.development.candidate_formula_mentions,
            report.development.complete_frontends,
            report.validation.questions,
            report.validation.candidate_formula_mentions,
            report.validation.complete_frontends,
            report.sealed.questions,
            report.sealed.candidate_formula_mentions,
            report.sealed.complete_frontends,
            report.answer_keys_read,
            report.production_authorizations,
            report.false_authorizations,
            report.live_mutations,
        ),
    )?;
    println!("{serialized}");
    Ok(())
}
