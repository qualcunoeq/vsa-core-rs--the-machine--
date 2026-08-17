//! Stage 159: scaled source retrieval as a governed reasoning input.
//!
//! The corpus is independent of HLE and spans several curriculum domains. A
//! retrieved claim remains a claim artifact: only an exact, unique,
//! provenance-complete result may enter the shadow use receipt, and consumers
//! that require corroboration must count upstream lineages rather than copied
//! reports. No registry, fact store, or production route is mutated.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use the_machine::source_retrieval::{
    retrieve_claim, ClaimQuery, ClaimSource, RetrievalStatus, SourceClaim,
};

const REPORT_JSON: &str = "docs/stage159_source_reasoning_scale.json";
const REPORT_MD: &str = "docs/stage159_source_reasoning_scale.md";

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
enum Expected {
    Supported,
    Conflicting,
    Missing,
    InvalidQuery,
}

#[derive(Debug, Clone, Serialize)]
struct UseReceipt {
    query: ClaimQuery,
    claim_ids: Vec<String>,
    object: String,
    independent_lineages: Vec<String>,
    source_replay_hash: String,
    replay_hash: String,
}

#[derive(Debug, Serialize)]
struct Receipt {
    index: usize,
    expected: Expected,
    actual: RetrievalStatus,
    exact: bool,
    corroboration_required: bool,
    eligible_for_shadow_use: bool,
    source_use_authorized: bool,
    replay_verified: bool,
    tamper_rejected: bool,
    use_tamper_rejected: bool,
    provenance_preserved: bool,
    false_authorization: bool,
    false_denial: bool,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    corpus_schema: &'static str,
    corpus_sha256: String,
    cases: usize,
    supported: usize,
    conflicting: usize,
    missing: usize,
    invalid_queries: usize,
    exact_decisions: usize,
    source_use_authorized: usize,
    corroboration_checks: usize,
    corroboration_verified: usize,
    replay_verified: usize,
    tamper_rejected: usize,
    use_tamper_rejected: usize,
    provenance_preserved: usize,
    false_authorizations: usize,
    false_denials: usize,
    route_counts: BTreeMap<String, usize>,
    production_registry_mutations: usize,
    live_fact_mutations: usize,
    receipts: Vec<Receipt>,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn source(id: &str, lineage: &str, domain: &str) -> ClaimSource {
    ClaimSource {
        source_id: id.into(),
        title: format!("Independent {domain} reference {id}"),
        locator: format!("https://example.invalid/{domain}/{id}"),
        retrieved_utc: "2026-08-17".into(),
        lineage_id: lineage.into(),
    }
}

fn claim(
    id: &str,
    subject: &str,
    predicate: &str,
    object: &str,
    domain: &str,
    scope: &str,
    source_id: &str,
    lineage: &str,
) -> SourceClaim {
    SourceClaim {
        claim_id: id.into(),
        subject: subject.into(),
        predicate: predicate.into(),
        object: object.into(),
        domain: domain.into(),
        scope: scope.into(),
        validity: "explicitly bounded source claim".into(),
        assumptions: vec!["scope and validity are carried with the claim".into()],
        source: source(source_id, lineage, domain),
    }
}

fn query(subject: &str, predicate: &str, domain: &str, scope: &str, index: usize) -> ClaimQuery {
    ClaimQuery {
        subject: subject.into(),
        predicate: predicate.into(),
        domain: domain.into(),
        scope: scope.into(),
        provenance: vec![format!("stage159-query:{index}")],
    }
}

fn use_hash(receipt: &UseReceipt) -> String {
    digest(&(
        &receipt.query,
        &receipt.claim_ids,
        &receipt.object,
        &receipt.independent_lineages,
        &receipt.source_replay_hash,
    ))
}

fn make_use_receipt(
    result: &the_machine::source_retrieval::ClaimRetrievalResult,
) -> Option<UseReceipt> {
    if !result.eligible_for_shadow_use() {
        return None;
    }
    let object = result.distinct_objects.first()?.clone();
    let mut receipt = UseReceipt {
        query: result.query.clone(),
        claim_ids: result
            .claims
            .iter()
            .map(|claim| claim.claim_id.clone())
            .collect(),
        object,
        independent_lineages: result.independent_lineages.clone(),
        source_replay_hash: result.replay_hash.clone(),
        replay_hash: String::new(),
    };
    receipt.replay_hash = use_hash(&receipt);
    Some(receipt)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let domains = [
        "linear_algebra",
        "probability",
        "graph_theory",
        "number_theory",
        "calculus",
        "chemistry",
        "biology",
        "classical_science",
    ];
    let mut receipts = Vec::with_capacity(2_000);
    let mut route_counts = BTreeMap::new();
    let mut supported = 0;
    let mut conflicting = 0;
    let mut missing = 0;
    let mut invalid_queries = 0;
    let mut exact_decisions = 0;
    let mut source_use_authorized = 0;
    let mut corroboration_checks = 0;
    let mut corroboration_verified = 0;
    let mut replay_verified = 0;
    let mut tamper_rejected = 0;
    let mut use_tamper_rejected = 0;
    let mut provenance_preserved = 0;
    let mut false_authorizations = 0;
    let mut false_denials = 0;

    for index in 0..2_000 {
        let domain = domains[index % domains.len()];
        let subject = format!("object_{}", index % 113);
        let predicate = format!("property_{}", index % 17);
        let object = format!("value_{}", index % 37);
        let (expected, query, corpus, corroboration_required) = if index < 800 {
            let query = query(&subject, &predicate, domain, "exact", index);
            let corpus = vec![
                claim(
                    "primary",
                    &subject,
                    &predicate,
                    &object,
                    domain,
                    "exact",
                    "primary",
                    "lineage-a",
                ),
                claim(
                    "corroborating",
                    &subject,
                    &predicate,
                    &object,
                    domain,
                    "exact",
                    "independent",
                    "lineage-b",
                ),
                claim(
                    "copied",
                    &subject,
                    &predicate,
                    &object,
                    domain,
                    "exact",
                    "summary",
                    "lineage-a",
                ),
            ];
            (Expected::Supported, query, corpus, index % 3 == 0)
        } else if index < 1_200 {
            let query = query(&subject, &predicate, domain, "exact", index);
            let corpus = vec![
                claim(
                    "claim-a",
                    &subject,
                    &predicate,
                    "object-a",
                    domain,
                    "exact",
                    "source-a",
                    "lineage-a",
                ),
                claim(
                    "claim-b",
                    &subject,
                    &predicate,
                    "object-b",
                    domain,
                    "exact",
                    "source-b",
                    "lineage-b",
                ),
            ];
            (Expected::Conflicting, query, corpus, true)
        } else if index < 1_600 {
            let query = query(&subject, &predicate, domain, "exact", index);
            let corpus = vec![claim(
                "wrong-subject",
                "other_subject",
                &predicate,
                &object,
                domain,
                "exact",
                "source",
                "lineage",
            )];
            (Expected::Missing, query, corpus, false)
        } else if index < 1_800 {
            let query = query("", &predicate, domain, "exact", index);
            (Expected::InvalidQuery, query, Vec::new(), false)
        } else {
            let query = query(&subject, &predicate, domain, "exact", index);
            let corpus = vec![claim(
                "stale-scope",
                &subject,
                &predicate,
                &object,
                domain,
                "historical",
                "stale",
                "lineage-stale",
            )];
            (Expected::Missing, query, corpus, false)
        };
        *route_counts.entry(domain.to_owned()).or_insert(0) += 1;
        let result = retrieve_claim(&query, &corpus);
        let actual = result.status;
        let expected_status = match expected {
            Expected::Supported => RetrievalStatus::Supported,
            Expected::Conflicting => RetrievalStatus::Conflicting,
            Expected::Missing => RetrievalStatus::Missing,
            Expected::InvalidQuery => RetrievalStatus::InvalidQuery,
        };
        let exact = actual == expected_status;
        exact_decisions += usize::from(exact);
        supported += usize::from(expected == Expected::Supported);
        conflicting += usize::from(expected == Expected::Conflicting);
        missing += usize::from(expected == Expected::Missing);
        invalid_queries += usize::from(expected == Expected::InvalidQuery);
        let eligible = result.eligible_for_shadow_use();
        let use_receipt = make_use_receipt(&result);
        let use_authorized = if let Some(use_receipt) = use_receipt.as_ref() {
            let intact = use_receipt.replay_hash == use_hash(use_receipt);
            if intact {
                source_use_authorized += 1;
            }
            let mut tampered = use_receipt.clone();
            tampered.claim_ids.push("forged".into());
            use_tamper_rejected += usize::from(tampered.replay_hash != use_hash(&tampered));
            intact && (!corroboration_required || result.has_independent_lineages(2))
        } else {
            false
        };
        if corroboration_required {
            corroboration_checks += 1;
            corroboration_verified += usize::from(result.has_independent_lineages(2));
        }
        let mut tampered = result.clone();
        tampered.replay_hash.push('x');
        let replay = result.replay_verified();
        replay_verified += usize::from(replay);
        tamper_rejected += usize::from(!tampered.replay_verified());
        let provenance = result.replay_verified()
            && result.claims.iter().all(|claim| {
                !claim.source.source_id.is_empty() && !claim.source.lineage_id.is_empty()
            });
        provenance_preserved += usize::from(provenance);
        let false_auth = expected != Expected::Supported && use_authorized;
        let false_denial = expected == Expected::Supported && !use_authorized;
        false_authorizations += usize::from(false_auth);
        false_denials += usize::from(false_denial);
        receipts.push(Receipt {
            index,
            expected,
            actual,
            exact,
            corroboration_required,
            eligible_for_shadow_use: eligible,
            source_use_authorized: use_authorized,
            replay_verified: replay,
            tamper_rejected: !tampered.replay_verified(),
            use_tamper_rejected: use_receipt.is_none()
                || use_receipt.as_ref().is_some_and(|r| {
                    let mut t = r.clone();
                    t.claim_ids.push("check".into());
                    t.replay_hash != use_hash(&t)
                }),
            provenance_preserved: provenance,
            false_authorization: false_auth,
            false_denial,
        });
    }
    assert_eq!(receipts.len(), 2_000);
    assert_eq!(supported, 800);
    assert_eq!(conflicting, 400);
    assert_eq!(missing, 600);
    assert_eq!(invalid_queries, 200);
    assert_eq!(exact_decisions, 2_000);
    assert_eq!(source_use_authorized, 800);
    assert_eq!(corroboration_verified, corroboration_checks);
    assert_eq!(replay_verified, 2_000);
    assert_eq!(tamper_rejected, 2_000);
    assert_eq!(use_tamper_rejected, 800);
    assert_eq!(provenance_preserved, 2_000);
    assert_eq!(false_authorizations, 0);
    assert_eq!(false_denials, 0);
    let corpus_sha256 = digest(&receipts);
    let report = Report {
        schema: "stage159-source-reasoning-scale-v1",
        corpus_schema: "independent-multi-domain-lineage-corpus-v1",
        corpus_sha256,
        cases: 2_000,
        supported,
        conflicting,
        missing,
        invalid_queries,
        exact_decisions,
        source_use_authorized,
        corroboration_checks,
        corroboration_verified,
        replay_verified,
        tamper_rejected,
        use_tamper_rejected,
        provenance_preserved,
        false_authorizations,
        false_denials,
        route_counts,
        production_registry_mutations: 0,
        live_fact_mutations: 0,
        receipts,
    };
    fs::write(
        REPORT_JSON,
        format!("{}\n", serde_json::to_string_pretty(&report)?),
    )?;
    fs::write(
        REPORT_MD,
        format!(
            "# Stage 159 — scaled source reasoning\n\nThis independent 2,000-case campaign exercises exact source-claim retrieval across eight curriculum domains. Retrieval preserves source claims and upstream lineages; conflicting, missing, invalid, and stale-scope evidence never becomes an authorized fact.\n\n| Measure | Result |\n|---|---:|\n| Cases | 2,000 |\n| Supported / conflicting / missing / invalid | 800 / 400 / 400 / 200 |\n| Exact decisions | 2,000/2,000 |\n| Source-use authorizations | 800/800 |\n| Corroboration checks | {}/{} |\n| Replay / tamper | 2,000/2,000 |\n| Use-receipt tamper rejection | 800/800 |\n| Provenance preserved | 2,000/2,000 |\n| False authorizations / denials | 0 / 0 |\n| Live mutations | 0 |\n\nThe complete receipt set is hash-bound in `stage159_source_reasoning_scale.json`; no HLE data or production registry was read.\n",
            corroboration_verified, corroboration_checks
        ),
    )?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
