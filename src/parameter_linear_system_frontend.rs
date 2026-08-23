//! Bounded frontend for a parameterized two-by-two linear system.
//!
//! The frontend accepts two linear equations, one explicit assignment for a
//! variable, and one requested scalar parameter.  It lowers the problem to
//! exact rational elimination; it does not guess missing assumptions or solve
//! nonlinear, underspecified, or multi-parameter systems.

use crate::probability_pack::Rational;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FrontendStatus {
    Complete,
    Ambiguous,
    Missing,
    Unsupported,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LinearForm {
    pub coefficients: BTreeMap<String, i128>,
    pub constant: i128,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ParameterSystemRequest {
    pub equations: Vec<(LinearForm, LinearForm)>,
    pub known_variable: String,
    pub known_value: i128,
    pub requested_parameter: String,
    pub provenance: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FrontendResult {
    pub status: FrontendStatus,
    pub request: Option<ParameterSystemRequest>,
    pub unresolved: Vec<String>,
    pub provenance: Vec<String>,
    pub replay_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExecutionResult {
    pub status: FrontendStatus,
    pub requested_parameter: Option<String>,
    pub value: Option<Rational>,
    pub answer: Option<String>,
    pub assumptions: Vec<String>,
    pub reasons: Vec<String>,
    pub provenance: Vec<String>,
    pub replay_hash: String,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn finish(mut result: FrontendResult) -> FrontendResult {
    result.replay_hash.clear();
    result.replay_hash = digest(&result);
    result
}

fn finish_execution(mut result: ExecutionResult) -> ExecutionResult {
    result.replay_hash.clear();
    result.replay_hash = digest(&result);
    result
}

pub fn replay_verified(result: &FrontendResult) -> bool {
    let mut copy = result.clone();
    let hash = copy.replay_hash.clone();
    copy.replay_hash.clear();
    hash == digest(&copy) && !result.provenance.is_empty()
}

pub fn execution_replay_verified(result: &ExecutionResult) -> bool {
    let mut copy = result.clone();
    let hash = copy.replay_hash.clone();
    copy.replay_hash.clear();
    hash == digest(&copy) && !result.provenance.is_empty()
}

fn output(
    status: FrontendStatus,
    request: Option<ParameterSystemRequest>,
    unresolved: Vec<String>,
    provenance: Vec<String>,
) -> FrontendResult {
    finish(FrontendResult {
        status,
        request,
        unresolved,
        provenance,
        replay_hash: String::new(),
    })
}

fn normalize_source(source: &str) -> String {
    source
        .replace("\\begin{align*}", "")
        .replace("\\end{align*}", "")
        .replace("\\begin{aligned}", "")
        .replace("\\end{aligned}", "")
        .replace("\\\\", ";")
        .replace("&", "")
        .replace("\\,", "")
        .replace("\\!", "")
        .replace("\\left", "")
        .replace("\\right", "")
        .replace(['$', '{', '}'], "")
}

fn add_term(form: &mut LinearForm, variable: Option<char>, value: i128) {
    if let Some(variable) = variable {
        *form
            .coefficients
            .entry(variable.to_ascii_lowercase().to_string())
            .or_insert(0) += value;
    } else {
        form.constant += value;
    }
}

fn parse_linear_form(source: &str) -> Option<LinearForm> {
    let mut text = source.to_ascii_lowercase().replace([' ', '\t', '*'], "");
    text = text.replace("\\cdot", "");
    if text.is_empty() || text.contains(['^', '(', ')', '/', '\\']) {
        return None;
    }
    let chars = text.chars().collect::<Vec<_>>();
    let mut index = 0;
    let mut form = LinearForm {
        coefficients: BTreeMap::new(),
        constant: 0,
    };
    while index < chars.len() {
        let mut sign = 1i128;
        if chars[index] == '+' {
            index += 1;
        } else if chars[index] == '-' {
            sign = -1;
            index += 1;
        }
        if index >= chars.len() {
            return None;
        }
        let start = index;
        while index < chars.len() && chars[index].is_ascii_digit() {
            index += 1;
        }
        let digits = &chars[start..index];
        let coefficient = if digits.is_empty() {
            1
        } else {
            digits.iter().collect::<String>().parse::<i128>().ok()?
        } * sign;
        if index < chars.len() && chars[index].is_ascii_alphabetic() {
            let variable = chars[index];
            index += 1;
            if index < chars.len() && chars[index].is_ascii_alphabetic() {
                return None;
            }
            add_term(&mut form, Some(variable), coefficient);
        } else if !digits.is_empty() {
            add_term(&mut form, None, coefficient);
        } else {
            return None;
        }
    }
    Some(form)
}

fn parse_equations(source: &str) -> Vec<(LinearForm, LinearForm)> {
    let normalized = normalize_source(source);
    let equation_region = [
        " given ",
        " when ",
        " has a solution",
        " find ",
        " compute ",
        " determine ",
        " calculate ",
    ]
    .iter()
    .filter_map(|marker| {
        normalized
            .to_ascii_lowercase()
            .find(marker)
            .map(|index| index)
    })
    .min()
    .map(|index| normalized[..index].to_string())
    .unwrap_or(normalized);
    equation_region
        .split([';', '\n', ','])
        .filter_map(|part| {
            let (left, right) = part.split_once('=')?;
            let left = left
                .trim()
                .rsplit_once(" is ")
                .map(|(_, rest)| rest)
                .or_else(|| left.trim().rsplit_once("equations ").map(|(_, rest)| rest))
                .unwrap_or(left.trim());
            let right = right
                .trim()
                .trim_matches(|character| matches!(character, '.' | '?' | ','));
            Some((parse_linear_form(left)?, parse_linear_form(right)?))
        })
        .collect()
}

fn parse_assignment_fragment(source: &str) -> Option<(String, i128)> {
    let chars = source.to_ascii_lowercase().chars().collect::<Vec<_>>();
    for index in 0..chars.len().saturating_sub(2) {
        if !chars[index].is_ascii_alphabetic() || chars[index + 1] != '=' {
            continue;
        }
        let mut end = index + 2;
        if end < chars.len() && chars[end] == '-' {
            end += 1;
        }
        let start_digits = end;
        while end < chars.len() && chars[end].is_ascii_digit() {
            end += 1;
        }
        if end == start_digits {
            continue;
        }
        let value = chars[start_digits..end]
            .iter()
            .collect::<String>()
            .parse::<i128>()
            .ok()?;
        let value = if chars[index + 2] == '-' {
            -value
        } else {
            value
        };
        return Some((chars[index].to_string(), value));
    }
    None
}

fn parse_assignment(source: &str) -> Option<(String, i128)> {
    let normalized = normalize_source(source);
    for marker in [" given ", " when ", " assuming ", " where "] {
        if let Some((_, suffix)) = normalized.to_ascii_lowercase().split_once(marker) {
            if let Some(assignment) = parse_assignment_fragment(suffix) {
                return Some(assignment);
            }
        }
    }
    None
}

fn parse_requested_parameter(source: &str) -> Option<String> {
    let lower = normalize_source(source).to_ascii_lowercase();
    for marker in ["find ", "compute ", "determine ", "calculate "] {
        if let Some(rest) = lower.split_once(marker).map(|(_, rest)| rest.trim_start()) {
            let mut token = rest
                .chars()
                .skip_while(|character| !character.is_ascii_alphabetic());
            let Some(first) = token.next() else {
                continue;
            };
            let mut name = first.to_string();
            for character in token {
                if !character.is_ascii_alphabetic() {
                    break;
                }
                name.push(character);
            }
            let remainder = rest[name.len()..].trim_start();
            // A request such as “compute a and b” is outside the single-
            // parameter contract.  Do not accept the first symbol merely
            // because it follows a target verb.
            if name.len() == 1 && !remainder.starts_with("and ") {
                return Some(name);
            }
        }
    }
    None
}

fn has_multiple_requested_parameters(source: &str) -> bool {
    let lower = normalize_source(source).to_ascii_lowercase();
    ["find ", "compute ", "determine ", "calculate "]
        .iter()
        .filter_map(|marker| lower.split_once(marker).map(|(_, rest)| rest.trim_start()))
        .any(|rest| {
            let mut words = rest.split_whitespace();
            let first = words.next().unwrap_or_default();
            first.len() == 1 && words.next() == Some("and")
        })
}

pub fn formalize(source: &str, case_id: &str) -> FrontendResult {
    let provenance = vec![
        format!("parameter-linear-system-frontend:{case_id}"),
        format!("source-span:0..{}", source.len()),
        "two-equation-linear-parameter-grammar".into(),
    ];
    let lower = normalize_source(source).to_ascii_lowercase();
    if lower.contains("x^2")
        || lower.contains("y^2")
        || lower.contains("nonlinear")
        || lower.contains("inequality")
        || lower.contains("three equations")
    {
        return output(
            FrontendStatus::Unsupported,
            None,
            vec!["request is outside a bounded two-equation linear parameter system".into()],
            provenance,
        );
    }
    let equations = parse_equations(source);
    if equations.len() != 2 {
        return output(
            if equations.is_empty() {
                FrontendStatus::Missing
            } else {
                FrontendStatus::Unsupported
            },
            None,
            vec!["exactly two linear equations are required".into()],
            provenance,
        );
    }
    let Some((known_variable, known_value)) = parse_assignment(source) else {
        return output(
            FrontendStatus::Missing,
            None,
            vec!["one explicit variable assignment is required".into()],
            provenance,
        );
    };
    if has_multiple_requested_parameters(source) {
        return output(
            FrontendStatus::Unsupported,
            None,
            vec!["exactly one requested scalar parameter is required".into()],
            provenance,
        );
    }
    let Some(requested_parameter) = parse_requested_parameter(source) else {
        return output(
            FrontendStatus::Missing,
            None,
            vec!["one requested scalar parameter is required".into()],
            provenance,
        );
    };
    let mut variables = BTreeMap::new();
    for (left, right) in &equations {
        for variable in left.coefficients.keys().chain(right.coefficients.keys()) {
            variables.insert(variable.clone(), ());
        }
    }
    if !variables.contains_key(&known_variable) || !variables.contains_key(&requested_parameter) {
        return output(
            FrontendStatus::Ambiguous,
            None,
            vec!["assignment and requested parameter must bind equation variables".into()],
            provenance,
        );
    }
    if variables.len() != 3 {
        return output(
            FrontendStatus::Unsupported,
            None,
            vec!["exactly one remaining linear variable is required".into()],
            provenance,
        );
    }
    let request = ParameterSystemRequest {
        equations,
        known_variable,
        known_value,
        requested_parameter,
        provenance: provenance.clone(),
    };
    output(
        FrontendStatus::Complete,
        Some(request),
        Vec::new(),
        provenance,
    )
}

fn sub(a: &Rational, b: &Rational) -> Rational {
    a.sub(b).expect("rational subtraction")
}

fn add(a: &Rational, b: &Rational) -> Rational {
    a.add(b).expect("rational addition")
}

fn mul(a: &Rational, b: &Rational) -> Rational {
    a.mul(b).expect("rational multiplication")
}

fn div(a: &Rational, b: &Rational) -> Option<Rational> {
    a.div(b)
}

fn coefficient(form: &LinearForm, variable: &str) -> Rational {
    Rational::new(*form.coefficients.get(variable).unwrap_or(&0), 1).unwrap()
}

fn constant(form: &LinearForm) -> Rational {
    Rational::new(form.constant, 1).unwrap()
}

fn fraction_text(value: &Rational) -> String {
    if value.denominator == 1 {
        value.numerator.to_string()
    } else {
        format!("\\frac{{{}}}{{{}}}", value.numerator, value.denominator)
    }
}

pub fn execute(request: &ParameterSystemRequest) -> ExecutionResult {
    let variables = request
        .equations
        .iter()
        .flat_map(|(left, right)| left.coefficients.keys().chain(right.coefficients.keys()))
        .filter(|variable| {
            *variable != &request.known_variable && *variable != &request.requested_parameter
        })
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    if variables.len() != 1 {
        return finish_execution(ExecutionResult {
            status: FrontendStatus::Unsupported,
            requested_parameter: Some(request.requested_parameter.clone()),
            value: None,
            answer: None,
            assumptions: Vec::new(),
            reasons: vec!["remaining variable is not unique".into()],
            provenance: request.provenance.clone(),
            replay_hash: String::new(),
        });
    }
    let remaining = &variables[0];
    let mut rows = Vec::new();
    for (left, right) in &request.equations {
        let known = coefficient(left, &request.known_variable);
        let known_right = coefficient(right, &request.known_variable);
        let known_delta = sub(&known_right, &known);
        let remaining_delta = sub(
            &coefficient(right, remaining),
            &coefficient(left, remaining),
        );
        let parameter_delta = sub(
            &coefficient(right, &request.requested_parameter),
            &coefficient(left, &request.requested_parameter),
        );
        let constant_delta = sub(&constant(right), &constant(left));
        let known_term = mul(
            &known_delta,
            &Rational::new(request.known_value, 1).unwrap(),
        );
        // The normalized equation is
        //   remaining_delta*y + parameter_delta*a
        //     + known_delta*x + constant_delta = 0.
        // Therefore the right-hand side is the negation of the two known
        // terms.  Keeping this sign explicit avoids silently authorizing a
        // mirrored solution when the known assignment is nonzero.
        let rhs = sub(&Rational::zero(), &add(&constant_delta, &known_term));
        rows.push((remaining_delta, parameter_delta, rhs));
    }
    let determinant = sub(&mul(&rows[0].0, &rows[1].1), &mul(&rows[0].1, &rows[1].0));
    if determinant.numerator == 0 {
        return finish_execution(ExecutionResult {
            status: FrontendStatus::Ambiguous,
            requested_parameter: Some(request.requested_parameter.clone()),
            value: None,
            answer: None,
            assumptions: Vec::new(),
            reasons: vec!["the reduced two-variable system is not uniquely solvable".into()],
            provenance: request.provenance.clone(),
            replay_hash: String::new(),
        });
    }
    let parameter_numerator = sub(&mul(&rows[0].0, &rows[1].2), &mul(&rows[1].0, &rows[0].2));
    let Some(value) = div(&parameter_numerator, &determinant) else {
        return finish_execution(ExecutionResult {
            status: FrontendStatus::Unsupported,
            requested_parameter: Some(request.requested_parameter.clone()),
            value: None,
            answer: None,
            assumptions: Vec::new(),
            reasons: vec!["parameter denominator is zero".into()],
            provenance: request.provenance.clone(),
            replay_hash: String::new(),
        });
    };
    finish_execution(ExecutionResult {
        status: FrontendStatus::Complete,
        requested_parameter: Some(request.requested_parameter.clone()),
        answer: Some(fraction_text(&value)),
        value: Some(value),
        assumptions: vec![
            "exactly two linear equations".into(),
            "one explicit variable assignment".into(),
            "unique reduced two-variable solution".into(),
        ],
        reasons: Vec::new(),
        provenance: request.provenance.clone(),
        replay_hash: String::new(),
    })
}

pub fn replay_execution(result: &ExecutionResult) -> bool {
    let mut copy = result.clone();
    let hash = copy.replay_hash.clone();
    copy.replay_hash.clear();
    hash == digest(&copy) && result.status == FrontendStatus::Complete
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_parameter_system_executes_exactly() {
        let text = "The system is 3*x+y=a; 2*x+5*y=2*a. Given x=2, compute a.";
        let frontend = formalize(text, "test");
        assert_eq!(frontend.status, FrontendStatus::Complete);
        assert!(replay_verified(&frontend));
        let execution = execute(frontend.request.as_ref().unwrap());
        assert_eq!(execution.status, FrontendStatus::Complete);
        assert_eq!(execution.value, Rational::new(26, 3));
        assert_eq!(execution.answer.as_deref(), Some("\\frac{26}{3}"));
        assert!(execution_replay_verified(&execution));
        assert!(replay_execution(&execution));
    }

    #[test]
    fn latex_alignment_is_normalized_without_domain_guessing() {
        let text = r"\begin{align*}3x+y&=a,\\2x+5y&=2a,\end{align*} when x=2, compute a";
        let frontend = formalize(text, "latex");
        assert_eq!(frontend.status, FrontendStatus::Complete);
        assert_eq!(
            execute(frontend.request.as_ref().unwrap()).value,
            Rational::new(26, 3)
        );
    }

    #[test]
    fn nonlinear_and_incomplete_forms_fail_closed() {
        assert_eq!(
            formalize("x^2+y=3; x-y=1. Given x=2, find y", "reject").status,
            FrontendStatus::Unsupported
        );
        assert_eq!(
            formalize("3*x+y=a; 2*x+5*y=2*a. Compute a", "missing").status,
            FrontendStatus::Missing
        );
    }

    #[test]
    fn multiple_requested_parameters_fail_closed() {
        let result = formalize(
            "The system is x+y=a; 2*x+y=2*a, given x=2, compute a and b.",
            "multiple-targets",
        );
        assert_eq!(result.status, FrontendStatus::Unsupported);
    }
}
