//! Bounded natural-language/LaTeX frequency-table frontend.
//!
//! This bridge recognizes only an explicit two-column LaTeX `tabular` whose
//! headers identify a measured value and a frequency/count.  It lowers the
//! table to the generic source-statistics `weighted_mean` formula; it does not
//! infer chart geometry, missing cells, or population semantics.

use crate::probability_pack::Rational;
use crate::source_formula_pack::{FormulaRequest, FormulaResult, FormulaStatus};
use crate::source_statistics_pack::evaluate_statistics;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FrequencyTableStatus {
    Complete,
    Ambiguous,
    Unsupported,
    Missing,
    Inconsistent,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FrequencyTableRow {
    pub value: Rational,
    pub frequency: Rational,
    pub provenance: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FrequencyTableArtifact {
    pub value_header: String,
    pub frequency_header: String,
    pub rows: Vec<FrequencyTableRow>,
    pub provenance: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FrequencyTableResult {
    pub status: FrequencyTableStatus,
    pub artifact: Option<FrequencyTableArtifact>,
    pub request: Option<FormulaRequest>,
    pub statistics: Option<FormulaResult>,
    pub alternatives: Vec<String>,
    pub reasons: Vec<String>,
    pub replay_hash: String,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(value).expect("frequency table serializes"))
    )
}

fn payload(result: &FrequencyTableResult) -> impl Serialize + '_ {
    (
        result.status,
        &result.artifact,
        &result.request,
        &result.statistics,
        &result.alternatives,
        &result.reasons,
    )
}

fn result(
    status: FrequencyTableStatus,
    artifact: Option<FrequencyTableArtifact>,
    request: Option<FormulaRequest>,
    statistics: Option<FormulaResult>,
    alternatives: Vec<String>,
    reasons: Vec<String>,
) -> FrequencyTableResult {
    let mut output = FrequencyTableResult {
        status,
        artifact,
        request,
        statistics,
        alternatives,
        reasons,
        replay_hash: String::new(),
    };
    output.replay_hash = digest(&(
        output.status,
        &output.artifact,
        &output.request,
        &output.statistics,
        &output.alternatives,
        &output.reasons,
    ));
    output
}

fn clean_cell(cell: &str) -> String {
    let mut output = cell.trim().replace("$", "");
    loop {
        let Some(start) = output.find("\\textbf{") else {
            break;
        };
        let content_start = start + "\\textbf{".len();
        let Some(end) = output[content_start..].find('}') else {
            break;
        };
        let end = content_start + end;
        let replacement = output[content_start..end].to_owned();
        output.replace_range(start..=end, &replacement);
    }
    output
        .replace("\\%", "%")
        .replace("\\hline", "")
        .trim()
        .to_owned()
}

fn rational_cell(cell: &str) -> Option<Rational> {
    let cleaned = clean_cell(cell)
        .replace(',', "")
        .replace('%', "")
        .trim()
        .to_owned();
    if let Some((numerator, denominator)) = cleaned.split_once('/') {
        return Rational::new(
            numerator.trim().parse().ok()?,
            denominator.trim().parse().ok()?,
        );
    }
    if let Some((whole, fraction)) = cleaned.split_once('.') {
        if whole.is_empty() || fraction.is_empty() || !fraction.chars().all(|c| c.is_ascii_digit())
        {
            return None;
        }
        let sign = if whole.starts_with('-') { -1 } else { 1 };
        let whole = whole.trim_start_matches('-').parse::<i128>().ok()?;
        let scale = 10_i128.checked_pow(fraction.len() as u32)?;
        let fraction = fraction.parse::<i128>().ok()?;
        return Rational::new(sign * (whole * scale + fraction), scale);
    }
    Rational::new(cleaned.parse().ok()?, 1)
}

