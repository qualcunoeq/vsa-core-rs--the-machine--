// ─── Math Computation Engine (Layer 1: Numeric/Symbolic Evaluator) ────────
//
// Wires basic math computation into the VSA QA pipeline so questions like
// "What is 2 + 2?" or "Compute sqrt(144)" produce answers instead of
// "I do not know."
//
// ## Architecture
//
//   question → pattern_matcher() → expression_parser() → evaluator() → answer
//                     ↓ (no match)
//                  return None → caller falls through to VSA QA
//
// ## Supported Patterns
//
// - "What is EXPR?"  / "What is the EXPR?"
// - "Compute EXPR"   / "Calculate EXPR"
// - "How many X?"    (depends on context)
// - "Is it X?"       (yes/no via comparison)
// - Direct number extraction from word problems
//
// ## Supported Math
//
// - Arithmetic: +, -, *, /, ^ (power), %
// - Functions: sqrt, abs, sin, cos, tan, log, ln, exp, ceil, floor, round
// - Combinatorics: factorial, nCr, nPr
// - Number theory: gcd, lcm, prime_factors, is_prime, largest_prime_divisor
// - Constants: pi, e
// - Word forms: "plus", "minus", "times", "divided by", "squared", "cubed"

use regex::Regex;

// ═══════════════════════════════════════════════════════════════════════════
// CONSTANTS
// ═══════════════════════════════════════════════════════════════════════════

// ═══════════════════════════════════════════════════════════════════════════
// MATH ENGINE
// ═══════════════════════════════════════════════════════════════════════════

/// Pure-function math evaluation engine.
///
/// Stateless: parsing and evaluation are deterministic functions.
/// No training, no persistence, no side effects.
pub struct MathEngine;

impl MathEngine {
    /// Try to answer a question via math computation.
    ///
    /// Returns `Some(answer)` if the question matches a math pattern AND
    /// the expression evaluates successfully.  Returns `None` otherwise.
    pub fn try_answer(question: &str) -> Option<String> {
        let normalized = Self::normalize_question(question);
        let lower = question.to_lowercase();

        // ── Derivative patterns ───────────────────────────────────────────
        // "derivative of EXPR" / "derivative of EXPR at X = VAL"
        // "differentiate EXPR" / "differentiate EXPR wrt X"
        // "d/dX (EXPR)" / "d/dX EXPR"
        // "second derivative of EXPR"
        // "slope of EXPR at X = VAL"
        if lower.contains("derivative") || lower.contains("differentiate")
            || lower.contains("d/d") || lower.contains("slope of")
        {
            return Self::try_derivative(question);
        }

        // ── Integral patterns ─────────────────────────────────────────────
        // "integral of EXPR" / "integrate EXPR" / "antiderivative of EXPR"
        // "integral of EXPR from A to B" / "integrate EXPR from A to B"
        if lower.contains("integral") || lower.contains("integrate")
            || lower.contains("antiderivative") || lower.contains("anti-derivative")
        {
            return Self::try_integral(question);
        }

        // ── Solve patterns ────────────────────────────────────────────────
        // "solve EXPR = VAL for X" / "solve EXPR"
        if lower.contains("solve") && lower.contains('=') {
            return Self::try_solve(question);
        }

        // Detect question patterns
        if let Some(expr) = Self::extract_expression(&normalized) {
            if let Some(result) = Self::evaluate(&expr) {
                return Some(result);
            }
        }

        // ── Word problem patterns (last resort) ───────────────────────────
        // "A train leaves at 2 PM traveling at 60 mph..."
        if let Some(result) = Self::try_word_problem(question) {
            return Some(result);
        }

        None
    }

    /// Try to answer a derivative question by delegating to the symbolic
    /// algebra engine.
    fn try_derivative(question: &str) -> Option<String> {
        let lower = question.to_lowercase();

        // Extract the variable (default to "x")
        let var = if lower.contains(" wrt ") || lower.contains(" with respect to ") {
            // Find the variable after "wrt" or "with respect to"
            let after_wrt = lower.split("wrt").last()
                .or_else(|| lower.split("with respect to").last())?
                .trim()
                .split(|c: char| !c.is_alphanumeric())
                .next()
                .unwrap_or("x")
                .to_string();
            if after_wrt.is_empty() { "x".to_string() } else { after_wrt }
        } else {
            "x".to_string()
        };

        // Determine the order (default to first derivative)
        let n = if lower.starts_with("second") || lower.contains("2nd") {
            2
        } else if lower.starts_with("third") || lower.contains("3rd") {
            3
        } else if lower.contains("nth") {
            // Can't handle generic nth without a specific n
            return None;
        } else {
            1
        };

        // Extract the function to differentiate
        // "derivative of EXPR" → get EXPR
        // "differentiate EXPR" → get EXPR
        // "slope of EXPR at X = VAL" → get EXPR
        // "d/dx EXPR" → get EXPR
        let expr_str = if lower.starts_with("d/d") {
            // "d/dx sin(x)" → "sin(x)"
            let after_dd = question[3..].trim();
            // Skip the variable character(s)
            let after_var = after_dd.splitn(2, |c: char| c == ' ' || c == '(' || c == ')')
                .last()
                .unwrap_or("")
                .trim();
            if after_var.is_empty() { return None; }
            after_var.to_string()
        } else if lower.contains("derivative of") {
            let after = lower.split("derivative of").last()?.trim();
            // Strip trailing "at X = VAL" or "with respect to X"
            let stripped = if let Some(at_pos) = after.rfind(" at ") {
                &after[..at_pos]
            } else if let Some(wrt_pos) = after.rfind("with respect") {
                &after[..wrt_pos]
            } else if let Some(wrt_pos) = after.rfind("wrt") {
                &after[..wrt_pos]
            } else {
                after
            };
            stripped.trim().to_string()
        } else if lower.starts_with("differentiate") || lower.starts_with("slope of") {
            let prefix = if lower.starts_with("slope of") { "slope of" } else { "differentiate" };
            let after = lower.split(prefix).last()?.trim();
            let stripped = if let Some(at_pos) = after.rfind(" at ") {
                &after[..at_pos]
            } else if let Some(wrt_pos) = after.rfind("with respect") {
                &after[..wrt_pos]
            } else if let Some(wrt_pos) = after.rfind("wrt") {
                &after[..wrt_pos]
            } else {
                after
            };
            stripped.trim().to_string()
        } else {
            return None;
        };

        if expr_str.is_empty() {
            return None;
        }

        // Parse the expression and differentiate
        let expr = crate::algebra::parse(&expr_str).ok()?;

        let simplified = if n == 1 {
            expr.differentiate(&var).simplify()
        } else {
            expr.differentiate_n(&var, n).simplify()
        };

        let result_str = format!("{}", simplified);

        // If the question asks for evaluation at a point, compute it
        if let Some(at_str) = lower.split(" at ").last() {
            if let Some(val_str) = at_str.strip_prefix("x = ").or_else(|| at_str.strip_prefix("x="))
                .or_else(|| {
                    let parts: Vec<&str> = at_str.split('=').collect();
                    if parts.len() == 2 { Some(parts[1].trim()) } else { None }
                })
            {
                if let Ok(val) = val_str.trim().parse::<f64>() {
                    if let Some(evaluated) = simplified.evaluate(&[(&var, val)]) {
                        return Some(Self::format_float(evaluated));
                    }
                }
            }
        }

        Some(result_str)
    }

