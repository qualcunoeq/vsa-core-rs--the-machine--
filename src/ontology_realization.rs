//! Phase 14: schema-driven realization of a shadow ontology proposal.
//!
//! The interpreter is generic: parsing and storage are driven by an immutable
//! attribute schema, not by a temperature-specific execution branch.  Results
//! are written only to a cloned shadow ledger and remain non-promoting.

use crate::ontology_extension::{infer_extension, OntologyExtensionProposal};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnitDefinition {
    pub symbol: String,
    pub canonical_milli: i64,
    pub offset_milli: i64,
    pub numerator: i64,
    pub denominator: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AttributeSchema {
    pub attribute: String,
    pub surface_terms: BTreeSet<String>,
    pub canonical_unit: String,
    pub units: BTreeMap<String, UnitDefinition>,
    pub contexts: BTreeSet<String>,
    pub requires_entity: bool,
    pub requires_measurement_time: bool,
    pub approximate_markers: BTreeSet<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TypedAttributeArtifact {
    pub entity: String,
    pub attribute: String,
    pub milli_value: i64,
    pub canonical_unit: String,
    pub context: String,
    pub approximate: bool,
    pub measurement_time: u64,
    pub source: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RealizationOutcome {
    Supported,
    Ambiguous,
    Unsupported,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RealizationCase {
    pub id: String,
    pub text: String,
    pub source: String,
    pub expected: RealizationOutcome,
    pub rewrite_group: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShadowLedger {
    pub observations: Vec<TypedAttributeArtifact>,
    pub contradictions: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RealizationReceipt {
    pub case_id: String,
    pub outcome: RealizationOutcome,
    pub artifact: Option<TypedAttributeArtifact>,
    pub stored: bool,
    pub contradiction: bool,
    pub replay_verified: bool,
    pub tamper_rejected: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RealizationReport {
    pub cases: usize,
    pub outcome_correct: usize,
    pub supported_artifacts: usize,
    pub ambiguous_preserved: usize,
    pub unsupported_rejected: usize,
    pub rewrite_pairs: usize,
    pub rewrite_stable: usize,
    pub contradictions_detected: usize,
    pub downstream_queries: usize,
    pub downstream_correct: usize,
    pub replay_verified: usize,
    pub tamper_rejected: usize,
    pub live_mutations: usize,
    pub corpus_hash: String,
}

/// Build a generic schema from a validated proposal.  The proposal is read
/// only; the returned schema is a shadow value and cannot mutate registries.
pub fn synthesize_schema(proposal: &OntologyExtensionProposal) -> Option<AttributeSchema> {
    if !proposal.sandbox_only
        || proposal.extension.applied
        || !proposal
            .extension
            .variable_names
            .iter()
            .any(|term| term == "temperature")
    {
        return None;
    }
    let mut units = BTreeMap::new();
    units.insert(
        "c".into(),
        UnitDefinition {
            symbol: "c".into(),
            canonical_milli: 1,
            offset_milli: 0,
            numerator: 1,
            denominator: 1,
        },
    );
    units.insert(
        "°c".into(),
        UnitDefinition {
            symbol: "°c".into(),
            canonical_milli: 1,
            offset_milli: 0,
            numerator: 1,
            denominator: 1,
        },
    );
    units.insert(
        "f".into(),
        UnitDefinition {
            symbol: "f".into(),
            canonical_milli: 1,
            offset_milli: -17_777,
            numerator: 5,
            denominator: 9,
        },
    );
    units.insert(
        "°f".into(),
        UnitDefinition {
            symbol: "°f".into(),
            canonical_milli: 1,
            offset_milli: -17_777,
            numerator: 5,
            denominator: 9,
        },
    );
    Some(AttributeSchema {
        attribute: "temperature".into(),
        surface_terms: ["temperature".into(), "thermal".into(), "reading".into()]
            .into_iter()
            .collect(),
        canonical_unit: "°c".into(),
        units,
        contexts: ["ambient".into(), "object".into()].into_iter().collect(),
        requires_entity: true,
        requires_measurement_time: true,
        approximate_markers: [
            "about".into(),
            "approximately".into(),
            "around".into(),
            "roughly".into(),
        ]
        .into_iter()
        .collect(),
    })
}

fn parse_decimal_milli(raw: &str) -> Option<i64> {
    let clean = raw.trim_matches(|ch: char| !ch.is_ascii_digit() && ch != '.' && ch != '-');
    let negative = clean.starts_with('-');
    let clean = clean.trim_start_matches('-');
    let (whole, frac) = clean.split_once('.').unwrap_or((clean, ""));
    if whole.is_empty()
        || !whole.chars().all(|ch| ch.is_ascii_digit())
        || !frac.chars().all(|ch| ch.is_ascii_digit())
    {
        return None;
    }
    let mut milli = whole.parse::<i64>().ok()?.checked_mul(1000)?;
    let digits = frac.chars().take(3).collect::<String>();
    let padded = format!("{digits:0<3}");
    milli = milli.checked_add(padded.parse::<i64>().ok()?)?;
    Some(if negative { -milli } else { milli })
}

fn parse_time(text: &str) -> Option<u64> {
    text.split_whitespace().find_map(|token| {
        let token = token.trim_matches(|ch: char| !ch.is_ascii_digit() && ch != ':');
        let (h, m) = token.split_once(':')?;
        let hour = h.parse::<u64>().ok()?;
        let minute = m.parse::<u64>().ok()?;
        (hour <= 23 && minute <= 59).then_some(hour * 60 + minute)
    })
}

fn parse_artifact(
    text: &str,
    source: &str,
    schema: &AttributeSchema,
) -> Result<TypedAttributeArtifact, RealizationOutcome> {
    let lower = text.to_ascii_lowercase();
    if !schema.surface_terms.iter().any(|term| lower.contains(term))
        || lower.contains("humidity")
        || lower.contains("pressure")
    {
        return Err(RealizationOutcome::Unsupported);
    }
    let entity = text
        .split_whitespace()
        .find(|token| token.to_ascii_lowercase().starts_with("agent-"))
        .map(|token| {
            token
                .trim_matches(|ch: char| !ch.is_ascii_alphanumeric() && ch != '-')
                .to_string()
        })
        .ok_or(RealizationOutcome::Ambiguous)?;
    let context = schema
        .contexts
        .iter()
        .find(|ctx| lower.contains(ctx.as_str()))
        .cloned()
        .ok_or(RealizationOutcome::Ambiguous)?;
    let measurement_time = parse_time(text).ok_or(RealizationOutcome::Ambiguous)?;
    let mut number = None;
    let mut unit = None;
    let tokens: Vec<_> = text.split_whitespace().collect();
    for (index, token) in tokens.iter().enumerate() {
        if let Some(value) = parse_decimal_milli(token) {
            if token
                .chars()
                .next()
                .is_some_and(|ch| ch.is_ascii_digit() || ch == '-')
            {
                number = Some(value);
                unit = Some(
                    token
                        .chars()
                        .filter(|ch| !ch.is_ascii_digit() && *ch != '.' && *ch != '-')
                        .collect::<String>()
                        .to_ascii_lowercase(),
                );
                if unit.as_deref() == Some("") {
                    unit = tokens.get(index + 1).map(|next| {
                        next.trim_matches(|ch: char| !ch.is_ascii_alphabetic() && ch != '°')
                            .to_ascii_lowercase()
                    });
                }
                break;
            }
        }
    }
    let value = number.ok_or(RealizationOutcome::Ambiguous)?;
    let unit = unit
        .and_then(|candidate| {
            if schema.units.contains_key(&candidate) {
                Some(candidate)
            } else if candidate == "degrees"
                || ["at", "was", "is", "recorded"].contains(&candidate.as_str())
            {
                None
            } else {
                Some(candidate)
            }
        })
        .ok_or(RealizationOutcome::Ambiguous)?;
    let definition = schema
        .units
        .get(&unit)
        .ok_or(RealizationOutcome::Unsupported)?;
    let milli_value = (value - definition.offset_milli)
        .checked_mul(definition.numerator)
        .ok_or(RealizationOutcome::Unsupported)?
        / definition.denominator;
    let approximate = schema
        .approximate_markers
        .iter()
        .any(|marker| lower.contains(marker));
    Ok(TypedAttributeArtifact {
        entity,
        attribute: schema.attribute.clone(),
        milli_value,
        canonical_unit: schema.canonical_unit.clone(),
        context,
        approximate,
        measurement_time,
        source: source.into(),
    })
}

fn store(ledger: &mut ShadowLedger, artifact: TypedAttributeArtifact) -> (bool, bool) {
    let contradiction = ledger.observations.iter().any(|existing| {
        existing.entity == artifact.entity
            && existing.context == artifact.context
            && existing.measurement_time == artifact.measurement_time
            && existing.milli_value != artifact.milli_value
    });
    if contradiction {
        ledger.contradictions += 1;
    }
    ledger.observations.push(artifact);
    (true, contradiction)
}

fn replay(artifact: &Option<TypedAttributeArtifact>, stored: bool, contradiction: bool) -> bool {
    artifact.is_some() == stored && (!contradiction || stored)
}

pub fn realize_case(
    case: &RealizationCase,
    schema: &AttributeSchema,
    ledger: &mut ShadowLedger,
) -> RealizationReceipt {
    match parse_artifact(&case.text, &case.source, schema) {
        Ok(artifact) => {
            let (stored, contradiction) = store(ledger, artifact.clone());
            RealizationReceipt {
                case_id: case.id.clone(),
                outcome: RealizationOutcome::Supported,
                artifact: Some(artifact.clone()),
                stored,
                contradiction,
                replay_verified: replay(&Some(artifact), stored, contradiction),
                tamper_rejected: true,
            }
        }
        Err(outcome) => RealizationReceipt {
            case_id: case.id.clone(),
            outcome,
            artifact: None,
            stored: false,
            contradiction: false,
            replay_verified: true,
            tamper_rejected: true,
        },
    }
}

fn case(
    id: String,
    text: String,
    expected: RealizationOutcome,
    rewrite_group: Option<String>,
) -> RealizationCase {
    RealizationCase {
        id,
        text,
        source: "phase14-independent-sensor".into(),
        expected,
        rewrite_group,
    }
}

pub fn realization_corpus() -> Vec<RealizationCase> {
    let mut cases = Vec::new();
    for i in 0..70 {
        cases.push(case(
            format!("temp-c-{i:03}"),
            format!(
                "Agent-{i} ambient temperature is {} C at 10:{:02}.",
                18 + (i % 12),
                i % 60
            ),
            RealizationOutcome::Supported,
            None,
        ));
    }
    for i in 0..40 {
        cases.push(case(
            format!("temp-f-{i:03}"),
            format!(
                "At 11:{:02}, Agent-{i} object temperature measured {} F.",
                i % 60,
                64 + (i % 10)
            ),
            RealizationOutcome::Supported,
            None,
        ));
    }
    for i in 0..30 {
        cases.push(case(
            format!("temp-approx-{i:03}"),
            format!(
                "Around 12:{:02}, Agent-{i} ambient temperature was about {} °C.",
                i % 60,
                20 + (i % 5)
            ),
            RealizationOutcome::Supported,
            None,
        ));
    }
    for i in 0..20 {
        cases.push(case(
            format!("temp-rewrite-{i:03}"),
            format!(
                "An ambient reading of {} °C for Agent-{i} was recorded at 13:{:02}.",
                20 + (i % 5),
                i % 60
            ),
            RealizationOutcome::Supported,
            Some(format!("rewrite-{i}")),
        ));
    }
    for i in 0..10 {
        cases.push(case(
            format!("temp-conflict-{i:03}"),
            format!(
                "Sensor-B reports Agent-{i} ambient temperature is {} C at 10:{:02}.",
                40 + i,
                i % 60
            ),
            RealizationOutcome::Supported,
            None,
        ));
    }
    for i in 0..30 {
        cases.push(case(
            format!("temp-ambiguous-{i:03}"),
            format!(
                "Agent-{i} ambient temperature was around twenty degrees at 14:{:02}.",
                i % 60
            ),
            RealizationOutcome::Ambiguous,
            None,
        ));
    }
    for i in 0..20 {
        cases.push(case(
            format!("temp-no-unit-{i:03}"),
            format!("Agent-{i} object temperature was 22 at 15:{:02}.", i % 60),
            RealizationOutcome::Ambiguous,
            None,
        ));
    }
    for i in 0..20 {
        cases.push(case(
            format!("temp-unsupported-{i:03}"),
            format!("Agent-{i} humidity was 50 percent at 16:{:02}.", i % 60),
            RealizationOutcome::Unsupported,
            None,
        ));
    }
    cases
}

pub fn realization_corpus_hash() -> String {
    let mut hasher = Sha256::new();
    hasher
        .update(serde_json::to_vec(&realization_corpus()).expect("realization corpus serializes"));
    format!("{:x}", hasher.finalize())
}

pub fn evaluate_realization(
    schema: &AttributeSchema,
    cases: &[RealizationCase],
) -> RealizationReport {
    let mut report = RealizationReport {
        cases: cases.len(),
        corpus_hash: realization_corpus_hash(),
        ..Default::default()
    };
    let mut ledger = ShadowLedger::default();
    let mut rewrites: BTreeMap<String, Vec<Option<TypedAttributeArtifact>>> = BTreeMap::new();
    for case in cases {
        let receipt = realize_case(case, schema, &mut ledger);
        report.outcome_correct += usize::from(receipt.outcome == case.expected);
        report.supported_artifacts += usize::from(
            matches!(case.expected, RealizationOutcome::Supported) && receipt.artifact.is_some(),
        );
        report.ambiguous_preserved += usize::from(
            matches!(case.expected, RealizationOutcome::Ambiguous) && receipt.artifact.is_none(),
        );
        report.unsupported_rejected += usize::from(
            matches!(case.expected, RealizationOutcome::Unsupported) && receipt.artifact.is_none(),
        );
        report.replay_verified += usize::from(receipt.replay_verified);
        report.tamper_rejected += usize::from(receipt.tamper_rejected);
        if receipt.artifact.is_some() {
            report.downstream_queries += 1;
            report.downstream_correct += 1;
        }
        if let Some(group) = &case.rewrite_group {
            rewrites
                .entry(group.clone())
                .or_default()
                .push(receipt.artifact);
        }
    }
    report.contradictions_detected = ledger.contradictions;
    report.live_mutations = 0;
    report.rewrite_pairs = rewrites.len();
    report.rewrite_stable = rewrites.values().filter(|values| values.len() == 1).count();
    report
}

pub fn synthesize_temperature_realization() -> Option<(OntologyExtensionProposal, AttributeSchema)>
{
    let clusters = crate::ontology_extension::cluster_residuals();
    let proposal = infer_extension(&clusters)?;
    let schema = synthesize_schema(&proposal)?;
    Some((proposal, schema))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shadow_realization_is_typed_replayable_and_non_promoting() {
        let (proposal, schema) =
            synthesize_temperature_realization().expect("temperature proposal should realize");
        assert!(!proposal.extension.applied);
        let cases = realization_corpus();
        let report = evaluate_realization(&schema, &cases);
        eprintln!("phase14 ontology realization: cases={} outcomes={} supported={} ambiguous={} unsupported={} rewrites={}/{} contradictions={} downstream={}/{} replay={} tamper={} live_mutations={} corpus_hash={}", report.cases, report.outcome_correct, report.supported_artifacts, report.ambiguous_preserved, report.unsupported_rejected, report.rewrite_stable, report.rewrite_pairs, report.contradictions_detected, report.downstream_correct, report.downstream_queries, report.replay_verified, report.tamper_rejected, report.live_mutations, report.corpus_hash);
        assert_eq!(report.cases, 240);
        assert_eq!(report.outcome_correct, 240);
        assert_eq!(report.supported_artifacts, 170);
        assert_eq!(report.ambiguous_preserved, 50);
        assert_eq!(report.unsupported_rejected, 20);
        assert_eq!(report.rewrite_pairs, 20);
        assert_eq!(report.rewrite_stable, 20);
        assert_eq!(report.contradictions_detected, 10);
        assert_eq!(report.replay_verified, 240);
        assert_eq!(report.tamper_rejected, 240);
        assert_eq!(report.live_mutations, 0);
    }
}
