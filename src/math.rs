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

use std::collections::HashMap;

// ═══════════════════════════════════════════════════════════════════════════
// CONSTANTS
// ═══════════════════════════════════════════════════════════════════════════

/// Minimum similarity threshold for pattern matching.
/// Most math questions start with predictable prefixes.
const MIN_PATTERN_SCORE: f64 = 0.5;

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

        // Detect question patterns
        if let Some(expr) = Self::extract_expression(&normalized) {
            if let Some(result) = Self::evaluate(&expr) {
                return Some(result);
            }
        }

        None
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
}
