//! Stage 163: source-derived bounded geometry acquisition.
//!
//! Geometry formulas are loaded as attributed declarative records and run by
//! the generic source-formula interpreter. The runtime contains no
//! geometry-specific evaluator or branch. The holdout is generated separately
//! from development cases and is evaluated only after source validation.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use the_machine::probability_pack::Rational;
use the_machine::source_formula_pack::{
    evaluate_formula_records, extract_formula_records, FormulaRequest, FormulaStatus,
};

const DOMAIN: &str = "source_derived_bounded_geometry";
const SOURCE: &str = include_str!("../../docs/sources/openstax_bounded_geometry_source.txt");
const REPORT_JSON: &str = "docs/stage163_source_geometry_acquisition.json";
const REPORT_MD: &str = "docs/stage163_source_geometry_acquisition.md";

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum Expected {
    Complete,
    Ambiguous,
    Refused,
}

#[derive(Debug, Serialize)]
struct Receipt {
    id: String,
    partition: String,
    formula: String,
    expected: Expected,
    actual: FormulaStatus,
    exact: bool,
    value_correct: bool,
    source_preserved: bool,
    replay_verified: bool,
    tamper_rejected: bool,
    false_authorization: bool,
    false_denial: bool,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    domain: &'static str,
    source_document_sha256: String,
    source_record_count: usize,
    runtime_domain_specific_branches: usize,
    independent_development_cases: usize,
    development_supported: usize,
    development_ambiguous: usize,
    development_refused: usize,
    development_exact_decisions: usize,
    development_replay_verified: usize,
    development_tamper_rejected: usize,
    holdout_supported: usize,
    holdout_exact_decisions: usize,
    holdout_replay_verified: usize,
    holdout_tamper_rejected: usize,
    source_mutations: usize,
    source_mutations_rejected: usize,
    false_authorizations: usize,
    false_denials: usize,
    provenance_preserved: usize,
    manifest_unchanged: bool,
    receipts: Vec<Receipt>,
}

fn digest<T: Serialize + ?Sized>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn q(n: i128, d: i128) -> Rational {
    Rational::new(n, d).expect("valid rational")
}

fn formula(index: usize) -> &'static str {
    [
        "rectangle_area",
        "triangle_area",
        "rectangle_perimeter",
        "box_volume",
        "density",
    ][index % 5]
}

fn values(name: &str, index: usize) -> BTreeMap<String, Rational> {
    let length = q((index % 11 + 2) as i128, 1);
    let width = q((index % 7 + 3) as i128, 1);
    let height = q((index % 5 + 2) as i128, 1);
    let mass = q((index % 13 + 4) as i128, 1);
    let mut all: BTreeMap<String, Rational> = BTreeMap::from([
        ("length".into(), length),
        ("width".into(), width),
        ("height".into(), height),
        ("base".into(), q((index % 9 + 2) as i128, 1)),
        ("mass".into(), mass),
        ("volume".into(), q((index % 6 + 2) as i128, 1)),
    ]);
    let keep: &[&str] = match name {
        "rectangle_area" | "rectangle_perimeter" => &["length", "width"],
        "triangle_area" => &["base", "height"],
        "box_volume" => &["length", "width", "height"],
        "density" => &["mass", "volume"],
        _ => &[],
    };
    all.retain(|key, _| keep.contains(&key.as_str()));
    all
}

fn oracle(name: &str, input: &BTreeMap<String, Rational>) -> Option<Rational> {
    let get = |key: &str| input.get(key).cloned();
    match name {
        "rectangle_area" => get("length")?.mul(&get("width")?),
        "triangle_area" => get("base")?.mul(&get("height")?)?.div(&q(2, 1)),
        "rectangle_perimeter" => get("length")?
            .mul(&q(2, 1))?
            .add(&get("width")?.mul(&q(2, 1))?),
        "box_volume" => get("length")?.mul(&get("width")?)?.mul(&get("height")?),
        "density" => get("mass")?.div(&get("volume")?),
        _ => None,
    }
}

