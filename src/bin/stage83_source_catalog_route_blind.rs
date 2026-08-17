//! Stage 83: route-blind execution of an extracted source formula catalog.
//!
//! The catalog is parsed from source-shaped records and executed by the
//! generic expression runtime.  Formula identifiers, aliases, and expression
//! shapes are data; this benchmark contains no formula-specific executor.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use the_machine::probability_pack::Rational;
use the_machine::source_formula_pack::{
    evaluate_formula_records, extract_formula_records, FormulaRequest, FormulaStatus,
};

const REPORT_JSON: &str = "docs/stage83_source_catalog_route_blind.json";
const REPORT_MD: &str = "docs/stage83_source_catalog_route_blind.md";
const DOMAIN: &str = "source_catalog_sequences_series";

const SOURCE_DOCUMENT: &str = r#"
BEGIN FORMULA arithmetic_nth_term
ALIASES: arithmetic sequence term|affine sequence
EXPRESSION: a1 + (n - 1) * d
INPUTS: a1, n, d
ASSUMPTIONS: n is a positive integer
CONSTRAINTS: positive_integer:n
SOURCE_ID: openstax-precalculus-2e:sequences-series
TITLE: Precalculus 2e
SECTION: Sequences, Series, and the Binomial Theorem
URL: https://openstax.org/details/books/precalculus-2e
LICENSE: CC BY 4.0; OpenStax attribution required
RETRIEVED: 2026-08-16
EVIDENCE: arithmetic sequence nth-term formula
END FORMULA
BEGIN FORMULA arithmetic_partial_sum
ALIASES: arithmetic series sum
EXPRESSION: n * (2 * a1 + (n - 1) * d) / 2
INPUTS: a1, n, d
ASSUMPTIONS: n is a positive integer
CONSTRAINTS: positive_integer:n
SOURCE_ID: openstax-precalculus-2e:sequences-series
TITLE: Precalculus 2e
SECTION: Sequences, Series, and the Binomial Theorem
URL: https://openstax.org/details/books/precalculus-2e
LICENSE: CC BY 4.0; OpenStax attribution required
RETRIEVED: 2026-08-16
EVIDENCE: arithmetic series partial-sum formula
END FORMULA
BEGIN FORMULA geometric_nth_term
ALIASES: geometric sequence term
EXPRESSION: a1 * r^(n-1)
INPUTS: a1, n, r
ASSUMPTIONS: n is a positive integer; exponent is n-1
CONSTRAINTS: positive_integer:n
SOURCE_ID: openstax-precalculus-2e:sequences-series
TITLE: Precalculus 2e
SECTION: Sequences, Series, and the Binomial Theorem
URL: https://openstax.org/details/books/precalculus-2e
LICENSE: CC BY 4.0; OpenStax attribution required
RETRIEVED: 2026-08-16
EVIDENCE: geometric sequence nth-term formula
END FORMULA
BEGIN FORMULA geometric_partial_sum
ALIASES: geometric series sum
EXPRESSION: a1 * (r^n - 1) / (r - 1)
INPUTS: a1, n, r
ASSUMPTIONS: n is a positive integer; r is not one
CONSTRAINTS: positive_integer:n; not_equal_integer:r=1
SOURCE_ID: openstax-precalculus-2e:sequences-series
TITLE: Precalculus 2e
SECTION: Sequences, Series, and the Binomial Theorem
URL: https://openstax.org/details/books/precalculus-2e
LICENSE: CC BY 4.0; OpenStax attribution required
RETRIEVED: 2026-08-16
EVIDENCE: geometric series partial-sum formula
END FORMULA
"#;

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum Expected {
    Supported,
    Ambiguous,
    Refused,
}

#[derive(Debug, Serialize)]
struct Receipt {
    index: usize,
    expected: Expected,
    actual: FormulaStatus,
    exact: bool,
    replay: bool,
    tamper_rejected: bool,
    source_present: bool,
    value_correct: bool,
    false_authorization: bool,
    false_denial: bool,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    source_document_sha256: String,
    catalog_records: usize,
    catalog_valid: bool,
    cases: usize,
    supported: usize,
    ambiguous: usize,
    refused: usize,
    exact_decisions: usize,
    value_correct: usize,
    replay_verified: usize,
    tamper_rejections: usize,
    source_provenance_preserved: usize,
    false_authorizations: usize,
    false_denials: usize,
    holdout_cases: usize,
    holdout_exact_decisions: usize,
    holdout_value_correct: usize,
    holdout_replays: usize,
    holdout_tamper_rejections: usize,
}

