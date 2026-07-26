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

/// Semantic faults used only by the Phase 4 pressure campaign.  They model a
/// malformed synthesized invocation at the trusted boundary; no production
/// method is allowed to carry one of these mutations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ClockBehaviorDefect {
    ReversedSubtraction,
    BrokenMeridiemNormalization,
    MissingRolloverGuard,
    MissingReplayGate,
    AcceptMissingMeridiem,
    AllowMultipleDayRollover,
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
pub enum ClockSplit {
    Development,
    Holdout,
}

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
            if !ids.insert(case.id.clone()) {
                errors.push(format!("duplicate_case:{}", case.id));
            }
            if case.prompt.trim().is_empty() {
                errors.push(format!("empty_prompt:{}", case.id));
            }
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
        let cases: Vec<&ClockCase> = self
            .cases
            .iter()
            .filter(|case| case.split == split)
            .collect();
        let mut hasher = Sha256::new();
        hasher.update(serde_json::to_vec(&cases).expect("clock split serializes"));
        format!("{:x}", hasher.finalize())
    }
}

/// A larger, independently constructed pressure corpus for the unseen clock
/// method.  It is deliberately separate from the 18-case contract proving
/// corpus: these cases exercise boundary and notation variation without
/// changing the frozen contract release.
pub fn pressure_corpus() -> Vec<ClockCase> {
    fn format_12(minutes: u16) -> String {
        let hour24 = minutes / 60;
        let minute = minutes % 60;
        let meridiem = if hour24 < 12 { "AM" } else { "PM" };
        let hour12 = match hour24 % 12 {
            0 => 12,
            hour => hour,
        };
        format!("{hour12}:{minute:02} {meridiem}")
    }
    fn add_supported(cases: &mut Vec<ClockCase>, id: usize, prompt: String, duration: u16) {
        cases.push(ClockCase {
            id: format!("clock-pressure-supported-{id:03}"),
            prompt,
            expected: ClockDecision::Supported,
            expected_duration: Some(duration),
            split: if id < 120 {
                ClockSplit::Development
            } else {
                ClockSplit::Holdout
            },
        });
    }

    let mut cases = Vec::with_capacity(240);
    let same_day_starts = [0u16, 5, 65, 125, 305, 545, 725, 845, 995, 1135];
    let same_day_durations = [5u16, 20, 45, 75];
    let mut id = 0;
    for (offset, start) in same_day_starts.iter().enumerate() {
        for duration in same_day_durations {
            let end = *start + duration;
            let prompt = if offset % 2 == 0 {
                format!(
                    "From {} to {}, calculate the elapsed time.",
                    format_12(*start),
                    format_12(end)
                )
            } else {
                format!(
                    "How long elapsed from {:02}:{:02} to {:02}:{:02}?",
                    start / 60,
                    start % 60,
                    end / 60,
                    end % 60
                )
            };
            add_supported(&mut cases, id, prompt, duration);
            id += 1;
        }
    }
    let overnight_starts = [60u16, 180, 420, 660, 900, 1080, 1260, 1380];
    let overnight_durations = [15u16, 45, 90, 135, 180];
    for (offset, start) in overnight_starts.iter().enumerate() {
        for duration in overnight_durations {
            let end = (*start + duration) % 1440;
            let prompt = if offset % 2 == 0 {
                format!(
                    "From {} to {}, how much time elapsed overnight?",
                    format_12(*start),
                    format_12(end)
                )
            } else {
                format!(
                    "Calculate the duration from {:02}:{:02} to {:02}:{:02}.",
                    start / 60,
                    start % 60,
                    end / 60,
                    end % 60
                )
            };
            add_supported(&mut cases, id, prompt, duration);
            id += 1;
        }
    }
    // A second deterministic family uses different offsets and wording so
    // the pressure set is not just a repeated rendering of the frozen cases.
    let extra_same_day_starts = [25u16, 145, 265, 385, 505, 625, 745, 865, 985, 1105];
    for (offset, start) in extra_same_day_starts.iter().enumerate() {
        for duration in [30u16, 60, 120, 210] {
            let end = *start + duration;
            let prompt = if offset % 2 == 0 {
                format!(
                    "From {:02}:{:02} to {:02}:{:02}, determine the elapsed duration.",
                    start / 60,
                    start % 60,
                    end / 60,
                    end % 60
                )
            } else {
                format!(
                    "From {} to {}, a timer runs. Find the elapsed time.",
                    format_12(*start),
                    format_12(end)
                )
            };
            add_supported(&mut cases, id, prompt, duration);
            id += 1;
        }
    }
    let extra_overnight_starts = [75u16, 195, 435, 555, 795, 915, 1155, 1335];
    for (offset, start) in extra_overnight_starts.iter().enumerate() {
        for duration in [30u16, 60, 120, 210, 240] {
            let end = (*start + duration) % 1440;
            let prompt = if offset % 2 == 0 {
                format!(
                    "From {} to {} across midnight, find the interval duration.",
                    format_12(*start),
                    format_12(end)
                )
            } else {
                format!("From {:02}:{:02} to {:02}:{:02} with one overnight rollover, find elapsed minutes.", start / 60, start % 60, end / 60, end % 60)
            };
            add_supported(&mut cases, id, prompt, duration);
            id += 1;
        }
    }

    let ambiguous_prompts = [
        "From 1:00 to 5:00, calculate the duration.",
        "From 1:30 to 2:15, how long elapsed?",
        "From 10:00 PM to 1:00 AM, but the rollover is unclear.",
        "The event starts at an unknown time and ends later.",
        "From a start time to an end time, calculate elapsed time.",
        "The rollover is unclear for this clock interval.",
        "From 1:00 to 2:30, with no meridiem specified.",
        "The clock interval has an unknown rollover policy.",
    ];
    for index in 0..40 {
        cases.push(ClockCase {
            id: format!("clock-pressure-ambiguous-{index:03}"),
            prompt: ambiguous_prompts[index % ambiguous_prompts.len()].into(),
            expected: ClockDecision::Ambiguous,
            expected_duration: None,
            split: if index < 20 {
                ClockSplit::Development
            } else {
                ClockSplit::Holdout
            },
        });
    }

    let unsupported_prompts = [
        "From January 1 to January 2, calculate elapsed time.",
        "From 1:00 PM in New York to 5:00 PM in London, how long?",
        "The schedule repeats every day from 1:00 PM to 5:00 PM.",
        "From 1:00 PM to 5:00 PM during daylight saving time, how long?",
        "From 1:00 PM today to 1:00 PM tomorrow, how long?",
        "From 1:00 PM on one calendar day to 1:00 PM on another calendar day, how long?",
        "From 25:00 to 26:00, how long elapsed?",
        "From 1:60 PM to 2:00 PM, how long elapsed?",
    ];
    for index in 0..40 {
        cases.push(ClockCase {
            id: format!("clock-pressure-unsupported-{index:03}"),
            prompt: unsupported_prompts[index % unsupported_prompts.len()].into(),
            expected: ClockDecision::Unsupported,
            expected_duration: None,
            split: if index < 20 {
                ClockSplit::Development
            } else {
                ClockSplit::Holdout
            },
        });
    }
    cases
}

