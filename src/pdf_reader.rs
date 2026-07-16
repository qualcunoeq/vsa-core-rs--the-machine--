//! PDF Reader — makes the Machine "see" PDF files.
//!
//! Uses the `pdf-extract` crate to extract text from PDF files,
//! then extracts definition-like SVO triples and stores them as facts.
//!
//! Admin socket command: `READ_PDF <path>`

use std::path::Path;

/// Extract text from a PDF file.
pub fn extract_text(path: &str) -> Result<String, String> {
    let path_obj = Path::new(path);
    if !path_obj.exists() {
        return Err(format!("File not found: {}", path));
    }
    let bytes = std::fs::read(path_obj).map_err(|e| format!("Failed to read file: {}", e))?;
    let text = pdf_extract::extract_text_from_mem(&bytes)
        .map_err(|e| format!("PDF extraction error: {}", e))?;
    if text.trim().is_empty() {
        return Err("PDF extracted no text (possibly scanned/image-based).".to_string());
    }
    Ok(text)
}

/// Extract definition-like SVO triples from raw PDF text.
/// Returns Vec<(subject, verb, object)> — suitable for qa.store_fact().
pub fn extract_definitions(text: &str, _source: &str) -> Vec<(String, String, String)> {
    let mut facts = Vec::new();
    let text = text.replace('\r', " ");
    let lines: Vec<&str> = text.split('\n')
        .map(|l| l.trim())
        .filter(|l| l.len() >= 15 && l.len() <= 400)
        .collect();

    let strip_patterns = [
        "openstax", "creative commons", "cnx.org", "want to cite",
        "all rights reserved", "access for free",
    ];

    for line in &lines {
        let lower = line.to_lowercase();
        if strip_patterns.iter().any(|p| lower.contains(p)) { continue; }
        if lower.starts_with("figure ") || lower.starts_with("table ")
            || lower.starts_with("example ") || lower.starts_with("exercise ")
            || lower.starts_with("solution") || lower.starts_with("checkpoint") { continue; }
        let math_count = line.chars().filter(|c| matches!(c, '{' | '}' | '[' | ']' | '^' | '_' | '$' | '\\')).count();
        if math_count > 8 { continue; }
        if !line.chars().next().map(|c| c.is_alphabetic()).unwrap_or(false) { continue; }

        // Pattern 1: "X is a/an/the Y"
        if let Some(subj) = try_extract_is_definition(line) {
            facts.push(subj);
            continue;
        }

        // Pattern 2: "X is called Y"
        if let Some(subj) = try_extract_is_called(line) {
            facts.push(subj);
            continue;
        }

        // Pattern 3: "X are called Y" (plural)
        if let Some(subj) = try_extract_are_called(line) {
            facts.push(subj);
            continue;
        }

        // Pattern 4: "X refers to/denotes/means Y"
        for verb in &["refers to", "denotes", "means"] {
            if let Some(subj) = try_extract_verb_definition(line, verb) {
                facts.push(subj);
                break;
            }
        }
    }

    facts
}

fn try_extract_is_definition(line: &str) -> Option<(String, String, String)> {
    // Pattern: "X is a/an/the Y"
    let parts: Vec<&str> = line.splitn(2, " is ").collect();
    if parts.len() != 2 { return None; }
    let term = parts[0].trim();
    let rest = parts[1].trim();
    if rest.starts_with("a ") || rest.starts_with("an ") || rest.starts_with("the ") {
        let defn = rest.split(" that").next().unwrap_or(rest)
            .split(" which").next().unwrap_or(rest);
        // Strip leading articles from the term for cleaner matching
        // "A derivative" → "derivative", "The derivative at a point" → "derivative at a point"
        let clean_term = term
            .trim_start_matches("A ")
            .trim_start_matches("An ")
            .trim_start_matches("The ")
            .trim_start_matches("a ")
            .trim_start_matches("an ")
            .trim_start_matches("the ");
        let t = normalize(clean_term);
        let d = normalize(defn);
        if !t.is_empty() && !d.is_empty() && t.len() > 2 {
            return Some((d, "is".to_string(), t));
        }
    }
    None
}

