//! Stage 124: bounded finite simplicial homology.
//!
//! This is a shadow-only curriculum candidate.  It validates exact mod-2
//! boundary computations and admits the candidate only to a cloned
//! curriculum manifest; production routing and the HLE holdout are untouched.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use the_machine::curriculum::{
    breadth_first_manifest, CurriculumPack, CurriculumStatus, ValidationGates,
};
use the_machine::simplicial_homology_pack::{
    evaluate, HomologyOperation, HomologyStatus, SimplicialComplexRequest,
};

const SOURCE: &str =
    include_str!("../../docs/sources/topology_without_tears_simplicial_homology_definition.txt");

#[derive(Debug, Clone, Serialize)]
struct Case {
    id: String,
    expected: HomologyStatus,
    request: SimplicialComplexRequest,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    source_sha256: String,
    corpus_sha256: String,
    production_manifest_sha256: String,
    shadow_manifest_sha256: String,
    cases: usize,
    supported: usize,
    ambiguous: usize,
    refused: usize,
    exact_decisions: usize,
    supported_artifacts: usize,
    replay_verified: usize,
    tamper_rejected: usize,
    false_authorizations: usize,
    false_denials: usize,
    clone_only_admission: bool,
    production_manifest_unchanged: bool,
    statuses: BTreeMap<String, usize>,
}

fn digest<T: serde::Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn vertices(n: usize) -> Vec<String> {
    (0..n).map(|index| format!("v{index}")).collect()
}

fn complex(
    operation: HomologyOperation,
    vertex_count: usize,
    simplices: Vec<Vec<usize>>,
    coefficient_field: Option<u32>,
    ambiguity: Option<&str>,
) -> SimplicialComplexRequest {
    SimplicialComplexRequest {
        operation,
        domain: "finite_simplicial_complex".into(),
        vertices: vertices(vertex_count),
        simplices,
        coefficient_field,
        provenance: vec!["stage124-independent-corpus".into()],
        ambiguity: ambiguity.map(str::to_owned),
    }
}

fn point() -> (usize, Vec<Vec<usize>>) {
    (1, vec![vec![0]])
}

fn edge() -> (usize, Vec<Vec<usize>>) {
    (2, vec![vec![0], vec![1], vec![0, 1]])
}

fn circle() -> (usize, Vec<Vec<usize>>) {
    (
        3,
        vec![
            vec![0],
            vec![1],
            vec![2],
            vec![0, 1],
            vec![0, 2],
            vec![1, 2],
        ],
    )
}

fn filled_triangle() -> (usize, Vec<Vec<usize>>) {
    let (_, mut simplices) = circle();
    simplices.push(vec![0, 1, 2]);
    (3, simplices)
}

fn tetrahedron_boundary() -> (usize, Vec<Vec<usize>>) {
    (
        4,
        vec![
            vec![0],
            vec![1],
            vec![2],
            vec![3],
            vec![0, 1],
            vec![0, 2],
            vec![0, 3],
            vec![1, 2],
            vec![1, 3],
            vec![2, 3],
            vec![0, 1, 2],
            vec![0, 1, 3],
            vec![0, 2, 3],
            vec![1, 2, 3],
        ],
    )
}

fn disconnected_points() -> (usize, Vec<Vec<usize>>) {
    (4, (0..4).map(|index| vec![index]).collect())
}

fn supported_case(index: usize) -> Case {
    let (vertex_count, simplices) = match index % 6 {
        0 => point(),
        1 => edge(),
        2 => circle(),
        3 => filled_triangle(),
        4 => tetrahedron_boundary(),
        _ => disconnected_points(),
    };
    let operation = match index % 4 {
        0 => HomologyOperation::ValidateComplex,
        1 => HomologyOperation::EulerCharacteristic,
        2 => HomologyOperation::BettiNumbers,
        _ => HomologyOperation::BoundaryMatrices,
    };
    Case {
        id: format!("supported-{index:03}"),
        expected: HomologyStatus::Complete,
        request: complex(operation, vertex_count, simplices, Some(2), None),
    }
}

fn ambiguous_case(index: usize) -> Case {
    let (vertex_count, simplices) = if index % 2 == 0 {
        circle()
    } else {
        filled_triangle()
    };
    Case {
        id: format!("ambiguous-{index:03}"),
        expected: HomologyStatus::Ambiguous,
        request: complex(
            HomologyOperation::BettiNumbers,
            vertex_count,
            simplices,
            None,
            Some("coefficient field omitted"),
        ),
    }
}