fn digest_bytes(value: &[u8]) -> String {
    format!("{:x}", Sha256::digest(value))
}

fn q(value: i128) -> Rational {
    Rational::new(value, 1).unwrap()
}

fn pow(base: &Rational, exponent: usize) -> Rational {
    let mut value = Rational::one();
    for _ in 0..exponent {
        value = value.mul(base).unwrap();
    }
    value
}

fn inputs(index: usize) -> BTreeMap<String, Rational> {
    BTreeMap::from([
        ("a1".into(), q(2 + (index % 7) as i128)),
        ("n".into(), q(2 + (index % 8) as i128)),
        ("d".into(), q(1 + (index % 5) as i128)),
        ("r".into(), q(2 + (index % 4) as i128)),
    ])
}

fn oracle(formula: &str, values: &BTreeMap<String, Rational>) -> Option<Rational> {
    let a1 = values.get("a1")?;
    let n = values.get("n")?;
    let d = values.get("d")?;
    let r = values.get("r")?;
    let one = q(1);
    match formula {
        "arithmetic_nth_term" => Some(a1.add(&n.sub(&one)?.mul(d)?)?),
        "arithmetic_partial_sum" => Some(
            n.mul(&a1.mul(&q(2))?.add(&n.sub(&one)?.mul(d)?)?)?
                .div(&q(2))?,
        ),
        "geometric_nth_term" => Some(a1.mul(&pow(r, n.numerator as usize - 1))?),
        "geometric_partial_sum" => Some(
            a1.mul(&pow(r, n.numerator as usize).sub(&one)?)?
                .div(&r.sub(&one)?)?,
        ),
        _ => None,
    }
}

fn request(formula: &str, index: usize) -> FormulaRequest {
    FormulaRequest {
        formula: formula.into(),
        inputs: inputs(index),
        domain: DOMAIN.into(),
        ambiguity: None,
        provenance: vec![format!("stage83-route-blind-case:{index}")],
    }
}

