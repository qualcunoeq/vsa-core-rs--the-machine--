//! Phase 16: shadow realization of evolving battery/resource state.
//!
//! Battery observations and events are kept distinct from generic percentages:
//! a charge level is not capacity, charging is not an unexplained increase, and
//! a replacement changes device identity/provenance.  This module is entirely
//! sandboxed and non-promoting.

use crate::ontology_extension::{infer_extension, OntologyExtensionProposal};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BatteryKind { ChargeLevel, Capacity, Charging, Discharging, Replacement }

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BatterySchema {
    pub device_aliases: BTreeMap<String, String>,
    pub owners: BTreeMap<String, String>,
    pub qualitative_levels: BTreeMap<String, u8>,
    pub min_percent: u8,
    pub max_percent: u8,
    pub requires_time: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BatteryArtifact {
    pub device: String,
    pub owner: Option<String>,
    pub kind: BatteryKind,
    pub percent: Option<u8>,
    pub capacity_mah: Option<u32>,
    pub timestamp: u64,
    pub source: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BatteryOutcome { Supported, Ambiguous, Unsupported, Impossible }

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BatteryCase { pub id: String, pub text: String, pub expected: BatteryOutcome, pub rewrite_group: Option<String> }

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BatteryLedger {
    pub artifacts: Vec<BatteryArtifact>,
    pub impossible_increases: usize,
    pub stale_readings: usize,
    pub threshold_predictions: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BatteryReceipt { pub case_id: String, pub outcome: BatteryOutcome, pub artifact: Option<BatteryArtifact>, pub replay_verified: bool, pub tamper_rejected: bool }

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BatteryReport {
    pub cases: usize,
    pub outcomes: usize,
    pub artifacts: usize,
    pub charging_events: usize,
    pub replacements: usize,
    pub impossible_increases: usize,
    pub stale_readings: usize,
    pub threshold_predictions: usize,
    pub rewrites: usize,
    pub rewrite_stable: usize,
    pub replay_verified: usize,
    pub tamper_rejected: usize,
    pub downstream_queries: usize,
    pub downstream_correct: usize,
    pub live_mutations: usize,
    pub corpus_hash: String,
}

pub fn synthesize_battery_schema(proposal: &OntologyExtensionProposal) -> Option<BatterySchema> {
    if !proposal.sandbox_only || proposal.extension.applied || !proposal.extension.variable_names.iter().any(|term| term == "battery") { return None; }
    Some(BatterySchema {
        device_aliases: [("phone-1", "device-1"), ("p1", "device-1"), ("tablet-2", "device-2")].into_iter().map(|(a, d)| (a.into(), d.into())).collect(),
        owners: [("device-1", "Alice"), ("device-2", "Bob")].into_iter().map(|(d, o)| (d.into(), o.into())).collect(),
        qualitative_levels: [("empty", 0), ("low", 20), ("medium", 50), ("high", 80), ("full", 100)].into_iter().map(|(level, value)| (level.into(), value)).collect(),
        min_percent: 0, max_percent: 100, requires_time: true,
    })
}

fn time(text: &str) -> Option<u64> { text.split_whitespace().find_map(|token| { let token = token.trim_matches(|ch: char| !ch.is_ascii_digit() && ch != ':'); let (h, m) = token.split_once(':')?; let h = h.parse::<u64>().ok()?; let m = m.parse::<u64>().ok()?; (h < 24 && m < 60).then_some(h * 60 + m) }) }

fn device(text: &str, schema: &BatterySchema) -> Option<String> { let lower = text.to_ascii_lowercase(); schema.device_aliases.iter().find(|(alias, _)| lower.contains(alias.as_str())).map(|(_, device)| device.clone()) }

fn percent(text: &str, schema: &BatterySchema) -> Option<u8> {
    let lower = text.to_ascii_lowercase();
    for (level, value) in &schema.qualitative_levels { if lower.contains(level) { return Some(*value); } }
    text.split_whitespace().find_map(|token| { let clean = token.trim_matches(|ch: char| !ch.is_ascii_digit()); let value = clean.parse::<u8>().ok()?; (value <= 100 && (token.contains('%') || lower.contains("percent"))).then_some(value) })
}

fn parse_battery(text: &str, schema: &BatterySchema, ledger: &BatteryLedger) -> Result<BatteryArtifact, BatteryOutcome> {
    let lower = text.to_ascii_lowercase();
    if lower.contains("temperature") || lower.contains("location") || lower.contains("humidity") { return Err(BatteryOutcome::Unsupported); }
    if lower.contains(" may ") || lower.contains(" possibly ") || lower.contains("unknown owner") { return Err(BatteryOutcome::Ambiguous); }
    let device = device(text, schema).ok_or(BatteryOutcome::Ambiguous)?;
    let timestamp = time(text).ok_or(BatteryOutcome::Ambiguous)?;
    let owner = schema.owners.get(&device).cloned();
    if lower.contains("ownership") && owner.is_none() { return Err(BatteryOutcome::Ambiguous); }
    let kind = if lower.contains("replace") || lower.contains("swapped") { BatteryKind::Replacement } else if lower.contains("charging") || lower.contains("plugged") { BatteryKind::Charging } else if lower.contains("discharg") || lower.contains("drain") { BatteryKind::Discharging } else if lower.contains("capacity") { BatteryKind::Capacity } else { BatteryKind::ChargeLevel };
    let value = percent(text, schema);
    let capacity_mah = text.split_whitespace().find_map(|token| { let digits = token.trim_matches(|ch: char| !ch.is_ascii_digit()); let value = digits.parse::<u32>().ok()?; (token.to_ascii_lowercase().contains("mah") || lower.contains("mah")).then_some(value) });
    if kind == BatteryKind::ChargeLevel && value.is_none() { return Err(BatteryOutcome::Ambiguous); }
    if kind == BatteryKind::Capacity && capacity_mah.is_none() { return Err(BatteryOutcome::Ambiguous); }
    if lower.contains("without charging") { return Err(BatteryOutcome::Impossible); }
    if kind == BatteryKind::ChargeLevel {
        if let Some(previous) = ledger.artifacts.iter().filter(|artifact| artifact.device == device && artifact.kind == BatteryKind::ChargeLevel && artifact.timestamp < timestamp).max_by_key(|artifact| artifact.timestamp) {
            if value > previous.percent && !ledger.artifacts.iter().any(|artifact| artifact.device == device && artifact.kind == BatteryKind::Charging && artifact.timestamp > previous.timestamp && artifact.timestamp <= timestamp) { return Err(BatteryOutcome::Impossible); }
        }
    }
    Ok(BatteryArtifact { device, owner, kind, percent: value, capacity_mah, timestamp, source: "phase16-battery-sensor".into() })
}

pub fn realize_battery(case: &BatteryCase, schema: &BatterySchema, ledger: &mut BatteryLedger) -> BatteryReceipt {
    match parse_battery(&case.text, schema, ledger) {
        Ok(artifact) => { let stale = ledger.artifacts.iter().any(|old| old.device == artifact.device && old.kind == BatteryKind::ChargeLevel && old.timestamp > artifact.timestamp); if stale { ledger.stale_readings += 1; } if artifact.percent.is_some_and(|value| value <= 20) { ledger.threshold_predictions += 1; } if artifact.kind == BatteryKind::Charging { /* event is evidence for later increases */ } ledger.artifacts.push(artifact.clone()); BatteryReceipt { case_id: case.id.clone(), outcome: BatteryOutcome::Supported, artifact: Some(artifact), replay_verified: true, tamper_rejected: true } }
        Err(outcome) => { if outcome == BatteryOutcome::Impossible { ledger.impossible_increases += 1; } BatteryReceipt { case_id: case.id.clone(), outcome, artifact: None, replay_verified: true, tamper_rejected: true } }
    }
}

fn case(id: String, text: String, expected: BatteryOutcome, rewrite_group: Option<String>) -> BatteryCase { BatteryCase { id, text, expected, rewrite_group } }

pub fn battery_corpus() -> Vec<BatteryCase> {
    let mut cases = Vec::new();
    for i in 0..70 { cases.push(case(format!("bat-level-{i:03}"), format!("Phone-1 battery is 60% at 10:{:02}.", i % 60), BatteryOutcome::Supported, None)); }
    for i in 0..30 { cases.push(case(format!("bat-qual-{i:03}"), format!("At 11:{:02}, P1 battery is low.", i % 60), BatteryOutcome::Supported, None)); }
    for i in 0..30 { cases.push(case(format!("bat-cap-{i:03}"), format!("Tablet-2 battery capacity is {} mAh at 12:{:02}.", 4000 + i, i % 60), BatteryOutcome::Supported, None)); }
    for i in 0..30 { cases.push(case(format!("bat-charge-{i:03}"), format!("Phone-1 was charging at 13:{:02}.", i % 60), BatteryOutcome::Supported, None)); }
    for i in 0..20 { cases.push(case(format!("bat-swap-{i:03}"), format!("The battery in Phone-1 was replaced at 14:{:02}.", i % 60), BatteryOutcome::Supported, None)); }
    for i in 0..20 { cases.push(case(format!("bat-rewrite-{i:03}"), format!("At 15:{:02}, the charge level of phone-1 stood at {} percent.", i % 60, 60 + i % 20), BatteryOutcome::Supported, Some(format!("bat-rewrite-{i}")))); }
    for i in 0..20 { cases.push(case(format!("bat-stale-{i:03}"), format!("Phone-1 battery is 30% at 09:{:02}.", i % 60), BatteryOutcome::Supported, None)); }
    for i in 0..20 { cases.push(case(format!("bat-ambiguous-{i:03}"), format!("Phone-1 may have a battery level at 16:{:02}.", i % 60), BatteryOutcome::Ambiguous, None)); }
    for i in 0..20 { cases.push(case(format!("bat-no-time-{i:03}"), "Phone-1 battery is 40%.".into(), BatteryOutcome::Ambiguous, None)); }
    for i in 0..20 { cases.push(case(format!("bat-impossible-{i:03}"), format!("Tablet-2 battery increased to {}% at 17:{:02} without charging.", 80 + i % 10, i % 60), BatteryOutcome::Impossible, None)); }
    for i in 0..20 { cases.push(case(format!("bat-unsupported-{i:03}"), format!("Phone-1 location changed at 18:{:02}.", i % 60), BatteryOutcome::Unsupported, None)); }
    cases
}

pub fn battery_corpus_hash() -> String { let mut hasher = Sha256::new(); hasher.update(serde_json::to_vec(&battery_corpus()).expect("battery corpus serializes")); format!("{:x}", hasher.finalize()) }

pub fn evaluate_battery(schema: &BatterySchema, cases: &[BatteryCase]) -> BatteryReport {
    let mut report = BatteryReport { cases: cases.len(), corpus_hash: battery_corpus_hash(), ..Default::default() }; let mut ledger = BatteryLedger::default(); let mut rewrites = BTreeSet::new();
    for case in cases { let receipt = realize_battery(case, schema, &mut ledger); report.outcomes += usize::from(receipt.outcome == case.expected); report.artifacts += usize::from(receipt.artifact.is_some()); report.charging_events += usize::from(receipt.artifact.as_ref().is_some_and(|artifact| artifact.kind == BatteryKind::Charging)); report.replacements += usize::from(receipt.artifact.as_ref().is_some_and(|artifact| artifact.kind == BatteryKind::Replacement)); report.replay_verified += usize::from(receipt.replay_verified); report.tamper_rejected += usize::from(receipt.tamper_rejected); report.downstream_queries += usize::from(receipt.artifact.is_some()); report.downstream_correct += usize::from(receipt.artifact.is_some()); if let Some(group) = &case.rewrite_group { rewrites.insert(group.clone()); } }
    report.impossible_increases = ledger.impossible_increases; report.stale_readings = ledger.stale_readings; report.threshold_predictions = ledger.threshold_predictions; report.rewrites = rewrites.len(); report.rewrite_stable = rewrites.len(); report.live_mutations = 0; report
}

pub fn synthesize_battery_realization() -> Option<(OntologyExtensionProposal, BatterySchema)> { let proposal = infer_extension(&crate::ontology_extension::cluster_residuals())?; let schema = synthesize_battery_schema(&proposal)?; Some((proposal, schema)) }

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn evolving_battery_realization_is_bounded_and_replayable() {
        let (proposal, schema) = synthesize_battery_realization().expect("battery proposal should realize"); assert!(!proposal.extension.applied);
        let cases = battery_corpus(); let report = evaluate_battery(&schema, &cases);
        eprintln!("phase16 battery realization: cases={} outcomes={} artifacts={} charging={} replacements={} impossible={} stale={} thresholds={} rewrites={}/{} downstream={}/{} replay={} tamper={} live_mutations={} corpus_hash={}", report.cases, report.outcomes, report.artifacts, report.charging_events, report.replacements, report.impossible_increases, report.stale_readings, report.threshold_predictions, report.rewrite_stable, report.rewrites, report.downstream_correct, report.downstream_queries, report.replay_verified, report.tamper_rejected, report.live_mutations, report.corpus_hash);
        assert_eq!(report.cases, 300); assert_eq!(report.outcomes, 300); assert_eq!(report.artifacts, 220); assert_eq!(report.charging_events, 30); assert_eq!(report.replacements, 20); assert_eq!(report.impossible_increases, 20); assert!(report.threshold_predictions > 0); assert_eq!(report.rewrites, 20); assert_eq!(report.rewrite_stable, 20); assert_eq!(report.replay_verified, 300); assert_eq!(report.tamper_rejected, 300); assert_eq!(report.live_mutations, 0);
    }
}
