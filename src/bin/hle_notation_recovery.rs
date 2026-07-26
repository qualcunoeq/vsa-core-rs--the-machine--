//! Phase 24 shadow evaluation for the bounded equations/expressions
//! normalization contract.
//!
//! It evaluates an independent development/boundary corpus and then applies
//! the frozen shadow normalizer to the matching HLE audit rows.  The HLE pass
//! reports reclassification candidates only; it never changes routing or
//! authorizes an answer.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::env;
use std::fs;
use the_machine::notation_normalization::{normalize_equation, NormalizationStatus};

#[derive(Debug, Clone, Serialize)]
struct CorpusCase {
    id: String,
    split: String,
    prompt: String,
    expected: NormalizationStatus,
    rewrite_group: Option<String>,
}

#[derive(Debug, Serialize)]
struct CaseResult {
    id: String,
    split: String,
    expected: NormalizationStatus,
    actual: NormalizationStatus,
    correct: bool,
    replay_verified: bool,
    symbol_bindings: Vec<String>,
    reason: String,
    rewrite_group: Option<String>,
}

#[derive(Debug, Deserialize)]
struct HleAudit {
    source_trace_sha256: String,
    records: Vec<HleRow>,
}

#[derive(Debug, Deserialize)]
struct HleRow {
    id: Option<String>,
    notation_family: String,
    downstream_outlook: String,
    question: String,
}

#[derive(Debug, Serialize)]
struct HleRecovery {
    source_audit_sha256: String,
    source_trace_sha256: String,
    candidate_rows: usize,
    accepted: usize,
    ambiguous: usize,
    unsupported: usize,
    replay_verified: usize,
    records: Vec<HleRecoveryRow>,
}

#[derive(Debug, Serialize)]
struct HleRecoveryRow {
    id: Option<String>,
    status: NormalizationStatus,
    replay_verified: bool,
    symbol_bindings: Vec<String>,
    reason: String,
}

#[derive(Debug, Serialize)]
struct Report {
    corpus_sha256: String,
    corpus_cases: usize,
    development_cases: usize,
    boundary_cases: usize,
    supported_expected: usize,
    ambiguous_expected: usize,
    unsupported_expected: usize,
    correct_decisions: usize,
    false_accepts: usize,
    false_rejections: usize,
    replay_verified: usize,
    rewrite_groups: usize,
    rewrite_regressions: usize,
    results: Vec<CaseResult>,
    hle_recovery: Option<HleRecovery>,
    method: String,
}

fn hash<T: Serialize>(value: &T) -> String {
    let bytes = serde_json::to_vec(value).expect("phase 24 data serializes");
    format!("{:x}", Sha256::digest(bytes))
}