    /// Try to answer an integral question by delegating to the symbolic
    /// algebra engine.
    fn try_integral(question: &str) -> Option<String> {
        let lower = question.to_lowercase();

        // Check for definite integral: "from A to B"
        let (lower_bound, upper_bound) = if lower.contains("from") && lower.contains("to") {
            let after_from = lower.split("from").last()?;
            let bounds: Vec<&str> = after_from.splitn(2, "to").collect();
            if bounds.len() == 2 {
                let a = bounds[0].trim().split_whitespace().next().unwrap_or("0");
                let b = bounds[1].trim().split_whitespace().next().unwrap_or("1");
                (Some(a.parse::<f64>().ok()?), Some(b.parse::<f64>().ok()?))
            } else {
                (None, None)
            }
        } else {
            (None, None)
        };

        // Extract the expression to integrate
        let var = if lower.contains(" d") {
            // ∫ f(x) dx pattern — the variable is after "d"
            let after_d = lower.split(" d").last()?;
            let var_candidate = after_d.trim().split_whitespace().next().unwrap_or("x");
            var_candidate.trim_end_matches('.').trim_end_matches('?').to_string()
        } else {
            "x".to_string()
        };

        let expr_str = if lower.contains("integral of") {
            let after = lower.split("integral of").last()?.trim();
            after.split("from").next().unwrap_or(after).trim().to_string()
        } else if lower.contains("integrate ") {
            let after = lower.split("integrate ").last()?.trim();
            after.split("from").next().unwrap_or(after).trim().to_string()
        } else if lower.contains("antiderivative of") || lower.contains("anti-derivative of") {
            let keyword = if lower.contains("antiderivative of") { "antiderivative of" } else { "anti-derivative of" };
            let after = lower.split(keyword).last()?.trim();
            after.split("from").next().unwrap_or(after).trim().to_string()
        } else {
            return None;
        };

        if expr_str.is_empty() {
            return None;
        }

        match (lower_bound, upper_bound) {
            (Some(a), Some(b)) => {
                // Definite integral: compute numeric result
                crate::algebra::integrate_definite(&expr_str, &var, a, b)
                    .map(|v| Self::format_float(v))
            }
            (None, None) => {
                // Indefinite integral: symbolic
                crate::algebra::integrate_str(&expr_str, &var)
                    .map(|s| {
                        if s.is_empty() { s } else { format!("{} + C", s) }
                    })
            }
            _ => None,
        }
    }

    /// Try to solve an equation by delegating to the symbolic algebra engine.
    fn try_solve(question: &str) -> Option<String> {
        let lower = question.to_lowercase();

        // Extract the variable (default to "x")
        let var = if lower.contains(" for ") {
            let after_for = lower.split(" for ").last()?;
            after_for.trim().split_whitespace().next().unwrap_or("x").to_string()
        } else {
            "x".to_string()
        };

        // Extract the equation: "solve 2*x + 1 = 0" → "2*x + 1 = 0"
        let eq_str = if let Some(eq_pos) = lower.find('=') {
            // Find the start of the equation (after "solve" or at the beginning)
            let start = if lower.starts_with("solve") {
                let after_solve = lower.strip_prefix("solve")?.trim();
                match after_solve.find(|c: char| c.is_alphanumeric() || c == '(' || c == '-' || c == '+' || c == 'x' || c == 'y') {
                    Some(pos) => &after_solve[pos..],
                    None => after_solve,
                }
            } else {
                &lower
            };
            // Strip trailing "for X" or variable specification
            let stripped = if let Some(for_pos) = start.rfind(" for ") {
                start[..for_pos].trim()
            } else {
                start.trim()
            };
            stripped.to_string()
        } else {
            return None;
        };

        if eq_str.is_empty() {
            return None;
        }

        // Detect systems: semicolons separate multiple equations
        if eq_str.contains(';') {
            return crate::algebra::solve_system_str(&eq_str).ok();
        }

        crate::algebra::solve_str(&eq_str, &var).ok()
    }



    /// Normalize the question: lowercase, strip punctuation, replace word forms.
    fn normalize_question(q: &str) -> String {
        let mut s = q.to_lowercase();

        // Handle factorial notation BEFORE stripping '!': "5!" → "factorial(5)"
        // This also handles "n!" in the middle of expressions.
        let mut with_factorial = String::new();
        let mut chars = s.chars().peekable();
        while let Some(c) = chars.next() {
            if c == '!' {
                // Find the number before the '!'
                if let Some(prev) = with_factorial.chars().last() {
                    if prev.is_ascii_digit() || prev == ')' {
                        // Replace trailing number with factorial(num)
                        // For simplicity: replace "X!" with "factorial(X)" for simple cases
                        let mut num_str = String::new();
                        while let Some(pc) = with_factorial.chars().last() {
                            if pc.is_ascii_digit() || pc == '.' {
                                num_str.push(pc);
                                with_factorial.pop();
                            } else {
                                break;
                            }
                        }
                        let num: String = num_str.chars().rev().collect();
                        if !num.is_empty() {
                            with_factorial.push_str(&format!("factorial({})", num));
                            continue;
                        }
                    }
                }
            }
            with_factorial.push(c);
        }
        s = with_factorial;

        // Replace word math operators with symbols
        let replacements: Vec<(&str, &str)> = vec![
            // Word forms → symbols
            (" plus ", " + "),
            (" minus ", " - "),
            (" times ", " * "),
            (" multiplied by ", " * "),
            (" divided by ", " / "),
            (" to the power of ", " ^ "),
            (" raised to ", " ^ "),
            (" modulo ", " % "),
            (" mod ", " % "),
            // Suffixes
            (" squared", "^2"),
            (" cubed", "^3"),
            // Question words to strip
            ("what is the ", ""),
            ("what is ", ""),
            ("compute ", ""),
            ("calculate ", ""),
            ("evaluate ", ""),
            ("find ", ""),
            ("determine ", ""),
            ("solve ", ""),
            // Articles
            (" a ", " "),
            (" an ", " "),
            (" the ", " "),
            // Punctuation (keep '!' — handled above)
            ("?", ""),
            (",", ""),
            (".", ""),
        ];

        for (from, to) in replacements {
            s = s.replace(from, to);
        }

        s.trim().to_string()
    }

    /// Extract a math expression from a normalized question.
    ///
    /// Handles:
    /// - "2 + 2" → "2 + 2"
    /// - "sqrt(144)" → "sqrt(144)"
    /// - "the largest prime divisor of 8139881" → special function
    /// - "the value of integral ..." → too complex, returns None
    fn extract_expression(q: &str) -> Option<String> {
        let q = q.trim();

        // Recognized function prefixes (must be checked before generic fallback)
        let func_prefixes = [
            "sqrt", "sin", "cos", "tan", "log", "ln", "exp", "abs",
            "ceil", "floor", "round", "factorial",
            "gcd", "lcm", "ncr", "npr",
            "is_prime", "largest_prime_divisor",
        ];

        // Direct arithmetic: starts with a number or parenthesis or function
        let starts_with_func = func_prefixes.iter().any(|f| q.starts_with(f));
        if q.starts_with(|c: char| c.is_ascii_digit() || c == '(' || c == '-')
            || starts_with_func
            || q.starts_with("pi")
            || q.starts_with("e")
        {
            if Self::looks_like_expression(q) {
                return Some(q.to_string());
            }
        }

        // "largest prime divisor of N" or "largest prime factor of N"
        if q.starts_with("largest prime divisor of ")
            || q.starts_with("largest prime factor of ")
            || q.starts_with("the largest prime divisor of ")
            || q.starts_with("the largest prime factor of ")
        {
            let num_str = q
                .split("of ")
                .last()
                .unwrap_or("")
                .trim()
                .trim_end_matches('.')
                .trim_end_matches('?');
            if let Ok(n) = num_str.parse::<u64>() {
                return Some(format!("largest_prime_divisor({})", n));
            }
        }

        // "how many X in Y?" — needs context, return None for now
        // "number of ..." — too vague, return None

        None
    }

