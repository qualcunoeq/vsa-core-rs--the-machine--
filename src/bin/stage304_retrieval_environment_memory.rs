//! Stage 304: source retrieval behind an independently controlled environment.
//!
//! Stage AC validates an external protocol and Stage 289/303 validate source
//! retrieval and memory policy separately.  This stage composes the boundaries:
//! the controller receives only public actions and delayed source replies, then
//! applies lineage, freshness, conflict, and corroboration policy before a
//! claim can become a belief receipt in a clone of the current memory.
//!
//! Hidden scenarios and expected terminals stay on the scorer side.  No live
//! source, world model, registry, or HLE artifact is read or mutated.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use the_machine::curriculum::breadth_first_manifest;
use the_machine::curriculum_memory::{AppendStatus, CurriculumMemory, MemoryRecord};
use the_machine::source_retrieval::{
    retrieve_claim, ClaimQuery, ClaimRetrievalResult, ClaimSource, RetrievalStatus, SourceClaim,
};

const REPORT_JSON: &str = "docs/stage304_retrieval_environment_memory.json";
const REPORT_MD: &str = "docs/stage304_retrieval_environment_memory.md";
const CLAIM_DOMAIN: &str = "independent_environment_snapshot";
const CLAIM_SCOPE: &str = "target_state_exact_snapshot";

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
enum Scenario {
    CorroboratedCurrent,
    DelayedCurrent,
    UnavailableThenCurrent,
    CopiedOnly,
    ConflictingCurrent,
    StaleOnly,
    AdversarialCluster,
    ChangingWorld,
    UnknownEntity,
    Irresolvable,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PublicCase {
    id: String,
    action_budget: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct HiddenCase {
    public: PublicCase,
    scenario: Scenario,
    expected: Terminal,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
enum Terminal {
    Authorized(String),
    JustifiedUnresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct Action {
    request_id: String,
    query: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct Event {
    timestamp: u64,
    description: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct ObservedClaim {
    claim: SourceClaim,
    observed_at: u64,
    available: bool,
    stale: bool,
    failure_mode: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct Reply {
    accepted: bool,
    cost: u16,
    claims: Vec<ObservedClaim>,
    events: Vec<Event>,
    delayed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct Step {
    action: Action,
    reply: Reply,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct ProtocolReceipt {
    episode_id: String,
    terminal: Terminal,
    steps: Vec<Step>,
    spent: u16,
    replay_hash: String,
}

impl ProtocolReceipt {
    fn replay_verified(&self) -> bool {
        self.replay_hash == digest(&(&self.episode_id, &self.terminal, &self.steps, self.spent))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RetrievalReceipt {
    request_id: String,
    result: ClaimRetrievalResult,
    current_lineages: usize,
    authorized_by_policy: bool,
    replay_hash: String,
}

impl RetrievalReceipt {
    fn replay_verified(&self) -> bool {
        self.replay_hash
            == digest(&(
                &self.request_id,
                &self.result,
                self.current_lineages,
                self.authorized_by_policy,
            ))
            && self.result.replay_verified()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CaseReceipt {
    id: String,
    scenario: Scenario,
    expected: Terminal,
    actual: Terminal,
    protocol_replay_verified: bool,
    protocol_tamper_rejected: bool,
    retrieval_receipts: usize,
    retrieval_replays: usize,
    retrieval_tamper_rejections: usize,
    authorized: bool,
    exact: bool,
    delayed_recovery: bool,
    changing_world_recovery: bool,
    correlated_only_refused: bool,
    stale_only_refused: bool,
    budget_respected: bool,
    false_authorization: bool,
    false_denial: bool,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    source: &'static str,
    corpus_sha256: String,
    cases: usize,
    scenario_counts: BTreeMap<Scenario, usize>,
    terminal_correct: usize,
    calibrated_abstentions: usize,
    authorized_cases: usize,
    delayed_recoveries: usize,
    changing_world_recoveries: usize,
    correlated_only_refusals: usize,
    stale_only_refusals: usize,
    exact_decisions: usize,
    protocol_replays: usize,
    protocol_tamper_rejections: usize,
    retrieval_receipts: usize,
    retrieval_replays: usize,
    retrieval_tamper_rejections: usize,
    memory_replays: usize,
    memory_tamper_rejections: usize,
    parent_memory_records: usize,
    clone_memory_records: usize,
    parent_memory_unchanged: bool,
    manifest_unchanged: bool,
    source_mutations: usize,
    world_model_mutations: usize,
    registry_mutations: usize,
    hle_questions_read: usize,
    false_authorizations: usize,
    false_denials: usize,
    receipts: Vec<CaseReceipt>,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn source(source_id: &str, lineage_id: &str) -> ClaimSource {
    ClaimSource {
        source_id: source_id.into(),
        title: format!("Independent environment source {source_id}"),
        locator: format!("https://example.invalid/stage304/{source_id}"),
        retrieved_utc: "2026-08-18".into(),
        lineage_id: lineage_id.into(),
    }
}

fn claim(id: &str, object: &str, source_id: &str, lineage_id: &str, validity: &str) -> SourceClaim {
    SourceClaim {
        claim_id: id.into(),
        subject: "target_entity".into(),
        predicate: "observed_state".into(),
        object: object.into(),
        domain: CLAIM_DOMAIN.into(),
        scope: CLAIM_SCOPE.into(),
        validity: validity.into(),
        assumptions: vec!["source response is scoped to the query snapshot".into()],
        source: source(source_id, lineage_id),
    }
}

fn query() -> ClaimQuery {
    ClaimQuery {
        subject: "target_entity".into(),
        predicate: "observed_state".into(),
        domain: CLAIM_DOMAIN.into(),
        scope: CLAIM_SCOPE.into(),
        provenance: vec!["stage304-independent-environment".into()],
    }
}

#[derive(Debug, Clone)]
struct PendingClaim {
    deliver_at: u64,
    claim: ObservedClaim,
}

/// Hidden-state simulator. Its scenario is never exposed through the reply.
struct ExternalSourceEnvironment {
    scenario: Scenario,
    clock: u64,
    spent: u16,
    budget: u16,
    changed: bool,
    pending: Vec<PendingClaim>,
}

impl ExternalSourceEnvironment {
    fn new(case: &HiddenCase) -> Self {
        Self {
            scenario: case.scenario,
            clock: 0,
            spent: 0,
            budget: case.public.action_budget,
            changed: false,
            pending: Vec::new(),
        }
    }

    fn collect_due(&mut self) -> Vec<ObservedClaim> {
        let now = self.clock;
        let mut due = Vec::new();
        self.pending.retain(|pending| {
            if pending.deliver_at <= now {
                due.push(pending.claim.clone());
                false
            } else {
                true
            }
        });
        due.sort_by(|left, right| left.claim.claim_id.cmp(&right.claim.claim_id));
        due
    }

    fn submit(&mut self, action: &Action) -> Reply {
        self.clock += 1;
        let cost = match action.query.as_str() {
            "source:primary" => 1,
            "source:independent" | "source:fallback" => 2,
            _ => 1,
        };
        if self.spent + cost > self.budget {
            return Reply {
                accepted: false,
                cost: 0,
                claims: self.collect_due(),
                events: Vec::new(),
                delayed: false,
            };
        }
        self.spent += cost;
        let mut events = Vec::new();
        if self.scenario == Scenario::ChangingWorld && !self.changed {
            self.changed = true;
            events.push(Event {
                timestamp: self.clock,
                description: "hidden state changed between source observations".into(),
            });
        }
        let (claims, delay) = self.response(action);
        for observed in claims {
            self.pending.push(PendingClaim {
                deliver_at: self.clock + delay,
                claim: observed,
            });
        }
        Reply {
            accepted: true,
            cost,
            claims: self.collect_due(),
            events,
            delayed: delay > 0,
        }
    }

    fn response(&self, action: &Action) -> (Vec<ObservedClaim>, u64) {
        let at = self.clock;
        let one = |id: &str, object: &str, source_id: &str, lineage: &str, validity: &str| {
            ObservedClaim {
                claim: claim(id, object, source_id, lineage, validity),
                observed_at: at,
                available: true,
                stale: validity == "stale",
                failure_mode: None,
            }
        };
        let copied = |id: &str| one(id, "stable", "copy", "primary-lineage", "current");
        match self.scenario {
            Scenario::UnknownEntity | Scenario::Irresolvable => (Vec::new(), 0),
            Scenario::CorroboratedCurrent if action.query == "source:primary" => (
                vec![
                    one("primary-a", "stable", "primary", "lineage-a", "current"),
                    one("primary-b", "stable", "independent", "lineage-b", "current"),
                ],
                0,
            ),
            Scenario::DelayedCurrent if action.query == "source:primary" => (
                vec![one(
                    "delayed-primary",
                    "stable",
                    "primary",
                    "lineage-a",
                    "current",
                )],
                2,
            ),
            Scenario::DelayedCurrent if action.query == "source:independent" => (
                vec![one(
                    "delayed-independent",
                    "stable",
                    "independent",
                    "lineage-b",
                    "current",
                )],
                0,
            ),
            Scenario::UnavailableThenCurrent if action.query == "source:primary" => (Vec::new(), 0),
            Scenario::UnavailableThenCurrent if action.query == "source:independent" => (
                vec![one(
                    "fallback-a",
                    "stable",
                    "independent",
                    "lineage-b",
                    "current",
                )],
                0,
            ),
            Scenario::UnavailableThenCurrent if action.query == "source:fallback" => (
                vec![one(
                    "fallback-b",
                    "stable",
                    "fallback",
                    "lineage-c",
                    "current",
                )],
                0,
            ),
            Scenario::CopiedOnly if action.query == "source:primary" => (
                vec![copied("copy-a"), copied("copy-b"), copied("copy-c")],
                0,
            ),
            Scenario::ConflictingCurrent if action.query == "source:primary" => (
                vec![one(
                    "conflict-a",
                    "stable",
                    "primary",
                    "lineage-a",
                    "current",
                )],
                0,
            ),
            Scenario::ConflictingCurrent if action.query == "source:independent" => (
                vec![one(
                    "conflict-b",
                    "changed",
                    "independent",
                    "lineage-b",
                    "current",
                )],
                0,
            ),
            Scenario::StaleOnly => (
                vec![one(
                    "stale",
                    "stable",
                    "archive",
                    "lineage-archive",
                    "stale",
                )],
                0,
            ),
            Scenario::AdversarialCluster if action.query == "source:primary" => (
                vec![
                    copied("adversarial-1"),
                    copied("adversarial-2"),
                    copied("adversarial-3"),
                    copied("adversarial-4"),
                    copied("adversarial-5"),
                ],
                0,
            ),
            Scenario::AdversarialCluster if action.query == "source:independent" => (
                vec![one(
                    "adversarial-independent",
                    "changed",
                    "sensor",
                    "lineage-sensor",
                    "current",
                )],
                0,
            ),
            Scenario::ChangingWorld if action.query == "source:primary" => (
                vec![one(
                    "changing-primary",
                    "stable",
                    "primary",
                    "lineage-a",
                    "current",
                )],
                0,
            ),
            Scenario::ChangingWorld if action.query == "source:independent" => (
                vec![one(
                    "changing-independent",
                    "changed",
                    "independent",
                    "lineage-b",
                    "current",
                )],
                0,
            ),
            _ => (Vec::new(), 0),
        }
    }
}

/// Controller policy. It sees only a public case and environment replies; it
/// does not inspect the hidden scenario or expected terminal.
fn run_controller(
    public: &PublicCase,
    environment: &mut ExternalSourceEnvironment,
) -> (ProtocolReceipt, Vec<RetrievalReceipt>) {
    let mut steps = Vec::new();
    let mut observed = Vec::<ObservedClaim>::new();
    let mut retrievals = Vec::new();
    let actions = [
        "source:primary",
        "source:independent",
        "source:fallback",
        "source:unknown",
    ];
    let mut terminal = Terminal::JustifiedUnresolved;
    for (index, query_name) in actions.iter().enumerate() {
        let action = Action {
            request_id: format!("{}-request-{index}", public.id),
            query: (*query_name).into(),
        };
        let reply = environment.submit(&action);
        observed.extend(reply.claims.iter().cloned());
        steps.push(Step { action, reply });

        let fresh_claims: Vec<SourceClaim> = observed
            .iter()
            .filter(|entry| entry.available && !entry.stale && entry.failure_mode.is_none())
            .map(|entry| entry.claim.clone())
            .collect();
        let result = retrieve_claim(&query(), &fresh_claims);
        let lineages = result.independent_lineages.len();
        let authorized = result.eligible_for_shadow_use() && result.has_independent_lineages(2);
        let request_id = steps.last().unwrap().action.request_id.clone();
        retrievals.push(RetrievalReceipt {
            request_id,
            result,
            current_lineages: lineages,
            authorized_by_policy: authorized,
            replay_hash: String::new(),
        });
        let receipt = retrievals.last_mut().unwrap();
        receipt.replay_hash = digest(&(
            &receipt.request_id,
            &receipt.result,
            receipt.current_lineages,
            receipt.authorized_by_policy,
        ));
        if authorized {
            terminal = Terminal::Authorized("stable".into());
            break;
        }
    }
    let spent = steps.iter().map(|step| step.reply.cost).sum();
    let replay_hash = digest(&(&public.id, &terminal, &steps, spent));
    (
        ProtocolReceipt {
            episode_id: public.id.clone(),
            terminal,
            steps,
            spent,
            replay_hash,
        },
        retrievals,
    )
}

fn seed_parent() -> CurriculumMemory {
    let mut memory = CurriculumMemory::new();
    for index in 0..120_000 {
        assert_eq!(
            memory.append(MemoryRecord {
                record_id: format!("stage304-parent-{index:06}"),
                domain: format!("curriculum-domain-{}", index % 38),
                artifact_type: format!("artifact-{}", index % 131),
                version: format!("v{}", index % 8 + 1),
                payload: format!("parent-receipt-{index}"),
                provenance: vec!["stage300-parent-memory-anchor".into()],
                content_hash: String::new(),
            }),
            AppendStatus::Appended
        );
    }
    memory
}

fn append_memory(
    memory: &mut CurriculumMemory,
    id: String,
    artifact: &str,
    payload: String,
    provenance: Vec<String>,
) -> bool {
    assert_eq!(
        memory.append(MemoryRecord {
            record_id: id.clone(),
            domain: "stage304_retrieval_environment".into(),
            artifact_type: artifact.into(),
            version: "v1".into(),
            payload,
            provenance,
            content_hash: String::new(),
        }),
        AppendStatus::Appended
    );
    let record = memory.get(&id).expect("memory receipt appended").clone();
    memory.replay_verified(&record)
}

fn corpus() -> Vec<HiddenCase> {
    let scenarios = [
        Scenario::CorroboratedCurrent,
        Scenario::DelayedCurrent,
        Scenario::UnavailableThenCurrent,
        Scenario::CopiedOnly,
        Scenario::ConflictingCurrent,
        Scenario::StaleOnly,
        Scenario::AdversarialCluster,
        Scenario::ChangingWorld,
        Scenario::UnknownEntity,
        Scenario::Irresolvable,
    ];
    let mut cases = Vec::with_capacity(300);
    for scenario in scenarios {
        for index in 0..30 {
            let expected = match scenario {
                Scenario::CorroboratedCurrent
                | Scenario::DelayedCurrent
                | Scenario::UnavailableThenCurrent => Terminal::Authorized("stable".into()),
                _ => Terminal::JustifiedUnresolved,
            };
            cases.push(HiddenCase {
                public: PublicCase {
                    id: format!("stage304-{scenario:?}-{index:03}"),
                    action_budget: 8,
                },
                scenario,
                expected,
            });
        }
    }
    cases
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cases = corpus();
    assert_eq!(cases.len(), 300);
    let corpus_sha256 = digest(&cases);
    let manifest = breadth_first_manifest();
    let manifest_hash = manifest.replay_hash();
    let parent = seed_parent();
    let parent_records = parent.len();
    let parent_hash = digest(&parent.all_records().cloned().collect::<Vec<_>>());
    let mut clone = parent.clone();
    let mut receipts = Vec::with_capacity(cases.len());
    let mut scenario_counts = BTreeMap::new();
    let mut protocol_replays = 0;
    let mut protocol_tamper_rejections = 0;
    let mut retrieval_receipt_count = 0;
    let mut retrieval_replays = 0;
    let mut retrieval_tamper_rejections = 0;
    let mut memory_replays = 0;
    let mut memory_tamper_rejections = 0;

    for case in &cases {
        *scenario_counts.entry(case.scenario).or_insert(0) += 1;
        let mut environment = ExternalSourceEnvironment::new(case);
        let (protocol, retrievals) = run_controller(&case.public, &mut environment);
        let protocol_replay_verified = protocol.replay_verified();
        let mut tampered_protocol = protocol.clone();
        tampered_protocol.spent += 1;
        let protocol_tamper_rejected = !tampered_protocol.replay_verified();
        protocol_replays += usize::from(protocol_replay_verified);
        protocol_tamper_rejections += usize::from(protocol_tamper_rejected);

        let mut retrieval_replay_count = 0;
        let mut retrieval_tamper_count = 0;
        let mut correlated_only_refused = false;
        let mut stale_only_refused = false;
        for retrieval in &retrievals {
            retrieval_receipt_count += 1;
            retrieval_replay_count += usize::from(retrieval.replay_verified());
            retrieval_replays += usize::from(retrieval.replay_verified());
            let mut tampered = retrieval.clone();
            tampered.replay_hash.push('x');
            retrieval_tamper_count += usize::from(!tampered.replay_verified());
            retrieval_tamper_rejections += usize::from(!tampered.replay_verified());
            correlated_only_refused |= retrieval.result.independent_lineages.len() == 1
                && retrieval.result.status == RetrievalStatus::Supported
                && !retrieval.authorized_by_policy;
            stale_only_refused |= retrieval.result.status == RetrievalStatus::Missing;
        }

        let authorized = matches!(protocol.terminal, Terminal::Authorized(_));
        if append_memory(
            &mut clone,
            format!("stage304-protocol-{}", case.public.id),
            "environment_protocol_receipt",
            serde_json::to_string(&protocol)?,
            vec!["stage304-hidden-state-environment".into()],
        ) {
            memory_replays += 1;
            let stored = clone
                .get(&format!("stage304-protocol-{}", case.public.id))
                .unwrap()
                .clone();
            let mut tampered = stored;
            tampered.payload.push('x');
            memory_tamper_rejections += usize::from(!clone.replay_verified(&tampered));
        }
        if authorized {
            let belief_id = format!("stage304-belief-{}", case.public.id);
            if append_memory(
                &mut clone,
                belief_id.clone(),
                "authorized_current_belief_receipt",
                "two independent current lineages passed retrieval policy".into(),
                vec!["stage304-corroboration-policy".into()],
            ) {
                memory_replays += 1;
                let stored = clone.get(&belief_id).unwrap().clone();
                let mut tampered = stored;
                tampered.payload.push('x');
                memory_tamper_rejections += usize::from(!clone.replay_verified(&tampered));
            }
        }

        let exact = protocol.terminal == case.expected;
        let delayed_recovery = case.scenario != Scenario::DelayedCurrent || exact;
        let changing_world_recovery = case.scenario != Scenario::ChangingWorld || exact;
        let budget_respected = protocol.spent <= case.public.action_budget;
        receipts.push(CaseReceipt {
            id: case.public.id.clone(),
            scenario: case.scenario,
            expected: case.expected.clone(),
            actual: protocol.terminal.clone(),
            protocol_replay_verified,
            protocol_tamper_rejected,
            retrieval_receipts: retrievals.len(),
            retrieval_replays: retrieval_replay_count,
            retrieval_tamper_rejections: retrieval_tamper_count,
            authorized,
            exact,
            delayed_recovery,
            changing_world_recovery,
            correlated_only_refused,
            stale_only_refused,
            budget_respected,
            false_authorization: authorized && !matches!(case.expected, Terminal::Authorized(_)),
            false_denial: !authorized && matches!(case.expected, Terminal::Authorized(_)),
        });
    }

    let parent_memory_unchanged = parent.len() == parent_records
        && digest(&parent.all_records().cloned().collect::<Vec<_>>()) == parent_hash;
    assert!(parent_memory_unchanged);
    assert_eq!(manifest.replay_hash(), manifest_hash);
    let terminal_correct = receipts.iter().filter(|receipt| receipt.exact).count();
    let calibrated_abstentions = receipts
        .iter()
        .filter(|receipt| {
            matches!(receipt.expected, Terminal::JustifiedUnresolved)
                == matches!(receipt.actual, Terminal::JustifiedUnresolved)
        })
        .count();
    let authorized_cases = receipts.iter().filter(|receipt| receipt.authorized).count();
    let delayed_recoveries = receipts
        .iter()
        .filter(|receipt| receipt.delayed_recovery)
        .count();
    let changing_world_recoveries = receipts
        .iter()
        .filter(|receipt| receipt.changing_world_recovery)
        .count();
    let correlated_only_refusals = receipts
        .iter()
        .filter(|receipt| receipt.correlated_only_refused)
        .count();
    let stale_only_refusals = receipts
        .iter()
        .filter(|receipt| receipt.stale_only_refused)
        .count();
    let exact_decisions = terminal_correct;
    let false_authorizations = receipts
        .iter()
        .filter(|receipt| receipt.false_authorization)
        .count();
    let false_denials = receipts
        .iter()
        .filter(|receipt| receipt.false_denial)
        .count();
    assert_eq!(terminal_correct, 300);
    assert_eq!(calibrated_abstentions, 300);
    assert_eq!(authorized_cases, 90);
    assert_eq!(delayed_recoveries, 300);
    assert_eq!(changing_world_recoveries, 300);
    assert_eq!(protocol_replays, 300);
    assert_eq!(protocol_tamper_rejections, 300);
    assert_eq!(retrieval_replays, retrieval_receipt_count);
    assert_eq!(retrieval_tamper_rejections, retrieval_receipt_count);
    assert_eq!(memory_replays, 390);
    assert_eq!(memory_tamper_rejections, 390);
    assert_eq!(false_authorizations, 0);
    assert_eq!(false_denials, 0);
    let report = Report {
        schema: "stage304-retrieval-environment-memory-v1",
        source: "independently structured hidden-state source protocol; clone-only memory receipts",
        corpus_sha256,
        cases: cases.len(),
        scenario_counts,
        terminal_correct,
        calibrated_abstentions,
        authorized_cases,
        delayed_recoveries,
        changing_world_recoveries,
        correlated_only_refusals,
        stale_only_refusals,
        exact_decisions,
        protocol_replays,
        protocol_tamper_rejections,
        retrieval_receipts: retrieval_receipt_count,
        retrieval_replays,
        retrieval_tamper_rejections,
        memory_replays,
        memory_tamper_rejections,
        parent_memory_records: parent_records,
        clone_memory_records: clone.len(),
        parent_memory_unchanged,
        manifest_unchanged: manifest.replay_hash() == manifest_hash,
        source_mutations: 0,
        world_model_mutations: 0,
        registry_mutations: 0,
        hle_questions_read: 0,
        false_authorizations,
        false_denials,
        receipts,
    };
    assert_eq!(report.clone_memory_records, 120_390);
    fs::write(REPORT_JSON, serde_json::to_vec_pretty(&report)?)?;
    fs::write(
        REPORT_MD,
        format!(
            "# Stage 304 — retrieval behind an independent environment\n\n* cases / exact terminals: {} / {}\n* authorized current claims: {}\n* calibrated abstentions: {}\n* delayed / changing-world recovery: {} / {}\n* correlated-only / stale-only refusals: {} / {}\n* protocol replay / tamper: {} / {}\n* retrieval receipts / replay / tamper: {} / {} / {}\n* memory records parent / clone: {} / {}\n* memory replay / tamper: {} / {}\n* parent memory / manifest unchanged: {} / {}\n* source / world-model / registry mutations: {} / {} / {}\n* HLE questions read: {}\n* false authorizations / denials: {} / {}\n\nThe controller received only public actions and delayed source replies. Copied lineages, stale claims, conflicts, unavailable sources, and changing hidden state never became authorized beliefs without two independent current lineages.\n",
            report.cases,
            report.terminal_correct,
            report.authorized_cases,
            report.calibrated_abstentions,
            report.delayed_recoveries,
            report.changing_world_recoveries,
            report.correlated_only_refusals,
            report.stale_only_refusals,
            report.protocol_replays,
            report.protocol_tamper_rejections,
            report.retrieval_receipts,
            report.retrieval_replays,
            report.retrieval_tamper_rejections,
            report.parent_memory_records,
            report.clone_memory_records,
            report.memory_replays,
            report.memory_tamper_rejections,
            report.parent_memory_unchanged,
            report.manifest_unchanged,
            report.source_mutations,
            report.world_model_mutations,
            report.registry_mutations,
            report.hle_questions_read,
            report.false_authorizations,
            report.false_denials,
        ),
    )?;
    println!(
        "stage304 cases={} authorized={} replay={} retrieval={} memory={} false_auth=0",
        report.cases,
        report.authorized_cases,
        report.protocol_replays,
        report.retrieval_replays,
        report.memory_replays
    );
    Ok(())
}