fn evaluate_case(
    records: &[the_machine::source_formula_pack::FormulaRecord],
    expected: Expected,
    formula: &str,
    index: usize,
    mut request: FormulaRequest,
) -> Receipt {
    let expected_value = oracle(formula, &request.inputs);
    let result = evaluate_formula_records(&request, DOMAIN, records);
    let value_correct = expected != Expected::Supported || result.value == expected_value;
    let exact = match expected {
        Expected::Supported => {
            result.status == FormulaStatus::Complete && value_correct && result.source.is_some()
        }
        Expected::Ambiguous => result.status == FormulaStatus::Ambiguous,
        Expected::Refused => result.status != FormulaStatus::Complete,
    };
    let replay = result.replay_verified();
    let mut tampered = result.clone();
    tampered.replay_hash.push('x');
    let tamper_rejected = !tampered.replay_verified();
    let false_authorization =
        expected != Expected::Supported && result.status == FormulaStatus::Complete;
    let false_denial = expected == Expected::Supported && !exact;
    request.inputs.clear();
    Receipt {
        index,
        expected,
        actual: result.status,
        exact,
        replay,
        tamper_rejected,
        source_present: result.source.is_some(),
        value_correct,
        false_authorization,
        false_denial,
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let source_hash = digest_bytes(SOURCE_DOCUMENT.as_bytes());
    let records = extract_formula_records(SOURCE_DOCUMENT).map_err(|errors| errors.join("; "))?;
    assert_eq!(records.len(), 4);
    let mut receipts = Vec::with_capacity(1_000);
    let formulas = [
        "arithmetic_nth_term",
        "arithmetic_partial_sum",
        "geometric_nth_term",
        "geometric_partial_sum",
    ];
    let aliases = [
        "arithmetic sequence term",
        "arithmetic series sum",
        "geometric sequence term",
        "geometric series sum",
    ];
    for index in 0..600 {
        let slot = index % formulas.len();
        let formula = if index % 2 == 0 {
            formulas[slot]
        } else {
            aliases[slot]
        };
        receipts.push(evaluate_case(
            &records,
            Expected::Supported,
            formulas[slot],
            index,
            request(formula, index),
        ));
    }
    for index in 0..200 {
        let slot = index % formulas.len();
        let mut req = request(aliases[slot], index + 600);
        req.ambiguity = Some("source wording admits multiple sequence interpretations".into());
        receipts.push(evaluate_case(
            &records,
            Expected::Ambiguous,
            formulas[slot],
            index + 600,
            req,
        ));
    }
    for index in 0..50 {
        receipts.push(evaluate_case(
            &records,
            Expected::Refused,
            "unlisted_formula",
            index + 800,
            request("unlisted_formula", index + 800),
        ));
    }
    for index in 0..50 {
        let mut req = request("arithmetic_nth_term", index + 850);
        req.inputs.remove("d");
        receipts.push(evaluate_case(
            &records,
            Expected::Refused,
            "arithmetic_nth_term",
            index + 850,
            req,
        ));
    }
    for index in 0..50 {
        let mut req = request("arithmetic_nth_term", index + 900);
        req.inputs.insert("n".into(), q(0));
        receipts.push(evaluate_case(
            &records,
            Expected::Refused,
            "arithmetic_nth_term",
            index + 900,
            req,
        ));
    }
    for index in 0..50 {
        let mut req = request("geometric_partial_sum", index + 950);
        req.inputs.insert("r".into(), q(1));
        receipts.push(evaluate_case(
            &records,
            Expected::Refused,
            "geometric_partial_sum",
            index + 950,
            req,
        ));
    }
    assert_eq!(receipts.len(), 1_000);
    let holdout: Vec<Receipt> = (0..200)
        .map(|offset| {
            let slot = (offset + 2) % formulas.len();
            evaluate_case(
                &records,
                Expected::Supported,
                formulas[slot],
                offset + 2_000,
                request(aliases[slot], offset + 2_000),
            )
        })
        .collect();
    let count =
        |items: &[Receipt], f: fn(&Receipt) -> bool| items.iter().filter(|item| f(item)).count();
    assert_eq!(count(&receipts, |r| r.exact), 1_000);
    assert_eq!(count(&receipts, |r| r.replay), 1_000);
    assert_eq!(count(&receipts, |r| r.tamper_rejected), 1_000);
    assert_eq!(count(&receipts, |r| r.false_authorization), 0);
    assert_eq!(count(&receipts, |r| r.false_denial), 0);
    assert_eq!(count(&holdout, |r| r.exact), 200);
    assert_eq!(count(&holdout, |r| r.value_correct), 200);
    assert_eq!(count(&holdout, |r| r.replay), 200);
    assert_eq!(count(&holdout, |r| r.tamper_rejected), 200);
    let report = Report {
        schema: "stage83-source-catalog-route-blind-v1",
        source_document_sha256: source_hash,
        catalog_records: records.len(),
        catalog_valid: true,
        cases: 1_000,
        supported: 600,
        ambiguous: 200,
        refused: 200,
        exact_decisions: count(&receipts, |r| r.exact),
        value_correct: count(&receipts, |r| r.value_correct),
        replay_verified: count(&receipts, |r| r.replay),
        tamper_rejections: count(&receipts, |r| r.tamper_rejected),
        source_provenance_preserved: count(&receipts, |r| r.source_present)
            + count(&holdout, |r| r.source_present),
        false_authorizations: count(&receipts, |r| r.false_authorization),
        false_denials: count(&receipts, |r| r.false_denial),
        holdout_cases: 200,
        holdout_exact_decisions: count(&holdout, |r| r.exact),
        holdout_value_correct: count(&holdout, |r| r.value_correct),
        holdout_replays: count(&holdout, |r| r.replay),
        holdout_tamper_rejections: count(&holdout, |r| r.tamper_rejected),
    };
    fs::write(
        REPORT_JSON,
        format!("{}\n", serde_json::to_string_pretty(&report)?),
    )?;
    fs::write(
        REPORT_MD,
        format!(
            "# Stage 83 — source catalog route-blind validation\n\n- Catalog records: {}\n- Catalog valid: {}\n- Development: {}/{} exact, {}/{} replay, {}/{} tamper\n- Source provenance: {}/{} complete cases\n- Holdout: {}/{} exact, {}/{} value, {}/{} replay, {}/{} tamper\n- False authorizations / denials: {} / {}\n- Source document SHA-256: `{}`\n",
            report.catalog_records,
            report.catalog_valid,
            report.exact_decisions,
            report.cases,
            report.replay_verified,
            report.cases,
            report.tamper_rejections,
            report.cases,
            report.source_provenance_preserved,
            report.cases + report.holdout_cases,
            report.holdout_exact_decisions,
            report.holdout_cases,
            report.holdout_value_correct,
            report.holdout_cases,
            report.holdout_replays,
            report.holdout_cases,
            report.holdout_tamper_rejections,
            report.holdout_cases,
            report.false_authorizations,
            report.false_denials,
            report.source_document_sha256,
        ),
    )?;
    Ok(())
}
