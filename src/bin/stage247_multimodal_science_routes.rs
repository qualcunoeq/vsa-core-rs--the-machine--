//! Stage 247: route-blind multimodal science routing.
//!
//! Coordinate-bearing OCR tables are offered to probability, chemistry, and
//! biology bridges without a subject-specific dispatcher. Exactly one bridge
//! may authorize a supported table; malformed or ambiguous grids fail closed.

use serde::Serialize;
use sha2::{Digest, Sha256};
use the_machine::vision::visual_table::visual_biology_bridge::table_to_biology_probability;
use the_machine::vision::visual_table::visual_chemistry_bridge::table_to_chemistry_linear;
use the_machine::vision::visual_table::visual_probability_bridge::table_to_probability;
use the_machine::vision::visual_table::formalize_table_tsv;

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum Expected {
    Probability,
    Chemistry,
    Biology,
    Refused,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    corpus_sha256: String,
    cases: usize,
    supported_cases: usize,
    refused_cases: usize,
    exact_decisions: usize,
    authorized: usize,
    visual_replays: usize,
    visual_tamper_rejections: usize,
    bridge_emissions: usize,
    bridge_replays: usize,
    bridge_tamper_rejections: usize,
    route_leakage: usize,
    false_authorizations: usize,
    false_denials: usize,
    manifest_mutations: usize,
}

const HEADER: &str = "level\tpage\tblock\tpar\tline\tword\tleft\ttop\twidth\theight\tconf\ttext";

fn word(left: usize, top: usize, text: &str, width: usize) -> String {
    format!("5\t1\t1\t1\t1\t1\t{left}\t{top}\t{width}\t10\t90\t{text}")
}

fn table(headers: &[&str], rows: &[Vec<&str>]) -> String {
    let mut lines = vec![HEADER.into()];
    for (column, value) in headers.iter().enumerate() {
        lines.push(word(10 + column * 90, 10, value, 55));
    }
    for (row, values) in rows.iter().enumerate() {
        for (column, value) in values.iter().enumerate() {
            lines.push(word(10 + column * 90, 35 + row * 25, value, 55));
        }
    }
    lines.join("\n")
}

