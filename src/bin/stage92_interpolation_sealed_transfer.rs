//! Stage 92: sealed transfer checkpoint for the newly acquired interpolation
//! domain.  The prior 5,000-case curriculum exam remains immutable; this
//! checkpoint adds an independently authored development/validation/sealed
//! partition for the new source-derived capability.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use the_machine::curriculum::breadth_first_manifest;
use the_machine::source_formula_pack::{
    evaluate_formula_records, extract_formula_records, FormulaStatus,
};
use the_machine::source_interpolation_frontend::{
    formalize_interpolation_text, replay_verified, InterpolationFrontendStatus,
};

const SOURCE: &str = include_str!("../../docs/sources/openstax_linear_interpolation_catalog.txt");
const DOMAIN: &str = "source_catalog_linear_interpolation";
const PRIOR_CHECKPOINT: &str = include_str!("../../docs/stage_k_sealed_curriculum_exam_5000.json");

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
enum Hidden {
    Supported,
    Ambiguous,
    Unsupported,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
enum Partition {
    Development,
    Validation,
    Sealed,
}

#[derive(Debug, Clone, Serialize)]
struct Question {
    id: String,
    text: String,
    hidden: Hidden,
    partition: Partition,
}

#[derive(Debug, Serialize)]
struct Receipt {
    id: String,
    partition: Partition,
    hidden: Hidden,
    actual_status: String,
    authorized: bool,
    replay_verified: bool,
    tamper_rejected: bool,
    provenance_preserved: bool,
    false_authorization: bool,
    false_denial: bool,
    text_sha256: String,
}

#[derive(Debug, Serialize)]
struct PartitionMetrics {
    cases: usize,
    supported: usize,
    ambiguous: usize,
    unsupported: usize,
    supported_authorized: usize,
    ambiguities_preserved: usize,
    unsupported_refused: usize,
    replay_verified: usize,
    tamper_rejections: usize,
    false_authorizations: usize,
    false_denials: usize,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    source_id: &'static str,
    source_sha256: String,
    catalog_sha256: String,
    prior_checkpoint_sha256: String,
    question_corpus_sha256: String,
    sealed_question_sha256: String,
    cases: usize,
    supported: usize,
    ambiguous: usize,
    unsupported: usize,
    supported_authorized: usize,
    ambiguities_preserved: usize,
    unsupported_refused: usize,
    replay_verified: usize,
    tamper_rejections: usize,
    provenance_preserved: usize,
    false_authorizations: usize,
    false_denials: usize,
    manifest_mutated: bool,
    partitions: BTreeMap<String, PartitionMetrics>,
    receipts: Vec<Receipt>,
}

fn digest<T: Serialize + ?Sized>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn partition(global: usize) -> Partition {
    if global < 360 {
        Partition::Development
    } else if global < 480 {
        Partition::Validation
    } else {
        Partition::Sealed
    }
}

fn hidden(local: usize) -> Hidden {
    match local % 10 {
        0..=5 => Hidden::Supported,
        6..=7 => Hidden::Ambiguous,
        _ => Hidden::Unsupported,
    }
}

fn question(global: usize) -> Question {
    let local = global % 120;
    let hidden = hidden(local);
    let text = match hidden {
        Hidden::Supported => {
            let x1 = 1 + local % 4;
            let x2 = x1 + 6 + local % 5;
            let target = x1 + (x2 - x1) / 2;
            let y1 = 4 + local % 9;
            let y2 = y1 + 8 + local % 11;
            match local % 3 {
                0 => format!("Using endpoints x1={x1},y1={y1} and x2={x2},y2={y2}, linearly interpolate the value at x={target}."),
                1 => format!("Estimate by linear interpolation with x1={x1}, y1={y1}, x2={x2}, y2={y2}, target x={target}."),
                _ => format!("The linear model joins x1={x1},y1={y1} to x2={x2},y2={y2}; find y at x={target} by interpolation."),
            }
        }
        Hidden::Ambiguous => match local % 2 {
            0 => "Linearly interpolate at x=5 or x=6 between x1=1,y1=4 and x2=11,y2=24.".into(),
            _ => "Interpolate or extrapolate at x=6 between x1=1,y1=4 and x2=11,y2=24.".into(),
        },
        Hidden::Unsupported => match local % 4 {
            0 => "Linearly interpolate at x=20 between x1=1,y1=4 and x2=11,y2=24.".into(),
            1 => "Use a quadratic spline at x=6 through x1=1,y1=4 and x2=11,y2=24.".into(),
            2 => "Linearly interpolate at x=6 between x1=4,y1=4 and x2=4,y2=24.".into(),
            _ => "Infer an unknown point from an unstated interpolation model.".into(),
        },
    };
    Question {
        id: format!("interpolation_transfer_{global:04}"),
        text,
        hidden,
        partition: partition(global),
    }
}

fn run(question: &Question, records: &[the_machine::source_formula_pack::FormulaRecord]) -> Receipt {
    let frontend = formalize_interpolation_text(&question.text, &question.id);
    let downstream = frontend
        .request
        .as_ref()
        .map(|request| evaluate_formula_records(request, DOMAIN, records));
    let actual_supported = frontend.status == InterpolationFrontendStatus::Complete
        && downstream.as_ref().is_some_and(|result| {
            result.status == FormulaStatus::Complete && result.value.is_some()
        });
    let actual_status = if actual_supported {
        "supported"
    } else if frontend.status == InterpolationFrontendStatus::Ambiguous {
        "ambiguous"
    } else if frontend.status == InterpolationFrontendStatus::Unsupported {
        "unsupported"
    } else {
        "unparsed"
    };
    let authorized = actual_supported
        && downstream.as_ref().is_some_and(|result| result.replay_verified());
    let mut frontend_tampered = frontend.clone();
    frontend_tampered.replay_hash.push('x');
    let downstream_replay = downstream.as_ref().is_none_or(|result| result.replay_verified());
    let downstream_tamper = downstream.as_ref().is_none_or(|result| {
        let mut copy = result.clone();
        copy.replay_hash.push('x');
        !copy.replay_verified()
    });
    let replay = replay_verified(&frontend) && downstream_replay;
    let tamper = !replay_verified(&frontend_tampered) && downstream_tamper;
    Receipt {
        id: question.id.clone(),
        partition: question.partition,
        hidden: question.hidden,
        actual_status: actual_status.into(),
        authorized,
        replay_verified: replay,
        tamper_rejected: tamper,
        provenance_preserved: !frontend.provenance.is_empty()
            && downstream.as_ref().is_none_or(|result| !result.provenance.is_empty()),
        false_authorization: question.hidden != Hidden::Supported && authorized,
        false_denial: question.hidden == Hidden::Supported && !authorized,
        text_sha256: digest(&question.text),
    }
}

fn partition_metrics(receipts: &[Receipt], partition: Partition) -> PartitionMetrics {
    let rows: Vec<_> = receipts.iter().filter(|row| row.partition == partition).collect();
    PartitionMetrics {
        cases: rows.len(),
        supported: rows.iter().filter(|row| row.hidden == Hidden::Supported).count(),
        ambiguous: rows.iter().filter(|row| row.hidden == Hidden::Ambiguous).count(),
        unsupported: rows.iter().filter(|row| row.hidden == Hidden::Unsupported).count(),
        supported_authorized: rows.iter().filter(|row| row.hidden == Hidden::Supported && row.authorized).count(),
        ambiguities_preserved: rows.iter().filter(|row| row.hidden == Hidden::Ambiguous && row.actual_status == "ambiguous").count(),
        unsupported_refused: rows.iter().filter(|row| row.hidden == Hidden::Unsupported && row.actual_status == "unsupported").count(),
        replay_verified: rows.iter().filter(|row| row.replay_verified).count(),
        tamper_rejections: rows.iter().filter(|row| row.tamper_rejected).count(),
        false_authorizations: rows.iter().filter(|row| row.false_authorization).count(),
        false_denials: rows.iter().filter(|row| row.false_denial).count(),
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let records = extract_formula_records(SOURCE).map_err(|errors| errors.join("; "))?;
    let questions: Vec<_> = (0..600).map(question).collect();
    let manifest_before = breadth_first_manifest().replay_hash();
    let mut receipts = Vec::with_capacity(questions.len());
    for question in &questions {
        receipts.push(run(question, &records));
    }
    let manifest_mutated = breadth_first_manifest().replay_hash() != manifest_before;
    let cases = receipts.len();
    let supported = receipts.iter().filter(|row| row.hidden == Hidden::Supported).count();
    let ambiguous = receipts.iter().filter(|row| row.hidden == Hidden::Ambiguous).count();
    let unsupported = receipts.iter().filter(|row| row.hidden == Hidden::Unsupported).count();
    let supported_authorized = receipts.iter().filter(|row| row.hidden == Hidden::Supported && row.authorized).count();
    let ambiguities_preserved = receipts.iter().filter(|row| row.hidden == Hidden::Ambiguous && row.actual_status == "ambiguous").count();
    let unsupported_refused = receipts.iter().filter(|row| row.hidden == Hidden::Unsupported && row.actual_status == "unsupported").count();
    let replay_verified = receipts.iter().filter(|row| row.replay_verified).count();
    let tamper_rejections = receipts.iter().filter(|row| row.tamper_rejected).count();
    let provenance_preserved = receipts.iter().filter(|row| row.provenance_preserved).count();
    let false_authorizations = receipts.iter().filter(|row| row.false_authorization).count();
    let false_denials = receipts.iter().filter(|row| row.false_denial).count();
    assert_eq!((cases, supported, ambiguous, unsupported), (600, 360, 120, 120));
    assert_eq!(supported_authorized, supported);
    assert_eq!(ambiguities_preserved, ambiguous);
    assert_eq!(unsupported_refused, unsupported);
    assert_eq!(replay_verified, cases);
    assert_eq!(tamper_rejections, cases);
    assert_eq!(provenance_preserved, cases);
    assert_eq!(false_authorizations, 0);
    assert_eq!(false_denials, 0);
    assert!(!manifest_mutated);
    let mut partitions = BTreeMap::new();
    partitions.insert("development".into(), partition_metrics(&receipts, Partition::Development));
    partitions.insert("validation".into(), partition_metrics(&receipts, Partition::Validation));
    partitions.insert("sealed".into(), partition_metrics(&receipts, Partition::Sealed));
    let report = Report {
        schema: "stage92-interpolation-sealed-transfer-v1",
        source_id: "openstax-precalculus-2e:linear-functions",
        source_sha256: digest(SOURCE),
        catalog_sha256: digest(&records),
        prior_checkpoint_sha256: digest(PRIOR_CHECKPOINT),
        question_corpus_sha256: digest(&questions),
        sealed_question_sha256: digest(&questions.iter().filter(|question| question.partition == Partition::Sealed).collect::<Vec<_>>()),
        cases,
        supported,
        ambiguous,
        unsupported,
        supported_authorized,
        ambiguities_preserved,
        unsupported_refused,
        replay_verified,
        tamper_rejections,
        provenance_preserved,
        false_authorizations,
        false_denials,
        manifest_mutated,
        partitions,
        receipts,
    };
    let serialized = serde_json::to_string_pretty(&report)?;
    std::fs::write("docs/stage92_interpolation_sealed_transfer.json", format!("{serialized}\n"))?;
    println!("{serialized}");
    Ok(())
}