fn header_kind(header: &str) -> Option<&'static str> {
    let lower = clean_cell(header).to_ascii_lowercase();
    if lower.contains("score")
        || lower.contains("value")
        || lower.contains("quantity")
        || lower.contains("outcome")
        || lower.contains("measurement")
    {
        Some("value")
    } else if lower.contains("number")
        || lower.contains("count")
        || lower.contains("frequency")
        || lower.contains("students")
        || lower.contains("occurrences")
    {
        Some("frequency")
    } else {
        None
    }
}

fn tabular_body(text: &str) -> Option<&str> {
    let lower = text.to_ascii_lowercase();
    let start = lower.find("\\begin{tabular")?;
    let body_start = text[start..].find('}')? + start + 1;
    let end = lower[body_start..].find("\\end{tabular}")? + body_start;
    Some(&text[body_start..end])
}

/// Lower one explicit two-column value/frequency LaTeX table into a weighted
/// mean request.  All values and frequencies must be present and exact.
pub fn formalize_frequency_table_text(text: &str) -> FrequencyTableResult {
    let lower = text.to_ascii_lowercase();
    if !(lower.contains("mean") || lower.contains("average")) {
        return result(
            FrequencyTableStatus::Missing,
            None,
            None,
            None,
            Vec::new(),
            vec!["no mean or average target was stated".into()],
        );
    }
    let Some(body) = tabular_body(text) else {
        return result(
            FrequencyTableStatus::Missing,
            None,
            None,
            None,
            Vec::new(),
            vec!["no explicit LaTeX tabular was found".into()],
        );
    };
    let rows = body
        .split("\\\\")
        .map(str::trim)
        .filter(|row| !row.is_empty() && !row.contains("\\hline") || row.contains('&'))
        .map(|row| {
            row.trim_matches('|')
                .split('&')
                .map(clean_cell)
                .collect::<Vec<_>>()
        })
        .filter(|row| row.len() >= 2)
        .collect::<Vec<_>>();
    let Some(header_index) = rows.iter().position(|row| {
        row.len() == 2 && header_kind(&row[0]).is_some() && header_kind(&row[1]).is_some()
    }) else {
        return result(
            FrequencyTableStatus::Ambiguous,
            None,
            None,
            None,
            vec!["frequency_table".into()],
            vec!["table headers do not identify one value/frequency orientation".into()],
        );
    };
    let header = &rows[header_index];
    let first_kind = header_kind(&header[0]).expect("header checked");
    let second_kind = header_kind(&header[1]).expect("header checked");
    if first_kind == second_kind {
        return result(
            FrequencyTableStatus::Ambiguous,
            None,
            None,
            None,
            vec!["frequency_table".into()],
            vec!["both table headers have the same semantic kind".into()],
        );
    }
    let value_column = usize::from(first_kind != "value");
    let frequency_column = 1 - value_column;
    let mut parsed_rows = Vec::new();
    let mut provenance = vec![format!("tabular-header:{}", header.join(" & "))];
    for (index, row) in rows.iter().enumerate().skip(header_index + 1) {
        if row.len() != 2 || row.iter().all(|cell| cell.is_empty()) {
            return result(
                FrequencyTableStatus::Ambiguous,
                None,
                None,
                None,
                vec!["frequency_table".into()],
                vec!["table rows do not have exactly two populated cells".into()],
            );
        }
        let Some(value) = rational_cell(&row[value_column]) else {
            return result(
                FrequencyTableStatus::Inconsistent,
                None,
                None,
                None,
                Vec::new(),
                vec!["table value is not an exact rational".into()],
            );
        };
        let Some(frequency) = rational_cell(&row[frequency_column]) else {
            return result(
                FrequencyTableStatus::Inconsistent,
                None,
                None,
                None,
                Vec::new(),
                vec!["table frequency is not an exact rational".into()],
            );
        };
        if frequency.denominator != 1 || frequency.numerator <= 0 {
            return result(
                FrequencyTableStatus::Inconsistent,
                None,
                None,
                None,
                Vec::new(),
                vec!["table frequencies must be positive integers".into()],
            );
        }
        provenance.push(format!("tabular-row-{index}:{}", row.join(" & ")));
        parsed_rows.push(FrequencyTableRow {
            value,
            frequency,
            provenance: format!("tabular-row-{index}"),
        });
    }
    if parsed_rows.len() < 2 {
        return result(
            FrequencyTableStatus::Unsupported,
            None,
            None,
            None,
            Vec::new(),
            vec!["a frequency table needs at least two data rows".into()],
        );
    }
    let weighted_sum = parsed_rows.iter().try_fold(Rational::zero(), |sum, row| {
        sum.add(&row.value.mul(&row.frequency)?)
    });
    let total_weight = parsed_rows
        .iter()
        .try_fold(Rational::zero(), |sum, row| sum.add(&row.frequency));
    let (Some(weighted_sum), Some(total_weight)) = (weighted_sum, total_weight) else {
        return result(
            FrequencyTableStatus::Inconsistent,
            None,
            None,
            None,
            Vec::new(),
            vec!["exact weighted aggregation exceeded the rational budget".into()],
        );
    };
    let artifact = FrequencyTableArtifact {
        value_header: header[value_column].clone(),
        frequency_header: header[frequency_column].clone(),
        rows: parsed_rows,
        provenance: provenance.clone(),
    };
    let request = FormulaRequest {
        formula: "weighted_mean".into(),
        inputs: BTreeMap::from([
            ("weighted_sum".into(), weighted_sum),
            ("total_weight".into(), total_weight),
        ]),
        domain: crate::source_statistics_pack::DOMAIN.into(),
        ambiguity: None,
        provenance,
    };
    let statistics = evaluate_statistics(&request);
    let status = match statistics.status {
        FormulaStatus::Complete => FrequencyTableStatus::Complete,
        FormulaStatus::Inconsistent => FrequencyTableStatus::Inconsistent,
        FormulaStatus::Missing => FrequencyTableStatus::Missing,
        FormulaStatus::Ambiguous => FrequencyTableStatus::Ambiguous,
        FormulaStatus::Unsupported | FormulaStatus::InvalidDomain => {
            FrequencyTableStatus::Unsupported
        }
    };
    result(
        status,
        Some(artifact),
        Some(request),
        Some(statistics),
        Vec::new(),
        Vec::new(),
    )
}