    /// Check if a string looks like a valid math expression.
    fn looks_like_expression(s: &str) -> bool {
        let has_operator = s.contains(|c: char| "+-*/^%".contains(c));
        let function_names = [
            "sqrt(", "sin(", "cos(", "tan(", "log(", "ln(", "exp(",
            "abs(", "ceil(", "floor(", "round(", "factorial(",
            "gcd(", "lcm(", "ncr(", "npr(",
            "largest_prime_divisor(", "is_prime(",
        ];
        let has_function = function_names.iter().any(|f| s.contains(f))
            || s.starts_with("pi")
            || s.starts_with("e")
            || s.starts_with('-');
        let has_number = s.chars().any(|c| c.is_ascii_digit());
        has_operator || has_function || has_number
    }

    /// Evaluate a math expression string and return the result.
    fn evaluate(expr: &str) -> Option<String> {
        let expr = expr.trim();

        // Handle special functions first
        if let Some(result) = Self::eval_special_function(expr) {
            return Some(result);
        }

        // Parse and evaluate arithmetic
        Self::eval_arithmetic(expr)
    }

    /// Evaluate special named functions.
    fn eval_special_function(expr: &str) -> Option<String> {
        // largest_prime_divisor(n)
        if expr.starts_with("largest_prime_divisor(") && expr.ends_with(')') {
            let inner = &expr["largest_prime_divisor(".len()..expr.len() - 1];
            if let Ok(n) = inner.trim().parse::<u64>() {
                return Self::largest_prime_divisor(n).map(|r| r.to_string());
            }
        }

        // is_prime(n)
        if expr.starts_with("is_prime(") && expr.ends_with(')') {
            let inner = &expr["is_prime(".len()..expr.len() - 1];
            if let Ok(n) = inner.trim().parse::<u64>() {
                return Some(if Self::is_prime(n) {
                    "yes".to_string()
                } else {
                    "no".to_string()
                });
            }
        }

        // factorial(n)
        if expr.starts_with("factorial(") && expr.ends_with(')') {
            let inner = &expr["factorial(".len()..expr.len() - 1];
            if let Ok(n) = inner.trim().parse::<u64>() {
                if n <= 20 {
                    return Self::factorial(n).map(|r| r.to_string());
                }
            }
        }

        // sqrt(x)
        if expr.starts_with("sqrt(") && expr.ends_with(')') {
            let inner = &expr["sqrt(".len()..expr.len() - 1];
            if let Ok(x) = inner.trim().parse::<f64>() {
                if x >= 0.0 {
                    let result = x.sqrt();
                    return Some(Self::format_float(result));
                }
            }
        }

        // abs(x)
        if expr.starts_with("abs(") && expr.ends_with(')') {
            let inner = &expr["abs(".len()..expr.len() - 1];
            if let Ok(x) = inner.trim().parse::<f64>() {
                return Some(Self::format_float(x.abs()));
            }
        }

        // sin(x), cos(x), tan(x) — in radians
        if expr.starts_with("sin(") && expr.ends_with(')') {
            let inner = &expr["sin(".len()..expr.len() - 1];
            if let Ok(x) = inner.trim().parse::<f64>() {
                return Some(Self::format_float(x.sin()));
            }
        }
        if expr.starts_with("cos(") && expr.ends_with(')') {
            let inner = &expr["cos(".len()..expr.len() - 1];
            if let Ok(x) = inner.trim().parse::<f64>() {
                return Some(Self::format_float(x.cos()));
            }
        }
        if expr.starts_with("tan(") && expr.ends_with(')') {
            let inner = &expr["tan(".len()..expr.len() - 1];
            if let Ok(x) = inner.trim().parse::<f64>() {
                return Some(Self::format_float(x.tan()));
            }
        }

        // log(x) — base 10, ln(x) — natural log
        if expr.starts_with("log(") && expr.ends_with(')') {
            let inner = &expr["log(".len()..expr.len() - 1];
            if let Ok(x) = inner.trim().parse::<f64>() {
                if x > 0.0 {
                    return Some(Self::format_float(x.log10()));
                }
            }
        }
        if expr.starts_with("ln(") && expr.ends_with(')') {
            let inner = &expr["ln(".len()..expr.len() - 1];
            if let Ok(x) = inner.trim().parse::<f64>() {
                if x > 0.0 {
                    return Some(Self::format_float(x.ln()));
                }
            }
        }

        // exp(x)
        if expr.starts_with("exp(") && expr.ends_with(')') {
            let inner = &expr["exp(".len()..expr.len() - 1];
            if let Ok(x) = inner.trim().parse::<f64>() {
                return Some(Self::format_float(x.exp()));
            }
        }

        // ceil(x), floor(x), round(x)
        if expr.starts_with("ceil(") && expr.ends_with(')') {
            let inner = &expr["ceil(".len()..expr.len() - 1];
            if let Ok(x) = inner.trim().parse::<f64>() {
                return Some(Self::format_float(x.ceil()));
            }
        }
        if expr.starts_with("floor(") && expr.ends_with(')') {
            let inner = &expr["floor(".len()..expr.len() - 1];
            if let Ok(x) = inner.trim().parse::<f64>() {
                return Some(Self::format_float(x.floor()));
            }
        }
        if expr.starts_with("round(") && expr.ends_with(')') {
            let inner = &expr["round(".len()..expr.len() - 1];
            if let Ok(x) = inner.trim().parse::<f64>() {
                return Some(Self::format_float(x.round()));
            }
        }

        // gcd(a, b), lcm(a, b)
        if expr.starts_with("gcd(") && expr.ends_with(')') {
            let inner = &expr["gcd(".len()..expr.len() - 1];
            if let Some((a, b)) = Self::split_args(inner) {
                if let (Ok(ai), Ok(bi)) = (a.trim().parse::<u64>(), b.trim().parse::<u64>()) {
                    return Some(Self::gcd(ai, bi).to_string());
                }
            }
        }
        if expr.starts_with("lcm(") && expr.ends_with(')') {
            let inner = &expr["lcm(".len()..expr.len() - 1];
            if let Some((a, b)) = Self::split_args(inner) {
                if let (Ok(ai), Ok(bi)) = (a.trim().parse::<u64>(), b.trim().parse::<u64>()) {
                    return Some(Self::lcm(ai, bi).to_string());
                }
            }
        }

        // nCr(n, r), nPr(n, r)
        if expr.starts_with("ncr(") && expr.ends_with(')') {
            let inner = &expr["ncr(".len()..expr.len() - 1];
            if let Some((n, r)) = Self::split_args(inner) {
                if let (Ok(ni), Ok(ri)) = (n.trim().parse::<u64>(), r.trim().parse::<u64>()) {
                    if ni <= 60 {
                        return Self::ncr(ni, ri).map(|r| r.to_string());
                    }
                }
            }
        }
        if expr.starts_with("npr(") && expr.ends_with(')') {
            let inner = &expr["npr(".len()..expr.len() - 1];
            if let Some((n, r)) = Self::split_args(inner) {
                if let (Ok(ni), Ok(ri)) = (n.trim().parse::<u64>(), r.trim().parse::<u64>()) {
                    if ni <= 60 {
                        return Self::npr(ni, ri).map(|r| r.to_string());
                    }
                }
            }
        }

        None
    }