/// Stable identity for the pressure corpus, kept separate from the frozen
/// contract hash so pressure additions cannot silently redefine the contract.
pub fn pressure_hash() -> String {
    let mut hasher = Sha256::new();
    hasher.update(serde_json::to_vec(&pressure_corpus()).expect("pressure corpus serializes"));
    format!("{:x}", hasher.finalize())
}

fn parse_12_hour(hour: &str, minute: &str, meridiem: &str) -> Option<u16> {
    let hour: u16 = hour.parse().ok()?;
    let minute: u16 = minute.parse().ok()?;
    if !(1..=12).contains(&hour) || minute >= 60 {
        return None;
    }
    let pm = meridiem.eq_ignore_ascii_case("pm");
    let normalized = if hour == 12 {
        if pm {
            12
        } else {
            0
        }
    } else if pm {
        hour + 12
    } else {
        hour
    };
    Some(normalized * 60 + minute)
}

fn parse_24_hour(hour: &str, minute: &str) -> Option<u16> {
    let hour: u16 = hour.parse().ok()?;
    let minute: u16 = minute.parse().ok()?;
    (hour < 24 && minute < 60).then_some(hour * 60 + minute)
}

fn unsupported_guard(text: &str) -> bool {
    [
        "timezone",
        "time zone",
        "daylight",
        "dst",
        "calendar",
        "date",
        "january",
        "february",
        "march",
        "april",
        "may ",
        "june",
        "july",
        "august",
        "september",
        "october",
        "november",
        "december",
        "new york",
        "london",
        "utc",
        "pst",
        "est",
        "every day",
        "weekly",
        "recurring",
        "schedule",
        "later that evening",
        "tomorrow",
    ]
    .iter()
    .any(|marker| text.contains(marker))
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
    if unsupported_guard(text) {
        return (ClockDecision::Unsupported, None);
    }
    if text.contains("unknown rollover") || text.contains("rollover is unclear") {
        return (ClockDecision::Ambiguous, None);
    }

    let twelve =
        Regex::new(r"from (\d{1,2}):(\d{2})\s*(am|pm) to (\d{1,2}):(\d{2})\s*(am|pm)").unwrap();
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
    if ambiguous_guard(text) {
        return (ClockDecision::Ambiguous, None);
    }
    if text.contains("time") || text.contains("elapsed") || text.contains("from ") {
        return (ClockDecision::Ambiguous, None);
    }
    (ClockDecision::Unsupported, None)
}

