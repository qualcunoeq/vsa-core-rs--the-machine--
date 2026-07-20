use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use serde::{Deserialize, Serialize};
use serde_json;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs::{create_dir_all, File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};
use the_machine::cognition::{AblationConfig, AutonomyBudget, ExperimentResult};
use the_machine::development::{
    assess_calculus_funnel, assess_finite_math_funnel, assess_math_funnel, assess_mechanics_funnel,
    assess_number_theory_funnel, SupportAssessment,
};
use the_machine::diagnostic::{seed_diagnostic_knowledge, seed_error_classifier};
use the_machine::meta_reasoning::{assess, ReasoningState};
use the_machine::predictive::PredictiveCodingLoop;
use the_machine::qa::QaEngine;
use the_machine::Hypervector;
use the_machine::VSABrain;

const CASES: &[&str] = &[
    "qa-depth",
    "memory-pressure",
    "ablation-matrix",
    "adaptation",
    "temporal-abstraction",
    "meta-reasoning",
    "autonomy-budget",
    "chaos-run",
    "hard-adaptation",
    "adversarial-qa",
    "latency-slo",
    "transformer-cousin",
    "hle",
    "hle-funnel",
    "hle-math-funnel",
    "hle-finite-math-funnel",
    "hle-number-theory-funnel",
    "hle-calculus-funnel",
    "hle-regressions",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Scale {
    Small,
    Medium,
    Large,
    Max,
}

impl Scale {
    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "small" => Ok(Scale::Small),
            "medium" => Ok(Scale::Medium),
            "large" => Ok(Scale::Large),
            "max" => Ok(Scale::Max),
            _ => Err(format!("unknown scale '{}'", value)),
        }
    }

    fn qa_depths(self) -> Vec<usize> {
        match self {
            Scale::Small => vec![5, 10],
            Scale::Medium => vec![5, 10, 25],
            Scale::Large => vec![5, 10, 25, 50, 100],
            Scale::Max => vec![10, 25, 50, 100, 250],
        }
    }

    fn memory_items(self) -> usize {
        match self {
            Scale::Small => 1_000,
            Scale::Medium => 10_000,
            Scale::Large => 100_000,
            Scale::Max => 500_000,
        }
    }

    fn adaptation_episodes(self) -> usize {
        match self {
            Scale::Small => 40,
            Scale::Medium => 400,
            Scale::Large => 4_000,
            Scale::Max => 20_000,
        }
    }

    fn temporal_cycles(self) -> usize {
        match self {
            Scale::Small => 120,
            Scale::Medium => 1_000,
            Scale::Large => 10_000,
            Scale::Max => 50_000,
        }
    }

    fn meta_cases(self) -> usize {
        match self {
            Scale::Small => 50,
            Scale::Medium => 500,
            Scale::Large => 5_000,
            Scale::Max => 25_000,
        }
    }

    fn autonomy_steps(self) -> usize {
        match self {
            Scale::Small => 50,
            Scale::Medium => 500,
            Scale::Large => 5_000,
            Scale::Max => 25_000,
        }
    }

    fn cousin_repetitions(self) -> usize {
        match self {
            Scale::Small => 4,
            Scale::Medium => 32,
            Scale::Large => 256,
            Scale::Max => 1_024,
        }
    }
}

#[derive(Clone, Debug)]
struct BenchConfig {
    case: String,
    out: PathBuf,
    seed: u64,
    scale: Scale,
    threads: usize,
    duration_minutes: u64,
    promote_hle_regressions: Option<PathBuf>,
    commit: String,
}

/// One replayable HLE decision.  Written as JSONL alongside aggregate results
/// so incorrect non-abstentions can be inspected without rerunning 2,500
/// questions or guessing which knowledge path supplied an answer.
#[derive(Serialize)]
struct HleQuestionTrace {
    id: Option<String>,
    category: String,
    /// A stable split derived from the question id/text and benchmark seed.
    /// Development may be used to create regressions; held-out is reporting
    /// only and must not be used for tool/prompt tuning.
    split: String,
    failure_cluster: String,
    required_capabilities: Vec<String>,
    /// Local image files preserved for this replay.  An empty list means the
    /// source corpus exposed only `has_image`, not an inspectable attachment.
    attachment_paths: Vec<String>,
    question: String,
    route: String,
    source_store: Option<String>,
    retrieved_triple: Option<(String, String, String)>,
    confidence: f64,
    /// Deterministic decomposition and verification emitted by the specialist
    /// orchestrator.  These make an abstention actionable (bad route, missing
    /// givens, unsupported tool, or failed choice constraint).
    plan_givens: Vec<String>,
    plan_requested: Option<String>,
    plan_units: Vec<String>,
    plan_constraints: Vec<String>,
    plan_answer_choices: Vec<(String, String)>,
    plan_equations: Vec<String>,
    plan_assumptions: Vec<String>,
    plan_source_fragments: Vec<String>,
    /// Operations emitted by the typed planner, rather than a benchmark-side
    /// keyword guess.  This is the primary capability label for development
    /// failure clustering.
    plan_required_capabilities: Vec<String>,
    plan_unresolved: Vec<String>,
    plan_solver_input: String,
    plan_methods: Vec<String>,
    /// The authorized directed method edge—not merely a route or formula ID.
    planned_derivation: Option<the_machine::methods::PlannedDerivationTrace>,
    /// Execution artifact paired with the authorized edge.  This makes a
    /// numeric answer replayable without treating planning as proof.
    execution_receipt: Option<the_machine::methods::ExecutionReceipt>,
    depth_two_plan: Option<the_machine::methods::DerivationPlan>,
    plan_execution_receipt: Option<the_machine::methods::PlanExecutionReceipt>,
    rejected_candidates: Vec<the_machine::methods::RejectedCandidateTrace>,
    tool_attempts: Vec<String>,
    verification_evidence: Vec<String>,
    verification: String,
    abstention_reason: Option<String>,
    answer: String,
    expected: String,
    score: String,
}

/// HLE exports have appeared in several attachment shapes.  Accept only
/// explicit local path fields, then resolve them inside the benchmark's
/// attachment staging directory; a boolean `has_image` is never treated as
/// image evidence.
fn hle_attachment_paths(entry: &serde_json::Value) -> Vec<PathBuf> {
    fn collect(value: &serde_json::Value, output: &mut Vec<String>) {
        match value {
            serde_json::Value::String(path) => output.push(path.clone()),
            serde_json::Value::Array(values) => {
                for value in values {
                    collect(value, output);
                }
            }
            serde_json::Value::Object(object) => {
                for key in ["path", "local_path", "image_path", "file"] {
                    if let Some(value) = object.get(key) {
                        collect(value, output);
                    }
                }
            }
            _ => {}
        }
    }

    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let staging = std::env::var_os("HLE_ATTACHMENT_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| manifest.join("data/hle_attachments"));
    let Ok(staging) = staging.canonicalize() else {
        return Vec::new();
    };
    let mut raw = Vec::new();
    for key in [
        "image_path",
        "image_paths",
        "attachment",
        "attachments",
        "images",
    ] {
        if let Some(value) = entry.get(key) {
            collect(value, &mut raw);
        }
    }
    raw.sort();
    raw.dedup();
    raw.into_iter()
        .filter_map(|path| {
            let path = PathBuf::from(path);
            let candidate = if path.is_absolute() {
                path
            } else {
                staging.join(path)
            };
            candidate
                .canonicalize()
                .ok()
                .filter(|candidate| candidate.starts_with(&staging))
        })
        .collect()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum HleSplit {
    Development,
    HeldOut,
}

impl HleSplit {
    fn label(self) -> &'static str {
        match self {
            Self::Development => "development",
            Self::HeldOut => "held_out",
        }
    }
}

/// The case format deliberately keeps the expected answer out of held-out
/// artifacts.  Candidates come only from development failures, and become
/// permanent regression tests only after a human has reviewed a proposed fix.
#[derive(Clone, Deserialize, Serialize)]
struct HleRegressionCandidate {
    id: Option<String>,
    question: String,
    expected: String,
    route: String,
    skill: String,
    #[serde(default)]
    required_capabilities: Vec<String>,
    /// Attachments are replayed with the case so a vision fix cannot be
    /// accidentally promoted as an image-free text regression.
    #[serde(default)]
    attachment_paths: Vec<String>,
    failure_cluster: String,
    observed_score: String,
    /// Empty for an abstention candidate.  Filled only when promotion has
    /// independently re-run the router and accepted its evidence.
    #[serde(default)]
    verification_evidence: Vec<String>,
}

#[derive(Default)]
struct HleStats {
    correct: usize,
    abstained: usize,
    hallucinated: usize,
    total: usize,
}

impl HleStats {
    fn record(&mut self, score: &str) {
        self.total += 1;
        match score {
            "correct" => self.correct += 1,
            "abstained" => self.abstained += 1,
            _ => self.hallucinated += 1,
        }
    }

    fn accuracy(&self) -> f64 {
        ratio(self.correct, self.total)
    }

    fn abstention_rate(&self) -> f64 {
        ratio(self.abstained, self.total)
    }

    fn hallucination_rate(&self) -> f64 {
        ratio(self.hallucinated, self.total)
    }
}

impl BenchConfig {
    fn parse(args: &[String]) -> Result<Self, String> {
        let mut case: Option<String> = None;
        let mut out = PathBuf::from("/tmp/cognition_bench.jsonl");
        let mut seed = 1_u64;
        let mut scale = Scale::Small;
        let mut threads = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(1);
        let mut duration_minutes = 1_u64;
        let mut promote_hle_regressions = None;

        let mut i = 1;
        while i < args.len() {
            match args[i].as_str() {
                "--case" => {
                    i += 1;
                    case = args.get(i).cloned();
                }
                "--out" => {
                    i += 1;
                    out = args
                        .get(i)
                        .map(PathBuf::from)
                        .ok_or_else(|| "--out requires a path".to_string())?;
                }
                "--seed" => {
                    i += 1;
                    seed = args
                        .get(i)
                        .ok_or_else(|| "--seed requires a value".to_string())?
                        .parse()
                        .map_err(|_| "--seed must be an integer".to_string())?;
                }
                "--scale" => {
                    i += 1;
                    scale = Scale::parse(
                        args.get(i)
                            .ok_or_else(|| "--scale requires a value".to_string())?,
                    )?;
                }
                "--threads" => {
                    i += 1;
                    threads = args
                        .get(i)
                        .ok_or_else(|| "--threads requires a value".to_string())?
                        .parse()
                        .map_err(|_| "--threads must be an integer".to_string())?;
                }
                "--duration-minutes" => {
                    i += 1;
                    duration_minutes = args
                        .get(i)
                        .ok_or_else(|| "--duration-minutes requires a value".to_string())?
                        .parse()
                        .map_err(|_| "--duration-minutes must be an integer".to_string())?;
                }
                "--promote-hle-regressions" => {
                    i += 1;
                    promote_hle_regressions =
                        Some(args.get(i).map(PathBuf::from).ok_or_else(|| {
                            "--promote-hle-regressions requires a candidate JSONL path".to_string()
                        })?);
                }
                "--help" | "-h" => return Err(usage()),
                value if !value.starts_with('-') && case.is_none() => {
                    case = Some(value.to_string());
                }
                value => return Err(format!("unknown argument '{}'\n{}", value, usage())),
            }
            i += 1;
        }

        let case = case.unwrap_or_else(|| "all".to_string());
        if case != "all" && !CASES.contains(&case.as_str()) {
            return Err(format!("unknown case '{}'\n{}", case, usage()));
        }

        Ok(BenchConfig {
            case,
            out,
            seed,
            scale,
            threads: threads.max(1),
            duration_minutes,
            promote_hle_regressions,
            commit: current_commit(),
        })
    }
}

fn usage() -> String {
    format!(
        "usage: cognition_bench [CASE] [--case CASE] [--out PATH] [--seed N] [--scale small|medium|large|max] [--threads N] [--duration-minutes N] [--promote-hle-regressions CANDIDATES.jsonl]\n\ncases: all, {}",
        CASES.join(", ")
    )
}

fn current_commit() -> String {
    Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .and_then(|output| {
            if output.status.success() {
                Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
            } else {
                None
            }
        })
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "unknown".to_string())
}

