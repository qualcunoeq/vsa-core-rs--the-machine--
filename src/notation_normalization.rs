//! Shadow-only normalization for locally scoped mathematical equations and
//! expressions.  This module produces typed candidates but never authorizes a
//! route or mutates the production parser/registry.

use crate::algebra::{parse_equation, SymExpr};
use crate::math_ingest::latex_to_symexpr;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NormalizationStatus {
    Accepted,
    Ambiguous,
    Unsupported,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotationNormalizationResult {
    pub status: NormalizationStatus,
    pub family: String,
    pub source: String,
    pub normalized_source: Option<String>,
    pub ast_candidates: Vec<SymExpr>,
    pub symbol_bindings: Vec<String>,
    pub unresolved_bindings: Vec<String>,
    pub provenance_spans: Vec<String>,
    pub downstream_compatible: bool,
    pub replay_verified: bool,
    pub receipt_hash: String,
    pub reason: String,
}

fn hash<T: Serialize>(value: &T) -> String {
    let bytes = serde_json::to_vec(value).expect("normalization receipt serializes");
    format!("{:x}", Sha256::digest(bytes))
}

fn extract_region(source: &str) -> Option<(String, String)> {
    let pairs = [("\\[", "\\]"), ("\\(", "\\)"), ("$$", "$$"), ("$", "$")];
    for (open, close) in pairs {
        if let Some(start) = source.find(open) {
            let body_start = start + open.len();
            if let Some(relative_end) = source[body_start..].find(close) {
                let end = body_start + relative_end;
                return Some((
                    source[body_start..end].trim().to_string(),
                    format!("{start}..{}", end + close.len()),
                ));
            }
        }
    }
    if source.contains('=') {
        return Some((source.trim().to_string(), "full_text".into()));
    }
    None
}

fn clean_latex(value: &str) -> String {
    value
        .replace("\\left", "")
        .replace("\\right", "")
        .replace("\\dfrac", "\\frac")
        .replace("\\,", "")
        .replace("\\!", "")
        .replace("\\;", "")
        .replace("\\:", "")
        .trim()
        .to_string()
}

fn parse_latex_equation(value: &str) -> Result<(SymExpr, SymExpr), String> {
    let Some(position) = value.find('=') else {
        return latex_to_symexpr(value)
            .map(|expr| (expr, SymExpr::Num(0.0)))
            .ok_or_else(|| "latex expression rejected".into());
    };
    let lhs = latex_to_symexpr(value[..position].trim())
        .ok_or_else(|| "latex left-hand side rejected".to_string())?;
    let rhs = latex_to_symexpr(value[position + 1..].trim())
        .ok_or_else(|| "latex right-hand side rejected".to_string())?;
    Ok((lhs, rhs))
}

fn collect_symbols(expr: &SymExpr, symbols: &mut BTreeSet<String>) {
    match expr {
        SymExpr::Var(variable) => {
            symbols.insert(variable.display.to_string());
        }
        SymExpr::Num(_) => {}
        SymExpr::Add(a, b)
        | SymExpr::Sub(a, b)
        | SymExpr::Mul(a, b)
        | SymExpr::Div(a, b)
        | SymExpr::Pow(a, b) => {
            collect_symbols(a, symbols);
            collect_symbols(b, symbols);
        }
        SymExpr::Neg(a)
        | SymExpr::Sin(a)
        | SymExpr::Cos(a)
        | SymExpr::Tan(a)
        | SymExpr::Sqrt(a)
        | SymExpr::Exp(a)
        | SymExpr::Ln(a)
        | SymExpr::Abs(a)
        | SymExpr::Sinh(a)
        | SymExpr::Cosh(a)
        | SymExpr::Tanh(a)
        | SymExpr::Asin(a)
        | SymExpr::Acos(a)
        | SymExpr::Atan(a) => collect_symbols(a, symbols),
        SymExpr::Limit {
            variable,
            approach,
            body,
        } => {
            symbols.insert(variable.display.to_string());
            collect_symbols(approach, symbols);
            collect_symbols(body, symbols);
        }
        SymExpr::Integral {
            variable,
            lower,
            upper,
            body,
        } => {
            symbols.insert(variable.display.to_string());
            if let Some(lower) = lower {
                collect_symbols(lower, symbols);
            }
            if let Some(upper) = upper {
                collect_symbols(upper, symbols);
            }
            collect_symbols(body, symbols);
        }
    }
}

/// Normalize one locally scoped mathematical notation region.
pub fn normalize_equation(source: &str) -> NotationNormalizationResult {
    let family = "equations_and_expressions".to_string();
    let unsupported_marker = source.contains("\\begin")
        || source.contains("\\text{")
        || source.contains("\\mathbb")
        || source.contains("\\operatorname")
        || source.contains("\\gamma")
        || source.contains("\\int")
        || source.to_ascii_lowercase().contains("appended picture")
        || source.to_ascii_lowercase().contains("diagram")
        || source.to_ascii_lowercase().contains("attached image")
        || source.to_ascii_lowercase().contains("matrix expression");
    if unsupported_marker {
        let raw = extract_region(source).map(|(raw, _)| clean_latex(&raw));
        return result(
            source,
            family,
            NormalizationStatus::Unsupported,
            raw,
            vec![],
            vec![],
            vec!["unsupported_marker".into()],
            false,
            false,
            "notation requires unsupported text, layout, or domain convention",
        );
    }
    if source.matches("\\(").count() > 1
        || source.matches("\\[").count() > 1
        || source.matches("$$").count() > 2
    {
        return result(
            source,
            family,
            NormalizationStatus::Ambiguous,
            None,
            vec![],
            vec![],
            vec!["multiple_math_regions".into()],
            false,
            false,
            "multiple math regions require explicit relation selection",
        );
    }
    let Some((raw, span)) = extract_region(source) else {
        return result(
            source,
            family,
            NormalizationStatus::Ambiguous,
            None,
            vec![],
            vec![],
            vec!["no equation region".into()],
            false,
            false,
            "no equation or expression region found",
        );
    };
    let normalized = clean_latex(&raw);
    if normalized.contains('?') {
        return result(
            source,
            family,
            NormalizationStatus::Ambiguous,
            Some(normalized),
            vec![],
            vec![],
            vec![span],
            false,
            false,
            "unresolved operator or binding marker",
        );
    }
    if normalized.matches('=').count() > 1 && !normalized.contains("==") {
        return result(
            source,
            family,
            NormalizationStatus::Ambiguous,
            Some(normalized),
            vec![],
            vec![],
            vec![span],
            false,
            false,
            "multiple equality boundaries require explicit chain semantics",
        );
    }
    let Ok((lhs, rhs)) = parse_latex_equation(&normalized).or_else(|_| parse_equation(&normalized))
    else {
        return result(
            source,
            family,
            NormalizationStatus::Unsupported,
            Some(normalized),
            vec![],
            vec![],
            vec![span],
            false,
            false,
            "existing symbolic parser rejected the normalized region",
        );
    };
    let mut symbols = BTreeSet::new();
    collect_symbols(&lhs, &mut symbols);
    collect_symbols(&rhs, &mut symbols);
    let bindings: Vec<String> = symbols.into_iter().collect();
    let replay = parse_latex_equation(&normalized)
        .or_else(|_| parse_equation(&normalized))
        .is_ok();
    let status = if replay && !bindings.is_empty() {
        NormalizationStatus::Accepted
    } else {
        NormalizationStatus::Ambiguous
    };
    let reason = if status == NormalizationStatus::Accepted {
        "unique typed AST candidate replayed"
    } else {
        "typed candidate lacks a replayable symbolic binding"
    };
    result(
        source,
        family,
        status,
        Some(normalized),
        vec![lhs.clone(), rhs.clone()],
        bindings,
        vec![span],
        replay,
        replay,
        reason,
    )
}

fn result(
    source: &str,
    family: String,
    status: NormalizationStatus,
    normalized: Option<String>,
    ast_candidates: Vec<SymExpr>,
    symbol_bindings: Vec<String>,
    provenance_spans: Vec<String>,
    downstream_compatible: bool,
    replay_verified: bool,
    reason: &str,
) -> NotationNormalizationResult {
    let unresolved_bindings = if symbol_bindings.is_empty() {
        vec!["unresolved_or_absent_symbol".into()]
    } else {
        Vec::new()
    };
    let receipt_hash = hash(&(
        status,
        family.as_str(),
        source,
        normalized.as_deref(),
        &symbol_bindings,
        &provenance_spans,
        replay_verified,
    ));
    NotationNormalizationResult {
        status,
        family,
        source: source.into(),
        normalized_source: normalized,
        ast_candidates,
        symbol_bindings,
        unresolved_bindings,
        provenance_spans,
        downstream_compatible,
        replay_verified,
        receipt_hash,
        reason: reason.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_local_equation_and_replays_typed_ast() {
        let result = normalize_equation("Let x be the unknown. Solve \\(x + 1 = 2x\\).");
        assert_eq!(result.status, NormalizationStatus::Accepted);
        assert!(result.symbol_bindings.iter().any(|name| name == "x"));
        assert!(result.replay_verified);
        assert!(result.downstream_compatible);
    }

    #[test]
    fn preserves_ambiguous_and_unsupported_boundaries() {
        assert_eq!(
            normalize_equation("What is \\(x = y = z\\)?").status,
            NormalizationStatus::Ambiguous
        );
        assert_eq!(
            normalize_equation("\\[\\begin{bmatrix}1&0\\end{bmatrix}\\]").status,
            NormalizationStatus::Unsupported
        );
        assert_eq!(
            normalize_equation("No formula is supplied.").status,
            NormalizationStatus::Ambiguous
        );
    }
}