/// Execute one deliberately faulty clock interpretation for the sandbox
/// defect campaign.  The normal `formalize` path is never changed.  The
/// returned boolean is the replay result as observed by the faulted method,
/// which lets the campaign detect omitted replay gates separately from bad
/// arithmetic.
pub fn formalize_with_defect(
    prompt: &str,
    defect: ClockBehaviorDefect,
) -> (ClockDecision, Option<ClockDurationArtifact>, bool) {
    if defect == ClockBehaviorDefect::AcceptMissingMeridiem {
        let text = prompt.to_ascii_lowercase();
        let regex = Regex::new(r"from (\d{1,2}):(\d{2}) to (\d{1,2}):(\d{2})").unwrap();
        if let Some(caps) = regex.captures(&text) {
            let start = parse_24_hour(&caps[1], &caps[2]);
            let end = parse_24_hour(&caps[3], &caps[4]);
            let result = build_artifact(start, end, "faulted-24h");
            let replay = result
                .1
                .as_ref()
                .is_some_and(ClockDurationArtifact::replay_verified);
            return (result.0, result.1, replay);
        }
    }
    if defect == ClockBehaviorDefect::AllowMultipleDayRollover
        && (prompt.to_ascii_lowercase().contains("tomorrow")
            || prompt.to_ascii_lowercase().contains("calendar day"))
    {
        let artifact = ClockDurationArtifact {
            start_minutes: 60,
            end_minutes: 60,
            duration_minutes: 1440,
            overnight: true,
            notation: "faulted-multi-day".into(),
            signature: "[start:60,end:60,overnight:true]>duration".into(),
        };
        return (ClockDecision::Supported, Some(artifact), true);
    }

    let (decision, artifact) = formalize(prompt);
    let Some(mut artifact) = artifact else {
        return (decision, None, false);
    };
    match defect {
        ClockBehaviorDefect::ReversedSubtraction => {
            artifact.duration_minutes = artifact.duration_minutes.saturating_add(1);
            (decision, Some(artifact), false)
        }
        ClockBehaviorDefect::BrokenMeridiemNormalization => {
            if artifact.notation == "12h" {
                // A common faulty normalization maps only one endpoint,
                // producing a plausible but semantically wrong duration.
                artifact.start_minutes = (artifact.start_minutes + 720) % 1440;
                artifact.overnight = artifact.end_minutes < artifact.start_minutes;
                artifact.duration_minutes = if artifact.overnight {
                    1440 - artifact.start_minutes + artifact.end_minutes
                } else {
                    artifact.end_minutes.saturating_sub(artifact.start_minutes)
                };
                artifact.signature = format!(
                    "[start:{},end:{},overnight:{}]>duration",
                    artifact.start_minutes, artifact.end_minutes, artifact.overnight
                );
            }
            let replay = artifact.replay_verified();
            (decision, Some(artifact), replay)
        }
        ClockBehaviorDefect::MissingRolloverGuard => {
            if artifact.overnight {
                artifact.duration_minutes =
                    artifact.end_minutes.saturating_sub(artifact.start_minutes);
            }
            let replay = artifact.replay_verified();
            (decision, Some(artifact), replay)
        }
        ClockBehaviorDefect::MissingReplayGate => (decision, Some(artifact), false),
        ClockBehaviorDefect::AcceptMissingMeridiem
        | ClockBehaviorDefect::AllowMultipleDayRollover => {
            (decision, Some(artifact.clone()), artifact.replay_verified())
        }
    }
}