    /// Simple arithmetic evaluator using operator precedence.
    /// Handles: +, -, *, /, ^ (power), constant substitution.
    fn eval_arithmetic(expr: &str) -> Option<String> {
        let expr = expr.trim();

        // Handle pure constants
        if expr == "pi" {
            return Some(Self::format_float(std::f64::consts::PI));
        }
        if expr == "e" {
            return Some(Self::format_float(std::f64::consts::E));
        }
        if expr == "infinity" || expr == "infinite" {
            return Some("infinite".to_string());
        }

        // Try to handle multi-step arithmetic via shunting-yard
        // For simplicity: detect simple binary operations
        // Order: ^ (right-assoc), * / (left-assoc), + - (left-assoc)
        let expr = Self::substitute_constants(expr);

        // Try parsing with precedence
        let result = Self::parse_add_sub(&expr)?;
        // Check for special float results
        if result.is_infinite() || result.is_nan() {
            return None;
        }
        Some(Self::format_float(result))
    }

    /// Substitute constant names with their values.
    fn substitute_constants(expr: &str) -> String {
        expr.replace("pi", &format!("{}", std::f64::consts::PI))
            .replace("e", &format!("{}", std::f64::consts::E))
    }

    /// Remove whitespace from an expression string for easier parsing.
    fn strip_spaces(s: &str) -> String {
        s.chars().filter(|c| !c.is_whitespace()).collect()
    }

    /// Parse addition and subtraction (lowest precedence).
    /// Operates on a space-free expression.
    fn parse_add_sub(s: &str) -> Option<f64> {
        let s = Self::strip_spaces(s);
        if s.is_empty() {
            return None;
        }

        // Find the rightmost + or - that is NOT inside parentheses,
        // splitting at the lowest-precedence operator first.
        let mut depth: i32 = 0;
        let mut split_pos: Option<usize> = None;
        let mut last_was_op = true; // leading '-' is negation

        for (i, c) in s.char_indices() {
            match c {
                '(' => depth += 1,
                ')' => depth -= 1,
                '+' if depth == 0 && !last_was_op => {
                    split_pos = Some(i);
                    last_was_op = true;
                }
                '-' if depth == 0 => {
                    if !last_was_op {
                        split_pos = Some(i);
                    }
                    last_was_op = true;
                    continue;
                }
                _ => {}
            }
            if !c.is_whitespace() {
                last_was_op = false;
            }
        }

        if let Some(pos) = split_pos {
            let left = &s[..pos];
            let right = &s[pos + 1..];
            let op = s.chars().nth(pos)?;
            let l = Self::parse_add_sub(left)?;
            // Right side of + or - may itself contain + or -,
            // so recurse through parse_add_sub instead of parse_mul_div.
            let r = Self::parse_add_sub(right)?;
            match op {
                '+' => Some(l + r),
                '-' => Some(l - r),
                _ => None,
            }
        } else {
            // No + or - at depth 0, try mul_div
            Self::parse_mul_div(&s)
        }
    }

    /// Parse multiplication, division, and power (higher precedence).
    fn parse_mul_div(s: &str) -> Option<f64> {
        let s = Self::strip_spaces(s);
        if s.is_empty() {
            return None;
        }

        // Handle ^ (power) — right-associative, find the rightmost ^ at depth 0
        let mut depth: i32 = 0;
        let mut pow_pos: Option<usize> = None;
        for (i, c) in s.char_indices() {
            match c {
                '(' => depth += 1,
                ')' => depth -= 1,
                '^' if depth == 0 => {
                    pow_pos = Some(i);
                    depth = 0; // reset for right-assoc tracking
                }
                _ => {}
            }
        }

        if let Some(pos) = pow_pos {
            let left = &s[..pos];
            let right = &s[pos + 1..];
            let l = Self::parse_mul_div(left)?;
            let r = Self::parse_add_sub(right)?;

            // For integer exponents, use powi when possible
            if r.fract() == 0.0 && r.is_finite() {
                let exp = r as i32;
                return Some(l.powi(exp));
            }
            return Some(l.powf(r));
        }

        // Handle * and / — left-associative, find the leftmost at depth 0
        depth = 0;
        let mut mul_pos: Option<usize> = None;
        for (i, c) in s.char_indices() {
            match c {
                '(' => depth += 1,
                ')' => depth -= 1,
                '*' | '/' if depth == 0 => {
                    mul_pos = Some(i);
                    break; // leftmost
                }
                _ => {}
            }
        }

        if let Some(pos) = mul_pos {
            let left = &s[..pos];
            let right = &s[pos + 1..];
            let op = s.chars().nth(pos)?;
            let l = Self::parse_atom(left)?;
            let r = Self::parse_mul_div(right)?;
            match op {
                '*' => Some(l * r),
                '/' => {
                    if r == 0.0 { None } else { Some(l / r) }
                }
                _ => None,
            }
        } else {
            // No operator at depth 0 — it's an atom
            Self::parse_atom(&s)
        }
    }

    /// Parse an atom: number, parenthesized expression, function, or constant.
    fn parse_atom(s: &str) -> Option<f64> {
        let s = s.trim();

        if s.is_empty() {
            return None;
        }

        // Handle parenthesized expressions properly by finding matching parens
        if s.starts_with('(') {
            // Find the matching closing paren
            let mut depth: i32 = 0;
            for (i, c) in s.char_indices() {
                match c {
                    '(' => depth += 1,
                    ')' => {
                        depth -= 1;
                        if depth == 0 {
                            // Found matching paren at position i
                            let inner = &s[1..i];
                            let rest = &s[i + 1..].trim();
                            // Parse the inner expression
                            let inner_val = Self::parse_add_sub(inner)?;
                            // If there's more after the paren (e.g., "(2+3)*4"),
                            // we need to handle it. But parse_mul_div handles this
                            // because it splits on * /, so we just return the inner value.
                            if rest.is_empty() {
                                return Some(inner_val);
                            }
                            // There's something after the closing paren, recurse
                            // by treating this as implicit multiplication or explicit op
                            return Self::parse_mul_div(&format!("{} {}", inner_val, rest));
                        }
                    }
                    _ => {}
                }
            }
            // No matching paren found
            return None;
        }

        // Function call — evaluate via special function and try to parse the result as a number
        if let Some(pos) = s.find('(') {
            // Find matching closing paren
            let func_name = &s[..pos];
            let after_open = &s[pos + 1..];
            let mut depth: i32 = 1;
            let mut close_pos: Option<usize> = None;
            for (i, c) in after_open.char_indices() {
                match c {
                    '(' => depth += 1,
                    ')' => {
                        depth -= 1;
                        if depth == 0 {
                            close_pos = Some(i);
                            break;
                        }
                    }
                    _ => {}
                }
            }
            if let Some(cp) = close_pos {
                let args = &after_open[..cp];
                let reconstructed = format!("{}({})", func_name, args);
                if let Some(result_str) = Self::eval_special_function(&reconstructed) {
                    if let Ok(val) = result_str.parse::<f64>() {
                        return Some(val);
                    }
                }
            }
        }

        // Plain number or variable
        if s == "pi" {
            return Some(std::f64::consts::PI);
        }
        if s == "e" {
            return Some(std::f64::consts::E);
        }

        // Number (including negatives)
        s.parse::<f64>().ok()
    }

    // ═══════════════════════════════════════════════════════════════════════
    // NUMBER THEORY HELPERS
    // ═══════════════════════════════════════════════════════════════════════

    /// Largest prime divisor of n.
    fn largest_prime_divisor(mut n: u64) -> Option<u64> {
        if n < 2 {
            return None;
        }
        let mut largest = 1;
        // Handle factor 2
        while n % 2 == 0 {
            largest = 2;
            n /= 2;
        }
        // Handle odd factors
        let mut i = 3;
        while i * i <= n {
            while n % i == 0 {
                largest = i;
                n /= i;
            }
            i += 2;
        }
        if n > 1 {
            largest = n;
        }
        Some(largest)
    }