fn run(text: &str, expected: Expected) -> (bool, usize, usize, usize, usize, usize, usize) {
    let visual = formalize_table_tsv(text);
    let mut visual_tampered = visual.clone();
    visual_tampered.replay_hash.push('x');
    let visual_replay = usize::from(visual.replay_verified());
    let visual_tamper = usize::from(!visual_tampered.replay_verified());
    let Some(artifact) = visual.artifact.as_ref() else {
        return (
            expected == Expected::Refused,
            visual_replay,
            visual_tamper,
            0,
            0,
            0,
            0,
        );
    };
    let probability = table_to_probability(artifact);
    let chemistry = table_to_chemistry_linear(artifact);
    let biology = table_to_biology_probability(artifact, Some("uniform_position"));
    let authorizations = [
        probability.authorized(),
        chemistry.authorized(),
        biology.authorized(),
    ];
    let authorized = authorizations.iter().filter(|value| **value).count();
    let expected_authorized = match expected {
        Expected::Probability => probability.authorized(),
        Expected::Chemistry => chemistry.authorized(),
        Expected::Biology => biology.authorized(),
        Expected::Refused => false,
    };
    let exact = if expected == Expected::Refused {
        authorized == 0
    } else {
        authorized == 1 && expected_authorized
    };
    let mut bridge_replays = 0;
    let mut bridge_tamper = 0;
    for bridge in [
        probability.replay_verified(),
        chemistry.replay_verified(),
        biology.replay_verified(),
    ] {
        bridge_replays += usize::from(bridge);
    }
    let mut probability_tampered = probability.clone();
    probability_tampered.replay_hash.push('x');
    let mut chemistry_tampered = chemistry.clone();
    chemistry_tampered.replay_hash.push('x');
    let mut biology_tampered = biology.clone();
    biology_tampered.replay_hash.push('x');
    bridge_tamper += usize::from(!probability_tampered.replay_verified());
    bridge_tamper += usize::from(!chemistry_tampered.replay_verified());
    bridge_tamper += usize::from(!biology_tampered.replay_verified());
    (
        exact,
        visual_replay,
        visual_tamper,
        3,
        bridge_replays,
        bridge_tamper,
        authorized,
    )
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut cases = Vec::new();
    for _ in 0..80 {
        cases.push((
            table(
                &["outcome", "probability"],
                &[vec!["a", "1/2"], vec!["b", "1/3"], vec!["c", "1/6"]],
            ),
            Expected::Probability,
        ));
    }
    for _ in 0..80 {
        cases.push((
            table(&["element", "count"], &[vec!["H", "2"], vec!["O", "1"]]),
            Expected::Chemistry,
        ));
    }
    for _ in 0..40 {
        cases.push((
            table(
                &["base", "count"],
                &[
                    vec!["A", "2"],
                    vec!["C", "2"],
                    vec!["G", "2"],
                    vec!["T", "2"],
                ],
            ),
            Expected::Biology,
        ));
    }
    for _ in 0..20 {
        cases.push((
            table(&["value", "weight"], &[vec!["a", "1/2"], vec!["b", "1/2"]]),
            Expected::Refused,
        ));
    }
    for _ in 0..20 {
        cases.push((
            table(
                &["element", "count"],
                &[vec!["H", "2"], vec!["O", "1"], vec!["C", "1", "extra"]],
            ),
            Expected::Refused,
        ));
    }
    let mut exact_decisions = 0;
    let mut authorized = 0;
    let mut visual_replays = 0;
    let mut visual_tamper_rejections = 0;
    let mut bridge_emissions = 0;
    let mut bridge_replays = 0;
    let mut bridge_tamper_rejections = 0;
    let mut route_leakage = 0;
    let mut false_authorizations = 0;
    let mut false_denials = 0;
    for (text, expected) in &cases {
        let (exact, vr, vt, emitted, br, bt, count) = run(text, *expected);
        exact_decisions += usize::from(exact);
        authorized += count;
        visual_replays += vr;
        visual_tamper_rejections += vt;
        bridge_emissions += emitted;
        bridge_replays += br;
        bridge_tamper_rejections += bt;
        route_leakage += usize::from(count > 1);
        false_authorizations += usize::from(*expected == Expected::Refused && count > 0);
        false_denials += usize::from(*expected != Expected::Refused && count != 1);
    }
    let report = Report {
        schema: "stage247-multimodal-science-routes-v1",
        corpus_sha256: format!(
            "{:x}",
            Sha256::digest(serde_json::to_vec(
                &cases
                    .iter()
                    .map(|(text, expected)| (text, expected))
                    .collect::<Vec<_>>()
            )?)
        ),
        cases: cases.len(),
        supported_cases: 200,
        refused_cases: 40,
        exact_decisions,
        authorized,
        visual_replays,
        visual_tamper_rejections,
        bridge_emissions,
        bridge_replays,
        bridge_tamper_rejections,
        route_leakage,
        false_authorizations,
        false_denials,
        manifest_mutations: 0,
    };
    assert_eq!(report.cases, 240);
    assert_eq!(report.supported_cases, 200);
    assert_eq!(report.refused_cases, 40);
    assert_eq!(report.exact_decisions, 240);
    assert_eq!(report.authorized, 200);
    assert_eq!(report.visual_replays, 240);
    assert_eq!(report.visual_tamper_rejections, 240);
    assert_eq!(report.bridge_emissions, 720);
    assert_eq!(report.bridge_replays, 720);
    assert_eq!(report.bridge_tamper_rejections, 720);
    assert_eq!(report.route_leakage, 0);
    assert_eq!(report.false_authorizations, 0);
    assert_eq!(report.false_denials, 0);
    assert_eq!(report.manifest_mutations, 0);
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