fn write_results(path: &Path, results: &[ExperimentResult]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            create_dir_all(parent).map_err(|err| err.to_string())?;
        }
    }
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|err| err.to_string())?;
    for result in results {
        let line = serde_json::to_string(result).map_err(|err| err.to_string())?;
        writeln!(file, "{}", line).map_err(|err| err.to_string())?;
    }
    Ok(())
}

fn result(
    cfg: &BenchConfig,
    experiment: &str,
    claim: &str,
    baseline: &str,
    metrics: HashMap<String, f64>,
    passed: bool,
    notes: impl Into<String>,
) -> ExperimentResult {
    ExperimentResult {
        experiment: experiment.to_string(),
        claim: claim.to_string(),
        commit: cfg.commit.clone(),
        seed: cfg.seed,
        dataset: None,
        baseline: baseline.to_string(),
        metrics,
        passed,
        notes: notes.into(),
    }
}

fn metric_pairs(pairs: &[(&str, f64)]) -> HashMap<String, f64> {
    pairs.iter().map(|(k, v)| (k.to_string(), *v)).collect()
}

fn elapsed_ms(start: Instant) -> f64 {
    start.elapsed().as_secs_f64() * 1_000.0
}

fn synthetic_fact(i: usize) -> (String, String, String) {
    (
        format!("agent_{}", i),
        "observed".to_string(),
        format!("signal_{}", i),
    )
}

fn bench_qa_depth(cfg: &BenchConfig) -> Vec<ExperimentResult> {
    let mut results = Vec::new();
    for depth in cfg.scale.qa_depths() {
        let mut qa = QaEngine::new();
        qa.store_fact("node_0", "leads_to", "node_1", "seed");
        for i in 0..depth {
            qa.store_rule(
                &format!("node_{}", i),
                "leads_to",
                &format!("node_{}", i + 1),
                &format!("node_{}", i + 1),
                "leads_to",
                &format!("node_{}", i + 2),
                "synthetic_chain",
            );
        }
        for i in 0..depth {
            qa.store_fact(
                &format!("distractor_{}", i),
                "points_to",
                &format!("noise_{}", i),
                "distractor",
            );
        }

        let start = Instant::now();
        let derived = qa.forward_chain(0.75);
        let reached = qa.verify_fact(
            &format!("node_{}", depth),
            "leads_to",
            &format!("node_{}", depth + 1),
        );
        let episode = qa.answer_combined_episode("qa-depth", "Who observed signal_0?");
        let trace_coverage = if episode.term_traces.is_empty() {
            0.0
        } else {
            1.0
        };
        let latency = elapsed_ms(start);

        results.push(result(
            cfg,
            &format!("qa-depth-{}", depth),
            "C-008",
            "direct fact lookup",
            metric_pairs(&[
                ("chain_depth", depth as f64),
                ("derived_facts", derived as f64),
                ("accuracy", if reached.0 { 1.0 } else { 0.0 }),
                ("trace_coverage", trace_coverage),
                ("confidence", reached.1),
                ("avg_latency_ms", latency),
                ("p95_latency_ms", latency),
            ]),
            reached.0 && trace_coverage > 0.0,
            "synthetic causal chain with distractors",
        ));
    }
    results
}

fn bench_memory_pressure(cfg: &BenchConfig) -> Vec<ExperimentResult> {
    let mut qa = QaEngine::new();
    let n = cfg.scale.memory_items();
    let start = Instant::now();
    for i in 0..n {
        let (s, v, o) = synthetic_fact(i);
        qa.store_fact(&s, &v, &o, "memory-pressure");
    }

    let mut hits = 0;
    let probes = 100.min(n);
    for i in 0..probes {
        let idx = i * (n / probes.max(1)).max(1);
        let (s, v, o) = synthetic_fact(idx.min(n - 1));
        if qa.verify_fact(&s, &v, &o).0 {
            hits += 1;
        }
    }
    let latency = elapsed_ms(start);
    vec![result(
        cfg,
        "memory-pressure",
        "C-001",
        "unbounded append-only fact scan",
        metric_pairs(&[
            ("memory_items", n as f64),
            ("accuracy", hits as f64 / probes as f64),
            ("avg_latency_ms", latency / probes as f64),
            ("p95_latency_ms", latency),
            ("fact_count", qa.fact_count() as f64),
        ]),
        hits == probes,
        "synthetic fact insertion and sparse recall probes",
    )]
}

fn bench_ablation_matrix(cfg: &BenchConfig) -> Vec<ExperimentResult> {
    let variants = [
        ("full", AblationConfig::default()),
        (
            "no-trace",
            AblationConfig {
                use_trace: false,
                ..AblationConfig::default()
            },
        ),
        (
            "no-associations",
            AblationConfig {
                use_associations: false,
                ..AblationConfig::default()
            },
        ),
        (
            "no-abstraction",
            AblationConfig {
                use_abstraction: false,
                ..AblationConfig::default()
            },
        ),
        (
            "no-soft-projection",
            AblationConfig {
                use_soft_projection: false,
                ..AblationConfig::default()
            },
        ),
        (
            "no-self-model",
            AblationConfig {
                use_self_model: false,
                ..AblationConfig::default()
            },
        ),
        (
            "no-tool-memory",
            AblationConfig {
                use_tool_memory: false,
                ..AblationConfig::default()
            },
        ),
    ];

    variants
        .iter()
        .map(|(name, ablation)| {
            let mut qa = QaEngine::new();
            qa.store_fact("the_machine", "runs", "experiments", "ablation");
            let mut episode =
                qa.answer_combined_episode(format!("ablation-{}", name), "What runs experiments?");
            episode.ablations = ablation.clone();
            let trace_coverage = if ablation.use_trace && !episode.term_traces.is_empty() {
                1.0
            } else {
                0.0
            };
            let answer_ok = episode
                .answer
                .as_ref()
                .map(|answer| answer.contains("the_machine"))
                .unwrap_or(false);
            result(
                cfg,
                &format!("ablation-matrix-{}", name),
                "C-009",
                "full model",
                metric_pairs(&[
                    ("accuracy", if answer_ok { 1.0 } else { 0.0 }),
                    ("trace_coverage", trace_coverage),
                    ("confidence", episode.confidence),
                    ("memory_items", qa.fact_count() as f64),
                ]),
                answer_ok,
                format!("ablation flags recorded for {}", name),
            )
        })
        .collect()
}

fn bench_adaptation(cfg: &BenchConfig) -> Vec<ExperimentResult> {
    let mut qa = QaEngine::new();
    let episodes = cfg.scale.adaptation_episodes();
    let train = episodes / 2;
    let mut before_hits = 0;
    let mut after_hits = 0;

    for i in 0..train {
        let question = format!("Who solved task_{}?", i);
        if qa
            .answer_combined(&question)
            .contains(&format!("agent_{}", i))
        {
            before_hits += 1;
        }
        qa.store_fact(
            &format!("agent_{}", i),
            "solve",
            &format!("task_{}", i),
            "feedback",
        );
        let after = qa.answer_combined_episode(format!("after-{}", i), &question);
        if after.confidence > 0.0 {
            after_hits += 1;
        }
    }

    let mut regressions = 0;
    for i in 0..train {
        let ok = qa
            .answer_combined(&format!("Who solved task_{}?", i))
            .contains(&format!("agent_{}", i));
        if !ok {
            regressions += 1;
        }
    }

    vec![result(
        cfg,
        "adaptation",
        "C-009",
        "no post-answer update",
        metric_pairs(&[
            ("memory_items", episodes as f64),
            ("accuracy", after_hits as f64 / train.max(1) as f64),
            ("before_accuracy", before_hits as f64 / train.max(1) as f64),
            ("regression_rate", regressions as f64 / train.max(1) as f64),
            ("trace_coverage", 1.0),
        ]),
        after_hits > before_hits && regressions == 0,
        "synthetic feedback inserts facts and retests prior questions",
    )]
}

fn bench_hard_adaptation(cfg: &BenchConfig) -> Vec<ExperimentResult> {
    let mut qa = QaEngine::new();
    let episodes = cfg.scale.adaptation_episodes().max(20);
    let train = episodes / 2;
    let mut before_false_positives = 0;
    let mut after_hits = 0;
    let mut regressions = 0;
    let mut transfer_hits = 0;
    let start = Instant::now();

    for i in 0..train {
        let subject = format!("agent_{}", i);
        let task = format!("novel_task_{}", i);
        if qa.verify_fact(&subject, "solve", &task).0 {
            before_false_positives += 1;
        }
        qa.store_fact(&subject, "solve", &task, "hard-feedback");
        if qa.verify_fact(&subject, "solve", &task).0 {
            after_hits += 1;
        }

        // Near-miss facts make exact recall harder than simple confidence checks.
        qa.store_fact(
            &format!("agent_{}_decoy", i),
            "solve",
            &format!("novel_task_{}_decoy", i),
            "hard-feedback-decoy",
        );
    }

    for i in 0..train {
        let subject = format!("agent_{}", i);
        let task = format!("novel_task_{}", i);
        if !qa.verify_fact(&subject, "solve", &task).0 {
            regressions += 1;
        }
        if !qa
            .verify_fact(&subject, "solve", &format!("novel_task_{}_decoy", i))
            .0
        {
            transfer_hits += 1;
        }
    }

    let latency = elapsed_ms(start);
    let train_f = train.max(1) as f64;
    let before_rate = before_false_positives as f64 / train_f;
    let after_accuracy = after_hits as f64 / train_f;
    let regression_rate = regressions as f64 / train_f;
    let near_miss_rejection = transfer_hits as f64 / train_f;

    vec![result(
        cfg,
        "hard-adaptation",
        "C-009",
        "confidence-only adaptation check",
        metric_pairs(&[
            ("memory_items", qa.fact_count() as f64),
            ("before_false_positive_rate", before_rate),
            ("accuracy", after_accuracy),
            ("regression_rate", regression_rate),
            ("near_miss_rejection", near_miss_rejection),
            ("avg_latency_ms", latency / train_f),
            ("p95_latency_ms", latency),
        ]),
        before_rate <= 0.01
            && after_accuracy >= 0.98
            && regression_rate <= 0.01
            && near_miss_rejection >= 0.98,
        "exact pre/post adaptation with near-miss decoys and regression replay",
    )]
}

fn bench_adversarial_qa(cfg: &BenchConfig) -> Vec<ExperimentResult> {
    let mut qa = QaEngine::new();
    let n = match cfg.scale {
        Scale::Small => 16,
        Scale::Medium => 128,
        Scale::Large => 1_024,
        Scale::Max => 8_192,
    };
    let probes = match cfg.scale {
        Scale::Small => 16,
        Scale::Medium => 64,
        Scale::Large => 256,
        Scale::Max => 1_024,
    };
    let mut rng = StdRng::seed_from_u64(cfg.seed);
    let start = Instant::now();

    for i in 0..n {
        qa.store_fact(
            &format!("agent_{:05}", i),
            "observed",
            &format!("signal_{:05}", i),
            "adversarial-qa",
        );
        qa.store_fact(
            &format!("agent_{:05}_shadow", i),
            "observed",
            &format!("signal_{:05}_shadow", i),
            "adversarial-qa-shadow",
        );
    }

    let mut exact_hits = 0;
    let mut false_positives = 0;
    let mut answer_hits = 0;
    let mut answer_probes = 0;
    for _ in 0..probes {
        let idx = rng.gen_range(0..n);
        let subject = format!("agent_{:05}", idx);
        let object = format!("signal_{:05}", idx);
        if qa.verify_fact(&subject, "observed", &object).0 {
            exact_hits += 1;
        }
        if answer_probes < probes.min(32) {
            answer_probes += 1;
            if qa
                .answer_combined(&format!("Who observed {}?", object))
                .contains(&subject)
            {
                answer_hits += 1;
            }
        }
        if qa
            .verify_fact(&subject, "observed", &format!("signal_{:05}_wrong", idx))
            .0
        {
            false_positives += 1;
        }
    }

    let latency = elapsed_ms(start);
    let probes_f = probes.max(1) as f64;
    let answer_probes_f = answer_probes.max(1) as f64;
    let exact_accuracy = exact_hits as f64 / probes_f;
    let answer_accuracy = answer_hits as f64 / answer_probes_f;
    let false_positive_rate = false_positives as f64 / probes_f;

    vec![result(
        cfg,
        "adversarial-qa",
        "C-008",
        "clean synthetic lookup without near-miss negatives",
        metric_pairs(&[
            ("memory_items", qa.fact_count() as f64),
            ("exact_accuracy", exact_accuracy),
            ("answer_accuracy", answer_accuracy),
            ("answer_probes", answer_probes as f64),
            ("false_positive_rate", false_positive_rate),
            ("avg_latency_ms", latency / probes_f),
            ("p95_latency_ms", latency),
        ]),
        exact_accuracy >= 0.98 && answer_accuracy >= 0.80 && false_positive_rate <= 0.02,
        "near-collision subjects/objects plus explicit negative probes",
    )]
}

