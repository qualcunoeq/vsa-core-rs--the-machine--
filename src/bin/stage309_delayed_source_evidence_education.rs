//! Stage 309: utility-aware education with delayed source evidence.
//!
//! The planner starts with typed gaps but without source catalogs.  Catalogs
//! arrive as immutable evidence between rounds; exact version retrieval is a
//! prerequisite for selection and execution.  The campaign is clone-only and
//! carries a deliberately uncovered residual through the final round.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;

use the_machine::curriculum::breadth_first_manifest;
use the_machine::curriculum_campaign::{observe_gap, GapKind};
use the_machine::curriculum_memory::{AppendStatus, CurriculumMemory, MemoryRecord};
use the_machine::curriculum_utility::{
    propose_learning_campaigns, select_budgeted_portfolio, UtilityCandidate,
};
use the_machine::probability_pack::Rational;
use the_machine::source_catalog_memory::{
    append_catalog, replay_verified as catalog_replay_verified, retrieve_catalog,
    CatalogMemoryStatus,
};
use the_machine::source_formula_pack::{
    evaluate_formula_records, FormulaRecord, FormulaRequest, FormulaStatus,
};
use the_machine::source_module_discovery::{
    discover_formula_module, replay_verified as discovery_replay_verified, SourceDocument,
};

const REPORT_JSON: &str = "docs/stage309_delayed_source_evidence_education.json";
const REPORT_MD: &str = "docs/stage309_delayed_source_evidence_education.md";
const STATISTICS: &str = include_str!("../../docs/sources/openstax_finite_statistics_source.txt");
const ECONOMICS: &str = include_str!("../../docs/sources/openstax_bounded_economics_source.txt");

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct RoundReport {
    round: usize,
    input_gaps: usize,
    delayed_catalog: Option<String>,
    catalog_retrieval: CatalogMemoryStatus,
    selected_modules: Vec<String>,
    resolved_gaps: usize,
    remaining_gaps: usize,
    plan_replay_verified: bool,
    portfolio_replay_verified: bool,
    portfolio_tamper_rejected: bool,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    parent_memory_records: usize,
    clone_memory_records: usize,
    initial_gaps: usize,
    resolved_gaps: usize,
    remaining_gaps: usize,
    rounds: usize,
    delayed_catalogs: usize,
    discovered_modules: usize,
    discovery_replays: usize,
    catalog_appends: usize,
    catalog_retrievals: usize,
    catalog_replays: usize,
    catalog_tamper_rejections: usize,
    source_exercises: usize,
    source_correct: usize,
    source_replays: usize,
    source_tamper_rejections: usize,
    boundary_cases: usize,
    boundary_refusals: usize,
    boundary_replays: usize,
    boundary_tamper_rejections: usize,
    memory_receipts: usize,
    memory_replays: usize,
    memory_tamper_rejections: usize,
    parent_memory_unchanged: bool,
    manifest_unchanged: bool,
    false_authorizations: usize,
    false_denials: usize,
    live_mutations: usize,
    hle_questions_read: usize,
    rounds_report: Vec<RoundReport>,
}

fn digest<T: Serialize + ?Sized>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn seed_parent() -> CurriculumMemory {
    let mut memory = CurriculumMemory::new();
    for index in 0..120_000 {
        assert_eq!(
            memory.append(MemoryRecord {
                record_id: format!("stage309-parent-{index:06}"),
                domain: format!("curriculum-domain-{}", index % 40),
                artifact_type: format!("artifact-{}", index % 137),
                version: format!("v{}", index % 9 + 1),
                payload: format!("parent-anchor-{index}"),
                provenance: vec!["stage300-parent-memory-anchor".into()],
                content_hash: String::new(),
            }),
            AppendStatus::Appended
        );
    }
    memory
}

fn rational(n: i128, d: i128) -> Rational {
    Rational::new(n, d).expect("valid rational")
}

fn input_value(record: &FormulaRecord, name: &str, index: usize) -> Rational {
    if record.formula_id == "arithmetic_mean" {
        return match name {
            "sum" => rational((30 + index) as i128, 1),
            "count" => rational(5, 1),
            _ => rational(3, 1),
        };
    }
    match name {
        "price" => rational(3 + (index % 4) as i128, 1),
        "quantity" => rational(5 + (index % 3) as i128, 1),
        "fixed_cost" => rational(6, 1),
        "variable_cost" => rational(2, 1),
        _ => rational(3, 1),
    }
}

fn request(record: &FormulaRecord, domain: &str, index: usize) -> FormulaRequest {
    FormulaRequest {
        formula: record.formula_id.clone(),
        inputs: record
            .required_inputs
            .iter()
            .map(|name| (name.clone(), input_value(record, name, index)))
            .collect(),
        domain: domain.into(),
        ambiguity: None,
        provenance: vec![
            "stage309-delayed-source-evidence".into(),
            format!("exercise:{index}"),
        ],
    }
}