    /// Primality test (trial division up to sqrt).
    fn is_prime(n: u64) -> bool {
        if n < 2 {
            return false;
        }
        if n == 2 {
            return true;
        }
        if n % 2 == 0 {
            return false;
        }
        let mut i = 3;
        while i * i <= n {
            if n % i == 0 {
                return false;
            }
            i += 2;
        }
        true
    }

    /// Factorial (n ≤ 20 for u64 safety).
    fn factorial(n: u64) -> Option<u64> {
        if n > 20 {
            return None;
        }
        Some((1..=n).product::<u64>())
    }

    /// GCD via Euclidean algorithm.
    fn gcd(a: u64, b: u64) -> u64 {
        if b == 0 {
            a
        } else {
            Self::gcd(b, a % b)
        }
    }

    /// LCM via GCD.
    fn lcm(a: u64, b: u64) -> u64 {
        a / Self::gcd(a, b) * b
    }

    /// n choose r (binomial coefficient).
    fn ncr(n: u64, r: u64) -> Option<u64> {
        if r > n {
            return Some(0);
        }
        let r = r.min(n - r);
        if r == 0 {
            return Some(1);
        }
        // Compute using multiplicative formula to avoid overflow
        let mut result = 1u128;
        for i in 1..=r {
            result = result * (n - r + i) as u128 / i as u128;
            if result > u64::MAX as u128 {
                return None;
            }
        }
        Some(result as u64)
    }

    /// n permute r.
    fn npr(n: u64, r: u64) -> Option<u64> {
        if r > n {
            return Some(0);
        }
        let mut result = 1u128;
        for i in 0..r {
            result *= (n - i) as u128;
            if result > u64::MAX as u128 {
                return None;
            }
        }
        Some(result as u64)
    }

    /// Split function arguments separated by ',' or space.
    fn split_args(s: &str) -> Option<(&str, &str)> {
        // First try comma (preserved in some cases)
        if let Some((a, b)) = s.split_once(',') {
            return Some((a.trim(), b.trim()));
        }
        // Then try space-separated (common after normalize strips commas)
        let trimmed = s.trim();
        if let Some(pos) = trimmed.find(|c: char| c.is_whitespace()) {
            let a = trimmed[..pos].trim();
            let b = trimmed[pos..].trim();
            if !a.is_empty() && !b.is_empty() {
                return Some((a, b));
            }
        }
        None
    }

    /// Format a float nicely, avoiding unnecessary decimals.
    fn format_float(x: f64) -> String {
        if x.is_infinite() {
            return "infinite".to_string();
        }
        if x.is_nan() {
            return "undefined".to_string();
        }
        // Check if it's a whole number (within precision)
        let rounded = x.round();
        if (x - rounded).abs() < 1e-12 {
            format!("{}", rounded as i64)
        } else if (x * 1e10).round() / 1e10 == x {
            // Fewer than 10 decimal places
            format!("{}", x)
        } else {
            // Limit to 10 decimal places
            format!("{:.10}", x).trim_end_matches('0')
                .trim_end_matches('.')
                .to_string()
        }
    }

    // ═══════════════════════════════════════════════════════════════════════
    // WORD PROBLEM SOLVER
    // ═══════════════════════════════════════════════════════════════════════
    //
    // Handles distance-rate-time (DRT) word problems:
    //
    //   "A train leaves Station A at 2:00 PM traveling at 60 mph toward
    //    Station B. Another train leaves Station B at 3:00 PM traveling at
    //    75 mph toward Station A. The stations are 300 miles apart. What
    //    time do they meet?"
    //
    // Architecture:
    //
    //   try_word_problem(q)
    //     └─ try_drt_problem(q)
    //          ├─ extract_speeds(q)        → Vec<f64>
    //          ├─ extract_departure_times(q) → Vec<f64>
    //          ├─ extract_distance(q)      → f64
    //          ├─ solve_approaching(...)   → "They meet at 4:47 PM"
    //          ├─ solve_same_direction(...) → "They meet after 3 hours"
    //          └─ try_single_vehicle(...)  → "180 miles"
    // ═══════════════════════════════════════════════════════════════════════

    /// Try to answer a word problem.
    pub fn try_word_problem(question: &str) -> Option<String> {
        let lower = question.to_lowercase();

        // Try distance-rate-time problems
        if let Some(result) = Self::try_drt_problem(&lower) {
            return Some(result);
        }

        None
    }

    /// Check if the question describes a DRT problem.
    fn is_drt_problem(q: &str) -> bool {
        let travel_keywords = [
            "train", "car", "truck", "bus", "plane", "boat", "ship",
            "bike", "cyclist", "driver", "walk", "run", "jog", "vehicle",
            "travel", "journey", "trip", "drive", "ride", "flight",
            "passenger", "commuter",
        ];
        if !travel_keywords.iter().any(|k| q.contains(k)) {
            return false;
        }

        let rate_keywords = [
            "mph", "km/h", "kph", "miles per hour", "kilometers per hour",
            "mi/h", "kmh", "speed", "traveling at", "travels at",
            "miles", "kilometers", "km", "distance", "hours", "minutes",
            "per hour",
        ];
        rate_keywords.iter().any(|k| q.contains(k))
    }

    /// Try to solve a distance-rate-time problem.
    fn try_drt_problem(q: &str) -> Option<String> {
        if !Self::is_drt_problem(q) {
            return None;
        }

        let speeds = Self::extract_speeds(q).unwrap_or_default();
        let departure_times = Self::extract_departure_times(q);
        let distance = Self::extract_distance(q);

        // Detect relationship from keywords
        let approaching = q.contains("toward") || q.contains("towards")
            || q.contains("approaching") || q.contains("meet")
            || q.contains("each other") || q.contains("heading")
            || q.contains("apart") || q.contains("between");
        let same_direction = q.contains("same direction") || q.contains("catch")
            || q.contains("overtake") || q.contains("passes")
            || q.contains("chase");

        // ── Two-vehicle approaching ───────────────────────────────────────
        if speeds.len() >= 2 {
            if let Some(d) = distance {
                if let Some(ref times) = departure_times {
                    if times.len() >= 2 {
                        if approaching {
                            return Self::solve_approaching(&speeds, times, d, q);
                        }
                        if same_direction {
                            return Self::solve_same_direction(&speeds, times, d, q);
                        }
                    }
                }
                // No departure times → assume simultaneous departure
                if approaching || same_direction {
                    let dummy = vec![0.0, 1.0]; // Δt=1h default if unknown
                    if approaching {
                        return Self::solve_approaching(&speeds, &dummy, d, q);
                    }
                }
            }
        }

        // ── Single-vehicle: have 2 of {speed, distance, time} ─────────────
        if let Some(result) = Self::try_single_vehicle(q, &speeds, distance) {
            return Some(result);
        }

        None
    }

    /// Extract numeric speeds from the text (e.g., "60 mph", "75 miles per hour").
    fn extract_speeds(q: &str) -> Option<Vec<f64>> {
        let re = Regex::new(r"(\d+(?:\.\d+)?)\s*(?:mph|mi/h|miles?\s*per\s*hour|km/h|kph|kilometers?\s*per\s*hour)")
            .ok()?;
        let speeds: Vec<f64> = re.captures_iter(q)
            .filter_map(|cap| cap[1].parse::<f64>().ok().filter(|&s| s > 0.0 && s < 10000.0))
            .collect();
        if speeds.is_empty() { None } else { Some(speeds) }
    }

