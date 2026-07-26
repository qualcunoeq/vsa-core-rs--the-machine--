//! Frozen, narrowly scoped ClockTimeDifferenceV1 contract and formalizer.
//!
//! This module is the unseen-contract proving ground for Phase 4.  It accepts
//! explicit same-day elapsed time and one bounded overnight rollover.  It does
//! not understand dates, time zones, DST, schedules, or vague time references.

use regex::Regex;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ClockDecision {
    Supported,
    Ambiguous,
    Unsupported,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClockDurationArtifact {
    pub start_minutes: u16,
    pub end_minutes: u16,
    pub duration_minutes: u16,
    pub overnight: bool,
    pub notation: String,
    pub signature: String,
}

impl ClockDurationArtifact {
    pub fn replay_verified(&self) -> bool {
        if self.start_minutes >= 1440 || self.end_minutes >= 1440 || self.duration_minutes == 0 {
            return false;
        }
        let expected = if self.overnight {
            1440u16 - self.start_minutes + self.end_minutes
        } else {
            self.end_minutes.saturating_sub(self.start_minutes)
        };
        expected == self.duration_minutes
            && (self.overnight || self.end_minutes > self.start_minutes)
            && !self.notation.is_empty()
            && !self.signature.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClockCase {
    pub id: String,
    pub prompt: String,
    pub expected: ClockDecision,
    pub expected_duration: Option<u16>,
    pub split: ClockSplit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ClockSplit { Development, Holdout }

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClockContract {
    pub contract_id: String,
    pub input_artifact: String,
    pub output_artifact: String,
    pub supported_forms: Vec<String>,
    pub required_bindings: Vec<String>,
    pub predicates: Vec<String>,
    pub cases: Vec<ClockCase>,
}

impl ClockContract {
    pub fn validation_errors(&self) -> Vec<String> {
        let mut errors = Vec::new();
        if self.contract_id != "ClockTimeDifferenceV1" {
            errors.push("unexpected_contract_id".into());
        }
        if self.input_artifact != "RawPrompt" || self.output_artifact != "ClockTimeDuration" {
            errors.push("incorrect_artifact_contract".into());
        }
        if self.supported_forms.len() != 4 || self.required_bindings != ["start_time", "end_time"] {
            errors.push("incomplete_supported_contract".into());
        }
        let mut ids = std::collections::BTreeSet::new();
        for case in &self.cases {
            if !ids.insert(case.id.clone()) { errors.push(format!("duplicate_case:{}", case.id)); }
            if case.prompt.trim().is_empty() { errors.push(format!("empty_prompt:{}", case.id)); }
            if case.expected == ClockDecision::Supported && case.expected_duration.is_none() {
                errors.push(format!("supported_missing_duration:{}", case.id));
            }
            if case.expected != ClockDecision::Supported && case.expected_duration.is_some() {
                errors.push(format!("negative_has_duration:{}", case.id));
            }
        }
        errors
    }

    pub fn release_hash(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(serde_json::to_vec(self).expect("clock contract serializes"));
        format!("{:x}", hasher.finalize())
    }

    pub fn split_hash(&self, split: ClockSplit) -> String {
        let cases: Vec<&ClockCase> = self.cases.iter().filter(|case| case.split == split).collect();
        let mut hasher = Sha256::new();
        hasher.update(serde_json::to_vec(&cases).expect("clock split serializes"));
        format!("{:x}", hasher.finalize())
    }
}

fn parse_12_hour(hour: &str, minute: &str, meridiem: &str) -> Option<u16> {
    let hour: u16 = hour.parse().ok()?;
    let minute: u16 = minute.parse().ok()?;
    if !(1..=12).contains(&hour) || minute >= 60 { return None; }
    let pm = meridiem.eq_ignore_ascii_case("pm");
    let normalized = if hour == 12 { if pm { 12 } else { 0 } } else if pm { hour + 12 } else { hour };
    Some(normalized * 60 + minute)
}

fn parse_24_hour(hour: &str, minute: &str) -> Option<u16> {
    let hour: u16 = hour.parse().ok()?;
    let minute: u16 = minute.parse().ok()?;
    (hour < 24 && minute < 60).then_some(hour * 60 + minute)
}

fn unsupported_guard(text: &str) -> bool {
    ["timezone", "time zone", "daylight", "dst", "calendar", "date", "january", "february", "march", "april", "may ", "june", "july", "august", "september", "october", "november", "december", "new york", "london", "utc", "pst", "est", "every day", "weekly", "recurring", "schedule", "later that evening", "tomorrow"]
        .iter().any(|marker| text.contains(marker))
}

fn ambiguous_guard(text: &str) -> bool {
    text.contains("unknown rollover")
        || text.contains("rollover is unclear")
        || Regex::new(r"from \d{1,2}:\d{2} to \d{1,2}:\d{2}")
            .is_ok_and(|regex| regex.is_match(text))
}

/// Formalize only explicit time pairs with a duration request.
pub fn formalize(prompt: &str) -> (ClockDecision, Option<ClockDurationArtifact>) {
    let text = prompt.to_ascii_lowercase().replace(['\n', '\r'], " ");
    let text = text.trim();
    if unsupported_guard(text) { return (ClockDecision::Unsupported, None); }
    if text.contains("unknown rollover") || text.contains("rollover is unclear") {
        return (ClockDecision::Ambiguous, None);
    }

    let twelve = Regex::new(r"from (\d{1,2}):(\d{2})\s*(am|pm) to (\d{1,2}):(\d{2})\s*(am|pm)").unwrap();
    if let Some(caps) = twelve.captures(text) {
        let start = parse_12_hour(&caps[1], &caps[2], &caps[3]);
        let end = parse_12_hour(&caps[4], &caps[5], &caps[6]);
        return build_artifact(start, end, "12h");
    }
    let twenty_four = Regex::new(r"from (\d{2}):(\d{2}) to (\d{2}):(\d{2})").unwrap();
    if let Some(caps) = twenty_four.captures(text) {
        let start = parse_24_hour(&caps[1], &caps[2]);
        let end = parse_24_hour(&caps[3], &caps[4]);
        return build_artifact(start, end, "24h");
    }
    if ambiguous_guard(text) { return (ClockDecision::Ambiguous, None); }
    if text.contains("time") || text.contains("elapsed") || text.contains("from ") {
        return (ClockDecision::Ambiguous, None);
    }
    (ClockDecision::Unsupported, None)
}

fn build_artifact(start: Option<u16>, end: Option<u16>, notation: &str) -> (ClockDecision, Option<ClockDurationArtifact>) {
    let (Some(start), Some(end)) = (start, end) else { return (ClockDecision::Ambiguous, None); };
    if end == start { return (ClockDecision::Ambiguous, None); }
    let overnight = end < start;
    let duration = if overnight { 1440 - start + end } else { end - start };
    let artifact = ClockDurationArtifact {
        start_minutes: start,
        end_minutes: end,
        duration_minutes: duration,
        overnight,
        notation: notation.into(),
        signature: format!("[start:{start},end:{end},overnight:{overnight}]>duration"),
    };
    if artifact.replay_verified() { (ClockDecision::Supported, Some(artifact)) } else { (ClockDecision::Unsupported, None) }
}

pub fn contract() -> ClockContract {
    let mut cases = Vec::new();
    let supported = [
        ("From 1:00 PM to 5:00 PM, how much time elapsed?", 240),
        ("From 09:15 to 11:45, how much time elapsed?", 150),
        ("From 10:00 PM to 1:00 AM, how much time elapsed?", 180),
        ("From 23:30 to 01:15, how much time elapsed?", 105),
        ("From 8:05 AM to 9:20 AM, how much time elapsed?", 75),
        ("From 14:10 to 16:00, how much time elapsed?", 110),
    ];
    for (index, (prompt, duration)) in supported.into_iter().enumerate() {
        cases.push(ClockCase { id: format!("clock-dev-{index:02}"), prompt: prompt.into(), expected: ClockDecision::Supported, expected_duration: Some(duration), split: ClockSplit::Development });
    }
    let holdout = [
        ("From 6:40 AM to 8:10 AM, how much time elapsed?", 90),
        ("From 18:20 to 20:05, how much time elapsed?", 105),
        ("From 11:15 PM to 12:45 AM, how much time elapsed?", 90),
        ("From 22:40 to 00:10, how much time elapsed?", 90),
    ];
    for (index, (prompt, duration)) in holdout.into_iter().enumerate() {
        cases.push(ClockCase { id: format!("clock-holdout-{index:02}"), prompt: prompt.into(), expected: ClockDecision::Supported, expected_duration: Some(duration), split: ClockSplit::Holdout });
    }
    let ambiguous = [
        "From 1:00 to 5:00, how much time elapsed?",
        "From 10:00 PM to 1:00 AM, but the rollover is unclear. How long?",
        "From a start time to an end time, calculate the duration.",
        "The meeting ended later. How much time elapsed?",
    ];
    for (index, prompt) in ambiguous.into_iter().enumerate() {
        cases.push(ClockCase { id: format!("clock-amb-{index:02}"), prompt: prompt.into(), expected: ClockDecision::Ambiguous, expected_duration: None, split: if index % 2 == 0 { ClockSplit::Development } else { ClockSplit::Holdout } });
    }
    let unsupported = [
        "From January 1 to January 2, how much time elapsed?",
        "From 1:00 PM in New York to 5:00 PM in London, how much time elapsed?",
        "The schedule repeats every day from 1:00 PM to 5:00 PM.",
        "From 1:00 PM to 5:00 PM during daylight saving time, how long?",
    ];
    for (index, prompt) in unsupported.into_iter().enumerate() {
        cases.push(ClockCase { id: format!("clock-unsup-{index:02}"), prompt: prompt.into(), expected: ClockDecision::Unsupported, expected_duration: None, split: if index % 2 == 0 { ClockSplit::Development } else { ClockSplit::Holdout } });
    }
    ClockContract {
        contract_id: "ClockTimeDifferenceV1".into(),
        input_artifact: "RawPrompt".into(),
        output_artifact: "ClockTimeDuration".into(),
        supported_forms: vec!["same_day_12h".into(), "same_day_24h".into(), "overnight_12h".into(), "overnight_24h".into()],
        required_bindings: vec!["start_time".into(), "end_time".into()],
        predicates: vec!["explicit_notation".into(), "bounded_rollover".into(), "no_calendar_or_external_time_context".into()],
        cases,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contract_is_frozen_and_formalizer_is_fail_closed() {
        let contract = contract();
        assert!(contract.validation_errors().is_empty());
        assert_eq!(contract.cases.len(), 18);
        assert_eq!(formalize(&contract.cases[0].prompt).0, ClockDecision::Supported);
        assert_eq!(formalize("From 1:00 to 5:00, how much time elapsed?").0, ClockDecision::Ambiguous);
        assert_eq!(formalize("From January 1 to January 2, how much time elapsed?").0, ClockDecision::Unsupported);
        assert!(!contract.release_hash().is_empty());
    }
}