fn bench_latency_slo(cfg: &BenchConfig) -> Vec<ExperimentResult> {
    let mut results = Vec::new();
    for res in bench_qa_depth(cfg) {
        let depth = res.metric("chain_depth").unwrap_or(0.0);
        let p95 = res.metric("p95_latency_ms").unwrap_or(f64::INFINITY);
        let slo_ms = (depth * depth * 0.75).max(50.0);
        let mut metrics = res.metrics.clone();
        metrics.insert("slo_ms".to_string(), slo_ms);
        metrics.insert(
            "slo_ratio".to_string(),
            if slo_ms > 0.0 {
                p95 / slo_ms
            } else {
                f64::INFINITY
            },
        );
        results.push(result(
            cfg,
            &format!("latency-slo-qa-depth-{}", depth as usize),
            "C-008",
            "unbounded chain expansion",
            metrics,
            res.passed && p95 <= slo_ms,
            "quality gate for QA latency growth",
        ));
    }

    let mem = bench_memory_pressure(cfg)
        .into_iter()
        .next()
        .expect("memory pressure emits one result");
    let n = mem.metric("memory_items").unwrap_or(0.0);
    let p95 = mem.metric("p95_latency_ms").unwrap_or(f64::INFINITY);
    let slo_ms = (n.max(1.0).log10() * 2_000.0).max(5_000.0);
    let mut metrics = mem.metrics.clone();
    metrics.insert("slo_ms".to_string(), slo_ms);
    metrics.insert(
        "slo_ratio".to_string(),
        if slo_ms > 0.0 {
            p95 / slo_ms
        } else {
            f64::INFINITY
        },
    );
    results.push(result(
        cfg,
        "latency-slo-memory-pressure",
        "C-001",
        "linear memory scan",
        metrics,
        mem.passed && p95 <= slo_ms,
        "quality gate for retrieval latency growth",
    ));
    results
}

fn seed_transformer_cousin_world(qa: &mut QaEngine) {
    qa.store_fact("ada_lovelace", "write", "compiler_notes", "cousin-seed");
    qa.store_fact("grace_hopper", "debug", "compiler", "cousin-seed");
    qa.store_fact("alan_turing", "formalize", "computation", "cousin-seed");
    qa.store_fact("the_machine", "use", "hypervectors", "cousin-seed");

    qa.store_rule(
        "ada_lovelace",
        "write",
        "compiler_notes",
        "compiler_notes",
        "inspire",
        "software",
        "cousin-rule",
    );
    qa.store_rule(
        "compiler_notes",
        "inspire",
        "software",
        "software",
        "enable",
        "automation",
        "cousin-rule",
    );
    qa.store_rule(
        "the_machine",
        "use",
        "hypervectors",
        "hypervectors",
        "support",
        "bitwise_reasoning",
        "cousin-rule",
    );
}

fn ratio(count: usize, total: usize) -> f64 {
    count as f64 / total.max(1) as f64
}

fn bench_transformer_cousin(cfg: &BenchConfig) -> Vec<ExperimentResult> {
    let mut qa = QaEngine::new();
    seed_transformer_cousin_world(&mut qa);

    let repetitions = cfg.scale.cousin_repetitions();
    let start = Instant::now();

    let grounded_prompts = [
        ("Who wrote compiler_notes?", "ada_lovelace"),
        ("Who debugged compiler?", "grace_hopper"),
        ("Who formalized computation?", "alan_turing"),
        ("Who used hypervectors?", "the_machine"),
    ];

    let mut grounded_hits = 0;
    let mut trace_hits = 0;
    let mut unknown_hits = 0;
    let mut feedback_before_unknown = 0;
    let mut feedback_after_hits = 0;
    let mut multi_hop_hits = 0;

    for i in 0..repetitions {
        let (question, expected) = grounded_prompts[i % grounded_prompts.len()];
        let episode = qa.answer_combined_episode(format!("cousin-grounded-{}", i), question);
        let answer = episode.answer.as_deref().unwrap_or("");
        if answer.contains(expected) {
            grounded_hits += 1;
        }
        if !episode.term_traces.is_empty() {
            trace_hits += 1;
        }

        let unknown = qa.answer_combined(&format!("Who solved missing_task_{}?", i));
        if unknown.contains("do not know") {
            unknown_hits += 1;
        }

        let feedback_question = format!("Who solved feedback_task_{}?", i);
        let before = qa.answer_combined(&feedback_question);
        if before.contains("do not know") {
            feedback_before_unknown += 1;
        }
        qa.store_fact(
            &format!("feedback_agent_{}", i),
            "solve",
            &format!("feedback_task_{}", i),
            "cousin-feedback",
        );
        let after =
            qa.answer_combined_episode(format!("cousin-feedback-{}", i), &feedback_question);
        if after
            .answer
            .as_deref()
            .unwrap_or("")
            .contains(&format!("feedback_agent_{}", i))
        {
            feedback_after_hits += 1;
        }

        let chain = qa.answer_chain("What happened after ada_lovelace wrote compiler_notes?");
        if chain.contains("software") || chain.contains("automation") {
            multi_hop_hits += 1;
        }
    }

    let latency = elapsed_ms(start);
    let grounded_accuracy = ratio(grounded_hits, repetitions);
    let trace_coverage = ratio(trace_hits, repetitions);
    let unknown_rejection = ratio(unknown_hits, repetitions);
    let feedback_gain = ratio(feedback_after_hits, repetitions)
        - (1.0 - ratio(feedback_before_unknown, repetitions));
    let multi_hop_accuracy = ratio(multi_hop_hits, repetitions);
    let aggregate_score = (grounded_accuracy
        + trace_coverage
        + unknown_rejection
        + ratio(feedback_after_hits, repetitions)
        + multi_hop_accuracy)
        / 5.0;

    vec![result(
        cfg,
        "transformer-cousin",
        "C-011",
        "behavioral transformer reference suite",
        metric_pairs(&[
            ("memory_items", qa.fact_count() as f64),
            ("grounded_qa_accuracy", grounded_accuracy),
            ("multi_hop_accuracy", multi_hop_accuracy),
            ("unknown_rejection", unknown_rejection),
            ("feedback_before_unknown", ratio(feedback_before_unknown, repetitions)),
            ("feedback_after_accuracy", ratio(feedback_after_hits, repetitions)),
            ("feedback_gain", feedback_gain),
            ("trace_coverage", trace_coverage),
            ("aggregate_score", aggregate_score),
            ("avg_latency_ms", latency / repetitions.max(1) as f64),
            ("p95_latency_ms", latency),
        ]),
        grounded_accuracy >= 0.95
            && multi_hop_accuracy >= 0.95
            && unknown_rejection >= 0.95
            && ratio(feedback_after_hits, repetitions) >= 0.95
            && trace_coverage >= 0.95,
        "bounded transformer-like behavior suite: grounded answers, chains, abstention, feedback, traces",
    )]
}