    /// Extract departure times (24h decimal hours).  Requires AM/PM.
    fn extract_departure_times(q: &str) -> Option<Vec<f64>> {
        let re = Regex::new(r"\b(\d{1,2})(?::(\d{2}))?\s*(AM|PM|am|pm)\b").ok()?;
        let times: Vec<f64> = re.captures_iter(q)
            .filter_map(|cap| {
                let hour: f64 = cap[1].parse().ok()?;
                let minute: f64 = cap.get(2).and_then(|m| m.as_str().parse().ok()).unwrap_or(0.0);
                let is_pm = cap.get(3).map(|m| m.as_str().to_lowercase() == "pm").unwrap_or(false);

                if hour < 1.0 || hour > 12.0 || minute >= 60.0 {
                    return None;
                }

                let mut decimal = hour + minute / 60.0;
                if is_pm && hour < 12.0 {
                    decimal += 12.0;
                }
                if !is_pm && hour >= 12.0 {
                    decimal -= 12.0; // 12 AM = midnight (0.0)
                }
                Some(decimal)
            })
            .collect();
        if times.is_empty() { None } else { Some(times) }
    }

    /// Extract total distance (the *last* "N miles/Km" pair that is NOT part
    /// of a speed expression like "60 miles per hour").
    fn extract_distance(q: &str) -> Option<f64> {
        let lower = q.to_lowercase();

        // Remove "N miles per hour" / "N mph" / "N km/h" patterns so they
        // don't contaminate distance extraction.
        let speed_re = Regex::new(
            r"\d+(?:\.\d+)?\s*(?:mph|mi/h|miles?\s*per\s*hour|km/h|kph|kilometers?\s*per\s*hour)"
        ).ok()?;
        let cleaned = speed_re.replace_all(&lower, "");

        // Look for "N miles", "N km", "N kilometers" bigrams in remaining text
        let words: Vec<&str> = cleaned.split_whitespace()
            .map(|w| w.trim_end_matches(|c: char| !c.is_ascii_alphanumeric()))
            .collect();
        let distances: Vec<f64> = words.windows(2)
            .filter_map(|pair| {
                let num = pair[0].parse::<f64>().ok()?;
                match pair[1] {
                    "miles" | "mile" | "kilometers" | "kilometer" | "km" => Some(num),
                    _ => None,
                }
            })
            .collect();

        distances.last().copied()
    }

    /// Solve the approaching scenario:  s₁·(t − d₁) + s₂·(t − d₂) = D
    /// Returns the answer formatted based on what the question asks.
    fn solve_approaching(speeds: &[f64], times: &[f64], distance: f64, q: &str) -> Option<String> {
        let s1 = speeds[0];
        let s2 = speeds[1];
        let d1 = times[0];
        let d2 = times[1];

        // Equation:  s1*(t - d1) + s2*(t - d2) = D
        //            (s1 + s2)*t - s1*d1 - s2*d2 = D
        //            t = (D + s1*d1 + s2*d2) / (s1 + s2)
        let denominator = s1 + s2;
        if denominator == 0.0 {
            return None;
        }
        let t = (distance + s1 * d1 + s2 * d2) / denominator;

        // Determine what the question asks for
        if q.contains("what time") || q.contains("when will") || q.contains("what hour")
            || q.contains("when do") || q.contains("what o'clock") || q.contains("what is the time")
        {
            // "What time do they meet?" → clock time
            let time_str = Self::format_time_of_day(t);
            return Some(format!("They meet at {}", time_str));
        }

        if q.contains("how far") || q.contains("what distance") || q.contains("how many miles")
            || q.contains("how many kilometers")
        {
            // "How far from Station A?" → distance first train traveled
            let dist_a = s1 * (t - d1);
            if dist_a < 0.0 {
                return None;
            }
            return Some(format!("They meet {:.2} miles from the first train's starting point", dist_a));
        }

        if q.contains("how long") || q.contains("how many hours") || q.contains("how much time")
        {
            // "How long until they meet?" → duration since first departure
            let duration = t - d1;
            let formatted = Self::format_duration(duration);
            return Some(format!("They meet after {}", formatted));
        }

        // Default: return the meeting time
        let time_str = Self::format_time_of_day(t);
        Some(format!("They meet at {}", time_str))
    }

    /// Solve the same-direction / catch-up scenario.
    /// s₁·(t − d₁) + gap = s₂·(t − d₂)   when there's an initial gap
    /// s₁·(t − d₁) = s₂·(t − d₂)         when starting from same point
    fn solve_same_direction(speeds: &[f64], times: &[f64], _distance: f64, q: &str) -> Option<String> {
        let s1 = speeds[0];
        let s2 = speeds[1];
        let d1 = times[0];
        let d2 = times[1];

        if s2 <= s1 {
            return Some(format!("The second vehicle never catches up (it is not faster)."));
        }

        // Equation: s1*(t - d1) = s2*(t - d2) → catch-up from same point
        // s1*t - s1*d1 = s2*t - s2*d2
        // (s1 - s2)*t = s1*d1 - s2*d2
        // t = (s1*d1 - s2*d2) / (s1 - s2)
        let denominator = s1 - s2;
        let t = (s1 * d1 - s2 * d2) / denominator;

        if q.contains("what time") || q.contains("when") {
            let time_str = Self::format_time_of_day(t);
            return Some(format!("The second vehicle catches up at {}", time_str));
        }

        let duration = t - d2.min(d1);
        let formatted = Self::format_duration(duration);
        Some(format!("The second vehicle catches up after {}", formatted))
    }

    /// Try a single-vehicle problem: have 2 of {speed, distance, time}.
    fn try_single_vehicle(q: &str, speeds: &[f64], distance: Option<f64>) -> Option<String> {
        // Extract a duration (time interval, not clock time)
        let duration_re = Regex::new(r"(\d+(?:\.\d+)?)\s*(?:hours?|minutes?|hrs?|h)\b").ok()?;
        let duration: Option<f64> = duration_re.captures_iter(q)
            .filter_map(|cap| cap[1].parse::<f64>().ok())
            .next();

        // We need 2 of 3: speed, distance, time
        let s = speeds.first().copied();
        let d = distance;
        let t = duration;

        match (s, d, t) {
            // d = r * t
            (Some(sp), None, Some(ti)) => {
                let dist = sp * ti;
                let unit = if q.contains("km") || q.contains("kilometer") { "km" } else { "miles" };
                Some(format!("{} {}", Self::format_float(dist), unit))
            }
            // r = d / t
            (None, Some(di), Some(ti)) => {
                if ti == 0.0 { return None; }
                let speed = di / ti;
                let unit = if q.contains("km") || q.contains("kilometer") { "km/h" } else { "mph" };
                Some(format!("{} {}", Self::format_float(speed), unit))
            }
            // t = d / r
            (Some(sp), Some(di), None) => {
                if sp == 0.0 { return None; }
                let time = di / sp;
                Some(format!("{} hours", Self::format_float(time)))
            }
            _ => None,
        }
    }

    /// Format a decimal time-of-day (e.g. 16.777 → "4:47 PM").
    fn format_time_of_day(decimal_hours: f64) -> String {
        let total_seconds = (decimal_hours * 3600.0).round() as i64;
        let hours_24 = total_seconds / 3600;
        let minutes = (total_seconds % 3600) / 60;
        let seconds = total_seconds % 60;

        let period = if hours_24 >= 12 { "PM" } else { "AM" };
        let display_hour = match hours_24 {
            0 | 12 => 12,
            h if h > 12 => h - 12,
            h => h,
        };

        if seconds > 0 {
            format!("{}:{:02}:{:02} {}", display_hour, minutes, seconds, period)
        } else {
            format!("{}:{:02} {}", display_hour, minutes, period)
        }
    }

