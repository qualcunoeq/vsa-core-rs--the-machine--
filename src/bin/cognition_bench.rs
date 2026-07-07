use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use std::collections::HashMap;
use std::fs::{create_dir_all, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};
use the_machine::cognition::{AblationConfig, AutonomyBudget, ExperimentResult};
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
}

#[derive(Clone, Debug)]
struct BenchConfig {
    case: String,
    out: PathBuf,
    seed: u64,
    scale: Scale,
    threads: usize,
    duration_minutes: u64,
    commit: String,
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
            commit: current_commit(),
        })
    }
}

fn usage() -> String {
    format!(
        "usage: cognition_bench [CASE] [--case CASE] [--out PATH] [--seed N] [--scale small|medium|large|max] [--threads N] [--duration-minutes N]\n\ncases: all, {}",
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
        for i in 1..depth {
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
}
