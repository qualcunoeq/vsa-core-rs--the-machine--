//! Retrieval-to-execution gate for source-derived science laws.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use the_machine::probability_pack::Rational;
use the_machine::science_law_pack::{evaluate_science, ScienceRequest, ScienceStatus};
use the_machine::source_retrieval::{
    retrieve_claim, ClaimQuery, ClaimSource, RetrievalStatus, SourceClaim,
};

fn rational(numerator: i128, denominator: i128) -> Rational {
    Rational::new(numerator, denominator).unwrap()
}

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn source(id: &str) -> ClaimSource {
    ClaimSource {
        source_id: id.into(),
        title: format!("Authoritative science source {id}"),
        locator: format!("https://example.invalid/science/{id}"),
        retrieved_utc: "2026-08-16".into(),
        lineage_id: id.into(),
    }
}

fn claim(id: &str, law: &str, source_id: &str) -> SourceClaim {
    SourceClaim {
        claim_id: id.into(),
        subject: "classical_science".into(),
        predicate: "supported_law".into(),
        object: law.into(),
        domain: "source_derived_classical_science".into(),
        scope: "si_consistent_exact".into(),
        validity: "explicit bounded law record".into(),
        assumptions: vec!["source formulation is selected by exact query".into()],
        source: source(source_id),
    }
}

fn query() -> ClaimQuery {
    ClaimQuery {
        subject: "classical_science".into(),
        predicate: "supported_law".into(),
        domain: "source_derived_classical_science".into(),
        scope: "si_consistent_exact".into(),
        provenance: vec!["source-science-bridge-corpus".into()],
    }
}

fn science_request(law: &str) -> ScienceRequest {
    ScienceRequest {
        law: law.into(),
        inputs: BTreeMap::from([
            ("n".into(), rational(1, 1)),
            ("R".into(), rational(8, 1)),
            ("T".into(), rational(300, 1)),
            ("V".into(), rational(100, 1)),
            ("Q".into(), rational(100, 1)),
            ("W".into(), rational(40, 1)),
            ("m".into(), rational(4, 1)),
            ("v".into(), rational(3, 1)),
            ("k".into(), rational(2, 1)),
            ("x".into(), rational(5, 1)),
        ]),
        domain: "source_derived_classical_science".into(),
        unit_scope: "si_consistent_exact".into(),
        ambiguity: None,
        provenance: vec!["retrieved-law-claim".into()],
    }
}

fn expected(law: &str) -> Rational {
    match law {
        "ideal_gas_pressure" => rational(24, 1),
        "first_law_delta_u" => rational(60, 1),
        "kinetic_energy" => rational(18, 1),
        "hooke_force" => rational(-10, 1),
        _ => unreachable!(),
    }
}

fn main() {
    let laws = [
        "ideal_gas_pressure",
        "first_law_delta_u",
        "kinetic_energy",
        "hooke_force",
    ];
    let mut exact = 0usize;
    let mut supported = 0usize;
    let mut ambiguous = 0usize;
    let mut refused = 0usize;
    let mut executed = 0usize;
    let mut values = 0usize;
    let mut replay = 0usize;
    let mut tamper = 0usize;
    let mut false_authorizations = 0usize;
    let mut records = Vec::new();

    for index in 0..120 {
        let law = laws[index % laws.len()];
        let retrieved = retrieve_claim(
            &query(),
            &[
                claim("primary", law, "source-a"),
                claim("corroborating", law, "source-b"),
            ],
        );
        let science = if retrieved.eligible_for_shadow_use() {
            Some(evaluate_science(&science_request(
                retrieved.distinct_objects.first().unwrap(),
            )))
        } else {
            None
        };
        let ok = retrieved.status == RetrievalStatus::Supported
            && retrieved.eligible_for_shadow_use()
            && science.as_ref().is_some_and(|result| {
                result.status == ScienceStatus::Complete && result.value == Some(expected(law))
            });
        exact += usize::from(ok);
        supported += usize::from(ok);
        executed += usize::from(science.is_some());
        values += usize::from(
            science
                .as_ref()
                .is_some_and(|result| result.value == Some(expected(law))),
        );
        replay += usize::from(retrieved.replay_verified());
        replay += usize::from(
            science
                .as_ref()
                .is_some_and(|result| result.replay_verified()),
        );
        let mut altered = retrieved.clone();
        altered.replay_hash.push('x');
        tamper += usize::from(!altered.replay_verified());
        if let Some(result) = science {
            let mut altered = result;
            altered.replay_hash.push('x');
            tamper += usize::from(!altered.replay_verified());
        }
        false_authorizations += usize::from(!ok);
        records.push((index, law, "supported", ok));
    }

    for index in 0..40 {
        let retrieved = retrieve_claim(
            &query(),
            &[
                claim("source-a", "kinetic_energy", "source-a"),
                claim("source-b", "elastic_potential_energy", "source-b"),
            ],
        );
        let ok = retrieved.status == RetrievalStatus::Conflicting
            && !retrieved.eligible_for_shadow_use();
        exact += usize::from(ok);
        ambiguous += usize::from(ok);
        replay += usize::from(retrieved.replay_verified());
        let mut altered = retrieved.clone();
        altered.replay_hash.push('x');
        tamper += usize::from(!altered.replay_verified());
        false_authorizations += usize::from(!ok);
        records.push((index, "conflict", "ambiguous", ok));
    }

    for index in 0..80 {
        let mut missing_query = query();
        missing_query.domain = "unsupported_domain".into();
        let retrieved = retrieve_claim(
            &missing_query,
            &[claim("other-domain", "kinetic_energy", "source-a")],
        );
        let ok =
            retrieved.status == RetrievalStatus::Missing && !retrieved.eligible_for_shadow_use();
        exact += usize::from(ok);
        refused += usize::from(ok);
        replay += usize::from(retrieved.replay_verified());
        let mut altered = retrieved.clone();
        altered.replay_hash.push('x');
        tamper += usize::from(!altered.replay_verified());
        false_authorizations += usize::from(!ok);
        records.push((index, "missing", "refused", ok));
    }

    assert_eq!(exact, 240);
    assert_eq!(supported, 120);
    assert_eq!(ambiguous, 40);
    assert_eq!(refused, 80);
    assert_eq!(executed, 120);
    assert_eq!(values, 120);
    assert_eq!(replay, 360);
    assert_eq!(tamper, 360);
    assert_eq!(false_authorizations, 0);
    let report = serde_json::json!({
        "schema": "stage-i-source-science-bridge-v1",
        "cases": 240,
        "supported": supported,
        "ambiguous": ambiguous,
        "refused": refused,
        "retrieved_and_executed": executed,
        "exact_values": values,
        "exact_decisions": exact,
        "replay_verified": replay,
        "tamper_rejected": tamper,
        "false_authorizations": false_authorizations,
        "records_hash": digest(&records),
    });
    let serialized = serde_json::to_string_pretty(&report).unwrap();
    std::fs::write(
        "docs/stage_i_source_science_bridge.json",
        format!("{serialized}\n"),
    )
    .unwrap();
    println!("{serialized}");
}