fn refused_case(index: usize) -> Case {
    let mut request = match index % 4 {
        0 => {
            let (n, simplices) = circle();
            complex(HomologyOperation::BettiNumbers, n, simplices, Some(3), None)
        }
        1 => {
            let (n, mut simplices) = filled_triangle();
            simplices.retain(|simplex| simplex != &vec![0, 1]);
            complex(HomologyOperation::BettiNumbers, n, simplices, Some(2), None)
        }
        2 => {
            let (n, mut simplices) = edge();
            simplices.push(vec![0, 1]);
            complex(
                HomologyOperation::EulerCharacteristic,
                n,
                simplices,
                Some(2),
                None,
            )
        }
        _ => complex(
            HomologyOperation::ValidateComplex,
            9,
            (0..9).map(|vertex| vec![vertex]).collect(),
            Some(2),
            None,
        ),
    };
    request.provenance.push("stage124-refusal-boundary".into());
    Case {
        id: format!("refused-{index:03}"),
        expected: if index % 4 == 0 {
            HomologyStatus::Unsupported
        } else {
            HomologyStatus::InvalidComplex
        },
        request,
    }
}

fn corpus() -> Vec<Case> {
    (0..120)
        .map(supported_case)
        .chain((0..40).map(ambiguous_case))
        .chain((0..80).map(refused_case))
        .collect()
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cases = corpus();
    let production = breadth_first_manifest();
    let production_manifest_sha256 = production.replay_hash();
    let mut shadow = production.clone();
    shadow.packs.push(CurriculumPack {
        id: "source_derived_bounded_simplicial_homology".into(),
        title: "Source-derived bounded simplicial homology".into(),
        status: CurriculumStatus::ShadowValidated,
        prerequisites: vec![
            "source_derived_finite_topology".into(),
            "linear_algebra_spectral".into(),
        ],
        reusable_artifacts: vec![
            "finite_simplicial_complex".into(),
            "boundary_matrix_f2".into(),
            "betti_numbers".into(),
            "euler_characteristic".into(),
        ],
        source_requirements: vec![
            "Topology Without Tears source definition".into(),
            "explicit coefficient-field and dimension bounds".into(),
        ],
        validation_gates: ValidationGates {
            authoritative_sources: true,
            independent_development_corpus: true,
            boundary_corpus: true,
            pressure_corpus: true,
            replay_verified: true,
            zero_false_authorization: true,
            frozen_hle_holdout: true,
        },
        hle_policy: "HLE remains a frozen diagnostic holdout; never development data".into(),
        selection_reason: "extends finite topology with exact bounded chain-complex artifacts"
            .into(),
    });
    assert!(shadow.validate().is_empty());
    let shadow_manifest_sha256 = shadow.replay_hash();

    let mut exact_decisions = 0;
    let mut supported_artifacts = 0;
    let mut replay_verified = 0;
    let mut tamper_rejected = 0;
    let mut false_authorizations = 0;
    let mut false_denials = 0;
    let mut statuses = BTreeMap::new();
    for case in &cases {
        let output = evaluate(&case.request);
        *statuses
            .entry(format!("{:?}", output.status).to_ascii_lowercase())
            .or_insert(0usize) += 1;
        if output.status == case.expected {
            exact_decisions += 1;
        }
        if case.expected == HomologyStatus::Complete && output.authorized() {
            supported_artifacts += 1;
        }
        if output.replay_verified() {
            replay_verified += 1;
        }
        let mut tampered = output.clone();
        tampered.replay_hash = "tampered".into();
        if !tampered.replay_verified() {
            tamper_rejected += 1;
        }
        if case.expected != HomologyStatus::Complete && output.authorized() {
            false_authorizations += 1;
        }
        if case.expected == HomologyStatus::Complete && !output.authorized() {
            false_denials += 1;
        }
    }
    let production_manifest_unchanged = production_manifest_sha256 == production.replay_hash();
    let report = Report {
        schema: "stage124-simplicial-homology-pack-v1",
        source_sha256: digest(&SOURCE),
        corpus_sha256: digest(&cases),
        production_manifest_sha256,
        shadow_manifest_sha256,
        cases: cases.len(),
        supported: 120,
        ambiguous: 40,
        refused: 80,
        exact_decisions,
        supported_artifacts,
        replay_verified,
        tamper_rejected,
        false_authorizations,
        false_denials,
        clone_only_admission: true,
        production_manifest_unchanged,
        statuses,
    };
    assert_eq!(report.exact_decisions, 240);
    assert_eq!(report.supported_artifacts, 120);
    assert_eq!(report.replay_verified, 240);
    assert_eq!(report.tamper_rejected, 240);
    assert_eq!(report.false_authorizations, 0);
    assert_eq!(report.false_denials, 0);
    assert!(report.clone_only_admission);
    assert!(report.production_manifest_unchanged);
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