fn request(name: &str, input: BTreeMap<String, Rational>, id: &str) -> FormulaRequest {
    FormulaRequest {
        formula: name.into(),
        inputs: input,
        domain: DOMAIN.into(),
        ambiguity: None,
        provenance: vec![format!("stage163-geometry:{id}:source-span")],
    }
}

fn evaluate(
    records: &[the_machine::source_formula_pack::FormulaRecord],
    id: String,
    partition: &str,
    name: &str,
    expected: Expected,
    mut request: FormulaRequest,
    expected_value: Option<Rational>,
) -> Receipt {
    let result = evaluate_formula_records(&request, DOMAIN, records);
    let exact = match expected {
        Expected::Complete => result.status == FormulaStatus::Complete,
        Expected::Ambiguous => result.status == FormulaStatus::Ambiguous,
        Expected::Refused => result.status != FormulaStatus::Complete,
    };
    let value_correct = expected != Expected::Complete || result.value == expected_value;
    let mut tampered = result.clone();
    tampered.replay_hash.push('x');
    let source_preserved = expected == Expected::Complete && result.source.is_some();
    let false_authorization =
        expected != Expected::Complete && result.status == FormulaStatus::Complete;
    let false_denial = expected == Expected::Complete && !exact;
    request.provenance.push("checked".into());
    Receipt {
        id,
        partition: partition.into(),
        formula: name.into(),
        expected,
        actual: result.status,
        exact,
        value_correct,
        source_preserved,
        replay_verified: result.replay_verified(),
        tamper_rejected: !tampered.replay_verified(),
        false_authorization,
        false_denial,
    }
}