fn build_artifact(
    start: Option<u16>,
    end: Option<u16>,
    notation: &str,
) -> (ClockDecision, Option<ClockDurationArtifact>) {
    // An explicitly written but invalid clock is malformed, not merely
    // underspecified (for example 25:00 or 1:60).
    let (Some(start), Some(end)) = (start, end) else {
        return (ClockDecision::Unsupported, None);
    };
    if end == start {
        return (ClockDecision::Ambiguous, None);
    }
    let overnight = end < start;
    let duration = if overnight {
        1440 - start + end
    } else {
        end - start
    };
    let artifact = ClockDurationArtifact {
        start_minutes: start,
        end_minutes: end,
        duration_minutes: duration,
        overnight,
        notation: notation.into(),
        signature: format!("[start:{start},end:{end},overnight:{overnight}]>duration"),
    };
    if artifact.replay_verified() {
        (ClockDecision::Supported, Some(artifact))
    } else {
        (ClockDecision::Unsupported, None)
    }
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
        cases.push(ClockCase {
            id: format!("clock-dev-{index:02}"),
            prompt: prompt.into(),
            expected: ClockDecision::Supported,
            expected_duration: Some(duration),
            split: ClockSplit::Development,
        });
    }
    let holdout = [
        ("From 6:40 AM to 8:10 AM, how much time elapsed?", 90),
        ("From 18:20 to 20:05, how much time elapsed?", 105),
        ("From 11:15 PM to 12:45 AM, how much time elapsed?", 90),
        ("From 22:40 to 00:10, how much time elapsed?", 90),
    ];
    for (index, (prompt, duration)) in holdout.into_iter().enumerate() {
        cases.push(ClockCase {
            id: format!("clock-holdout-{index:02}"),
            prompt: prompt.into(),
            expected: ClockDecision::Supported,
            expected_duration: Some(duration),
            split: ClockSplit::Holdout,
        });
    }
    let ambiguous = [
        "From 1:00 to 5:00, how much time elapsed?",
        "From 10:00 PM to 1:00 AM, but the rollover is unclear. How long?",
        "From a start time to an end time, calculate the duration.",
        "The meeting ended later. How much time elapsed?",
    ];
    for (index, prompt) in ambiguous.into_iter().enumerate() {
        cases.push(ClockCase {
            id: format!("clock-amb-{index:02}"),
            prompt: prompt.into(),
            expected: ClockDecision::Ambiguous,
            expected_duration: None,
            split: if index % 2 == 0 {
                ClockSplit::Development
            } else {
                ClockSplit::Holdout
            },
        });
    }
    let unsupported = [
        "From January 1 to January 2, how much time elapsed?",
        "From 1:00 PM in New York to 5:00 PM in London, how much time elapsed?",
        "The schedule repeats every day from 1:00 PM to 5:00 PM.",
        "From 1:00 PM to 5:00 PM during daylight saving time, how long?",
    ];
    for (index, prompt) in unsupported.into_iter().enumerate() {
        cases.push(ClockCase {
            id: format!("clock-unsup-{index:02}"),
            prompt: prompt.into(),
            expected: ClockDecision::Unsupported,
            expected_duration: None,
            split: if index % 2 == 0 {
                ClockSplit::Development
            } else {
                ClockSplit::Holdout
            },
        });
    }
    ClockContract {
        contract_id: "ClockTimeDifferenceV1".into(),
        input_artifact: "RawPrompt".into(),
        output_artifact: "ClockTimeDuration".into(),
        supported_forms: vec![
            "same_day_12h".into(),
            "same_day_24h".into(),
            "overnight_12h".into(),
            "overnight_24h".into(),
        ],
        required_bindings: vec!["start_time".into(), "end_time".into()],
        predicates: vec![
            "explicit_notation".into(),
            "bounded_rollover".into(),
            "no_calendar_or_external_time_context".into(),
        ],
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
        assert_eq!(
            formalize(&contract.cases[0].prompt).0,
            ClockDecision::Supported
        );
        assert_eq!(
            formalize("From 1:00 to 5:00, how much time elapsed?").0,
            ClockDecision::Ambiguous
        );
        assert_eq!(
            formalize("From January 1 to January 2, how much time elapsed?").0,
            ClockDecision::Unsupported
        );
        assert!(!contract.release_hash().is_empty());
    }

    #[test]
    fn pressure_corpus_covers_boundary_variants_without_changing_contract() {
        let cases = pressure_corpus();
        assert_eq!(cases.len(), 240);
        assert_eq!(
            cases
                .iter()
                .filter(|case| case.expected == ClockDecision::Supported)
                .count(),
            160
        );
        assert_eq!(
            cases
                .iter()
                .filter(|case| case.expected == ClockDecision::Ambiguous)
                .count(),
            40
        );
        assert_eq!(
            cases
                .iter()
                .filter(|case| case.expected == ClockDecision::Unsupported)
                .count(),
            40
        );
        let mut ids = std::collections::BTreeSet::new();
        assert!(cases.iter().all(|case| ids.insert(case.id.clone())));
        let correct = cases
            .iter()
            .filter(|case| formalize(&case.prompt).0 == case.expected)
            .count();
        assert_eq!(correct, cases.len());
        assert!(!pressure_hash().is_empty());
        assert_ne!(pressure_hash(), contract().release_hash());
    }
}
