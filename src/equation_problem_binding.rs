//! Shadow, non-authorizing binding of equation problems.
//!
//! `EquationProblemBindingV1` deliberately stops at a typed problem
//! representation.  It does not select a solver and it never authorizes an
//! answer merely because symbols were successfully extracted.

use regex::Regex;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BindingStatus {
    Complete,
    Ambiguous,
    Unsupported,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeclaredStatus {
    Declared,
    Inferred,
    Mixed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceSpan {
    pub start: usize,
    pub end: usize,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SymbolScope {
    pub id: String,
    pub parent: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SymbolBinding {
    pub symbol: String,
    pub expression: Option<String>,
    pub type_name: Option<String>,
    pub domain: Option<String>,
    pub declared_status: DeclaredStatus,
    pub scope: SymbolScope,
    pub source_spans: Vec<SourceSpan>,
    pub unresolved_alternatives: Vec<String>,
    pub assumptions: Vec<String>,
    pub dependencies: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RequestedUnknown {
    pub candidates: Vec<String>,
    pub selected: Option<String>,
    pub source_spans: Vec<SourceSpan>,
    pub unresolved_alternatives: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IndexedObjectBinding {
    pub object: String,
    pub index: String,
    pub index_domain: Option<String>,
    pub body: Option<String>,
    pub declared_status: DeclaredStatus,
    pub source_spans: Vec<SourceSpan>,
    pub unresolved_alternatives: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FunctionDomainBinding {
    pub function: String,
    pub domain: Option<String>,
    pub codomain: Option<String>,
    pub declared_status: DeclaredStatus,
    pub source_spans: Vec<SourceSpan>,
    pub unresolved_alternatives: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParenthesizedForm {
    FunctionApplication,
    Grouping,
    Tuple,
    Interval,
    OperatorArgument,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParenthesizedCandidate {
    pub head: Option<String>,
    pub body: String,
    pub form: ParenthesizedForm,
    pub source_spans: Vec<SourceSpan>,
    pub evidence: Vec<String>,
    pub declared: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConstraintArtifact {
    pub expression: String,
    pub symbols: Vec<String>,
    pub kind: String,
    pub source_spans: Vec<SourceSpan>,
    pub assumptions: Vec<String>,
    pub dependencies: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EquationProblemBinding {
    pub status: BindingStatus,
    pub input: String,
    pub symbols: Vec<SymbolBinding>,
    pub requested_unknown: RequestedUnknown,
    pub indexed_objects: Vec<IndexedObjectBinding>,
    pub function_domains: Vec<FunctionDomainBinding>,
    pub parenthesized_candidates: Vec<ParenthesizedCandidate>,
    pub constraints: Vec<ConstraintArtifact>,
    pub assumptions: Vec<String>,
    pub unresolved_alternatives: Vec<String>,
    pub dependencies: Vec<String>,
    pub reason: String,
    pub replay_hash: String,
    /// Always false for this shadow bridge.  A downstream solver must be
    /// selected and independently verified by a later capability.
    pub downstream_authorized: bool,
}

impl EquationProblemBinding {
    pub fn replay_verified(&self) -> bool {
        self.replay_hash == replay_hash(self)
            && !self.input.is_empty()
            && !self.downstream_authorized
            && self
                .symbols
                .iter()
                .all(|binding| !binding.symbol.is_empty() && !binding.scope.id.is_empty())
            && self.constraints.iter().all(|constraint| {
                !constraint.expression.is_empty()
                    && constraint.dependencies.iter().all(|name| {
                        self.symbols.iter().any(|binding| binding.symbol == *name)
                            || self
                                .requested_unknown
                                .candidates
                                .iter()
                                .any(|candidate| candidate == name)
                            || self
                                .indexed_objects
                                .iter()
                                .any(|object| object.object == *name || object.index == *name)
                            || ["kg", "s", "m/s", "meter", "meters", "joule", "newton"]
                                .contains(&name.as_str())
                    })
            })
    }
}

fn replay_hash(binding: &EquationProblemBinding) -> String {
    #[derive(Serialize)]
    struct Payload<'a> {
        status: BindingStatus,
        input: &'a str,
        symbols: &'a [SymbolBinding],
        requested_unknown: &'a RequestedUnknown,
        indexed_objects: &'a [IndexedObjectBinding],
        function_domains: &'a [FunctionDomainBinding],
        parenthesized_candidates: &'a [ParenthesizedCandidate],
        constraints: &'a [ConstraintArtifact],
        assumptions: &'a [String],
        unresolved_alternatives: &'a [String],
        dependencies: &'a [String],
        reason: &'a str,
        downstream_authorized: bool,
    }
    let payload = Payload {
        status: binding.status,
        input: &binding.input,
        symbols: &binding.symbols,
        requested_unknown: &binding.requested_unknown,
        indexed_objects: &binding.indexed_objects,
        function_domains: &binding.function_domains,
        parenthesized_candidates: &binding.parenthesized_candidates,
        constraints: &binding.constraints,
        assumptions: &binding.assumptions,
        unresolved_alternatives: &binding.unresolved_alternatives,
        dependencies: &binding.dependencies,
        reason: &binding.reason,
        downstream_authorized: binding.downstream_authorized,
    };
    format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(&payload).expect("binding serializes"))
    )
}

fn span(input: &str, needle: &str) -> SourceSpan {
    let start = input.find(needle).unwrap_or(0);
    SourceSpan {
        start,
        end: start + needle.len(),
        text: needle.to_string(),
    }
}

fn symbols_in(expression: &str) -> Vec<String> {
    let re = Regex::new(r"[A-Za-z][A-Za-z0-9_]*").expect("symbol regex");
    let reserved: BTreeSet<&str> = [
        "let",
        "where",
        "given",
        "solve",
        "for",
        "find",
        "and",
        "or",
        "the",
        "is",
        "compute",
        "calculate",
        "determine",
        "evaluate",
        "assuming",
        "under",
        "constraint",
        "constraints",
        "system",
        "satisfy",
        "observed",
        "measured",
        "with",
        "from",
        "to",
        "in",
    ]
    .into_iter()
    .collect();
    let mut result = BTreeSet::new();
    for m in re.find_iter(expression) {
        let token = m.as_str();
        if !reserved.contains(token.to_ascii_lowercase().as_str()) {
            result.insert(token.to_string());
        }
    }
    result.into_iter().collect()
}

fn infer_type(expression: &str) -> (Option<String>, Option<String>) {
    let lower = expression.to_ascii_lowercase();
    let domain = if lower.contains("kg")
        || lower.contains("meter")
        || lower.contains("m/s")
        || lower.contains("joule")
    {
        Some("unit_annotated_quantity".into())
    } else if lower.contains("integer") || lower.contains("natural") {
        Some("integer_domain".into())
    } else if lower.contains("real") || lower.contains("ℝ") {
        Some("real_domain".into())
    } else {
        None
    };
    let type_name = if lower.contains("function") || lower.contains("f(") {
        Some("function".into())
    } else if lower.contains("matrix") || lower.contains("[[") {
        Some("matrix".into())
    } else if lower.chars().any(|c| c.is_ascii_digit()) {
        Some("scalar".into())
    } else {
        None
    };
    (type_name, domain)
}

fn collect_parenthesized_candidates(
    input: &str,
    function_domains: &[FunctionDomainBinding],
) -> Vec<ParenthesizedCandidate> {
    let pattern = Regex::new(r"\b([A-Za-z][A-Za-z0-9_]*)\s*\(([^()]*)\)")
        .expect("parenthesized expression regex");
    let known_functions: BTreeSet<&str> = [
        "abs", "arccos", "arcsin", "arctan", "cos", "det", "exp", "log", "max", "min", "sin",
        "sqrt", "sum", "tan", "tanh", "trace",
    ]
    .into_iter()
    .collect();
    pattern
        .captures_iter(input)
        .map(|capture| {
            let head = capture.get(1).expect("head").as_str().to_string();
            let body = capture.get(2).expect("body").as_str().trim().to_string();
            let whole = capture.get(0).expect("parenthesized span").as_str();
            let declared = function_domains
                .iter()
                .any(|domain| domain.function == head);
            let explicit_function_language = input.to_ascii_lowercase().contains("function")
                || input.to_ascii_lowercase().contains("parametric function")
                || (head
                    .chars()
                    .next()
                    .is_some_and(|character| character.is_ascii_lowercase())
                    && Regex::new(&format!(r"\b{}\s*\([^)]*\)\s*=", regex::escape(&head)))
                        .expect("function definition regex")
                        .is_match(input));
            let known = known_functions.contains(head.to_ascii_lowercase().as_str());
            let (form, evidence) = if declared || known || explicit_function_language {
                let mut evidence = vec!["call-like head has function evidence".into()];
                if declared {
                    evidence.push("head has an explicit domain declaration".into());
                }
                if known {
                    evidence.push("head is a bounded named operator".into());
                }
                if explicit_function_language {
                    evidence.push("surrounding text declares function semantics".into());
                }
                (ParenthesizedForm::FunctionApplication, evidence)
            } else if body.contains(',') {
                (
                    ParenthesizedForm::Tuple,
                    vec!["comma-separated body is structurally tuple-like".into()],
                )
            } else if head.chars().all(|character| character.is_ascii_uppercase()) {
                (
                    ParenthesizedForm::OperatorArgument,
                    vec!["uppercase head is not assumed to be a function".into()],
                )
            } else {
                (
                    ParenthesizedForm::Grouping,
                    vec!["no declaration or bounded function evidence".into()],
                )
            };
            ParenthesizedCandidate {
                head: Some(head),
                body,
                form,
                source_spans: vec![span(input, whole)],
                evidence,
                declared,
            }
        })
        .collect()
}

/// Bind a problem without invoking a solver. The grammar is intentionally
/// conservative: unresolved semantics become `Ambiguous` or `Unsupported`.
pub fn bind_equation_problem(input: &str) -> EquationProblemBinding {
    let normalized = input.replace(['\n', '\r'], " ").trim().to_string();
    let lower = normalized.to_ascii_lowercase();
    let mut symbols = Vec::new();
    let mut indexed_objects = Vec::new();
    let mut function_domains = Vec::new();
    let mut constraints = Vec::new();
    let mut assumptions = Vec::new();
    let mut unresolved = Vec::new();
    let mut dependencies = BTreeSet::new();
    let mut status = BindingStatus::Complete;
    let mut hard_unsupported = false;
    let mut reason = "typed problem binding complete; solver invocation deferred".to_string();

    let unsupported_markers = [
        "partial differential",
        "pde",
        "infinite-dimensional",
        "visual diagram",
        "requires a diagram",
        "quantum field",
        "unknown convention",
        "unsupported representation",
    ];
    if unsupported_markers
        .iter()
        .any(|marker| lower.contains(marker))
    {
        status = BindingStatus::Unsupported;
        hard_unsupported = true;
        reason = "representation or domain is outside EquationProblemBindingV1".into();
    }
    if (lower.contains("x and x")
        || lower.contains("scope a") && lower.contains("scope b") && lower.matches("x").count() > 2)
        && status == BindingStatus::Complete
    {
        status = BindingStatus::Ambiguous;
        unresolved.push("same symbol appears in multiple scopes".into());
        reason = "symbol scope is not uniquely resolved".into();
    }
    if lower.contains("by convention")
        || lower.contains("usual convention")
        || lower.contains("usually means")
    {
        status = BindingStatus::Ambiguous;
        unresolved.push("conventional assumption is not stated".into());
        reason = "conventional notation cannot be promoted to a fact".into();
    }

    let declaration_re = Regex::new(r"(?i)(?:let|define|denote|given|where)\s+([A-Za-z][A-Za-z0-9_]*)\s*(?::\s*([^=,.;]+))?\s*(?::=|=|is)\s*([^,.;]+)").expect("declaration regex");
    for capture in declaration_re.captures_iter(&normalized) {
        let name = capture.get(1).unwrap().as_str().to_string();
        let expression = capture.get(3).map(|m| m.as_str().trim().to_string());
        let (type_name, domain) = infer_type(expression.as_deref().unwrap_or(""));
        let mut binding = SymbolBinding {
            symbol: name.clone(),
            expression,
            type_name,
            domain,
            declared_status: DeclaredStatus::Declared,
            scope: SymbolScope {
                id: "root".into(),
                parent: None,
            },
            source_spans: vec![span(&normalized, &name)],
            unresolved_alternatives: Vec::new(),
            assumptions: assumptions.clone(),
            dependencies: Vec::new(),
        };
        if let Some(annotation) = capture.get(2) {
            binding.domain = Some(annotation.as_str().trim().to_string());
        }
        symbols.push(binding);
    }

    let assumption_re =
        Regex::new(r"(?i)(?:assuming|under|given)\s+([^.;]+)").expect("assumption regex");
    for capture in assumption_re.captures_iter(&normalized) {
        let value = capture.get(1).unwrap().as_str().trim().to_string();
        if !value.is_empty() && !assumptions.contains(&value) {
            assumptions.push(value);
        }
    }
    for binding in &mut symbols {
        binding.assumptions = assumptions.clone();
        if let Some(expression) = &binding.expression {
            binding.dependencies = symbols_in(expression)
                .into_iter()
                .filter(|name| name != &binding.symbol)
                .collect();
        }
    }

    let request_re = Regex::new(r"(?i)(?:solve|find|calculate|compute|determine|evaluate)\s+(?:for\s+)?([A-Za-z][A-Za-z0-9_]*)").expect("request regex");
    let mut request_candidates = request_re
        .captures_iter(&normalized)
        .filter_map(|capture| capture.get(1).map(|m| m.as_str().to_string()))
        .collect::<Vec<_>>();
    if lower.contains("find x and y") || lower.contains("solve for x and y") {
        request_candidates = vec!["x".into(), "y".into()];
    }
    request_candidates.retain(|candidate| {
        !["the", "unknown", "value", "answer"].contains(&candidate.to_ascii_lowercase().as_str())
    });
    request_candidates.sort();
    request_candidates.dedup();
    if request_candidates.len() != 1 {
        status = BindingStatus::Ambiguous;
        unresolved.push("requested unknown is not unique".into());
        reason = "requested target has zero or multiple candidates".into();
    }
    let requested_unknown = RequestedUnknown {
        selected: (request_candidates.len() == 1).then(|| request_candidates[0].clone()),
        source_spans: request_candidates
            .iter()
            .map(|candidate| span(&normalized, candidate))
            .collect(),
        unresolved_alternatives: if request_candidates.len() == 1 {
            Vec::new()
        } else {
            request_candidates.clone()
        },
        candidates: request_candidates,
    };

    let indexed_re =
        Regex::new(r"\b([A-Za-z][A-Za-z0-9_]*)_([A-Za-z][A-Za-z0-9_]*)\b").expect("index regex");
    for capture in indexed_re.captures_iter(&normalized) {
        let object = capture.get(1).unwrap().as_str().to_string();
        let index = capture.get(2).unwrap().as_str().to_string();
        let index_domain = Regex::new(&format!(
            r"(?i){}\s*(?:=|in)\s*([^,.;]+)",
            regex::escape(&index)
        ))
        .ok()
        .and_then(|re| re.captures(&normalized))
        .and_then(|c| c.get(1).map(|m| m.as_str().trim().to_string()));
        if index_domain.is_none() {
            status = BindingStatus::Ambiguous;
            unresolved.push(format!("index domain for {object}_{index} is unstated"));
            reason = "indexed object has no authorized index domain".into();
        }
        indexed_objects.push(IndexedObjectBinding {
            object: object.clone(),
            index: index.clone(),
            index_domain,
            body: None,
            declared_status: DeclaredStatus::Inferred,
            source_spans: vec![span(&normalized, capture.get(0).unwrap().as_str())],
            unresolved_alternatives: Vec::new(),
        });
    }

    let function_re =
        Regex::new(r"\b([A-Za-z][A-Za-z0-9_]*)\s*:\s*([^,.;]+?)\s*(?:[-=]>|\\\\to)\s*([^,.;]+)")
            .expect("function domain regex");
    for capture in function_re.captures_iter(&normalized) {
        function_domains.push(FunctionDomainBinding {
            function: capture.get(1).unwrap().as_str().into(),
            domain: Some(capture.get(2).unwrap().as_str().trim().into()),
            codomain: Some(capture.get(3).unwrap().as_str().trim().into()),
            declared_status: DeclaredStatus::Declared,
            source_spans: vec![span(&normalized, capture.get(0).unwrap().as_str())],
            unresolved_alternatives: Vec::new(),
        });
    }
    let parenthesized_candidates = collect_parenthesized_candidates(&normalized, &function_domains);
    let known_function_heads: BTreeSet<&str> = [
        "abs", "arccos", "arcsin", "arctan", "cos", "det", "exp", "log", "max", "min", "sin",
        "sqrt", "sum", "tan", "tanh", "trace",
    ]
    .into_iter()
    .collect();
    let has_unsupported_function_application = parenthesized_candidates.iter().any(|candidate| {
        candidate.form == ParenthesizedForm::FunctionApplication
            && !candidate.declared
            && !known_function_heads.contains(candidate.head.as_deref().unwrap_or_default())
            && !function_domains
                .iter()
                .any(|domain| domain.function == candidate.head.as_deref().unwrap_or_default())
    });
    if has_unsupported_function_application && function_domains.is_empty() {
        status = BindingStatus::Ambiguous;
        unresolved.push("function domain or codomain is unstated".into());
        reason = "function semantics require an explicit domain and codomain".into();
    }

    let explicit_constraints = lower.contains("constraint")
        || lower.contains("system")
        || lower.contains("satisf")
        || lower.contains("equations");
    let observation =
        lower.contains("observed") || lower.contains("measured") || lower.contains("reports");
    for part in normalized.split(|c| c == ';' || c == '\n') {
        if !part.contains('=') || part.contains(":=") {
            continue;
        }
        if observation && !explicit_constraints {
            status = BindingStatus::Ambiguous;
            unresolved.push("equation may describe an observation rather than a constraint".into());
            reason = "observations are not silently converted into equations".into();
            continue;
        }
        let expression = part.trim().to_string();
        let names = symbols_in(&expression);
        for name in &names {
            dependencies.insert(name.clone());
        }
        constraints.push(ConstraintArtifact {
            expression: expression.clone(),
            symbols: names.clone(),
            kind: if explicit_constraints {
                "constraint".into()
            } else {
                "declared_relation".into()
            },
            source_spans: vec![span(&normalized, &expression)],
            assumptions: assumptions.clone(),
            dependencies: names,
        });
    }
    if explicit_constraints && constraints.len() == 1 && constraints[0].expression.contains(" and ")
    {
        let expression = constraints.remove(0).expression;
        constraints = expression
            .split(" and ")
            .filter(|piece| piece.contains('='))
            .map(|piece| {
                let expression = piece.trim().to_string();
                let names = symbols_in(&expression);
                for name in &names {
                    dependencies.insert(name.clone());
                }
                ConstraintArtifact {
                    expression: expression.clone(),
                    symbols: names.clone(),
                    kind: "constraint".into(),
                    source_spans: vec![span(&normalized, &expression)],
                    assumptions: assumptions.clone(),
                    dependencies: names,
                }
            })
            .collect();
    }
    if lower.contains("either")
        || lower.contains("several constraint systems")
        || lower.contains("or the system")
    {
        status = BindingStatus::Ambiguous;
        unresolved.push("multiple constraint systems are possible".into());
        reason = "constraint-system choice is unresolved".into();
    }
    {
        let declared: BTreeSet<String> = symbols
            .iter()
            .map(|binding| binding.symbol.clone())
            .collect();
        let inferred: BTreeSet<String> = constraints
            .iter()
            .flat_map(|constraint| constraint.symbols.iter().cloned())
            .collect();
        for name in inferred.difference(&declared) {
            symbols.push(SymbolBinding {
                symbol: name.clone(),
                expression: None,
                type_name: None,
                domain: None,
                declared_status: DeclaredStatus::Inferred,
                scope: SymbolScope {
                    id: "root".into(),
                    parent: None,
                },
                source_spans: vec![span(&normalized, name)],
                unresolved_alternatives: Vec::new(),
                assumptions: assumptions.clone(),
                dependencies: Vec::new(),
            });
        }
    }
    if symbols.is_empty() && constraints.is_empty() && status == BindingStatus::Complete {
        status = BindingStatus::Unsupported;
        reason = "no typed symbol or constraint structure found".into();
    }
    if hard_unsupported {
        status = BindingStatus::Unsupported;
        reason = "representation or domain is outside EquationProblemBindingV1".into();
    }
    if symbols.is_empty()
        && constraints.is_empty()
        && !unresolved.is_empty()
        && status == BindingStatus::Unsupported
        && !hard_unsupported
    {
        status = BindingStatus::Ambiguous;
        reason = "binding evidence is incomplete or ambiguous".into();
    }
    if status == BindingStatus::Complete && requested_unknown.selected.is_none() {
        status = BindingStatus::Ambiguous;
    }
    let mut binding = EquationProblemBinding {
        status,
        input: normalized,
        symbols,
        requested_unknown,
        indexed_objects,
        function_domains,
        parenthesized_candidates,
        constraints,
        assumptions,
        unresolved_alternatives: unresolved,
        dependencies: dependencies.into_iter().collect(),
        reason,
        replay_hash: String::new(),
        downstream_authorized: false,
    };
    binding.replay_hash = replay_hash(&binding);
    binding
}

/// Explicit primitive view over local symbol binding. This is a diagnostic
/// helper; it never selects a downstream method.
pub fn bind_local_symbol(input: &str, symbol: &str) -> Option<SymbolBinding> {
    bind_equation_problem(input)
        .symbols
        .into_iter()
        .find(|binding| binding.symbol == symbol)
}

/// Explicit primitive view over requested-unknown binding.
pub fn bind_requested_unknown(input: &str) -> RequestedUnknown {
    bind_equation_problem(input).requested_unknown
}

/// Extract only assumptions that were explicitly stated in the input.
pub fn propagate_assumption(input: &str) -> Vec<String> {
    bind_equation_problem(input).assumptions
}

/// Explicit primitive view over indexed-object binding.
pub fn bind_indexed_object(input: &str) -> Vec<IndexedObjectBinding> {
    bind_equation_problem(input).indexed_objects
}

/// Explicit primitive view over function-domain binding.
pub fn bind_function_domain(input: &str) -> Vec<FunctionDomainBinding> {
    bind_equation_problem(input).function_domains
}

/// Construct coupled constraints without executing them.
pub fn construct_coupled_constraints(input: &str) -> Vec<ConstraintArtifact> {
    bind_equation_problem(input).constraints
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn binds_symbols_target_assumptions_and_constraints_without_solving() {
        let result = bind_equation_problem(
            "Let m = 2 kg and a = 3 m/s^2. Assuming m > 0; constraint F = m*a; solve for F",
        );
        assert_eq!(result.status, BindingStatus::Complete);
        assert_eq!(result.requested_unknown.selected.as_deref(), Some("F"));
        assert!(result.constraints.len() >= 1);
        assert!(!result.downstream_authorized);
        assert!(result.replay_verified());
    }

    #[test]
    fn preserves_ambiguous_target_and_index_domain() {
        let result = bind_equation_problem("Let a_i = 2*i. Find a_n");
        assert_eq!(result.status, BindingStatus::Ambiguous);
        assert!(!result.unresolved_alternatives.is_empty());
        assert!(result.replay_verified());
    }

    #[test]
    fn rejects_unsupported_representation() {
        let result =
            bind_equation_problem("Solve the PDE on an infinite-dimensional function space");
        assert_eq!(result.status, BindingStatus::Unsupported);
        assert!(result.replay_verified());
    }

    #[test]
    fn distinguishes_grouping_from_function_application() {
        let grouped = bind_equation_problem("Let A(X) = (x, y). Constraint z = A(X). Solve for z.");
        assert!(grouped
            .parenthesized_candidates
            .iter()
            .any(|candidate| candidate.form == ParenthesizedForm::OperatorArgument));
        let function = bind_equation_problem("Let f(x) = x^2. Evaluate f(3).");
        assert!(function
            .parenthesized_candidates
            .iter()
            .any(|candidate| candidate.form == ParenthesizedForm::FunctionApplication));
        assert_eq!(function.status, BindingStatus::Ambiguous);
    }
}
