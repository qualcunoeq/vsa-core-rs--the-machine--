//! Stage 126: controlled technical-language ingestion for simplicial homology.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use the_machine::simplicial_homology_frontend::{formalize, FrontendResult, FrontendStatus};
use the_machine::simplicial_homology_pack::{evaluate, HomologyStatus};

#[derive(Debug, Clone, Serialize)]
struct Case {
    id: String,
    expected: FrontendStatus,
    text: String,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    corpus_sha256: String,
    cases: usize,
    supported: usize,
    ambiguous: usize,
    unsupported: usize,
    exact_frontend_decisions: usize,
    frontend_replay_verified: usize,
    frontend_tamper_rejected: usize,
    downstream_authorized: usize,
    downstream_replay_verified: usize,
    downstream_tamper_rejected: usize,
    false_authorizations: usize,
    false_denials: usize,
    statuses: BTreeMap<String, usize>,
}

fn digest<T: serde::Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn shape(index: usize) -> (&'static str, &'static str, &'static str) {
    match index % 4 {
        0 => ("Vertices:", "Simplices:", "Compute Betti numbers"),
        1 => (
            "Vertex set:",
            "Simplex list:",
            "Find the Euler characteristic",
        ),
        2 => ("On vertices ", "Faces:", "Construct the boundary matrices"),
        _ => ("Vertices:", "Faces:", "Validate the complex"),
    }
}

fn supported_text(index: usize) -> String {
    let (vertex_marker, simplex_marker, operation) = shape(index);
    format!(
        "{operation} for the finite simplicial complex. {vertex_marker} [a,b,c]. {simplex_marker} [[a],[b],[c],[a,b],[a,c],[b,c],[a,b,c]]. Coefficients: F_2."
    )
}

fn ambiguous_text(index: usize) -> String {
    let (vertex_marker, simplex_marker, operation) = shape(index);
    if index % 2 == 0 {
        format!(
            "{operation} for the finite simplicial complex. {vertex_marker} [a,b,c]. {simplex_marker} [[a],[b],[c],[a,b],[a,c],[b,c],[a,b,c]]."
        )
    } else {
        format!(
            "Study the finite simplicial complex. {vertex_marker} [a,b,c]. {simplex_marker} [[a],[b],[c],[a,b],[a,c],[b,c],[a,b,c]]. Coefficients: F_2."
        )
    }
}

fn unsupported_text(index: usize) -> String {
    match index % 3 {
        0 => "Compute persistent homology for an infinite complex on vertices [a,b].".into(),
        1 => "Compute Betti numbers over the integers for the finite complex on vertices [a,b]."
            .into(),
        _ => "Use numerical approximation to analyze a continuous complex with vertices [a,b]."
            .into(),
    }
}

fn corpus() -> Vec<Case> {
    (0..120)
        .map(|index| Case {
            id: format!("supported-{index:03}"),
            expected: FrontendStatus::Complete,
            text: supported_text(index),
        })
        .chain((0..40).map(|index| Case {
            id: format!("ambiguous-{index:03}"),
            expected: FrontendStatus::Ambiguous,
            text: ambiguous_text(index),
        }))
        .chain((0..80).map(|index| Case {
            id: format!("unsupported-{index:03}"),
            expected: FrontendStatus::Unsupported,
            text: unsupported_text(index),
        }))
        .collect()
}

fn tamper_frontend(result: &FrontendResult) -> bool {
    let mut tampered = result.clone();
    tampered.replay_hash = "tampered".into();
    !tampered.replay_verified()
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cases = corpus();
    let mut exact_frontend_decisions = 0;
    let mut frontend_replay_verified = 0;
    let mut frontend_tamper_rejected = 0;
    let mut downstream_authorized = 0;
    let mut downstream_replay_verified = 0;
    let mut downstream_tamper_rejected = 0;
    let mut false_authorizations = 0;
    let mut false_denials = 0;
    let mut statuses = BTreeMap::new();
    for case in &cases {
        let frontend = formalize(&case.text);
        *statuses
            .entry(format!("{:?}", frontend.status).to_ascii_lowercase())
            .or_insert(0usize) += 1;
        if frontend.status == case.expected {
            exact_frontend_decisions += 1;
        }
        if frontend.replay_verified() {
            frontend_replay_verified += 1;
        }
        if tamper_frontend(&frontend) {
            frontend_tamper_rejected += 1;
        }
        if case.expected == FrontendStatus::Complete {
            let Some(request) = frontend.request.as_ref() else {
                false_denials += 1;
                continue;
            };
            let execution = evaluate(request);
            if execution.status == HomologyStatus::Complete && execution.authorized() {
                downstream_authorized += 1;
            } else {
                false_denials += 1;
            }
            if execution.replay_verified() {
                downstream_replay_verified += 1;
            }
            let mut tampered = execution.clone();
            tampered.replay_hash = "tampered".into();
            if !tampered.replay_verified() {
                downstream_tamper_rejected += 1;
            }
        } else if frontend.status == FrontendStatus::Complete {
            false_authorizations += 1;
        }
    }
    let report = Report {
        schema: "stage126-simplicial-language-frontend-v1",
        corpus_sha256: digest(&cases),
        cases: cases.len(),
        supported: 120,
        ambiguous: 40,
        unsupported: 80,
        exact_frontend_decisions,
        frontend_replay_verified,
        frontend_tamper_rejected,
        downstream_authorized,
        downstream_replay_verified,
        downstream_tamper_rejected,
        false_authorizations,
        false_denials,
        statuses,
    };
    assert_eq!(report.cases, 240);
    assert_eq!(report.exact_frontend_decisions, 240);
    assert_eq!(report.frontend_replay_verified, 240);
    assert_eq!(report.frontend_tamper_rejected, 240);
    assert_eq!(report.downstream_authorized, 120);
    assert_eq!(report.downstream_replay_verified, 120);
    assert_eq!(report.downstream_tamper_rejected, 120);
    assert_eq!(report.false_authorizations, 0);
    assert_eq!(report.false_denials, 0);
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
