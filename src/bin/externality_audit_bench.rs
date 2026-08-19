//! Goal 3 pilot: audit a frozen third-party release for contamination signals.
//!
//! The 100-case release is intentionally reported as a pilot.  It cannot
//! satisfy the long-run 3,000--5,000-question external-exam gate.

use std::{env, fs};

use serde::Serialize;
use the_machine::third_party_corpus_benchmark::{audit_externality, ThirdPartyCorpus};

const REPORT_JSON: &str = "docs/goal3_externality_audit_pilot.json";
const REPORT_MD: &str = "docs/goal3_externality_audit_pilot.md";

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    audit: the_machine::third_party_corpus_benchmark::ExternalityAudit,
    validation_errors: Vec<String>,
}

fn main() {
    let path = env::args()
        .nth(1)
        .unwrap_or_else(|| "data/third_party_gsm8k_restricted_release_v2.json".into());
    let corpus: ThirdPartyCorpus = serde_json::from_str(
        &fs::read_to_string(&path).expect("third-party release must be readable"),
    )
    .expect("third-party release must be valid JSON");
    let validation_errors = corpus.validation_errors();
    assert!(
        validation_errors.is_empty(),
        "invalid release: {validation_errors:?}"
    );
    let audit = audit_externality(&corpus);
    assert!(audit.source_independence);
    assert!(!audit.final_exam_eligible);
    assert_eq!(audit.verdict, "external_pilot_not_final_exam_eligible");
    let report = Report {
        schema: "goal3-externality-audit-pilot-v1",
        audit,
        validation_errors,
    };
    let serialized = serde_json::to_string_pretty(&report).unwrap();
    fs::write(REPORT_JSON, format!("{serialized}\n")).expect("write externality report");
    fs::write(
        REPORT_MD,
        format!(
            "# Goal 3 — externality audit pilot\n\n\
             - Release: `{}`\n\
             - Cases: {} (development {}, holdout {})\n\
             - Source independence: {}\n\
             - Exact lexical/template overlap: {}/{}\n\
             - Answer/development exposure: {}/{}\n\
             - Final-exam eligible: {}\n\
             - Verdict: `{}`\n\n\
             This is a 100-case third-party pilot. It is intentionally not\
             reported as the required 3,000–5,000-question external exam.\n",
            report.audit.release_id,
            report.audit.total_cases,
            report.audit.development_cases,
            report.audit.holdout_cases,
            report.audit.source_independence,
            report.audit.lexical_overlap_count,
            report.audit.template_overlap_count,
            report.audit.answer_exposure_count,
            report.audit.development_exposure_count,
            report.audit.final_exam_eligible,
            report.audit.verdict,
        ),
    )
    .expect("write externality markdown report");
    println!("{serialized}");
}
