//! Bounded frontend for explicit propositional expressions.
use crate::source_logic_pack::{LogicExpr, LogicOperation, LogicRequest};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LogicFrontendStatus {
    Complete,
    Ambiguous,
    Missing,
    Unsupported,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LogicFrontendResult {
    pub status: LogicFrontendStatus,
    pub request: Option<LogicRequest>,
    pub unresolved: Vec<String>,
    pub provenance: Vec<String>,
    pub replay_hash: String,
}
fn digest<T: Serialize>(v: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(v).unwrap()))
}
fn finish(mut r: LogicFrontendResult) -> LogicFrontendResult {
    r.replay_hash.clear();
    r.replay_hash = digest(&r);
    r
}
pub fn replay_verified(r: &LogicFrontendResult) -> bool {
    let mut c = r.clone();
    let h = c.replay_hash.clone();
    c.replay_hash.clear();
    h == digest(&c) && !r.provenance.is_empty()
}
fn tokens(s: &str) -> Vec<String> {
    s.replace("(", " ( ")
        .replace(")", " ) ")
        .replace("<->", " iff ")
        .replace("->", " implies ")
        .split_whitespace()
        .map(|x| x.to_ascii_lowercase())
        .collect()
}
fn atom(t: &[String], i: &mut usize) -> Option<LogicExpr> {
    if *i >= t.len() {
        return None;
    }
    let x = &t[*i];
    if x == "(" {
        *i += 1;
        let e = iff(t, i)?;
        if t.get(*i)? != ")" {
            return None;
        }
        *i += 1;
        Some(e)
    } else {
        *i += 1;
        match x.as_str() {
            "true" => Some(LogicExpr::True),
            "false" => Some(LogicExpr::False),
            n if n.len() == 1 && n.chars().next()?.is_ascii_alphabetic() => {
                Some(LogicExpr::Var(n.into()))
            }
            _ => None,
        }
    }
}
fn not(t: &[String], i: &mut usize) -> Option<LogicExpr> {
    if t.get(*i).is_some_and(|x| x == "not" || x == "~") {
        *i += 1;
        Some(LogicExpr::Not(Box::new(not(t, i)?)))
    } else {
        atom(t, i)
    }
}
fn and(t: &[String], i: &mut usize) -> Option<LogicExpr> {
    let mut e = not(t, i)?;
    while t.get(*i).is_some_and(|x| x == "and" || x == "∧") {
        *i += 1;
        e = LogicExpr::And(Box::new(e), Box::new(not(t, i)?));
    }
    Some(e)
}
fn or(t: &[String], i: &mut usize) -> Option<LogicExpr> {
    let mut e = and(t, i)?;
    while t.get(*i).is_some_and(|x| x == "or" || x == "∨") {
        *i += 1;
        e = LogicExpr::Or(Box::new(e), Box::new(and(t, i)?));
    }
    Some(e)
}
fn iff(t: &[String], i: &mut usize) -> Option<LogicExpr> {
    let mut e = or(t, i)?;
    while let Some(op) = t.get(*i).map(String::as_str) {
        if op != "implies" && op != "iff" {
            break;
        }
        *i += 1;
        let r = or(t, i)?;
        e = if op == "implies" {
            LogicExpr::Implies(Box::new(e), Box::new(r))
        } else {
            LogicExpr::Iff(Box::new(e), Box::new(r))
        };
    }
    Some(e)
}
fn expression(s: &str) -> Option<LogicExpr> {
    let t = tokens(s);
    let mut i = 0;
    let e = iff(&t, &mut i)?;
    if i == t.len() {
        Some(e)
    } else {
        None
    }
}
fn assignment(s: &str) -> Vec<(String, bool)> {
    s.split(',')
        .filter_map(|part| {
            let (p, v) = part.trim().split_once('=')?;
            Some((
                p.trim().to_ascii_lowercase(),
                match v.trim().trim_end_matches('.').to_ascii_lowercase().as_str() {
                    "true" | "t" => true,
                    "false" | "f" => false,
                    _ => return None,
                },
            ))
        })
        .collect()
}
pub fn formalize_logic_text(text: &str, case_id: &str) -> LogicFrontendResult {
    let lower = text.to_ascii_lowercase();
    let provenance = vec![
        format!("source-logic-frontend:{case_id}"),
        "explicit-propositional-parser".into(),
    ];
    let fail = |s: LogicFrontendStatus, u: &str| {
        finish(LogicFrontendResult {
            status: s,
            request: None,
            unresolved: vec![u.into()],
            provenance: provenance.clone(),
            replay_hash: String::new(),
        })
    };
    if [
        "quantifier",
        "forall",
        "exists",
        "fuzzy",
        "predicate",
        "probability",
        "infinite",
    ]
    .iter()
    .any(|x| lower.contains(x))
    {
        return fail(LogicFrontendStatus::Unsupported,"quantified, fuzzy, predicate, probabilistic, or infinite semantics are outside the bounded pack");
    }
    if lower.contains(" either ") {
        return fail(
            LogicFrontendStatus::Ambiguous,
            "logical target or connective is not unique",
        );
    }
    let operation = if lower.contains("equivalent") {
        LogicOperation::Equivalent
    } else if lower.contains("tautology")
        || lower.contains("always true")
        || lower.contains("valid")
    {
        LogicOperation::Tautology
    } else if lower.contains("contradiction") || lower.contains("always false") {
        LogicOperation::Contradiction
    } else if lower.contains("evaluate")
        || lower.contains("truth value")
        || lower.contains("determine")
    {
        LogicOperation::Evaluate
    } else {
        return fail(
            LogicFrontendStatus::Unsupported,
            "no bounded propositional operation is explicit",
        );
    };
    let body = lower
        .split_once(" with ")
        .map(|(a, _)| a)
        .or_else(|| lower.split_once(" where ").map(|(a, _)| a))
        .unwrap_or(&lower);
    let body = body
        .strip_prefix("evaluate ")
        .or_else(|| body.strip_prefix("determine "))
        .or_else(|| body.strip_prefix("find "))
        .unwrap_or(body);
    let body = body
        .strip_prefix("whether ")
        .or_else(|| body.strip_prefix("are "))
        .unwrap_or(body)
        .trim_end_matches(['.', '?']);
    let body = body
        .strip_suffix(" is a tautology")
        .or_else(|| body.strip_suffix(" is a contradiction"))
        .unwrap_or(body);
    let expr_text = body.split_once(':').map(|(_, x)| x).unwrap_or(body).trim();
    let (first, second) = if operation == LogicOperation::Equivalent {
        match expr_text.split_once(" equivalent to ") {
            Some((a, b)) => (a, b),
            None => {
                return fail(
                    LogicFrontendStatus::Missing,
                    "equivalence needs two explicit expressions",
                )
            }
        }
    } else {
        (expr_text, "")
    };
    let Some(expr) = expression(first.trim()) else {
        return fail(
            LogicFrontendStatus::Missing,
            "expression is outside the bounded grammar",
        );
    };
    let comparison = if operation == LogicOperation::Equivalent {
        match expression(second.trim()) {
            Some(x) => Some(x),
            None => {
                return fail(
                    LogicFrontendStatus::Missing,
                    "comparison expression is outside the bounded grammar",
                )
            }
        }
    } else {
        None
    };
    let assignments = lower
        .split_once(" with ")
        .map(|(_, x)| assignment(x))
        .unwrap_or_default();
    if operation == LogicOperation::Evaluate && assignments.is_empty() {
        return fail(
            LogicFrontendStatus::Missing,
            "evaluation requires explicit variable truth assignments",
        );
    };
    finish(LogicFrontendResult {
        status: LogicFrontendStatus::Complete,
        request: Some(LogicRequest {
            operation,
            expression: expr,
            comparison,
            assignments,
            ambiguity: None,
            provenance: provenance.clone(),
        }),
        unresolved: Vec::new(),
        provenance,
        replay_hash: String::new(),
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parse_eval() {
        let r = formalize_logic_text("Evaluate not p with p=true", "t");
        assert_eq!(r.status, LogicFrontendStatus::Complete);
        assert!(replay_verified(&r));
    }
}
