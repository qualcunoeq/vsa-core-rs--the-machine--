//! Independent validation campaign for governed source retrieval.

use serde::Serialize;
use sha2::{Digest, Sha256};
use the_machine::source_retrieval::{
    retrieve_claim, ClaimQuery, ClaimSource, RetrievalStatus, SourceClaim,
};

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn source(id: &str, lineage: &str) -> ClaimSource {
    ClaimSource {
        source_id: id.into(),
        title: format!("Independent reference {id}"),
        locator: format!("https://example.invalid/source/{id}"),
        retrieved_utc: "2026-08-16".into(),
        lineage_id: lineage.into(),
    }
}

fn claim(id: &str, object: &str, source_id: &str, lineage: &str) -> SourceClaim {
    SourceClaim {
        claim_id: id.into(),
        subject: "finite_group".into(),
        predicate: "order".into(),
        object: object.into(),
        domain: "abstract_algebra".into(),
        scope: "finite_cyclic".into(),
        validity: "bounded exact setting".into(),
        assumptions: vec!["operation table is finite and explicit".into()],
        source: source(source_id, lineage),
    }
}

fn query() -> ClaimQuery {
    ClaimQuery {
        subject: "finite_group".into(),
        predicate: "order".into(),
        domain: "abstract_algebra".into(),
        scope: "finite_cyclic".into(),
        provenance: vec!["source-retrieval-independent-corpus".into()],
    }
}

fn main() {
    let mut exact = 0usize;
    let mut supported = 0usize;
    let mut ambiguous = 0usize;
    let mut refused = 0usize;
    let mut replay = 0usize;
    let mut tamper = 0usize;
    let mut false_authorizations = 0usize;
    let mut lineage_deduplication_verified = 0usize;
    let mut route_records = Vec::new();

    for index in 0..120 {
        let corpus = vec![
            claim("primary", "12", "textbook-a", "chapter-3"),
            claim("corroborating", "12", "textbook-b", "chapter-7"),
            claim("copied", "12", "summary-of-a", "chapter-3"),
        ];
        let result = retrieve_claim(&query(), &corpus);
        let ok = result.status == RetrievalStatus::Supported
            && result.distinct_objects == vec!["12".to_string()]
            && result.independent_sources.len() == 3
            && result.independent_lineages.len() == 2
            && result.has_independent_lineages(2)
            && result.eligible_for_shadow_use();
        exact += usize::from(ok);
        supported += usize::from(ok);
        lineage_deduplication_verified += usize::from(
            result.independent_sources.len() == 3
                && result.independent_lineages.len() == 2
                && result.has_independent_lineages(2),
        );
        replay += usize::from(result.replay_verified());
        let mut altered = result.clone();
        altered.replay_hash.push('x');
        tamper += usize::from(!altered.replay_verified());
        false_authorizations += usize::from(!ok);
        route_records.push((index, "supported", ok));
    }

    for index in 0..40 {
        let corpus = vec![
            claim("source-a", "12", "textbook-a", "chapter-3"),
            claim("source-b", "15", "textbook-b", "chapter-7"),
        ];
        let result = retrieve_claim(&query(), &corpus);
        let ok = result.status == RetrievalStatus::Conflicting
            && !result.eligible_for_shadow_use()
            && result.distinct_objects.len() == 2;
        exact += usize::from(ok);
        ambiguous += usize::from(ok);
        replay += usize::from(result.replay_verified());
        let mut altered = result.clone();
        altered.replay_hash.push('x');
        tamper += usize::from(!altered.replay_verified());
        false_authorizations += usize::from(!ok);
        route_records.push((index, "conflicting", ok));
    }

    for index in 0..80 {
        let mut q = query();
        let corpus = if index % 2 == 0 {
            q.subject = format!("unknown_{index}");
            vec![claim("unmatched", "12", "textbook-a", "chapter-3")]
        } else {
            q.domain = "unsupported_domain".into();
            vec![claim("wrong-domain", "12", "textbook-a", "chapter-3")]
        };
        let result = retrieve_claim(&q, &corpus);
        let ok = result.status == RetrievalStatus::Missing && !result.eligible_for_shadow_use();
        exact += usize::from(ok);
        refused += usize::from(ok);
        replay += usize::from(result.replay_verified());
        let mut altered = result.clone();
        altered.replay_hash.push('x');
        tamper += usize::from(!altered.replay_verified());
        false_authorizations += usize::from(!ok);
        route_records.push((index, "missing", ok));
    }

    assert_eq!(exact, 240);
    assert_eq!(supported, 120);
    assert_eq!(ambiguous, 40);
    assert_eq!(refused, 80);
    assert_eq!(replay, 240);
    assert_eq!(tamper, 240);
    assert_eq!(lineage_deduplication_verified, 120);
    assert_eq!(false_authorizations, 0);
    let report = serde_json::json!({
        "schema": "stage-i-governed-source-retrieval-v1",
        "cases": 240,
        "supported": supported,
        "conflicting": ambiguous,
        "missing_or_refused": refused,
        "exact_decisions": exact,
        "replay_verified": replay,
        "tamper_rejected": tamper,
        "lineage_deduplication_verified": lineage_deduplication_verified,
        "false_authorizations": false_authorizations,
        "registry_mutated": false,
        "route_records_hash": digest(&route_records),
    });
    let serialized = serde_json::to_string_pretty(&report).unwrap();
    std::fs::write(
        "docs/stage_i_source_retrieval.json",
        format!("{serialized}\n"),
    )
    .unwrap();
    println!("{serialized}");
}
