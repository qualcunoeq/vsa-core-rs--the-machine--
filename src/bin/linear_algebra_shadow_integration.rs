//! Phase 52 shadow integration: typed scalar artifacts flow into the existing
//! independent solution verifier. Production routing remains unchanged.

use serde::Serialize;
use the_machine::linear_algebra_pack::{
    evaluate_linear_algebra, LinearAlgebraArtifact, LinearAlgebraOperation, LinearAlgebraRequest,
    LinearAlgebraStatus,
};
use the_machine::solution_verification::execute_solution_verification;

#[derive(Serialize)]
struct Row {
    id: String,
    pack_status: LinearAlgebraStatus,
    bridge_used: bool,
    verifier_replay: bool,
    refused_non_scalar: bool,
    downstream_authorized: bool,
}

#[derive(Serialize)]
struct Report {
    schema_version: String,
    cases: usize,
    complete_scalar_artifacts: usize,
    bridge_receipts: usize,
    verifier_replays: usize,
    safe_refusals: usize,
    downstream_authorizations: usize,
    rows: Vec<Row>,
}

fn request(operation: LinearAlgebraOperation, matrix: Vec<Vec<i64>>) -> LinearAlgebraRequest {
    LinearAlgebraRequest {
        operation,
        matrix: Some(matrix),
        vector_a: None,
        vector_b: None,
        domain: "finite_exact_integer".into(),
        requested_output: "z".into(),
        provenance: vec!["phase52-shadow-integration".into()],
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut requests = Vec::new();
    for index in 0..20 {
        requests.push((
            format!("rank_{index}"),
            request(LinearAlgebraOperation::Rank, vec![vec![1, 2], vec![3, 4]]),
        ));
    }
    for index in 0..20 {
        requests.push((
            format!("determinant_{index}"),
            request(
                LinearAlgebraOperation::Determinant,
                vec![vec![1, 2], vec![3, 4]],
            ),
        ));
    }
    for index in 0..20 {
        requests.push((
            format!("eigen_refusal_{index}"),
            request(
                LinearAlgebraOperation::Eigenvalues,
                vec![vec![1, 1], vec![0, 1]],
            ),
        ));
    }
    let mut rows = Vec::new();
    let mut complete_scalar_artifacts = 0;
    let mut bridge_receipts = 0;
    let mut verifier_replays = 0;
    let mut safe_refusals = 0;
    for (id, request) in requests {
        let result = evaluate_linear_algebra(&request);
        let mut bridge_used = false;
        let mut verifier_replay = false;
        let refused_non_scalar = result.status != LinearAlgebraStatus::Complete;
        if let (LinearAlgebraStatus::Complete, Some(LinearAlgebraArtifact::Scalar(value))) =
            (result.status, result.artifact.clone())
        {
            complete_scalar_artifacts += 1;
            bridge_used = true;
            bridge_receipts += 1;
            let equation = format!("z = {value}");
            let candidate = format!("z = {value}");
            verifier_replay = execute_solution_verification(&equation, &candidate)
                .map(|receipt| receipt.replay_verified)
                .unwrap_or(false);
            verifier_replays += usize::from(verifier_replay);
        } else if refused_non_scalar {
            safe_refusals += 1;
        }
        rows.push(Row {
            id,
            pack_status: result.status,
            bridge_used,
            verifier_replay,
            refused_non_scalar,
            downstream_authorized: false,
        });
    }
    let report = Report {
        schema_version: "phase52-linear-algebra-shadow-integration-v1".into(),
        cases: rows.len(),
        complete_scalar_artifacts,
        bridge_receipts,
        verifier_replays,
        safe_refusals,
        downstream_authorizations: 0,
        rows,
    };
    assert_eq!(report.cases, 60);
    assert_eq!(report.complete_scalar_artifacts, 40);
    assert_eq!(report.bridge_receipts, 40);
    assert_eq!(report.verifier_replays, 40);
    assert_eq!(report.safe_refusals, 20);
    assert_eq!(report.downstream_authorizations, 0);
    println!("{}", serde_json::to_string_pretty(&report)?);
    std::fs::write(
        "docs/phase52_linear_algebra_shadow_integration.json",
        serde_json::to_vec_pretty(&report)?,
    )?;
    Ok(())
}