    /// Format a duration in hours (e.g. 2.777 → "2 hours and 47 minutes").
    fn format_duration(hours: f64) -> String {
        if hours < 0.0 {
            return "0 minutes".to_string();
        }
        let total_minutes = (hours * 60.0).round() as i64;
        let h = total_minutes / 60;
        let m = total_minutes % 60;

        match (h, m) {
            (0, 0) => "0 minutes".to_string(),
            (0, m) => format!("{} minute{}", m, if m == 1 { "" } else { "s" }),
            (h, 0) => format!("{} hour{}", h, if h == 1 { "" } else { "s" }),
            (h, m) => format!("{} hour{} and {} minute{}",
                h, if h == 1 { "" } else { "s" },
                m, if m == 1 { "" } else { "s" }),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// TESTS
// ═══════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_arithmetic() {
        assert_eq!(MathEngine::try_answer("What is 2 + 2?"), Some("4".to_string()));
        assert_eq!(MathEngine::try_answer("What is 10 - 3?"), Some("7".to_string()));
        assert_eq!(MathEngine::try_answer("What is 4 * 5?"), Some("20".to_string()));
        assert_eq!(MathEngine::try_answer("What is 20 / 4?"), Some("5".to_string()));
    }

    #[test]
    fn test_word_forms() {
        assert_eq!(MathEngine::try_answer("What is 2 plus 2?"), Some("4".to_string()));
        assert_eq!(MathEngine::try_answer("What is 10 minus 3?"), Some("7".to_string()));
        assert_eq!(MathEngine::try_answer("What is 4 times 5?"), Some("20".to_string()));
        assert_eq!(MathEngine::try_answer("What is 20 divided by 4?"), Some("5".to_string()));
    }

    #[test]
    fn test_power_and_sqrt() {
        assert_eq!(MathEngine::try_answer("What is 3 squared?"), Some("9".to_string()));
        assert_eq!(MathEngine::try_answer("What is sqrt(144)?"), Some("12".to_string()));
        assert_eq!(MathEngine::try_answer("Compute 2 ^ 10"), Some("1024".to_string()));
    }

    #[test]
    fn test_largest_prime_divisor() {
        assert_eq!(
            MathEngine::try_answer("What is the largest prime divisor of 8139881?"),
            Some("5003".to_string())
        );
    }

    #[test]
    fn test_factorial() {
        assert_eq!(MathEngine::try_answer("What is 5!"), Some("120".to_string()));
        // Should not match "5!" directly since ! isn't parsed yet
        assert_eq!(MathEngine::try_answer("Compute factorial(5)"), Some("120".to_string()));
    }

    #[test]
    fn test_constants() {
        let pi_result = MathEngine::try_answer("What is pi?");
        assert!(pi_result.is_some());
        assert_eq!(pi_result.unwrap(), "3.1415926536");
    }

    #[test]
    fn test_trig_functions() {
        let result = MathEngine::try_answer("Compute sin(0)");
        assert_eq!(result, Some("0".to_string()));

        let result = MathEngine::try_answer("Compute cos(0)");
        assert_eq!(result, Some("1".to_string()));
    }

    #[test]
    fn test_gcd_lcm() {
        assert_eq!(
            MathEngine::try_answer("What is gcd(12, 8)?"),
            Some("4".to_string())
        );
        assert_eq!(
            MathEngine::try_answer("What is lcm(12, 8)?"),
            Some("24".to_string())
        );
    }

    #[test]
    fn test_ncr_npr() {
        assert_eq!(
            MathEngine::try_answer("Compute nCr(5, 2)"),
            Some("10".to_string())
        );
        assert_eq!(
            MathEngine::try_answer("Compute nPr(5, 2)"),
            Some("20".to_string())
        );
    }

    #[test]
    fn test_non_math_returns_none() {
        assert_eq!(
            MathEngine::try_answer("Who raised rates?"),
            None
        );
        assert_eq!(
            MathEngine::try_answer("What is the meaning of life?"),
            None
        );
    }

    #[test]
    fn test_complex_arithmetic() {
        assert_eq!(
            MathEngine::try_answer("What is 2 + 3 * 4?"),
            Some("14".to_string())
        );
        assert_eq!(
            MathEngine::try_answer("What is (2 + 3) * 4?"),
            Some("20".to_string())
        );
    }

    #[test]
    fn test_is_prime() {
        assert_eq!(
            MathEngine::try_answer("What is is_prime(17)?"),
            Some("yes".to_string())
        );
        assert_eq!(
            MathEngine::try_answer("What is is_prime(15)?"),
            Some("no".to_string())
        );
    }

    #[test]
    fn test_format_float() {
        assert_eq!(MathEngine::format_float(4.0), "4");
        assert_eq!(MathEngine::format_float(3.5), "3.5");
        assert_eq!(MathEngine::format_float(0.0), "0");
        assert_eq!(MathEngine::format_float(f64::INFINITY), "infinite");
        assert_eq!(MathEngine::format_float(f64::NAN), "undefined");
    }

    #[test]
    fn test_solve_system_2x2() {
        let result = MathEngine::try_answer("solve x + y = 3; x - y = 1");
        assert_eq!(result, Some("x = 2, y = 1".to_string()));
    }

    #[test]
    fn test_solve_system_3x3() {
        let result = MathEngine::try_answer("solve x + y + z = 6; 2*x - y + z = 3; x + 2*y - z = 2");
        assert_eq!(result, Some("x = 1, y = 2, z = 3".to_string()));
    }

    #[test]
    fn test_solve_system_scaled() {
        let result = MathEngine::try_answer("solve 2*x + y = 5; x - y = 1");
        assert_eq!(result, Some("x = 2, y = 1".to_string()));
    }

    #[test]
    fn test_solve_system_with_for_clause() {
        // The " for " clause should be stripped before system detection
        let result = MathEngine::try_answer("solve x + y = 3; x - y = 1 for x and y");
        assert_eq!(result, Some("x = 2, y = 1".to_string()));
    }

    // ═══════════════════════════════════════════════════════════════════════
    // WORD PROBLEM TESTS
    // ═══════════════════════════════════════════════════════════════════════

    #[test]
    fn test_two_train_classic() {
        // Train 1: 2:00 PM, 60 mph — Train 2: 3:00 PM, 75 mph — 300 miles
        let q = "A train leaves Station A at 2:00 PM traveling at 60 mph toward \
                  Station B. Another train leaves Station B at 3:00 PM traveling at \
                  75 mph toward Station A. The stations are 300 miles apart. \
                  What time do they meet?";
        let result = MathEngine::try_answer(q);
        assert!(result.is_some(), "classic train problem should be solvable");
        let s = result.unwrap();
        assert!(s.contains("4:46:40 PM") || s.contains("4:47 PM"),
            "expected meeting time ~4:47 PM, got: {}", s);
    }

    #[test]
    fn test_two_train_how_far() {
        // Same problem, asking for distance from first station
        let q = "A train leaves Station A at 2:00 PM traveling at 60 mph toward \
                  Station B. Another train leaves Station B at 3:00 PM traveling at \
                  75 mph toward Station A. The stations are 300 miles apart. \
                  How far from Station A do they meet?";
        let result = MathEngine::try_answer(q);
        assert!(result.is_some(), "should answer how-far question");
        let s = result.unwrap();
        assert!(s.contains("166.67"), "expected ~166.67 miles, got: {}", s);
    }

    #[test]
    fn test_two_train_how_long() {
        // Same problem, asking for elapsed time
        let q = "A train leaves Station A at 2:00 PM traveling at 60 mph toward \
                  Station B. Another train leaves Station B at 3:00 PM traveling at \
                  75 mph toward Station A. The stations are 300 miles apart. \
                  How long until they meet?";
        let result = MathEngine::try_answer(q);
        assert!(result.is_some(), "should answer how-long question");
        let s = result.unwrap();
        assert!(s.contains("2 hours") && (s.contains("46 minutes") || s.contains("47 minutes")),
            "expected ~2h47m, got: {}", s);
    }

    #[test]
    fn test_two_train_same_departure() {
        // Both depart at the same time: 2 PM, 60 mph & 75 mph, 270 miles
        let q = "A train leaves Station A at 2:00 PM traveling at 60 mph. \
                  Another train leaves Station B at 2:00 PM traveling at 75 mph \
                  toward Station A. The stations are 270 miles apart. \
                  When do they meet?";
        let result = MathEngine::try_answer(q);
        assert!(result.is_some(), "same-departure problem should be solvable");
        let s = result.unwrap();
        // 60t + 75t = 270 → 135t = 270 → t = 2 hours after 2 PM = 4 PM
        assert!(s.contains("4:00 PM"), "expected 4:00 PM, got: {}", s);
    }

    #[test]
    fn test_two_train_quiet_narrative() {
        // Plain text without question mark — still contains "meet" cue
        let q = "A train leaves at 2 PM going 60 mph toward another station. \
                  The other train leaves at 3 PM going 75 mph. \
                  The stations are 300 miles apart. What time do they meet?";
        let result = MathEngine::try_answer(q);
        assert!(result.is_some(), "quiet narrative should still parse");
        let s = result.unwrap();
        assert!(s.contains("4:"), "expected meeting time ~4:xx PM, got: {}", s);
    }

    #[test]
    fn test_single_vehicle_distance() {
        // d = r * t
        let q = "A train travels at 60 mph for 3 hours. How far does it go?";
        let result = MathEngine::try_answer(q);
        assert_eq!(result, Some("180 miles".to_string()));
    }

    #[test]
    fn test_single_vehicle_speed() {
        // r = d / t
        let q = "A train travels 300 miles in 5 hours. What is its speed?";
        let result = MathEngine::try_answer(q);
        assert_eq!(result, Some("60 mph".to_string()));
    }

    #[test]
    fn test_single_vehicle_time() {
        // t = d / r
        let q = "A car travels 240 miles at 60 mph. How long does it take?";
        let result = MathEngine::try_answer(q);
        assert_eq!(result, Some("4 hours".to_string()));
    }

    #[test]
    fn test_non_train_returns_none() {
        // Unrelated question should not trigger the word problem solver
        let result = MathEngine::try_answer("Who raised rates?");
        assert_eq!(result, None);
    }

    #[test]
    fn test_word_problem_no_rate_keywords() {
        // Contains "train" but no rate keywords → should not trigger
        let result = MathEngine::try_answer("What is a train?");
        assert_eq!(result, None);
    }

    #[test]
    fn test_format_time_of_day() {
        assert_eq!(MathEngine::format_time_of_day(0.0), "12:00 AM");
        assert_eq!(MathEngine::format_time_of_day(12.0), "12:00 PM");
        assert_eq!(MathEngine::format_time_of_day(14.0), "2:00 PM");
        assert_eq!(MathEngine::format_time_of_day(14.5), "2:30 PM");
        assert_eq!(MathEngine::format_time_of_day(16.7777778), "4:46:40 PM");
        assert_eq!(MathEngine::format_time_of_day(6.0), "6:00 AM");
    }

    #[test]
    fn test_format_duration() {
        assert_eq!(MathEngine::format_duration(0.0), "0 minutes");
        assert_eq!(MathEngine::format_duration(1.0), "1 hour");
        assert_eq!(MathEngine::format_duration(2.0), "2 hours");
        assert_eq!(MathEngine::format_duration(0.5), "30 minutes");
        assert_eq!(MathEngine::format_duration(2.5), "2 hours and 30 minutes");
        assert_eq!(MathEngine::format_duration(2.7777778), "2 hours and 47 minutes");
    }

    #[test]
    fn test_extract_speeds() {
        let q = "traveling at 60 mph toward station. going 75 miles per hour.";
        let speeds = MathEngine::extract_speeds(q).unwrap();
        assert_eq!(speeds, vec![60.0, 75.0]);
    }

    #[test]
    fn test_extract_departure_times() {
        let q = "leaves at 2:00 PM. another leaves at 3:30 PM.";
        let times = MathEngine::extract_departure_times(q).unwrap();
        assert_eq!(times, vec![14.0, 15.5]);
    }

    #[test]
    fn test_extract_distance() {
        let q = "The stations are 300 miles apart.";
        let d = MathEngine::extract_distance(q).unwrap();
        assert_eq!(d, 300.0);
    }

    #[test]
    fn test_extract_distance_ignores_speed() {
        // "60 miles per hour" should NOT be extracted as distance
        let q = "traveling at 60 miles per hour. The stations are 300 miles apart.";
        let d = MathEngine::extract_distance(q).unwrap();
        assert_eq!(d, 300.0, "should skip '60 miles per hour'");
    }

    #[test]
    fn test_extract_distance_only_last() {
        // Multiple distances — return the last one (total distance)
        let q = "A train travels 120 miles, then another 80 miles. The total trip is 200 miles.";
        let d = MathEngine::extract_distance(q).unwrap();
        assert_eq!(d, 200.0, "should return the last distance match");
    }

    #[test]
    fn test_train_leaves_at_2pm_exact() {
        // Matches the classic problem phrasing
        let q = "A train leaves Station A at 2:00 PM traveling at 60 mph. \
                  Another train leaves Station B at 3:00 PM traveling at 75 mph. \
                  The stations are 300 miles apart. When do they meet?";
        let result = MathEngine::try_answer(q);
        assert!(result.is_some(), "should solve the exact classic 'train leaves at 2pm' problem");
        let s = result.unwrap();
        assert!(s.contains("4:46") || s.contains("4:47"), 
            "expected meeting time ~4:47 PM, got: {}", s);
    }

    #[test]
    fn test_two_train_kilometers() {
        // Kilometers version
        let q = "A train departs at 2:00 PM traveling at 100 km/h. \
                  Another train departs at 3:00 PM traveling at 120 km/h. \
                  The stations are 500 km apart. What time do they meet?";
        let result = MathEngine::try_answer(q);
        assert!(result.is_some(), "should solve km version");
        let s = result.unwrap();
        // 100(t-14) + 120(t-15) = 500 → 220t = 500 + 1400 + 1800 = 3700
        // t = 3700/220 = 16.818... → 4:49 PM
        assert!(s.contains("4:49") || s.contains("4:50"),
            "expected ~4:49 PM, got: {}", s);
    }

    #[test]
    fn test_two_train_no_meet_keyword() {
        // No explicit "meet" keyword, just "apart" and "toward"
        let q = "A train leaves at 2 PM at 60 mph toward the south. \
                  Another train leaves at 3 PM at 75 mph toward the north. \
                  They are 300 miles apart. What time do they meet?";
        let result = MathEngine::try_answer(q);
        assert!(result.is_some(), "should detect approaching without 'meet' keyword");
    }

    #[test]
    fn test_single_vehicle_kilometers() {
        let q = "A train travels at 80 km/h for 2 hours. How far does it go?";
        let result = MathEngine::try_answer(q);
        assert_eq!(result, Some("160 km".to_string()));
    }

    #[test]
    fn test_train_not_confused_with_restraint() {
        // "restraint" contains "train" but should not trigger
        let q = "What is the meaning of restraint?";
        let result = MathEngine::try_answer(q);
        assert_eq!(result, None);
    }
}