fn independent_corpus() -> Vec<CorpusCase> {
    let supported = [
        "Solve \\(x + 1 = 2x\\).",
        "Let y be defined by \\(2y = 10\\). Find y.",
        "The relation is \\(a = b + c\\), with a,b,c locally scoped.",
        "Evaluate \\(\\frac{x+1}{2} = 3\\).",
        "Rewrite the equation \\(\\left(3x\\right) = 9\\).",
        "Given \\(u^2 + v^2 = 1\\), identify the constraint.",
        "The display formula is $$m = \\frac{p}{q}$$.",
        "For the locally defined variable z, use \\(z - 4 = 0\\).",
        "The chained prose says that \\(r = s + 1\\).",
        "Use the expression \\(f(x) = x^2 + 1\\).",
    ];
    let mut cases = Vec::new();
    for repeat in 0..4 {
        for (index, prompt) in supported.iter().enumerate() {
            cases.push(CorpusCase {
                id: format!("supported-{repeat:02}-{index:02}"),
                split: "development".into(),
                prompt: prompt.to_string(),
                expected: NormalizationStatus::Accepted,
                rewrite_group: None,
            });
        }
    }
    let ambiguous = [
        "What is \\(x = y = z\\)?",
        "The symbols are present but no equation is supplied.",
        "Solve \\(x + ? = 2\\).",
        "Which convention is intended for \\perp?",
        "Evaluate the expression shown later.",
        "Given \\(a = b\\) and \\(b = c\\), choose one equality.",
        "A formula is referenced but its variables are not specified.",
        "The answer depends on whether |x| denotes a norm or cardinality.",
        "Use the displayed relation without stating its scope.",
        "Find the value of an unspecified symbol.",
    ];
    for (index, prompt) in ambiguous.iter().enumerate() {
        cases.push(CorpusCase {
            id: format!("ambiguous-{index:02}"),
            split: "boundary".into(),
            prompt: prompt.to_string(),
            expected: NormalizationStatus::Ambiguous,
            rewrite_group: None,
        });
    }
    let unsupported = [
        "\\[\\begin{bmatrix}1&0\\\\0&1\\end{bmatrix}\\]",
        "Use \\text{the standard Cartan convention} in \\(K_{ij}\\).",
        "Interpret the appended picture of the commutative diagram.",
        "Compute the Hurwitz invariant using \\operatorname{Hurwitz}.",
        "Use the domain-specific operator \\operatorname{curl}.",
        "Read the chemical structure from the attached image diagram.",
        "Apply the unprovided physics convention to \\(\\gamma_\\mu\\).",
        "The formula contains an unsupported \\int with visual bounds.",
        "Use the biological nomenclature in the attached image.",
        "Evaluate a matrix expression requiring layout semantics.",
    ];
    for (index, prompt) in unsupported.iter().enumerate() {
        cases.push(CorpusCase {
            id: format!("unsupported-{index:02}"),
            split: "boundary".into(),
            prompt: prompt.to_string(),
            expected: NormalizationStatus::Unsupported,
            rewrite_group: None,
        });
    }
    let rewrites = [
        ("x+1=2", "x + 1 = 2"),
        ("2x=10", "10 = 2x"),
        ("a=b+c", "a = b + c"),
        ("\\frac{x}{2}=3", "\\dfrac{x}{2} = 3"),
        ("\\left(y-1\\right)=0", "y - 1 = 0"),
        ("z^2=4", "z ^ 2 = 4"),
        ("m=p/q", "m = p / q"),
        ("r=s+1", "r = s + 1"),
        ("f(x)=x^2", "f(x) = x ^ 2"),
        ("u=v", "u = v"),
    ];
    for (index, (left, right)) in rewrites.iter().enumerate() {
        for (variant, prompt) in [format!("\\({left}\\)"), format!("\\({right}\\)")]
            .iter()
            .enumerate()
        {
            cases.push(CorpusCase {
                id: format!("rewrite-{index:02}-{variant}"),
                split: "development".into(),
                prompt: prompt.clone(),
                expected: NormalizationStatus::Accepted,
                rewrite_group: Some(format!("rewrite-{index:02}")),
            });
        }
    }
    cases
}

fn evaluate(cases: &[CorpusCase]) -> (Vec<CaseResult>, usize, usize) {
    let mut results = Vec::new();
    let mut correct = 0;
    let mut replay = 0;
    for case in cases {
        let result = normalize_equation(&case.prompt);
        let is_correct = result.status == case.expected;
        correct += usize::from(is_correct);
        replay +=
            usize::from(result.status == NormalizationStatus::Accepted && result.replay_verified);
        results.push(CaseResult {
            id: case.id.clone(),
            split: case.split.clone(),
            expected: case.expected,
            actual: result.status,
            correct: is_correct,
            replay_verified: result.replay_verified,
            symbol_bindings: result.symbol_bindings,
            reason: result.reason,
            rewrite_group: case.rewrite_group.clone(),
        });
    }
    (results, correct, replay)
}