/// The curated portion of the knowledge file uses the canonical SVO schema.
/// Later records are an auto-extraction dump with a different field layout;
/// they are not trusted enough to enter the benchmark's grounded fact store.
fn is_canonical_knowledge_fact(line: &str) -> bool {
    line.starts_with(r#"{"subject": "#)
        && line.contains(r#"", "verb": "#)
        && line.contains(r#"", "object": "#)
        && line.contains(r#"", "source": "#)
        && line.ends_with(r#""type": "fact"}"#)
}

/// Store a corpus entry only when it is a complete SVO fact.  Knowledge files
/// also contain metadata and partial extraction records; treating those as
/// empty facts pollutes every QA index without adding retrievable knowledge.
fn store_usable_knowledge_entry(qa: &mut QaEngine, entry: &serde_json::Value) -> bool {
    // The corpus interleaves extracted metadata with asserted knowledge.  Only
    // entries explicitly marked as facts are reliable SVO knowledge triples.
    if entry["type"].as_str() != Some("fact") {
        return false;
    }
    let Some(subject) = entry["subject"].as_str() else {
        return false;
    };
    let Some(verb) = entry["verb"].as_str() else {
        return false;
    };
    let Some(object) = entry["object"].as_str() else {
        return false;
    };
    if !QaEngine::has_usable_fact_fields(subject, verb, object) {
        return false;
    }

    let source = entry["source"].as_str().unwrap_or("knowledge");
    // A complete SVO shape makes this entry useful for candidate retrieval,
    // but does not independently establish that its source, assumptions, and
    // scope entail an arbitrary HLE question.  Keep the corpus searchable
    // while marking it as non-answering evidence until a record is promoted
    // through the curated provenance pipeline.
    qa.store_fact(
        subject,
        verb,
        object,
        &format!("candidate corpus import: {source}"),
    );
    true
}

/// A tiny stable hash avoids depending on HashMap's randomized hasher.  The
/// same corpus, seed, and question identity always receive the same split.
fn stable_hle_hash(seed: u64, identity: &str) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64 ^ seed;
    for byte in identity.bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

/// Reserve 20% for reporting.  Do not tune routing, formula caches, or answer
/// normalization based on this partition.
fn hle_split(seed: u64, id: Option<&str>, question: &str) -> HleSplit {
    let identity = id.unwrap_or(question);
    if stable_hle_hash(seed, identity) % 10 < 2 {
        HleSplit::HeldOut
    } else {
        HleSplit::Development
    }
}

/// A conservative, deterministic diagnosis for development failures.  It is a
/// triage label, not an answer signal: no held-out expected answer influences
/// it.  Multiple labels are allowed when a question needs a composition.
fn required_capabilities(route: &str, question: &str, has_image: bool) -> Vec<String> {
    let text = question.to_ascii_lowercase();
    let mut capabilities = Vec::new();
    // HLE has decorative image attachments.  Only label vision when the text
    // itself refers to a visual, or says that an attached image is being used.
    let visual_reference = ["image", "figure", "diagram", "graph", "chart", "table"]
        .iter()
        .any(|term| text.contains(term));
    let attached_visual_reference = has_image
        && ["shown", "pictured", "see the", "above", "below"]
            .iter()
            .any(|term| text.contains(term));
    if visual_reference || attached_visual_reference {
        capabilities.push("ocr_diagram".to_string());
    }
    match route {
        "Math" | "math_engine" => {
            let algebraic = [
                "solve",
                "equation",
                "simplify",
                "integral",
                "derivative",
                "polynomial",
                "prime",
            ]
            .iter()
            .any(|term| text.contains(term));
            capabilities.push(
                if algebraic {
                    "algebra_cas"
                } else {
                    "elementary_arithmetic"
                }
                .to_string(),
            );
        }
        "Physics" => {
            capabilities.push("numerical_physics".to_string());
        }
        "Chess" => capabilities.push("fen_chess".to_string()),
        "Code" => capabilities.push("code_execution".to_string()),
        "Vision" => capabilities.push("ocr_diagram".to_string()),
        "LifeScience"
            if ["chemical", "chemistry", "synthesis", "molecule", "reaction"]
                .iter()
                .any(|term| text.contains(term)) =>
        {
            capabilities.push("chemistry_synthesis".to_string())
        }
        _ => {
            capabilities.push("factual_retrieval".to_string());
        }
    }
    capabilities.sort();
    capabilities.dedup();
    capabilities
}

/// Replay only human-promoted, development-split regressions.  A case cannot
/// enter this suite until the current engine has already solved it exactly;
/// future tool changes must keep it correct and must not turn it into a wrong
/// non-abstention.  No held-out identity is ever read here.
fn bench_hle_regressions(cfg: &BenchConfig) -> Vec<ExperimentResult> {
    let path = Path::new("data/hle_regressions.jsonl");
    let Ok(file) = File::open(path) else {
        return vec![result(
            cfg,
            "hle-regressions",
            "No promoted development regressions yet.",
            "ok",
            metric_pairs(&[("cases", 0.0), ("correct", 0.0), ("incorrect", 0.0)]),
            true,
            "Run HLE, review development abstentions, then promote only fixed cases.",
        )];
    };
    let mut cases = 0usize;
    let mut correct = 0usize;
    let mut incorrect = 0usize;
    for line in BufReader::new(file).lines().map_while(Result::ok) {
        let Ok(case) = serde_json::from_str::<HleRegressionCandidate>(&line) else {
            continue;
        };
        if hle_split(cfg.seed, case.id.as_deref(), &case.question) != HleSplit::Development {
            continue;
        }
        cases += 1;
        if verified_regression_result(&case).is_some() {
            correct += 1;
        } else {
            incorrect += 1;
        }
    }
    vec![result(
        cfg,
        "hle-regressions",
        "Every promoted development case remains exactly correct.",
        if incorrect == 0 { "ok" } else { "regression" },
        metric_pairs(&[("cases", cases as f64), ("correct", correct as f64), ("incorrect", incorrect as f64)]),
        incorrect == 0,
        format!("Replayed {cases} development-only HLE regressions: {correct} correct, {incorrect} incorrect."),
    )]
}

/// A solver regression must prove two things on every replay: it produces the
/// benchmark's exact answer, and it still carries evidence accepted by the
/// router.  Exact text without evidence is a hallucination regression, not a
/// success; evidence without the exact answer is merely an abstention.
fn verified_regression_result(
    case: &HleRegressionCandidate,
) -> Option<the_machine::router::OrchestratedAnswer> {
    let attachments: Vec<PathBuf> = case.attachment_paths.iter().map(PathBuf::from).collect();
    let result = the_machine::router::QuestionRouter::orchestrate_with_attachments(
        &case.question,
        &attachments,
    );
    result
        .answer
        .as_deref()
        .filter(|answer| {
            the_machine::router::QuestionRouter::exact_answers_match(answer, &case.expected)
        })
        .filter(|_| !result.evidence.is_empty())?;
    Some(result)
}

fn promotion_eligible(candidate: &HleRegressionCandidate) -> bool {
    candidate.observed_score == "abstained" && !candidate.required_capabilities.is_empty()
}

fn hle_failure_cluster(route: &str, capabilities: &[String], score: &str) -> String {
    let capability = capabilities
        .first()
        .map(String::as_str)
        .unwrap_or("unclassified");
    format!("route={route};capability={capability};outcome={score}")
}

/// Promotion is deliberately explicit and rechecks every candidate with the
/// current engine.  A still-abstaining or incorrect development case can never
/// enter the permanent suite; held-out identities are rejected as well.
fn promote_fixed_hle_regressions(
    candidates_path: &Path,
    seed: u64,
) -> Result<(usize, usize), String> {
    let candidate_file = File::open(candidates_path).map_err(|err| err.to_string())?;
    let permanent_path = Path::new("data/hle_regressions.jsonl");
    let mut known = HashSet::new();
    if let Ok(existing) = File::open(permanent_path) {
        for line in BufReader::new(existing).lines().map_while(Result::ok) {
            if let Ok(case) = serde_json::from_str::<HleRegressionCandidate>(&line) {
                known.insert(case.id.unwrap_or(case.question));
            }
        }
    }
    let mut permanent = OpenOptions::new()
        .create(true)
        .append(true)
        .open(permanent_path)
        .map_err(|err| err.to_string())?;
    let mut checked = 0usize;
    let mut promoted = 0usize;
    for line in BufReader::new(candidate_file).lines().map_while(Result::ok) {
        let Ok(mut candidate) = serde_json::from_str::<HleRegressionCandidate>(&line) else {
            continue;
        };
        if hle_split(seed, candidate.id.as_deref(), &candidate.question) != HleSplit::Development {
            continue;
        }
        // Candidate files are a labelled *abstention* queue.  Incorrect
        // non-abstentions stay in traces/error clusters, never become solver
        // targets that could normalize a previous hallucination.
        if !promotion_eligible(&candidate) {
            continue;
        }
        checked += 1;
        if let Some(result) = verified_regression_result(&candidate) {
            let key = candidate
                .id
                .clone()
                .unwrap_or_else(|| candidate.question.clone());
            if known.insert(key) {
                candidate.observed_score = "fixed_verified".to_string();
                candidate.verification_evidence = result
                    .evidence
                    .iter()
                    .map(the_machine::router::VerificationEvidence::summary)
                    .collect();
                let line = serde_json::to_string(&candidate).map_err(|err| err.to_string())?;
                writeln!(permanent, "{}", line).map_err(|err| err.to_string())?;
                promoted += 1;
            }
        }
    }
    Ok((checked, promoted))
}

fn bench_hle(cfg: &BenchConfig) -> Vec<ExperimentResult> {
    // Humanity's Last Exam: score only the router's verified orchestration
    // result.  QaEngine retrieval may still be useful as a candidate source,
    // but it must never bypass the router's evidence and choice gates.
    let data_path = "data/hle.jsonl";
    let file = match File::open(data_path) {
        Ok(f) => BufReader::new(f),
        Err(e) => {
            return vec![result(
                cfg,
                "hle",
                "The machine can answer 0% of HLE questions but abstains reliably.",
                "error",
                metric_pairs(&[("error", 1.0)]),
                false,
                &format!("Cannot open {}: {}", data_path, e),
            )];
        }
    };

    let max_questions = match cfg.scale {
        Scale::Small => 50,
        Scale::Medium => 250,
        Scale::Large => 1000,
        Scale::Max => 2500,
    };
    // Allows an interrupted full HLE run to be resumed in deterministic,
    // disjoint batches without rewriting or filtering the source dataset.
    // The default remains the complete benchmark from question zero.
    let start_question = std::env::var("HLE_START")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(0);

    // Formula caches stay in their symbolic domain tools; they are not
    // ingested as fuzzy SVO facts.  Preload them here so an HLE result records
    // that the full validated physics/math formula stores were available.
    let (physics_cached, math_cached) =
        the_machine::router::QuestionRouter::preload_domain_knowledge();
    eprintln!(
        "       Loaded {} validated physics and {} validated math cache formulas (domain-gated).",
        physics_cached, math_cached
    );

    let trace_path = PathBuf::from(format!("{}.traces.jsonl", cfg.out.display()));
    if let Some(parent) = trace_path.parent() {
        if !parent.as_os_str().is_empty() {
            if let Err(err) = create_dir_all(parent) {
                return vec![result(
                    cfg,
                    "hle",
                    "HLE trace output must be writable for auditability.",
                    "error",
                    metric_pairs(&[("error", 1.0)]),
                    false,
                    format!("Cannot create HLE trace directory: {}", err),
                )];
            }
        }
    }
    let mut trace_file = match File::create(&trace_path) {
        Ok(file) => file,
        Err(err) => {
            return vec![result(
                cfg,
                "hle",
                "HLE trace output must be writable for auditability.",
                "error",
                metric_pairs(&[("error", 1.0)]),
                false,
                format!("Cannot create {}: {}", trace_path.display(), err),
            )]
        }
    };
    let regression_path =
        PathBuf::from(format!("{}.regression_candidates.jsonl", cfg.out.display()));
    let mut regression_file = match File::create(&regression_path) {
        Ok(file) => file,
        Err(err) => {
            return vec![result(
                cfg,
                "hle",
                "HLE regression candidates must be writable for review.",
                "error",
                metric_pairs(&[("error", 1.0)]),
                false,
                format!("Cannot create {}: {}", regression_path.display(), err),
            )]
        }
    };

    let start = Instant::now();

    let mut aggregate = HleStats::default();
    let mut development = HleStats::default();
    let mut held_out = HleStats::default();
    let mut category_stats: HashMap<String, (usize, usize, usize)> = HashMap::new(); // cat -> (correct, abstained, total)
    let mut failure_clusters: BTreeMap<String, usize> = BTreeMap::new();
    let mut development_samples: BTreeMap<String, Vec<HleRegressionCandidate>> = BTreeMap::new();

    for (source_index, line) in file.lines().enumerate() {
        if source_index < start_question {
            continue;
        }
        if aggregate.total >= max_questions {
            break;
        }
        let line = match line {
            Ok(l) => l,
            Err(_) => continue,
        };
        let entry: serde_json::Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(_) => continue,
        };

        let question = entry["question"].as_str().unwrap_or("");
        let expected = entry["answer"].as_str().unwrap_or("").trim();
        let category = entry["category"].as_str().unwrap_or("uncategorized");
        let has_image = entry["has_image"].as_bool().unwrap_or(false);
        let attachment_paths = hle_attachment_paths(&entry);
        // Note: some HLE questions have spurious image attachments (e.g. Q1181
        // "What is the largest prime divisor of 8139881" has has_image=true but
        // the image is decorative and the question is answerable without it).
        // We do NOT skip image-tagged questions — the math engine handles them.

        let orchestration = the_machine::router::QuestionRouter::orchestrate_with_attachments(
            question,
            &attachment_paths,
        );
        let answer = orchestration.answer.clone();
        let split = hle_split(cfg.seed, entry["id"].as_str(), question);

        let is_abstained = answer.is_none();
        // HLE exact-match answers must not be credited by substring ("14"
        // must not satisfy expected "4").  The shared normalizer tolerates
        // harmless casing, whitespace, punctuation, and LaTex wrappers.
        let is_correct = answer.as_deref().is_some_and(|answer| {
            the_machine::router::QuestionRouter::exact_answers_match(answer, expected)
        });

        let score = if is_correct {
            "correct"
        } else if is_abstained {
            "abstained"
        } else {
            "incorrect"
        };
        aggregate.record(score);
        match split {
            HleSplit::Development => development.record(score),
            HleSplit::HeldOut => held_out.record(score),
        }
        let route = if orchestration
            .attempts
            .iter()
            .any(|attempt| attempt == "MathEngine: solved")
        {
            "math_engine".to_string()
        } else {
            format!("{:?}", orchestration.plan.domain)
        };
        let source_store = orchestration
            .evidence
            .iter()
            .find_map(|evidence| match evidence {
                the_machine::router::VerificationEvidence::AuthoritativeSource { source } => {
                    Some(source.clone())
                }
                _ => None,
            });
        let retrieved_triple = None;
        let confidence = if is_abstained { 0.0 } else { 1.0 };
        // Capability labels are a development instrument.  Held-out questions
        // retain their aggregate score only; their content never enters a
        // labelled queue, cluster, or future regression fixture.
        let planned_capabilities: Vec<String> = orchestration
            .plan
            .required_capabilities
            .iter()
            .map(|capability| format!("{:?}", capability))
            .collect();
        let capabilities = if split == HleSplit::Development {
            if planned_capabilities.is_empty() {
                required_capabilities(&route, question, has_image)
            } else {
                planned_capabilities.clone()
            }
        } else {
            Vec::new()
        };
        let failure_cluster = hle_failure_cluster(&route, &capabilities, score);
        let skill = orchestration
            .plan
            .methods
            .first()
            .cloned()
            .unwrap_or_else(|| "no_plan".to_string());
        if split == HleSplit::Development && score != "correct" {
            *failure_clusters.entry(failure_cluster.clone()).or_default() += 1;
        }
        let trace = HleQuestionTrace {
            id: entry["id"].as_str().map(str::to_string),
            category: category.to_string(),
            split: split.label().to_string(),
            failure_cluster: failure_cluster.clone(),
            required_capabilities: capabilities.clone(),
            attachment_paths: attachment_paths
                .iter()
                .map(|path| path.display().to_string())
                .collect(),
            question: question.to_string(),
            route: route.clone(),
            source_store,
            retrieved_triple,
            confidence,
            plan_givens: orchestration.plan.givens.clone(),
            plan_requested: orchestration.plan.problem.requested.clone(),
            plan_units: orchestration.plan.problem.units.clone(),
            plan_constraints: orchestration.plan.problem.constraints.clone(),
            plan_answer_choices: orchestration.plan.problem.answer_choices.clone(),
            plan_equations: orchestration.plan.problem.equations.clone(),
            plan_assumptions: orchestration.plan.problem.assumptions.clone(),
            plan_source_fragments: orchestration.plan.problem.source_fragments.clone(),
            plan_required_capabilities: planned_capabilities,
            plan_unresolved: orchestration
                .plan
                .problem
                .unresolved
                .iter()
                .map(|reason| format!("{:?}", reason))
                .collect(),
            plan_solver_input: orchestration.plan.problem.solver_input.clone(),
            plan_methods: orchestration.plan.methods.clone(),
            planned_derivation: orchestration.planned_derivation.clone(),
            execution_receipt: orchestration.execution_receipt.clone(),
            depth_two_plan: orchestration.depth_two_plan.clone(),
            plan_execution_receipt: orchestration.plan_execution_receipt.clone(),
            rejected_candidates: orchestration.rejected_candidates.clone(),
            tool_attempts: orchestration.attempts.clone(),
            verification_evidence: orchestration
                .evidence
                .iter()
                .map(the_machine::router::VerificationEvidence::summary)
                .collect(),
            verification: orchestration.verification,
            abstention_reason: orchestration
                .abstention_reason
                .map(|reason| format!("{:?}", reason)),
            answer: answer
                .unwrap_or_else(|| "I do not know the answer to that question.".to_string()),
            expected: expected.to_string(),
            score: score.to_string(),
        };
        if let Ok(line) = serde_json::to_string(&trace) {
            // A trace failure must not silently create an aggregate-only run.
            if writeln!(trace_file, "{}", line).is_err() {
                eprintln!("HLE trace write failed at question {}", aggregate.total);
                break;
            }
        }
        // Only abstentions become solver-regression candidates.  Incorrect
        // non-abstentions remain visible in traces and clusters, but using
        // them as a promotion pool would risk teaching a previous hallucination
        // to pass by answer-key coincidence.
        if split == HleSplit::Development && score == "abstained" {
            let candidate = HleRegressionCandidate {
                id: entry["id"].as_str().map(str::to_string),
                question: question.to_string(),
                expected: expected.to_string(),
                route: route.clone(),
                skill,
                required_capabilities: capabilities.clone(),
                attachment_paths: attachment_paths
                    .iter()
                    .map(|path| path.display().to_string())
                    .collect(),
                failure_cluster,
                observed_score: score.to_string(),
                verification_evidence: Vec::new(),
            };
            // Keep a compact, deterministic, balanced review queue: the first
            // eight development failures for each primary capability.
            let sample_key = candidate
                .required_capabilities
                .first()
                .cloned()
                .unwrap_or_else(|| "unclassified".to_string());
            let samples = development_samples.entry(sample_key).or_default();
            if samples.len() < 8 {
                samples.push(candidate.clone());
            }
            if let Ok(line) = serde_json::to_string(&candidate) {
                if writeln!(regression_file, "{}", line).is_err() {
                    eprintln!(
                        "HLE regression candidate write failed at question {}",
                        aggregate.total
                    );
                    break;
                }
            }
        }

        let cat_entry = category_stats
            .entry(category.to_string())
            .or_insert((0, 0, 0));
        cat_entry.0 += if is_correct { 1 } else { 0 };
        cat_entry.1 += if is_abstained { 1 } else { 0 };
        cat_entry.2 += 1;
    }

    let latency = elapsed_ms(start);
    let trace_total = aggregate.total;
    let accuracy = aggregate.accuracy();
    let abstention_rate = aggregate.abstention_rate();
    let hallucination_rate = aggregate.hallucination_rate();

    let notes = format!(
        "HLE benchmark: {}/{} correct ({:.1}%), {} abstained ({:.1}%), {} hallucinated ({:.1}%)",
        aggregate.correct,
        trace_total,
        accuracy * 100.0,
        aggregate.abstained,
        abstention_rate * 100.0,
        aggregate.hallucinated,
        hallucination_rate * 100.0
    );

    // Log per-category breakdown
    eprintln!("\n── HLE Category Breakdown ──");
    let mut cats: Vec<_> = category_stats.into_iter().collect();
    cats.sort_by(|a, b| b.1 .2.cmp(&a.1 .2));
    for (cat, (corr, abst, tot)) in &cats {
        let cat_acc = ratio(*corr, *tot) * 100.0;
        let cat_abst = ratio(*abst, *tot) * 100.0;
        eprintln!(
            "  {:>25}: {:3} qs  acc={:5.1}%  abst={:5.1}%",
            cat, tot, cat_acc, cat_abst
        );
    }
    eprintln!("───────────────────────────\n");
    eprintln!(
        "       Wrote per-question traces to {}",
        trace_path.display()
    );
    eprintln!(
        "       Development: {}/{} correct; held-out: {}/{} correct.",
        development.correct, development.total, held_out.correct, held_out.total
    );
    eprintln!(
        "       Wrote development-only regression candidates to {}",
        regression_path.display()
    );
    let sample_path = PathBuf::from(format!("{}.development_samples.jsonl", cfg.out.display()));
    let mut sample_file = match File::create(&sample_path) {
        Ok(file) => file,
        Err(err) => {
            return vec![result(
                cfg,
                "hle",
                "HLE development samples must be writable for review.",
                "error",
                metric_pairs(&[("error", 1.0)]),
                false,
                format!("Cannot create {}: {}", sample_path.display(), err),
            )]
        }
    };
    for sample in development_samples.values().flatten() {
        if let Ok(line) = serde_json::to_string(sample) {
            if writeln!(sample_file, "{}", line).is_err() {
                return vec![result(
                    cfg,
                    "hle",
                    "HLE development samples must be writable for review.",
                    "error",
                    metric_pairs(&[("error", 1.0)]),
                    false,
                    format!("Cannot write {}", sample_path.display()),
                )];
            }
        }
    }
    eprintln!(
        "       Wrote balanced development review samples to {}",
        sample_path.display()
    );

    let cluster_path = PathBuf::from(format!("{}.error_clusters.jsonl", cfg.out.display()));
    let mut cluster_file = match File::create(&cluster_path) {
        Ok(file) => file,
        Err(err) => {
            return vec![result(
                cfg,
                "hle",
                "HLE failure clusters must be writable for auditability.",
                "error",
                metric_pairs(&[("error", 1.0)]),
                false,
                format!("Cannot create {}: {}", cluster_path.display(), err),
            )]
        }
    };
    for (cluster, count) in &failure_clusters {
        let record = serde_json::json!({ "cluster": cluster, "count": count });
        if writeln!(cluster_file, "{}", record).is_err() {
            return vec![result(
                cfg,
                "hle",
                "HLE failure clusters must be writable for auditability.",
                "error",
                metric_pairs(&[("error", 1.0)]),
                false,
                format!("Cannot write {}", cluster_path.display()),
            )];
        }
    }
    eprintln!(
        "       Wrote route/skill failure clusters to {}",
        cluster_path.display()
    );

    if let Some(candidates_path) = &cfg.promote_hle_regressions {
        match promote_fixed_hle_regressions(candidates_path, cfg.seed) {
            Ok((checked, promoted)) => eprintln!(
                "       Rechecked {} development candidates; promoted {} fixed cases to data/hle_regressions.jsonl.",
                checked, promoted
            ),
            Err(err) => eprintln!(
                "       Did not promote HLE regressions from {}: {}",
                candidates_path.display(), err
            ),
        }
    }

    let held_out_notes = format!(
        "Held-out HLE report: {}/{} correct ({:.1}%), {} abstained ({:.1}%), {} incorrect ({:.1}%). Do not tune on this split.",
        held_out.correct,
        held_out.total,
        held_out.accuracy() * 100.0,
        held_out.abstained,
        held_out.abstention_rate() * 100.0,
        held_out.hallucinated,
        held_out.hallucination_rate() * 100.0,
    );

    vec![
        result(
            cfg,
            "hle",
            "Held-out HLE score, abstention, and hallucination rate.",
            "baseline abstention",
            metric_pairs(&[
                ("accuracy", held_out.accuracy()),
                ("abstention_rate", held_out.abstention_rate()),
                ("hallucination_rate", held_out.hallucination_rate()),
                ("correct", held_out.correct as f64),
                ("abstained", held_out.abstained as f64),
                ("hallucinated", held_out.hallucinated as f64),
                ("total_questions", held_out.total as f64),
                ("avg_latency_ms", latency / trace_total.max(1) as f64),
                ("p95_latency_ms", latency),
            ]),
            // "Pass" means >90% abstention (the system knows its limits)
            held_out.abstention_rate() >= 0.90,
            held_out_notes,
        ),
        result(
            cfg,
            "hle-development",
            "Development HLE score used only to cluster failures and propose regressions.",
            "not a generalization metric",
            metric_pairs(&[
                ("accuracy", development.accuracy()),
                ("abstention_rate", development.abstention_rate()),
                ("hallucination_rate", development.hallucination_rate()),
                ("correct", development.correct as f64),
                ("abstained", development.abstained as f64),
                ("hallucinated", development.hallucinated as f64),
                ("total_questions", development.total as f64),
            ]),
            true,
            notes,
        ),
    ]
}

#[derive(Serialize)]
struct HleFunnelTrace {
    id: Option<String>,
    category: String,
    question: String,
    has_image: bool,
    #[serde(flatten)]
    assessment: the_machine::development::FunnelAssessment,
}

#[derive(Serialize)]
struct HleMathFunnelTrace {
    id: Option<String>,
    category: String,
    question: String,
    has_image: bool,
    #[serde(flatten)]
    assessment: the_machine::development::MathFunnelAssessment,
}

#[derive(Serialize)]
struct HleFiniteMathFunnelTrace {
    id: Option<String>,
    category: String,
    question: String,
    has_image: bool,
    #[serde(flatten)]
    assessment: the_machine::development::FiniteMathFunnelAssessment,
}

#[derive(Serialize)]
struct HleNumberTheoryFunnelTrace {
    id: Option<String>,
    category: String,
    question: String,
    has_image: bool,
    #[serde(flatten)]
    assessment: the_machine::development::NumberTheoryFunnelAssessment,
}

#[derive(Serialize)]
struct HleCalculusFunnelTrace {
    id: Option<String>,
    category: String,
    question: String,
    has_image: bool,
    #[serde(flatten)]
    assessment: the_machine::development::CalculusFunnelAssessment,
}

/// Non-executing coastline scan.  It deliberately calls only extraction and
/// typed registry planning; no specialist answer, retrieval, CAS, or image
/// tool is invoked.  This separates "outside the island" from an integration
/// failure on a question that should have been supported.
fn bench_hle_funnel(cfg: &BenchConfig) -> Vec<ExperimentResult> {
    let data_path = "data/hle.jsonl";
    let file = match File::open(data_path) {
        Ok(file) => BufReader::new(file),
        Err(err) => {
            return vec![result(
                cfg,
                "hle-funnel",
                "HLE funnel scan could not open the benchmark",
                "error",
                metric_pairs(&[("error", 1.0)]),
                false,
                err.to_string(),
            )]
        }
    };
    let max_questions = match cfg.scale {
        Scale::Small => 50,
        Scale::Medium => 500,
        Scale::Large => 1_000,
        Scale::Max => 2_500,
    };
    let output = PathBuf::from(format!("{}.funnel.jsonl", cfg.out.display()));
    let mut output_file = match File::create(&output) {
        Ok(file) => file,
        Err(err) => {
            return vec![result(
                cfg,
                "hle-funnel",
                "HLE funnel output is not writable",
                "error",
                metric_pairs(&[("error", 1.0)]),
                false,
                err.to_string(),
            )]
        }
    };
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    let mut mechanics_candidates = 0usize;
    let mut near_contract = 0usize;
    let mut inspected = 0usize;
    let mut top_candidates = Vec::new();
    for line in file.lines().take(max_questions) {
        let Ok(line) = line else { continue };
        let Ok(entry) = serde_json::from_str::<serde_json::Value>(&line) else {
            continue;
        };
        let question = entry["question"].as_str().unwrap_or("").to_string();
        let assessment = assess_mechanics_funnel(&question);
        let label = assessment.assessment.label().to_string();
        *counts.entry(label).or_default() += 1;
        inspected += 1;
        if assessment.assessment != SupportAssessment::OutsideDomain {
            mechanics_candidates += 1;
            if assessment.near_supported_contract {
                near_contract += 1;
            }
            if top_candidates.len() < 50 {
                top_candidates.push((
                    entry["id"].as_str().unwrap_or("?").to_string(),
                    assessment.assessment.label().to_string(),
                    question.clone(),
                ));
            }
        }
        let trace = HleFunnelTrace {
            id: entry["id"].as_str().map(str::to_string),
            category: entry["category"]
                .as_str()
                .unwrap_or("uncategorized")
                .to_string(),
            question,
            has_image: entry["has_image"].as_bool().unwrap_or(false),
            assessment,
        };
        if let Ok(serialized) = serde_json::to_string(&trace) {
            let _ = writeln!(output_file, "{serialized}");
        }
    }
    eprintln!("\n── HLE Mechanics Funnel ──");
    eprintln!("Scanned {inspected} questions; mechanics-signal candidates: {mechanics_candidates}; near current contract: {near_contract}");
    for (label, count) in &counts {
        eprintln!("  {label}: {count}");
    }
    eprintln!("Top mechanics candidates (first {}):", top_candidates.len());
    for (id, label, question) in &top_candidates {
        eprintln!("  {id} [{label}] {question}");
    }
    let mut metrics = HashMap::new();
    metrics.insert("questions_scanned".to_string(), inspected as f64);
    metrics.insert(
        "mechanics_candidates".to_string(),
        mechanics_candidates as f64,
    );
    metrics.insert("near_supported_contract".to_string(), near_contract as f64);
    for (label, count) in counts {
        metrics.insert(format!("assessment_{label}"), count as f64);
    }
    vec![result(
        cfg,
        "hle-funnel",
        "Non-executing HLE support-boundary reconnaissance",
        "no solver execution",
        metrics,
        true,
        format!("wrote funnel traces to {}", output.display()),
    )]
}

/// Non-executing mathematics reconnaissance.  This pass deliberately does
/// not call the algebra engine: it only measures which HLE prompts expose a
/// bounded operation and therefore justify building a typed math island.
fn bench_hle_math_funnel(cfg: &BenchConfig) -> Vec<ExperimentResult> {
    let data_path = "data/hle.jsonl";
    let file = match File::open(data_path) {
        Ok(file) => BufReader::new(file),
        Err(err) => {
            return vec![result(
                cfg,
                "hle-math-funnel",
                "HLE math funnel scan could not open the benchmark",
                "error",
                metric_pairs(&[("error", 1.0)]),
                false,
                err.to_string(),
            )]
        }
    };
    let max_questions = match cfg.scale {
        Scale::Small => 50,
        Scale::Medium => 500,
        Scale::Large => 1_000,
        Scale::Max => 2_500,
    };
    let output = PathBuf::from(format!("{}.math-funnel.jsonl", cfg.out.display()));
    let mut output_file = match File::create(&output) {
        Ok(file) => file,
        Err(err) => {
            return vec![result(
                cfg,
                "hle-math-funnel",
                "HLE math funnel output is not writable",
                "error",
                metric_pairs(&[("error", 1.0)]),
                false,
                err.to_string(),
            )]
        }
    };
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    let mut category_counts: BTreeMap<String, usize> = BTreeMap::new();
    let mut candidates = 0usize;
    let mut inspected = 0usize;
    let mut top_candidates = Vec::new();
    for line in file.lines().take(max_questions) {
        let Ok(line) = line else { continue };
        let Ok(entry) = serde_json::from_str::<serde_json::Value>(&line) else {
            continue;
        };
        let question = entry["question"].as_str().unwrap_or("").to_string();
        let category = entry["category"].as_str().unwrap_or("uncategorized");
        let assessment = assess_math_funnel(&question, category);
        *counts
            .entry(assessment.task_kind.label().to_string())
            .or_default() += 1;
        if assessment.math_signal {
            *category_counts.entry(category.to_string()).or_default() += 1;
        }
        inspected += 1;
        if assessment.executor_candidate {
            candidates += 1;
            if top_candidates.len() < 50 {
                top_candidates.push((
                    entry["id"].as_str().unwrap_or("?").to_string(),
                    assessment.task_kind.label().to_string(),
                    question.clone(),
                ));
            }
        }
        let trace = HleMathFunnelTrace {
            id: entry["id"].as_str().map(str::to_string),
            category: category.to_string(),
            question,
            has_image: entry["has_image"].as_bool().unwrap_or(false),
            assessment,
        };
        if let Ok(serialized) = serde_json::to_string(&trace) {
            let _ = writeln!(output_file, "{serialized}");
        }
    }
    eprintln!("\n── HLE Mathematics Funnel ──");
    eprintln!("Scanned {inspected} questions; math-operation candidates: {candidates}");
    eprintln!("Task distribution:");
    for (kind, count) in &counts {
        eprintln!("  {kind}: {count}");
    }
    eprintln!("Math-signal source categories:");
    for (category, count) in &category_counts {
        eprintln!("  {category}: {count}");
    }
    eprintln!("Executable candidates (first {}):", top_candidates.len());
    for (id, kind, question) in &top_candidates {
        eprintln!("  {id} [{kind}] {question}");
    }
    let mut metrics = HashMap::new();
    metrics.insert("questions_scanned".to_string(), inspected as f64);
    metrics.insert("executor_candidates".to_string(), candidates as f64);
    for (kind, count) in counts {
        metrics.insert(format!("task_{kind}"), count as f64);
    }
    for (category, count) in category_counts {
        metrics.insert(
            format!("source_category_{}", category.replace(['/', ' ', '-'], "_")),
            count as f64,
        );
    }
    vec![result(
        cfg,
        "hle-math-funnel",
        "Non-executing mathematics task-family reconnaissance",
        "no solver execution",
        metrics,
        true,
        format!("wrote math funnel traces to {}", output.display()),
    )]
}

/// Detailed non-executing reconnaissance for the finite-combinatorics slice
/// identified by the broad math funnel.  It deliberately invokes no solver:
/// the output measures whether sampling/order/replacement semantics are
/// explicit enough to authorize a future exact operation.
fn bench_hle_finite_math_funnel(cfg: &BenchConfig) -> Vec<ExperimentResult> {
    let data_path = "data/hle.jsonl";
    let file = match File::open(data_path) {
        Ok(file) => BufReader::new(file),
        Err(err) => {
            return vec![result(
                cfg,
                "hle-finite-math-funnel",
                "finite-math funnel could not open benchmark",
                "error",
                metric_pairs(&[("error", 1.0)]),
                false,
                err.to_string(),
            )]
        }
    };
    let max_questions = match cfg.scale {
        Scale::Small => 250,
        Scale::Medium => 500,
        Scale::Large => 1_000,
        Scale::Max => 2_500,
    };
    let output = PathBuf::from(format!("{}.finite-math-funnel.jsonl", cfg.out.display()));
    let mut output_file = match File::create(&output) {
        Ok(file) => file,
        Err(err) => {
            return vec![result(
                cfg,
                "hle-finite-math-funnel",
                "finite-math funnel output is not writable",
                "error",
                metric_pairs(&[("error", 1.0)]),
                false,
                err.to_string(),
            )]
        }
    };
    let mut task_counts: BTreeMap<String, usize> = BTreeMap::new();
    let mut support_counts: BTreeMap<String, usize> = BTreeMap::new();
    let mut inspected = 0usize;
    let mut bounded = 0usize;
    let mut explicit_uniform = 0usize;
    let mut candidates = Vec::new();
    for line in file.lines().take(max_questions) {
        let Ok(line) = line else { continue };
        let Ok(entry) = serde_json::from_str::<serde_json::Value>(&line) else {
            continue;
        };
        let question = entry["question"].as_str().unwrap_or("").to_string();
        let category = entry["category"].as_str().unwrap_or("uncategorized");
        let broad = assess_math_funnel(&question, category);
        if broad.task_kind != the_machine::development::MathTaskKind::FiniteCombinatorics {
            continue;
        }
        let assessment = assess_finite_math_funnel(&question, category);
        inspected += 1;
        *task_counts
            .entry(assessment.task_kind.label().to_string())
            .or_default() += 1;
        *support_counts
            .entry(assessment.support.label().to_string())
            .or_default() += 1;
        if assessment.bounded_operation {
            bounded += 1;
        }
        if assessment.uniformity_explicit {
            explicit_uniform += 1;
        }
        if assessment.bounded_operation && candidates.len() < 100 {
            candidates.push((
                entry["id"].as_str().unwrap_or("?").to_string(),
                assessment.task_kind.label().to_string(),
                question.clone(),
            ));
        }
        let trace = HleFiniteMathFunnelTrace {
            id: entry["id"].as_str().map(str::to_string),
            category: category.to_string(),
            question,
            has_image: entry["has_image"].as_bool().unwrap_or(false),
            assessment,
        };
        if let Ok(serialized) = serde_json::to_string(&trace) {
            let _ = writeln!(output_file, "{serialized}");
        }
    }
    eprintln!("\n── HLE Finite Mathematics Funnel ──");
    eprintln!("Inspected {inspected} finite-combinatorics prompts; explicit bounded candidates: {bounded}; explicit uniformity: {explicit_uniform}");
    eprintln!("Task distribution:");
    for (kind, count) in &task_counts {
        eprintln!("  {kind}: {count}");
    }
    eprintln!("Support distribution:");
    for (support, count) in &support_counts {
        eprintln!("  {support}: {count}");
    }
    eprintln!("Bounded candidates (first {}):", candidates.len());
    for (id, kind, question) in &candidates {
        eprintln!("  {id} [{kind}] {question}");
    }
    let mut metrics = HashMap::new();
    metrics.insert("finite_prompts_scanned".to_string(), inspected as f64);
    metrics.insert("explicit_bounded_operation".to_string(), bounded as f64);
    metrics.insert("uniformity_explicit".to_string(), explicit_uniform as f64);
    for (kind, count) in task_counts {
        metrics.insert(format!("task_{kind}"), count as f64);
    }
    for (support, count) in support_counts {
        metrics.insert(format!("support_{support}"), count as f64);
    }
    vec![result(
        cfg,
        "hle-finite-math-funnel",
        "Non-executing finite mathematics support reconnaissance",
        "no solver execution",
        metrics,
        true,
        format!("wrote finite-math funnel traces to {}", output.display()),
    )]
}

/// Non-executing number-theory reconnaissance over the broad funnel's
/// 182-number-theory slice.  Explicit operands and a target are required
/// before a prompt is considered a bounded arithmetic candidate.
fn bench_hle_number_theory_funnel(cfg: &BenchConfig) -> Vec<ExperimentResult> {
    let file = match File::open("data/hle.jsonl") {
        Ok(file) => BufReader::new(file),
        Err(err) => {
            return vec![result(
                cfg,
                "hle-number-theory-funnel",
                "number-theory funnel could not open benchmark",
                "error",
                metric_pairs(&[("error", 1.0)]),
                false,
                err.to_string(),
            )]
        }
    };
    let max_questions = match cfg.scale {
        Scale::Small => 250,
        Scale::Medium => 500,
        Scale::Large => 1_000,
        Scale::Max => 2_500,
    };
    let output = PathBuf::from(format!("{}.number-theory-funnel.jsonl", cfg.out.display()));
    let mut output_file = match File::create(&output) {
        Ok(file) => file,
        Err(err) => {
            return vec![result(
                cfg,
                "hle-number-theory-funnel",
                "number-theory funnel output is not writable",
                "error",
                metric_pairs(&[("error", 1.0)]),
                false,
                err.to_string(),
            )]
        }
    };
    let mut task_counts: BTreeMap<String, usize> = BTreeMap::new();
    let mut support_counts: BTreeMap<String, usize> = BTreeMap::new();
    let mut inspected = 0usize;
    let mut bounded = 0usize;
    let mut top_candidates = Vec::new();
    for line in file.lines().take(max_questions) {
        let Ok(line) = line else { continue };
        let Ok(entry) = serde_json::from_str::<serde_json::Value>(&line) else {
            continue;
        };
        let question = entry["question"].as_str().unwrap_or("").to_string();
        let category = entry["category"].as_str().unwrap_or("uncategorized");
        let broad = assess_math_funnel(&question, category);
        if broad.task_kind != the_machine::development::MathTaskKind::NumberTheory {
            continue;
        }
        let assessment = assess_number_theory_funnel(&question, category);
        inspected += 1;
        *task_counts
            .entry(assessment.task_kind.label().to_string())
            .or_default() += 1;
        *support_counts
            .entry(assessment.support.label().to_string())
            .or_default() += 1;
        if assessment.support
            == the_machine::development::NumberTheorySupportAssessment::ExplicitBoundedComputation
        {
            bounded += 1;
            if top_candidates.len() < 100 {
                top_candidates.push((
                    entry["id"].as_str().unwrap_or("?").to_string(),
                    assessment.task_kind.label().to_string(),
                    question.clone(),
                ));
            }
        }
        let trace = HleNumberTheoryFunnelTrace {
            id: entry["id"].as_str().map(str::to_string),
            category: category.to_string(),
            question,
            has_image: entry["has_image"].as_bool().unwrap_or(false),
            assessment,
        };
        if let Ok(serialized) = serde_json::to_string(&trace) {
            let _ = writeln!(output_file, "{serialized}");
        }
    }
    eprintln!("\n── HLE Number Theory Funnel ──");
    eprintln!(
        "Inspected {inspected} number-theory prompts; explicit bounded candidates: {bounded}"
    );
    eprintln!("Task distribution:");
    for (kind, count) in &task_counts {
        eprintln!("  {kind}: {count}");
    }
    eprintln!("Support distribution:");
    for (support, count) in &support_counts {
        eprintln!("  {support}: {count}");
    }
    for (id, kind, question) in &top_candidates {
        eprintln!("  {id} [{kind}] {question}");
    }
    let mut metrics = HashMap::new();
    metrics.insert(
        "number_theory_prompts_scanned".to_string(),
        inspected as f64,
    );
    metrics.insert("explicit_bounded_computation".to_string(), bounded as f64);
    for (kind, count) in task_counts {
        metrics.insert(format!("task_{kind}"), count as f64);
    }
    for (support, count) in support_counts {
        metrics.insert(format!("support_{support}"), count as f64);
    }
    vec![result(
        cfg,
        "hle-number-theory-funnel",
        "Non-executing number-theory support reconnaissance",
        "no solver execution",
        metrics,
        true,
        format!("wrote number-theory funnel traces to {}", output.display()),
    )]
}

fn bench_hle_calculus_funnel(cfg: &BenchConfig) -> Vec<ExperimentResult> {
    let file = match File::open("data/hle.jsonl") {
        Ok(file) => BufReader::new(file),
        Err(err) => {
            return vec![result(
                cfg,
                "hle-calculus-funnel",
                "calculus funnel could not open benchmark",
                "error",
                metric_pairs(&[("error", 1.0)]),
                false,
                err.to_string(),
            )]
        }
    };
    let max_questions = match cfg.scale {
        Scale::Small => 250,
        Scale::Medium => 500,
        Scale::Large => 1_000,
        Scale::Max => 2_500,
    };
    let output = PathBuf::from(format!("{}.calculus-funnel.jsonl", cfg.out.display()));
    let mut output_file = match File::create(&output) {
        Ok(file) => file,
        Err(err) => {
            return vec![result(
                cfg,
                "hle-calculus-funnel",
                "calculus funnel output is not writable",
                "error",
                metric_pairs(&[("error", 1.0)]),
                false,
                err.to_string(),
            )]
        }
    };
    let mut task_counts: BTreeMap<String, usize> = BTreeMap::new();
    let mut support_counts: BTreeMap<String, usize> = BTreeMap::new();
    let mut inspected = 0usize;
    let mut bounded = 0usize;
    let mut candidates = Vec::new();
    for line in file.lines().take(max_questions) {
        let Ok(line) = line else { continue };
        let Ok(entry) = serde_json::from_str::<serde_json::Value>(&line) else {
            continue;
        };
        let question = entry["question"].as_str().unwrap_or("").to_string();
        let category = entry["category"].as_str().unwrap_or("uncategorized");
        let broad = assess_math_funnel(&question, category);
        if broad.task_kind != the_machine::development::MathTaskKind::ElementaryCalculus {
            continue;
        }
        let assessment = assess_calculus_funnel(&question, category);
        inspected += 1;
        *task_counts
            .entry(assessment.task_kind.label().to_string())
            .or_default() += 1;
        *support_counts
            .entry(assessment.support.label().to_string())
            .or_default() += 1;
        if assessment.support
            == the_machine::development::CalculusSupportAssessment::ExplicitBoundedOperation
        {
            bounded += 1;
            if candidates.len() < 100 {
                candidates.push((
                    entry["id"].as_str().unwrap_or("?").to_string(),
                    assessment.task_kind.label().to_string(),
                    question.clone(),
                ));
            }
        }
        let trace = HleCalculusFunnelTrace {
            id: entry["id"].as_str().map(str::to_string),
            category: category.to_string(),
            question,
            has_image: entry["has_image"].as_bool().unwrap_or(false),
            assessment,
        };
        if let Ok(serialized) = serde_json::to_string(&trace) {
            let _ = writeln!(output_file, "{serialized}");
        }
    }
    eprintln!("\n── HLE Calculus Funnel ──");
    eprintln!("Inspected {inspected} calculus prompts; explicit bounded candidates: {bounded}");
    eprintln!("Task distribution:");
    for (kind, count) in &task_counts {
        eprintln!("  {kind}: {count}");
    }
    eprintln!("Support distribution:");
    for (support, count) in &support_counts {
        eprintln!("  {support}: {count}");
    }
    for (id, kind, question) in &candidates {
        eprintln!("  {id} [{kind}] {question}");
    }
    let mut metrics = HashMap::new();
    metrics.insert("calculus_prompts_scanned".to_string(), inspected as f64);
    metrics.insert("explicit_bounded_operation".to_string(), bounded as f64);
    for (kind, count) in task_counts {
        metrics.insert(format!("task_{kind}"), count as f64);
    }
    for (support, count) in support_counts {
        metrics.insert(format!("support_{support}"), count as f64);
    }
    vec![result(
        cfg,
        "hle-calculus-funnel",
        "Non-executing calculus support reconnaissance",
        "no solver execution",
        metrics,
        true,
        format!("wrote calculus funnel traces to {}", output.display()),
    )]
}

fn bench_temporal_abstraction(cfg: &BenchConfig) -> Vec<ExperimentResult> {
    let cycles = cfg.scale.temporal_cycles();
    let mut loop_state = PredictiveCodingLoop::new(1_000, 32, 8);
    let states: Vec<Hypervector> = (0..8)
        .map(|i| Hypervector::encode_text_ngram(&format!("state_{}", i), 3))
        .collect();
    let mut total_error = 0.0;
    let mut rng = StdRng::seed_from_u64(cfg.seed);
    let start = Instant::now();

    for i in 0..cycles {
        let regime_offset = if i < cycles / 2 { 0 } else { 3 };
        let mut c_idx = (i + regime_offset) % states.len();
        if rng.gen_bool(0.05) {
            c_idx = rng.gen_range(0..states.len());
        }
        total_error += loop_state.cycle(&states[c_idx], c_idx, Some(c_idx % 3), 0.5);
    }

    let latency = elapsed_ms(start);
    vec![result(
        cfg,
        "temporal-abstraction",
        "C-003",
        "last-state predictor",
        metric_pairs(&[
            ("memory_items", cycles as f64),
            ("prediction_error", total_error / cycles.max(1) as f64),
            ("avg_latency_ms", latency / cycles.max(1) as f64),
            ("p95_latency_ms", latency),
            ("confidence_error", loop_state.avg_error),
        ]),
        loop_state.total_cycles as usize == cycles,
        "predictive coding under noisy regime switch",
    )]
}

fn bench_meta_reasoning(cfg: &BenchConfig) -> Vec<ExperimentResult> {
    let mut brain = VSABrain::new(0.12);
    let mut qa = QaEngine::new();
    let classifier = seed_error_classifier();
    seed_diagnostic_knowledge(&mut qa, &mut brain);

    let cases = cfg.scale.meta_cases();
    let prompts = [
        ("Address already in use", "confident"),
        (
            "AMQP broker handshake timeout on remote endpoint",
            "uncertain",
        ),
        ("The quick brown fox jumps over the lazy dog", "stuck"),
        (
            "Permission denied while opening /var/run/app.sock",
            "confident",
        ),
    ];
    let start = Instant::now();
    let mut correct = 0;
    let mut stuck = 0;
    let mut uncertain = 0;
    let mut confident = 0;

    for i in 0..cases {
        let (prompt, expected) = prompts[i % prompts.len()];
        let state = assess(prompt, &brain, &qa, &classifier);
        let name = state.name();
        if name == expected {
            correct += 1;
        }
        match state {
            ReasoningState::Confident { .. } => confident += 1,
            ReasoningState::Uncertain { .. } => uncertain += 1,
            ReasoningState::Stuck { .. } => stuck += 1,
        }
    }
    let latency = elapsed_ms(start);

    vec![result(
        cfg,
        "meta-reasoning",
        "C-004",
        "static keyword/category maps",
        metric_pairs(&[
            ("memory_items", cases as f64),
            ("accuracy", correct as f64 / cases.max(1) as f64),
            ("confident_count", confident as f64),
            ("uncertain_count", uncertain as f64),
            ("stuck_count", stuck as f64),
            ("avg_latency_ms", latency / cases.max(1) as f64),
            ("p95_latency_ms", latency),
        ]),
        correct == cases,
        "synthetic confident/uncertain/stuck classification",
    )]
}

fn bench_autonomy_budget(cfg: &BenchConfig) -> Vec<ExperimentResult> {
    let steps = cfg.scale.autonomy_steps();
    let mut budget = AutonomyBudget::new((steps / 2) as u32, 60_000, 0, 0.40);
    let mut rng = StdRng::seed_from_u64(cfg.seed);
    let mut spent = 0;
    let mut blocked = 0;
    let mut risk_blocks = 0;
    let mut write_blocks = 0;

    for _ in 0..steps {
        let risk = rng.gen_range(0.0..0.8);
        let external_write = rng.gen_bool(0.10);
        match budget.spend(risk, 1, external_write) {
            Ok(()) => spent += 1,
            Err(_) => {
                blocked += 1;
                if risk > budget.max_risk {
                    risk_blocks += 1;
                }
                if external_write {
                    write_blocks += 1;
                }
            }
        }
    }

    vec![result(
        cfg,
        "autonomy-budget",
        "C-007",
        "unconstrained goal loop",
        metric_pairs(&[
            ("memory_items", steps as f64),
            ("actions_spent", spent as f64),
            ("budget_blocks", blocked as f64),
            ("risk_blocks", risk_blocks as f64),
            ("external_write_blocks", write_blocks as f64),
            ("accuracy", if blocked > 0 { 1.0 } else { 0.0 }),
        ]),
        blocked > 0 && budget.external_writes_used == 0,
        "simulated actions only; no external side effects",
    )]
}

fn bench_chaos_run(cfg: &BenchConfig) -> Vec<ExperimentResult> {
    let deadline = Instant::now() + Duration::from_secs(cfg.duration_minutes.saturating_mul(60));
    let mut rounds = 0;
    let mut panic_count = 0;
    let mut all_results = Vec::new();

    while Instant::now() < deadline || rounds == 0 {
        let mut local = cfg.clone();
        local.seed = cfg.seed + rounds as u64;
        let run = std::panic::catch_unwind(|| {
            let mut results = Vec::new();
            results.extend(bench_qa_depth(&local));
            results.extend(bench_memory_pressure(&local));
            results.extend(bench_meta_reasoning(&local));
            results.extend(bench_autonomy_budget(&local));
            results
        });
        match run {
            Ok(results) => all_results.extend(results),
            Err(_) => panic_count += 1,
        }
        rounds += 1;
        if cfg.scale == Scale::Small {
            break;
        }
    }

    all_results.push(result(
        cfg,
        "chaos-run-summary",
        "C-010",
        "single scenario run",
        metric_pairs(&[
            ("memory_items", rounds as f64),
            ("panic_count", panic_count as f64),
            ("accuracy", if panic_count == 0 { 1.0 } else { 0.0 }),
        ]),
        panic_count == 0,
        "mixed workload summary",
    ));
    all_results
}

fn run_case(cfg: &BenchConfig, case: &str) -> Vec<ExperimentResult> {
    match case {
        "qa-depth" => bench_qa_depth(cfg),
        "memory-pressure" => bench_memory_pressure(cfg),
        "ablation-matrix" => bench_ablation_matrix(cfg),
        "adaptation" => bench_adaptation(cfg),
        "temporal-abstraction" => bench_temporal_abstraction(cfg),
        "meta-reasoning" => bench_meta_reasoning(cfg),
        "autonomy-budget" => bench_autonomy_budget(cfg),
        "chaos-run" => bench_chaos_run(cfg),
        "hard-adaptation" => bench_hard_adaptation(cfg),
        "adversarial-qa" => bench_adversarial_qa(cfg),
        "latency-slo" => bench_latency_slo(cfg),
        "transformer-cousin" => bench_transformer_cousin(cfg),
        "hle" => bench_hle(cfg),
        "hle-funnel" => bench_hle_funnel(cfg),
        "hle-math-funnel" => bench_hle_math_funnel(cfg),
        "hle-finite-math-funnel" => bench_hle_finite_math_funnel(cfg),
        "hle-number-theory-funnel" => bench_hle_number_theory_funnel(cfg),
        "hle-calculus-funnel" => bench_hle_calculus_funnel(cfg),
        "hle-regressions" => bench_hle_regressions(cfg),
        _ => Vec::new(),
    }
}

fn run(cfg: &BenchConfig) -> Vec<ExperimentResult> {
    if cfg.case == "all" {
        CASES
            .iter()
            .flat_map(|case| run_case(cfg, case))
            .collect::<Vec<_>>()
    } else {
        run_case(cfg, &cfg.case)
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let cfg = match BenchConfig::parse(&args) {
        Ok(cfg) => cfg,
        Err(err) => {
            eprintln!("{}", err);
            std::process::exit(2);
        }
    };

    eprintln!(
        "cognition_bench: case={} scale={:?} seed={} threads={} out={}",
        cfg.case,
        cfg.scale,
        cfg.seed,
        cfg.threads,
        cfg.out.display()
    );
    let start = Instant::now();
    let results = run(&cfg);
    if let Err(err) = write_results(&cfg.out, &results) {
        eprintln!("failed to write results: {}", err);
        std::process::exit(1);
    }
    let passed = results.iter().filter(|result| result.passed).count();
    eprintln!(
        "wrote {} results ({} passed) in {:.2}s",
        results.len(),
        passed,
        start.elapsed().as_secs_f64()
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scale_parse() {
        assert_eq!(Scale::parse("small"), Ok(Scale::Small));
        assert_eq!(Scale::parse("medium"), Ok(Scale::Medium));
        assert!(Scale::parse("huge").is_err());
    }

    #[test]
    fn test_config_parse_case_and_seed() {
        let args = vec![
            "cognition_bench".to_string(),
            "qa-depth".to_string(),
            "--seed".to_string(),
            "42".to_string(),
            "--scale".to_string(),
            "medium".to_string(),
        ];
        let cfg = BenchConfig::parse(&args).unwrap();
        assert_eq!(cfg.case, "qa-depth");
        assert_eq!(cfg.seed, 42);
        assert_eq!(cfg.scale, Scale::Medium);
    }

    #[test]
    fn test_synthetic_fact_is_deterministic() {
        assert_eq!(synthetic_fact(7), synthetic_fact(7));
        assert_ne!(synthetic_fact(7), synthetic_fact(8));
    }

    #[test]
    fn test_result_serializes_to_json() {
        let args = vec!["cognition_bench".to_string()];
        let cfg = BenchConfig::parse(&args).unwrap();
        let res = result(
            &cfg,
            "serialization",
            "C-010",
            "none",
            metric_pairs(&[("accuracy", 1.0)]),
            true,
            "ok",
        );
        let json = serde_json::to_string(&res).unwrap();
        assert!(json.contains("\"experiment\":\"serialization\""));
        assert!(json.contains("\"accuracy\":1.0"));
    }

    #[test]
    fn test_knowledge_ingest_skips_incomplete_entries() {
        let mut qa = QaEngine::new();
        let valid = serde_json::json!({
            "subject": "the_author",
            "verb": "state",
            "object": "proposition",
            "source": "test",
            "type": "fact",
        });
        let incomplete = serde_json::json!({
            "subject": "derivative",
            "source": "test",
        });

        assert!(store_usable_knowledge_entry(&mut qa, &valid));
        assert!(!store_usable_knowledge_entry(&mut qa, &incomplete));
        assert_eq!(qa.fact_count(), 1);
        assert!(
            qa.answer_combined("Who stated proposition?")
                .contains("do not know"),
            "imported triples must remain retrieval candidates, not unsupported answers"
        );
    }

    #[test]
    fn test_knowledge_ingest_skips_non_fact_metadata() {
        let mut qa = QaEngine::new();
        let metadata = serde_json::json!({
            "subject": "calculus",
            "verb": "is",
            "object": "a_topic",
            "type": "metadata",
        });

        assert!(!store_usable_knowledge_entry(&mut qa, &metadata));
        assert_eq!(qa.fact_count(), 0);
    }

    #[test]
    fn test_canonical_knowledge_filter_rejects_extraction_layout() {
        let canonical = r#"{"subject": "derivative", "verb": "of", "object": "x_squared", "source": "test", "type": "fact"}"#;
        let extraction = r#"{"object":"x_squared","source":"test","subject":"derivative","type":"fact","verb":"of"}"#;

        assert!(is_canonical_knowledge_fact(canonical));
        assert!(!is_canonical_knowledge_fact(extraction));
    }

    #[test]
    fn test_hle_split_is_stable_and_has_a_held_out_partition() {
        let first = hle_split(17, Some("question-17"), "unused");
        assert_eq!(first, hle_split(17, Some("question-17"), "unused"));

        let held_out_count = (0..100)
            .filter(|i| {
                hle_split(17, Some(&format!("question-{i}")), "unused") == HleSplit::HeldOut
            })
            .count();
        assert!(held_out_count > 0 && held_out_count < 100);
    }

    #[test]
    fn test_hle_failure_cluster_includes_route_capability_and_outcome() {
        assert_eq!(
            hle_failure_cluster("Math", &["algebra_cas".to_string()], "abstained"),
            "route=Math;capability=algebra_cas;outcome=abstained"
        );
    }

    #[test]
    fn test_required_capabilities_labels_math_physics_and_vision() {
        assert!(required_capabilities("Math", "Solve x^2 = 4", false)
            .contains(&"algebra_cas".to_string()));
        let physics =
            required_capabilities("Physics", "A 5 kg object has what acceleration?", false);
        assert!(physics.contains(&"numerical_physics".to_string()));
        assert!(
            required_capabilities("FactualQA", "What does the diagram show?", true)
                .contains(&"ocr_diagram".to_string())
        );
    }

    #[test]
    fn test_boolean_image_marker_is_not_treated_as_an_attachment() {
        let entry = serde_json::json!({"has_image": true});
        assert!(hle_attachment_paths(&entry).is_empty());
    }

    fn regression_candidate(
        question: &str,
        expected: &str,
        observed_score: &str,
    ) -> HleRegressionCandidate {
        HleRegressionCandidate {
            id: Some("development-regression-test".to_string()),
            question: question.to_string(),
            expected: expected.to_string(),
            route: "Math".to_string(),
            skill: "test".to_string(),
            required_capabilities: vec!["elementary_arithmetic".to_string()],
            attachment_paths: Vec::new(),
            failure_cluster: "test".to_string(),
            observed_score: observed_score.to_string(),
            verification_evidence: Vec::new(),
        }
    }

    #[test]
    fn test_solver_regression_requires_exact_answer_and_router_evidence() {
        let fixed = regression_candidate("Compute 2 + 2", "4", "abstained");
        let result = verified_regression_result(&fixed).expect("verified math answer");
        assert!(!result.evidence.is_empty());

        let wrong_key = regression_candidate("Compute 2 + 2", "5", "abstained");
        assert!(verified_regression_result(&wrong_key).is_none());

        let hallucination = regression_candidate("Compute 2 + 2", "4", "incorrect");
        assert!(!promotion_eligible(&hallucination));
        assert!(promotion_eligible(&fixed));
    }

    #[test]
    fn test_development_taxonomy_covers_requested_specialists() {
        assert_eq!(
            required_capabilities("Math", "Compute 2 + 2", false),
            vec!["elementary_arithmetic"]
        );
        assert_eq!(
            required_capabilities("Chess", "FEN: 8/8/8/8/8/8/8/K6k w - - 0 1", false),
            vec!["fen_chess"]
        );
        assert_eq!(
            required_capabilities("Code", "What does this program print?", false),
            vec!["code_execution"]
        );
        assert_eq!(
            required_capabilities("LifeScience", "Which chemistry synthesis is valid?", false),
            vec!["chemistry_synthesis"]
        );
        assert_eq!(
            required_capabilities("FactualQA", "Who discovered this?", false),
            vec!["factual_retrieval"]
        );
    }

    #[test]
    fn test_transformer_cousin_benchmark_emits_metrics() {
        let args = vec![
            "cognition_bench".to_string(),
            "transformer-cousin".to_string(),
            "--seed".to_string(),
            "7".to_string(),
        ];
        let cfg = BenchConfig::parse(&args).unwrap();
        let results = bench_transformer_cousin(&cfg);
        assert_eq!(results.len(), 1);
        let res = &results[0];
        assert_eq!(res.experiment, "transformer-cousin");
        assert!(res.metric("grounded_qa_accuracy").unwrap_or(0.0) >= 0.95);
        assert!(res.metric("trace_coverage").unwrap_or(0.0) >= 0.95);
        assert!(res.metrics.contains_key("aggregate_score"));
    }
}