impl FrequencyTableResult {
    pub fn replay_verified(&self) -> bool {
        self.replay_hash == digest(&payload(self))
            && self
                .statistics
                .as_ref()
                .is_none_or(FormulaResult::replay_verified)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TABLE: &str = r#"
    Find the average score.
    \begin{tabular}{|c|c|}
    \hline
    Score & Number of Students \\
    \hline
    100 & 1 \\
    80 & 2 \\
    60 & 1 \\
    \hline
    \end{tabular}
    "#;

    #[test]
    fn explicit_frequency_table_reaches_generic_weighted_mean() {
        let output = formalize_frequency_table_text(TABLE);
        assert_eq!(output.status, FrequencyTableStatus::Complete);
        assert_eq!(
            output
                .statistics
                .as_ref()
                .and_then(|result| result.value.clone()),
            Rational::new(80, 1)
        );
        assert!(output.replay_verified());
        let mut tampered = output.clone();
        tampered.replay_hash.push('x');
        assert!(!tampered.replay_verified());
    }

    #[test]
    fn ambiguous_headers_do_not_authorize() {
        let output = formalize_frequency_table_text(
            "Find the mean. \\begin{tabular}{|c|c|} Value & Score \\\\ 1 & 2 \\\\ 3 & 4 \\\\ \\end{tabular}",
        );
        assert_eq!(output.status, FrequencyTableStatus::Ambiguous);
        assert!(output.request.is_none());
        assert!(output.replay_verified());
    }
}