fn try_extract_is_called(line: &str) -> Option<(String, String, String)> {
    let parts: Vec<&str> = line.splitn(2, " is called ").collect();
    if parts.len() != 2 { return None; }
    let desc = parts[0].trim();
    let name_raw = parts[1].trim();
    let name = name_raw.trim_start_matches("a ").trim_start_matches("an ").trim_start_matches("the ");
    let name = name.split(" which").next().unwrap_or(name)
        .split(" that").next().unwrap_or(name)
        .split(',').next().unwrap_or(name).trim();
    let d = normalize(desc);
    let n = normalize(name);
    if !d.is_empty() && !n.is_empty() && n.len() > 2 {
        return Some((n, "is".to_string(), d));
    }
    None
}

fn try_extract_are_called(line: &str) -> Option<(String, String, String)> {
    let parts: Vec<&str> = line.splitn(2, " are called ").collect();
    if parts.len() != 2 { return None; }
    let desc = parts[0].trim();
    let name_raw = parts[1].trim();
    let name = name_raw.trim_start_matches("a ").trim_start_matches("an ").trim_start_matches("the ");
    let d = normalize(desc);
    let n = normalize(name.split(" which").next().unwrap_or(name)
        .split(" that").next().unwrap_or(name).trim());
    if !d.is_empty() && !n.is_empty() && n.len() > 2 {
        return Some((n, "are".to_string(), d));
    }
    None
}

fn try_extract_verb_definition(line: &str, verb: &str) -> Option<(String, String, String)> {
    let search = format!(" {} ", verb);
    if !line.contains(&search) { return None; }
    let parts: Vec<&str> = line.splitn(2, &search).collect();
    if parts.len() != 2 { return None; }
    let subj = parts[0].trim();
    let obj = parts[1].trim().split(" which").next().unwrap_or(parts[1])
        .split(" that").next().unwrap_or(parts[1]).trim();
    let s = normalize(subj);
    let o = normalize(obj);
    if !s.is_empty() && !o.is_empty() && s.len() > 2 {
        return Some((s, verb.replace(' ', "_"), o));
    }
    None
}

fn normalize(s: &str) -> String {
    let s = s.to_lowercase();
    let result: String = s.chars()
        .map(|c| if c.is_alphanumeric() { c } else { '_' })
        .collect();
    let result: String = result.split('_')
        .filter(|w| !w.is_empty())
        .collect::<Vec<_>>()
        .join("_");
    if result.len() > 100 { result[..100].to_string() } else { result }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_text() {
        let result = extract_text("data/openstax_pdfs/prealgebra-2e_-_WEB.pdf");
        assert!(result.is_ok(), "Should extract text from prealgebra PDF");
        let text = result.unwrap();
        assert!(text.len() > 1000, "Should extract substantial text");
    }

    #[test]
    fn test_extract_definitions_basic() {
        let text = "A variable is a symbol that represents a number.\n\
                     The coefficient is the number multiplied by a variable.\n\
                     This process is called factoring.\n";
        let facts = extract_definitions(text, "test.pdf");
        assert!(facts.len() >= 2, "Should extract at least 2 definitions, got {}: {:?}", facts.len(), facts);
        assert!(facts.iter().any(|f| f.2.contains("variable")), "Should find variable");
        assert!(facts.iter().any(|f| f.2.contains("coefficient")), "Should find coefficient");
    }

    #[test]
    fn test_extract_definitions_key_terms() {
        let text = "acceleration is the rate of change of the velocity\n\
                     definite integral a primary operation of calculus\n\
                     chain rule the chain rule defines the derivative";
        let facts = extract_definitions(text, "test.pdf");
        assert!(!facts.is_empty(), "Should extract at least one definition");
    }

    #[test]
    fn test_file_not_found() {
        let result = extract_text("/nonexistent/file.pdf");
        assert!(result.is_err(), "Should error on missing file");
    }

    #[test]
    fn test_normalize() {
        assert_eq!(normalize("The derivative!"), "the_derivative");
        assert_eq!(normalize("rate of change"), "rate_of_change");
        assert_eq!(normalize("  hello   world  "), "hello_world");
    }
}
