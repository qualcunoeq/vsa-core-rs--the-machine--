//! Source-derived bounded propositional truth-table reasoning.

use crate::source_formula_pack::SourceCitation;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

pub const DOMAIN: &str = "source_derived_bounded_truth_tables";
pub const SOURCE_ID: &str = "openstax-contemporary-mathematics:truth-tables";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum LogicExpr {
    Var(String),
    True,
    False,
    Not(Box<LogicExpr>),
    And(Box<LogicExpr>, Box<LogicExpr>),
    Or(Box<LogicExpr>, Box<LogicExpr>),
    Implies(Box<LogicExpr>, Box<LogicExpr>),
    Iff(Box<LogicExpr>, Box<LogicExpr>),
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LogicOperation {
    Evaluate,
    Tautology,
    Contradiction,
    Equivalent,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LogicRequest {
    pub operation: LogicOperation,
    pub expression: LogicExpr,
    pub comparison: Option<LogicExpr>,
    pub assignments: Vec<(String, bool)>,
    pub ambiguity: Option<String>,
    pub provenance: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum LogicArtifact {
    Boolean(bool),
    TruthTable(Vec<(Vec<bool>, bool)>),
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LogicStatus {
    Complete,
    Missing,
    Ambiguous,
    Unsupported,
    TooManyVariables,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LogicResult {
    pub status: LogicStatus,
    pub artifact: Option<LogicArtifact>,
    pub operation: LogicOperation,
    pub source: SourceCitation,
    pub reasons: Vec<String>,
    pub provenance: Vec<String>,
    pub replay_hash: String,
}
pub fn source() -> SourceCitation {
    SourceCitation{source_id:SOURCE_ID.into(),title:"Contemporary Mathematics".into(),section:"2.3 Constructing Truth Tables; 2.4 Truth Tables for the Conditional and Biconditional".into(),url:"https://openstax.org/books/contemporary-mathematics/pages/2-3-constructing-truth-tables".into(),license:"CC BY-NC-SA 4.0; OpenStax attribution required".into(),retrieved_utc:"2026-08-17".into(),evidence_span:"truth values for negation, conjunction, disjunction, conditional, biconditional, and exhaustive validity tables".into()}
}
pub fn validate_source_document(d: &str) -> bool {
    [
        "SOURCE_ID:",
        "URL:",
        "EVIDENCE:",
        "negation",
        "conjunction",
        "biconditional",
    ]
    .iter()
    .all(|m| d.to_ascii_lowercase().contains(&m.to_ascii_lowercase()))
}
fn digest<T: Serialize>(v: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(v).unwrap()))
}
fn eval(e: &LogicExpr, a: &[(String, bool)]) -> Option<bool> {
    match e {
        LogicExpr::True => Some(true),
        LogicExpr::False => Some(false),
        LogicExpr::Var(n) => a.iter().find(|(k, _)| k == n).map(|(_, v)| *v),
        LogicExpr::Not(x) => Some(!eval(x, a)?),
        LogicExpr::And(x, y) => Some(eval(x, a)? && eval(y, a)?),
        LogicExpr::Or(x, y) => Some(eval(x, a)? || eval(y, a)?),
        LogicExpr::Implies(x, y) => Some(!eval(x, a)? || eval(y, a)?),
        LogicExpr::Iff(x, y) => Some(eval(x, a)? == eval(y, a)?),
    }
}
fn vars(e: &LogicExpr, o: &mut BTreeSet<String>) {
    match e {
        LogicExpr::Var(n) => {
            o.insert(n.clone());
        }
        LogicExpr::True | LogicExpr::False => {}
        LogicExpr::Not(x) => vars(x, o),
        LogicExpr::And(x, y)
        | LogicExpr::Or(x, y)
        | LogicExpr::Implies(x, y)
        | LogicExpr::Iff(x, y) => {
            vars(x, o);
            vars(y, o);
        }
    }
}
fn finish(mut r: LogicResult) -> LogicResult {
    r.replay_hash.clear();
    r.replay_hash = digest(&r);
    r
}
pub fn replay_verified(r: &LogicResult) -> bool {
    let mut c = r.clone();
    let h = c.replay_hash.clone();
    c.replay_hash.clear();
    h == digest(&c) && !r.provenance.is_empty()
}
pub fn evaluate(q: &LogicRequest) -> LogicResult {
    let base = |s, a, why: Vec<String>| {
        finish(LogicResult {
            status: s,
            artifact: a,
            operation: q.operation,
            source: source(),
            reasons: why,
            provenance: q.provenance.clone(),
            replay_hash: String::new(),
        })
    };
    if q.provenance.is_empty() {
        return base(
            LogicStatus::Missing,
            None,
            vec!["provenance required".into()],
        );
    }
    if let Some(x) = &q.ambiguity {
        return base(LogicStatus::Ambiguous, None, vec![x.clone()]);
    }
    let mut set = BTreeSet::new();
    vars(&q.expression, &mut set);
    if let Some(c) = &q.comparison {
        vars(c, &mut set);
    }
    if set.len() > 4 {
        return base(
            LogicStatus::TooManyVariables,
            None,
            vec!["truth tables are bounded to four variables".into()],
        );
    }
    match q.operation {
        LogicOperation::Evaluate => {
            if q.assignments.len() != set.len()
                || set
                    .iter()
                    .any(|n| !q.assignments.iter().any(|(k, _)| k == n))
            {
                return base(
                    LogicStatus::Missing,
                    None,
                    vec!["every variable needs one explicit truth assignment".into()],
                );
            }
            base(
                LogicStatus::Complete,
                Some(LogicArtifact::Boolean(
                    eval(&q.expression, &q.assignments).unwrap(),
                )),
                Vec::new(),
            )
        }
        LogicOperation::Equivalent => {
            let Some(c) = &q.comparison else {
                return base(
                    LogicStatus::Missing,
                    None,
                    vec!["equivalence requires two expressions".into()],
                );
            };
            let names: Vec<_> = set.into_iter().collect();
            let mut table = Vec::new();
            for mask in 0..(1usize << names.len()) {
                let a = names
                    .iter()
                    .enumerate()
                    .map(|(i, n)| (n.clone(), mask & (1 << i) != 0))
                    .collect::<Vec<_>>();
                table.push((
                    a.iter().map(|(_, v)| *v).collect(),
                    eval(&q.expression, &a).unwrap() == eval(c, &a).unwrap(),
                ));
            }
            base(
                LogicStatus::Complete,
                Some(LogicArtifact::TruthTable(table)),
                Vec::new(),
            )
        }
        LogicOperation::Tautology | LogicOperation::Contradiction => {
            let names: Vec<_> = set.into_iter().collect();
            let mut table = Vec::new();
            for mask in 0..(1usize << names.len()) {
                let a = names
                    .iter()
                    .enumerate()
                    .map(|(i, n)| (n.clone(), mask & (1 << i) != 0))
                    .collect::<Vec<_>>();
                table.push((
                    a.iter().map(|(_, v)| *v).collect(),
                    eval(&q.expression, &a).unwrap(),
                ));
            }
            base(
                LogicStatus::Complete,
                Some(LogicArtifact::TruthTable(table)),
                Vec::new(),
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn q(e: LogicExpr) -> LogicRequest {
        LogicRequest {
            operation: LogicOperation::Evaluate,
            expression: e,
            comparison: None,
            assignments: vec![("p".into(), true)],
            ambiguity: None,
            provenance: vec!["test".into()],
        }
    }
    #[test]
    fn evaluates_not() {
        let r = evaluate(&q(LogicExpr::Not(Box::new(LogicExpr::Var("p".into())))));
        assert_eq!(r.artifact, Some(LogicArtifact::Boolean(false)));
        assert!(replay_verified(&r));
    }
}