fn hle_recovery(path: &str) -> Result<HleRecovery, Box<dyn std::error::Error>> {
    let bytes = fs::read(path)?;
    let audit: HleAudit = serde_json::from_slice(&bytes)?;
    let rows: Vec<_> = audit
        .records
        .into_iter()
        .filter(|row| {
            row.notation_family == "equations_and_expressions"
                && row.downstream_outlook == "likely_normalization_only"
        })
        .collect();
    let mut accepted = 0;
    let mut ambiguous = 0;
    let mut unsupported = 0;
    let mut replay = 0;
    let mut records = Vec::new();
    for row in rows.iter() {
        let result = normalize_equation(&row.question);
        match result.status {
            NormalizationStatus::Accepted => accepted += 1,
            NormalizationStatus::Ambiguous => ambiguous += 1,
            NormalizationStatus::Unsupported => unsupported += 1,
        }
        replay +=
            usize::from(result.status == NormalizationStatus::Accepted && result.replay_verified);
        records.push(HleRecoveryRow {
            id: row.id.clone(),
            status: result.status,
            replay_verified: result.replay_verified,
            symbol_bindings: result.symbol_bindings,
            reason: result.reason,
        });
    }
    Ok(HleRecovery {
        source_audit_sha256: format!("{:x}", Sha256::digest(bytes)),
        source_trace_sha256: audit.source_trace_sha256,
        candidate_rows: rows.len(),
        accepted,
        ambiguous,
        unsupported,
        replay_verified: replay,
        records,
    })
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output = env::args()
        .nth(1)
        .unwrap_or_else(|| "/tmp/hle_notation_recovery_2147e9e.json".into());
    let audit_path = env::args()
        .nth(2)
        .unwrap_or_else(|| "/tmp/hle_notation_audit_2147e9e.json".into());
    let cases = independent_corpus();
    let (results, correct, replay) = evaluate(&cases);
    let mut expected = BTreeMap::new();
    let mut splits = BTreeMap::new();
    let mut groups = BTreeMap::new();
    for case in &cases {
        *expected.entry(case.expected).or_insert(0usize) += 1;
        *splits.entry(case.split.clone()).or_insert(0usize) += 1;
        if let Some(group) = &case.rewrite_group {
            groups
                .entry(group.clone())
                .or_insert(Vec::new())
                .push(case.id.clone());
        }
    }
    let mut rewrite_regressions = 0;
    for ids in groups.values() {
        let statuses: Vec<_> = results
            .iter()
            .filter(|result| ids.contains(&result.id))
            .map(|result| result.actual)
            .collect();
        if statuses
            .iter()
            .any(|status| *status != NormalizationStatus::Accepted)
            || statuses.windows(2).any(|pair| pair[0] != pair[1])
        {
            rewrite_regressions += 1;
        }
    }
    let hle = hle_recovery(&audit_path).ok();
    let false_accepts = results
        .iter()
        .filter(|result| {
            result.expected != NormalizationStatus::Accepted
                && result.actual == NormalizationStatus::Accepted
        })
        .count();
    let false_rejections = results
        .iter()
        .filter(|result| {
            result.expected == NormalizationStatus::Accepted
                && result.actual != NormalizationStatus::Accepted
        })
        .count();
    let report = Report { corpus_sha256: hash(&cases), corpus_cases: cases.len(), development_cases: *splits.get("development").unwrap_or(&0), boundary_cases: *splits.get("boundary").unwrap_or(&0), supported_expected: *expected.get(&NormalizationStatus::Accepted).unwrap_or(&0), ambiguous_expected: *expected.get(&NormalizationStatus::Ambiguous).unwrap_or(&0), unsupported_expected: *expected.get(&NormalizationStatus::Unsupported).unwrap_or(&0), correct_decisions: correct, false_accepts, false_rejections, replay_verified: replay, rewrite_groups: groups.len(), rewrite_regressions, results, hle_recovery: hle, method: "shadow-only bounded equations/expressions normalization; independent corpus and HLE reclassification are non-authorizing".into() };
    fs::write(&output, serde_json::to_vec_pretty(&report)?)?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn corpus_is_deterministic_and_has_positive_negative_boundaries() {
        let corpus = independent_corpus();
        assert_eq!(corpus.len(), 80);
        assert_eq!(hash(&corpus), hash(&independent_corpus()));
        let (results, correct, _) = evaluate(&corpus);
        assert_eq!(results.len(), 80);
        assert_eq!(correct, 80);
        assert_eq!(
            results
                .iter()
                .filter(|r| r.expected == NormalizationStatus::Accepted)
                .count(),
            60
        );
        assert_eq!(
            results
                .iter()
                .filter(|r| r.expected == NormalizationStatus::Ambiguous)
                .count(),
            10
        );
        assert_eq!(
            results
                .iter()
                .filter(|r| r.expected == NormalizationStatus::Unsupported)
                .count(),
            10
        );
    }
}