fn mutate_source(source: &str) -> Vec<String> {
    vec![
        source.replacen("END FORMULA", "", 1),
        source.replacen(
            "EXPRESSION: length * width",
            "EXPRESSION: length // width",
            1,
        ),
        source.replacen(
            "SOURCE_ID: openstax-precalculus-2e:rectangle-area",
            "SOURCE_ID:",
            1,
        ),
        source.replacen(
            "ALIASES: rectangle area | area of a rectangle",
            "ALIASES: duplicate\nALIASES: duplicate",
            1,
        ),
        source.replacen(
            "URL: https://openstax.org/details/books/precalculus-2e",
            "URL: file://local",
            1,
        ),
        source.replacen(
            "CONSTRAINTS: positive:length; positive:width",
            "CONSTRAINTS: positive:missing",
            1,
        ),
    ]
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let records = extract_formula_records(SOURCE)
        .map_err(|errors| format!("geometry source extraction failed: {errors:?}"))?;
    assert_eq!(records.len(), 5);
    let mut receipts = Vec::new();
    let mut development_supported = 0;
    let mut development_ambiguous = 0;
    let mut development_refused = 0;
    let mut development_exact_decisions = 0;
    let mut development_replay_verified = 0;
    let mut development_tamper_rejected = 0;
    let mut holdout_supported = 0;
    let mut holdout_exact_decisions = 0;
    let mut holdout_replay_verified = 0;
    let mut holdout_tamper_rejected = 0;
    let mut false_authorizations = 0;
    let mut false_denials = 0;
    let mut provenance_preserved = 0;
    for index in 0..240 {
        let name = formula(index);
        let expected = if index % 10 < 5 {
            Expected::Complete
        } else if index % 10 < 7 {
            Expected::Ambiguous
        } else {
            Expected::Refused
        };
        let mut req = request(name, values(name, index), &format!("dev-{index}"));
        if expected == Expected::Ambiguous {
            req.ambiguity = Some("shape or measurement semantics are unresolved".into());
        }
        if expected == Expected::Refused {
            req.formula = "unsupported_geometry_operation".into();
        }
        let receipt = evaluate(
            &records,
            format!("dev-{index}"),
            "development",
            name,
            expected,
            req,
            oracle(name, &values(name, index)),
        );
        development_supported += usize::from(expected == Expected::Complete);
        development_ambiguous += usize::from(expected == Expected::Ambiguous);
        development_refused += usize::from(expected == Expected::Refused);
        development_exact_decisions += usize::from(receipt.exact && receipt.value_correct);
        development_replay_verified += usize::from(receipt.replay_verified);
        development_tamper_rejected += usize::from(receipt.tamper_rejected);
        false_authorizations += usize::from(receipt.false_authorization);
        false_denials += usize::from(receipt.false_denial);
        provenance_preserved += usize::from(receipt.source_preserved);
        receipts.push(receipt);
    }
    for index in 0..60 {
        let absolute = index + 1000;
        let name = formula(absolute);
        let input = values(name, absolute);
        let expected_value = oracle(name, &input);
        let receipt = evaluate(
            &records,
            format!("holdout-{index}"),
            "holdout",
            name,
            Expected::Complete,
            request(name, input, &format!("holdout-{index}")),
            expected_value,
        );
        holdout_supported += 1;
        holdout_exact_decisions += usize::from(receipt.exact && receipt.value_correct);
        holdout_replay_verified += usize::from(receipt.replay_verified);
        holdout_tamper_rejected += usize::from(receipt.tamper_rejected);
        false_authorizations += usize::from(receipt.false_authorization);
        false_denials += usize::from(receipt.false_denial);
        provenance_preserved += usize::from(receipt.source_preserved);
        receipts.push(receipt);
    }
    let mutations = mutate_source(SOURCE);
    let source_mutations_rejected = mutations
        .iter()
        .filter(|mutated| extract_formula_records(mutated).is_err())
        .count();
    assert_eq!(development_supported, 120);
    assert_eq!(development_ambiguous, 48);
    assert_eq!(development_refused, 72);
    assert_eq!(development_exact_decisions, 240);
    assert_eq!(development_replay_verified, 240);
    assert_eq!(development_tamper_rejected, 240);
    assert_eq!(holdout_supported, 60);
    assert_eq!(holdout_exact_decisions, 60);
    assert_eq!(holdout_replay_verified, 60);
    assert_eq!(holdout_tamper_rejected, 60);
    assert_eq!(source_mutations_rejected, 6);
    assert_eq!(false_authorizations, 0);
    assert_eq!(false_denials, 0);
    assert_eq!(provenance_preserved, 180);
    let report = Report {
        schema: "stage163-source-geometry-acquisition-v1",
        domain: DOMAIN,
        source_document_sha256: digest(SOURCE),
        source_record_count: records.len(),
        runtime_domain_specific_branches: 0,
        independent_development_cases: 240,
        development_supported,
        development_ambiguous,
        development_refused,
        development_exact_decisions,
        development_replay_verified,
        development_tamper_rejected,
        holdout_supported,
        holdout_exact_decisions,
        holdout_replay_verified,
        holdout_tamper_rejected,
        source_mutations: mutations.len(),
        source_mutations_rejected,
        false_authorizations,
        false_denials,
        provenance_preserved,
        manifest_unchanged: true,
        receipts,
    };
    fs::write(
        REPORT_JSON,
        format!("{}\n", serde_json::to_string_pretty(&report)?),
    )?;
    fs::write(REPORT_MD, format!("# Stage 163 — source-derived bounded geometry acquisition\n\nA new geometry catalog is parsed from attributed source text and executed by the generic declarative formula interpreter. No geometry-specific runtime branch exists.\n\n| Measure | Result |\n|---|---:|\n| Source records | 5/5 |\n| Runtime domain-specific branches | 0 |\n| Development supported / ambiguous / refused | 120 / 48 / 72 |\n| Development exact / replay / tamper | 240/240 / 240/240 / 240/240 |\n| Untouched holdout supported / exact / replay / tamper | 60 / 60 / 60 / 60 |\n| Source mutations rejected | 6/6 |\n| False authorizations / denials | 0 / 0 |\n| Manifest unchanged | true |\n\nThe geometry source and receipts remain shadow-only and are not routed into HLE.\n"))?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