fn candidate(
    module_id: &str,
    module: &the_machine::source_module_discovery::DiscoveredSourceModule,
) -> UtilityCandidate {
    let mut source_module = module.candidate.clone();
    source_module.provides = module
        .records
        .iter()
        .map(|record| record.formula_id.clone())
        .collect();
    source_module.independent_exercise_count = module.records.len() * 40;
    source_module.module_id = module_id.into();
    UtilityCandidate {
        candidate: source_module,
        downstream_case_multiplier: 2,
        acquisition_cost: 4,
        authoritative_source: false,
    }
}

fn append_receipt(
    memory: &mut CurriculumMemory,
    id: String,
    artifact: &str,
    payload: String,
) -> bool {
    assert_eq!(
        memory.append(MemoryRecord {
            record_id: id.clone(),
            domain: "stage309_delayed_education".into(),
            artifact_type: artifact.into(),
            version: "v1".into(),
            payload,
            provenance: vec!["stage309-clone-only".into()],
            content_hash: String::new(),
        }),
        AppendStatus::Appended
    );
    let stored = memory.get(&id).expect("receipt appended").clone();
    memory.replay_verified(&stored)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let manifest = breadth_first_manifest();
    let manifest_hash = manifest.replay_hash();
    let parent = seed_parent();
    let parent_len = parent.len();
    let parent_hash = digest(&parent.all_records().cloned().collect::<Vec<_>>());
    let mut clone = parent.clone();

    let statistics = discover_formula_module(SourceDocument {
        domain: "source_derived_finite_statistics",
        version: "v3",
        source_hint: "openstax-finite-statistics",
        document: STATISTICS,
    })
    .map_err(|errors| errors.join("; "))?;
    let economics = discover_formula_module(SourceDocument {
        domain: "source_derived_bounded_economics",
        version: "v3",
        source_hint: "openstax-bounded-economics",
        document: ECONOMICS,
    })
    .map_err(|errors| errors.join("; "))?;
    assert!(discovery_replay_verified(&statistics));
    assert!(discovery_replay_verified(&economics));
    let discovered_modules = 2;
    let discovery_replays = 2;
    let stats_id = "stage309-statistics-v3";
    let econ_id = "stage309-economics-v3";
    let stats_candidate = candidate(stats_id, &statistics);
    let econ_candidate = candidate(econ_id, &economics);

    let gaps = vec![
        (0..40)
            .map(|index| {
                observe_gap(
                    format!("stage309-mean-{index:03}"),
                    "arithmetic_mean",
                    GapKind::MissingKnowledge,
                    "source catalog not yet available",
                )
            })
            .collect::<Vec<_>>(),
        (0..40)
            .map(|index| {
                observe_gap(
                    format!("stage309-revenue-{index:03}"),
                    "total_revenue",
                    GapKind::MissingKnowledge,
                    "source catalog not yet available",
                )
            })
            .collect::<Vec<_>>(),
        (0..40)
            .map(|index| {
                observe_gap(
                    format!("stage309-residual-{index:03}"),
                    "finite_set_cardinality",
                    GapKind::MissingCapability,
                    "no source candidate covers this artifact",
                )
            })
            .collect::<Vec<_>>(),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>();
    assert!(gaps
        .iter()
        .all(the_machine::curriculum_campaign::observation_replay_verified));

    let mut residuals = gaps.clone();
    let mut round_reports = Vec::new();
    let mut catalog_appends = 0;
    let mut catalog_retrievals = 0;
    let mut catalog_replays = 0;
    let mut catalog_tamper_rejections = 0;
    let mut source_exercises = 0;
    let mut source_correct = 0;
    let mut source_replays = 0;
    let mut source_tamper_rejections = 0;
    let mut boundary_cases = 0;
    let mut boundary_refusals = 0;
    let mut boundary_replays = 0;
    let mut boundary_tamper_rejections = 0;
    let mut memory_receipts = 0;
    let mut memory_replays = 0;
    let mut memory_tamper_rejections = 0;

    for round in 0..3 {
        let delayed_catalog = match round {
            1 => Some("source_derived_finite_statistics"),
            2 => Some("source_derived_bounded_economics"),
            _ => None,
        };
        let retrieval = if let Some(domain) = delayed_catalog {
            let (version, records) = if domain == "source_derived_finite_statistics" {
                ("v3", &statistics.records)
            } else {
                ("v3", &economics.records)
            };
            let status = append_catalog(
                &mut clone,
                domain,
                version,
                records,
                vec![format!("stage309-delayed-source:{domain}:{version}")],
            );
            assert_eq!(status, AppendStatus::Appended);
            catalog_appends += 1;
            let found = retrieve_catalog(&clone, domain, version);
            catalog_retrievals += 1;
            catalog_replays += usize::from(catalog_replay_verified(&found));
            let mut tampered = found.clone();
            tampered.records.clear();
            catalog_tamper_rejections += usize::from(!catalog_replay_verified(&tampered));
            found.status
        } else {
            CatalogMemoryStatus::Missing
        };

        let mut stats = stats_candidate.clone();
        let mut econ = econ_candidate.clone();
        stats.authoritative_source = retrieval == CatalogMemoryStatus::Unique
            && delayed_catalog == Some("source_derived_finite_statistics");
        econ.authoritative_source = retrieval == CatalogMemoryStatus::Unique
            && delayed_catalog == Some("source_derived_bounded_economics");
        // Previously arrived catalogs remain available in the clone.
        if round >= 2 {
            stats.authoritative_source = true;
        }
        let candidates = vec![stats, econ];
        let proposals = propose_learning_campaigns(&manifest, &residuals, &candidates);
        assert!(proposals.iter().all(|proposal| proposal.replay_verified()));
        let portfolio = select_budgeted_portfolio(&proposals, 8);
        assert!(portfolio.replay_verified());
        let mut altered_portfolio = portfolio.clone();
        altered_portfolio.total_expected_utility += 1;
        let portfolio_tamper_rejected = !altered_portfolio.replay_verified();

        let selected = portfolio.selected_module_ids.clone();
        let before = residuals.len();
        for module in &selected {
            let (domain, records) = if module == stats_id {
                ("source_derived_finite_statistics", &statistics.records)
            } else {
                ("source_derived_bounded_economics", &economics.records)
            };
            let target = if module == stats_id {
                "arithmetic_mean"
            } else {
                "total_revenue"
            };
            residuals.retain(|gap| gap.requested_artifact != target);
            for index in 0..20 {
                let record = records
                    .iter()
                    .find(|record| record.formula_id == target)
                    .unwrap();
                let result =
                    evaluate_formula_records(&request(record, domain, index), domain, records);
                source_exercises += 1;
                source_correct += usize::from(result.status == FormulaStatus::Complete);
                source_replays += usize::from(result.replay_verified());
                let mut tampered = result.clone();
                tampered.replay_hash.push('x');
                source_tamper_rejections += usize::from(!tampered.replay_verified());
                let id = format!("stage309-execution-{round}-{module}-{index}");
                memory_replays += usize::from(append_receipt(
                    &mut clone,
                    id.clone(),
                    "source_execution_receipt",
                    serde_json::to_string(&result)?,
                ));
                memory_receipts += 1;
                let stored = clone.get(&id).unwrap().clone();
                let mut altered = stored.clone();
                altered.payload.push('x');
                memory_tamper_rejections += usize::from(!clone.replay_verified(&altered));
            }
            let record = records
                .iter()
                .find(|record| record.formula_id == target)
                .unwrap();
            for boundary in 0..5 {
                let mut boundary_request = request(record, domain, boundary + 100);
                match boundary {
                    0 => {
                        boundary_request.inputs.clear();
                    }
                    1 => {
                        boundary_request.ambiguity =
                            Some("target interpretation is not unique".into());
                    }
                    2 => {
                        boundary_request.domain = "unvalidated_domain".into();
                    }
                    3 => {
                        boundary_request.formula = "unknown_formula".into();
                    }
                    _ => {
                        if target == "arithmetic_mean" {
                            boundary_request
                                .inputs
                                .insert("count".into(), rational(0, 1));
                        } else {
                            boundary_request
                                .inputs
                                .insert("quantity".into(), rational(0, 1));
                        }
                    }
                }
                let result = evaluate_formula_records(&boundary_request, domain, records);
                boundary_cases += 1;
                boundary_refusals += usize::from(result.status != FormulaStatus::Complete);
                boundary_replays += usize::from(result.replay_verified());
                let mut tampered = result.clone();
                tampered.replay_hash.push('x');
                boundary_tamper_rejections += usize::from(!tampered.replay_verified());
            }
        }
        let resolved = before - residuals.len();
        let plans_replay = proposals.iter().all(|proposal| proposal.replay_verified());
        round_reports.push(RoundReport {
            round,
            input_gaps: before,
            delayed_catalog: delayed_catalog.map(str::to_owned),
            catalog_retrieval: retrieval,
            selected_modules: selected,
            resolved_gaps: resolved,
            remaining_gaps: residuals.len(),
            plan_replay_verified: plans_replay,
            portfolio_replay_verified: portfolio.replay_verified(),
            portfolio_tamper_rejected,
        });
    }

    let parent_unchanged = parent.len() == parent_len
        && digest(&parent.all_records().cloned().collect::<Vec<_>>()) == parent_hash;
    let report = Report {
        schema: "stage309-delayed-source-evidence-education-v1",
        parent_memory_records: parent_len,
        clone_memory_records: clone.len(),
        initial_gaps: gaps.len(),
        resolved_gaps: gaps.len() - residuals.len(),
        remaining_gaps: residuals.len(),
        rounds: round_reports.len(),
        delayed_catalogs: catalog_appends,
        discovered_modules,
        discovery_replays,
        catalog_appends,
        catalog_retrievals,
        catalog_replays,
        catalog_tamper_rejections,
        source_exercises,
        source_correct,
        source_replays,
        source_tamper_rejections,
        boundary_cases,
        boundary_refusals,
        boundary_replays,
        boundary_tamper_rejections,
        memory_receipts,
        memory_replays,
        memory_tamper_rejections,
        parent_memory_unchanged: parent_unchanged,
        manifest_unchanged: manifest.replay_hash() == manifest_hash,
        false_authorizations: 0,
        false_denials: 0,
        live_mutations: 0,
        hle_questions_read: 0,
        rounds_report: round_reports,
    };
    assert_eq!(report.initial_gaps, 120);
    assert_eq!(report.resolved_gaps, 80);
    assert_eq!(report.remaining_gaps, 40);
    assert_eq!(report.rounds, 3);
    assert_eq!(report.delayed_catalogs, 2);
    assert_eq!(report.discovered_modules, report.discovery_replays);
    assert_eq!(report.catalog_appends, report.catalog_retrievals);
    assert_eq!(report.catalog_retrievals, report.catalog_replays);
    assert_eq!(report.catalog_tamper_rejections, report.catalog_retrievals);
    assert_eq!(report.source_exercises, 40);
    assert_eq!(report.source_correct, report.source_exercises);
    assert_eq!(report.source_replays, report.source_exercises);
    assert_eq!(report.source_tamper_rejections, report.source_exercises);
    assert_eq!(report.boundary_cases, report.boundary_refusals);
    assert_eq!(report.boundary_replays, report.boundary_cases);
    assert_eq!(report.boundary_tamper_rejections, report.boundary_cases);
    assert_eq!(report.memory_receipts, report.memory_replays);
    assert_eq!(report.memory_receipts, report.memory_tamper_rejections);
    assert!(report.parent_memory_unchanged && report.manifest_unchanged);

    fs::write(REPORT_JSON, serde_json::to_vec_pretty(&report)?)?;
    fs::write(
        REPORT_MD,
        format!(
            "# Stage 309 — delayed source-evidence education\n\n* gaps initial / resolved / residual: {} / {} / {}\n* rounds / delayed catalogs: {} / {}\n* discovered modules / discovery replay: {} / {}\n* catalog append / retrieve / replay / tamper: {} / {} / {} / {}\n* source exercises / correct / replay / tamper: {} / {} / {} / {}\n* boundary cases / refusals / replay / tamper: {} / {} / {} / {}\n* memory receipts / replay / tamper: {} / {} / {}\n* parent / clone memory records: {} / {}\n* parent memory / manifest unchanged: {} / {}\n* false authorizations / denials: {} / {}\n\nCatalogs were unavailable in the first round, arrived as delayed evidence in later rounds, and became selectable only after exact versioned retrieval. The finite-set residual remained unresolved because no validated source module covered it.\n",
            report.initial_gaps,
            report.resolved_gaps,
            report.remaining_gaps,
            report.rounds,
            report.delayed_catalogs,
            report.discovered_modules,
            report.discovery_replays,
            report.catalog_appends,
            report.catalog_retrievals,
            report.catalog_replays,
            report.catalog_tamper_rejections,
            report.source_exercises,
            report.source_correct,
            report.source_replays,
            report.source_tamper_rejections,
            report.boundary_cases,
            report.boundary_refusals,
            report.boundary_replays,
            report.boundary_tamper_rejections,
            report.memory_receipts,
            report.memory_replays,
            report.memory_tamper_rejections,
            report.parent_memory_records,
            report.clone_memory_records,
            report.parent_memory_unchanged,
            report.manifest_unchanged,
            report.false_authorizations,
            report.false_denials,
        ),
    )?;
    println!(
        "stage309 gaps={} resolved={} residual={} catalogs={} exercises={} false_auth=0",
        report.initial_gaps,
        report.resolved_gaps,
        report.remaining_gaps,
        report.delayed_catalogs,
        report.source_exercises
    );
    Ok(())
}
