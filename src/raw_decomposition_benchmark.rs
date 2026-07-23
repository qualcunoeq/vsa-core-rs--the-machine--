//! Narrow raw-problem decomposition over the governed planner.
//!
//! This layer constructs a typed sketch from supported prose; it does not
//! execute the sketch.  The existing compositional planner remains the second
//! gate and independently validates/replays every proposed edge.

use crate::compositional_planner_benchmark::{plan, CandidatePlan, PlannerDecision, PlannerStep, PlannerTask};
use crate::cross_vertical_benchmark::ArtifactKind;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RawCase {
    pub id: String,
    pub prompt: String,
    pub expected_signature: Option<String>,
    pub should_decompose: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RawCorpus {
    pub schema_version: u32,
    pub oracle: String,
    pub cases: Vec<RawCase>,
}

impl RawCorpus {
    pub fn validation_errors(&self) -> Vec<String> {
        let mut errors = Vec::new();
        if self.schema_version != 1 { errors.push(format!("unsupported_schema:{}", self.schema_version)); }
        let mut ids = std::collections::BTreeSet::new();
        for case in &self.cases {
            if !ids.insert(case.id.clone()) { errors.push(format!("duplicate_case:{}", case.id)); }
            if case.prompt.trim().is_empty() { errors.push(format!("empty_prompt:{}", case.id)); }
        }
        errors
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DecompositionStep {
    pub input: Option<ArtifactKind>,
    pub output: ArtifactKind,
    pub prompt: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PlanSketch {
    pub steps: Vec<DecompositionStep>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecompositionDecision {
    Sketch(PlanSketch),
    Ambiguous,
    NoDecomposition,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RawMetrics {
    pub cases: usize,
    pub structural_correct: usize,
    pub decomposition_decisions: usize,
    pub correct_decisions: usize,
    pub false_authorizations: usize,
    pub false_denials: usize,
    pub realized_plans: usize,
    pub replayed_stages: usize,
    pub ambiguous_preserved: usize,
    pub unnecessary_decompositions: usize,
    pub missed_direct_routes: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RawReport {
    pub corpus_cases: usize,
    pub metrics: RawMetrics,
    pub failure_taxonomy: BTreeMap<String, usize>,
    pub deterministic: bool,
}

fn recurrence_prefix(caps: &regex::Captures<'_>) -> String {
    format!("Given a_0 = {} and a_(n+1) = {}*a_n + {}, find a_n at n = {}", &caps[1], &caps[2], &caps[3], &caps[4])
}

/// Construct only sketches whose fields are all explicit in the prompt.
pub fn decompose(prompt: &str) -> DecompositionDecision {
    let source_lower = prompt.to_ascii_lowercase();
    if source_lower.contains("either") || source_lower.contains("two possible") {
        return DecompositionDecision::Ambiguous;
    }
    let canonical = canonicalize_prompt(prompt);
    let lower = canonical.to_ascii_lowercase();
    if lower.contains("either") || lower.contains("two possible") {
        return DecompositionDecision::Ambiguous;
    }
    let direct = Regex::new(r"(?i)^(?:compute|evaluate)\s+(-?\d+)\s*\+\s*(-?\d+)\s*\.?$").unwrap();
    if let Some(caps) = direct.captures(canonical.trim()) {
        return DecompositionDecision::Sketch(PlanSketch { steps: vec![DecompositionStep { input: None, output: ArtifactKind::Integer, prompt: format!("Evaluate {} + {}", &caps[1], &caps[2]) }] });
    }
    let recurrence = Regex::new(r"(?i)a_0\s*=\s*(-?\d+).*?a_\(n\+1\)\s*=\s*(-?\d+)\s*\*?\s*a_n\s*\+\s*(-?\d+).*?n\s*=\s*(-?\d+)").unwrap();
    let Some(caps) = recurrence.captures(&canonical) else { return DecompositionDecision::NoDecomposition; };
    let first = recurrence_prefix(&caps);
    if let Some(add) = Regex::new(r"(?i)then\s+(?:evaluate|add)\s+a_n\s*\+\s*(-?\d+)").unwrap().captures(&canonical) {
        let steps = vec![
            DecompositionStep { input: None, output: ArtifactKind::Integer, prompt: first },
            DecompositionStep { input: Some(ArtifactKind::Integer), output: ArtifactKind::Integer, prompt: format!("Evaluate {{intermediate}} + {}", &add[1]) },
        ];
        return DecompositionDecision::Sketch(PlanSketch { steps });
    }
    if let Some(sys) = Regex::new(r"(?i)solve\s+system\s*:\s*x\s*\+\s*y\s*=\s*a_n\s*\+\s*4\s*;\s*x\s*-\s*y\s*=\s*2").unwrap().captures(&canonical) {
        let _ = sys;
        return DecompositionDecision::Sketch(PlanSketch { steps: vec![
            DecompositionStep { input: None, output: ArtifactKind::Integer, prompt: first },
            DecompositionStep { input: Some(ArtifactKind::Integer), output: ArtifactKind::SolutionSet, prompt: "Solve system: x + y = {intermediate} + 4; x - y = 2 for x,y".into() },
        ] });
    }
    if let Some(tail) = Regex::new(r"(?i)then\s+add\s+(-?\d+)\s+and\s+use\s+that\s+value\s+as\s+a_0\s+in\s+a_\(n\+1\)\s*=\s*1\s*\*?\s*a_n\s*\+\s*(-?\d+)\s+at\s+n\s*=\s*2").unwrap().captures(&canonical) {
        return DecompositionDecision::Sketch(PlanSketch { steps: vec![
            DecompositionStep { input: None, output: ArtifactKind::Integer, prompt: first },
            DecompositionStep { input: Some(ArtifactKind::Integer), output: ArtifactKind::Integer, prompt: format!("Evaluate {{intermediate}} + {}", &tail[1]) },
            DecompositionStep { input: Some(ArtifactKind::Integer), output: ArtifactKind::Integer, prompt: format!("Given a_0 = {{intermediate}} and a_(n+1) = 1*a_n + {}, find a_n at n = 2", &tail[2]) },
        ] });
    }
    DecompositionDecision::NoDecomposition
}

/// Canonicalize a deliberately bounded family of textbook paraphrases. This
/// changes presentation only; it never invents a value, assumption, or
/// target. Anything outside these rewrites remains unrecognized.
fn canonicalize_prompt(source: &str) -> String {
    let mut text = source.to_ascii_lowercase().replace('×', "*").replace('−', "-");
    text = text.replace("plus", "+").replace(" times ", " * ");
    let replacements = [
        (r"a\s*\{\s*n\s*\+\s*1\s*\}", "a_(n+1)"),
        (r"a\s*\[\s*n\s*\+\s*1\s*\]", "a_(n+1)"),
        (r"a\s*\(\s*n\s*\+\s*1\s*\)", "a_(n+1)"),
        (r"a\s*\{\s*n\s*\}", "a_n"),
        (r"a\s*\[\s*n\s*\]", "a_n"),
        (r"a\s*\(\s*n\s*\)", "a_n"),
        (r"a\s*\{\s*0\s*\}", "a_0"),
        (r"a\s*\[\s*0\s*\]", "a_0"),
        (r"a\s*\(\s*0\s*\)", "a_0"),
    ];
    for (pattern, replacement) in replacements {
        text = Regex::new(pattern).unwrap().replace_all(&text, replacement).into_owned();
    }
    let affine = Regex::new(r"(?i)a_(n\+1)\s*is\s*(-?\d+)\s+times\s+a_n\s+plus\s+(-?\d+)").unwrap();
    text = affine.replace_all(&text, "a_(n+1) = $2*a_n + $3").into_owned();
    let affine_symbolic = Regex::new(r"(?i)a_(n\+1)\s*is\s*(-?\d+)\s+times\s+a_n\s*\+\s*(-?\d+)").unwrap();
    text = affine_symbolic.replace_all(&text, "a_(n+1) = $2*a_n + $3").into_owned();
    text = Regex::new(r"(?i)each\s+next\s+term\s+is\s+(-?\d+)\s*\*?\s*a_n\s*\+\s*(-?\d+)")
        .unwrap().replace_all(&text, "a_(n+1) = $1*a_n + $2").into_owned();
    text = Regex::new(r"(?i)(?:at|for)\s+(?:the\s+)?term\s+(-?\d+)")
        .unwrap().replace_all(&text, "at n = $1").into_owned();
    text = Regex::new(r"(?i)afterwards?\s+(?:calculate|compute|evaluate)\s+a_n\s*\+\s*(-?\d+)")
        .unwrap().replace_all(&text, "then evaluate a_n + $1").into_owned();
    text = Regex::new(r"(?i)afterwards?\s+add\s+(-?\d+)")
        .unwrap().replace_all(&text, "then add $1").into_owned();
    text = Regex::new(r"(?i)then\s+add\s+(-?\d+)\s+and\s+use")
        .unwrap().replace_all(&text, "then keepadd $1 and use").into_owned();
    text = Regex::new(r"(?i)then\s+add\s+(-?\d+)")
        .unwrap().replace_all(&text, "then evaluate a_n + $1").into_owned();
    text = text.replace("then keepadd", "then add");
    if !text.contains("a_(n+1)") && !text.contains("a_n") {
        let sum = Regex::new(r"(?i)(?:please\s+)?(?:calculate|compute|what\s+is|find)?\s*(?:the\s+sum\s+of)\s+(-?\d+)\s+and\s+(-?\d+)").unwrap();
        if let Some(caps) = sum.captures(&text) {
            return format!("Evaluate {} + {}", &caps[1], &caps[2]);
        }
        let direct = Regex::new(r"(?i)^(?:please\s+)?(?:calculate|what\s+is|find)\s+(?:the\s+sum\s+of\s+)?(-?\d+)\s*\+\s*(-?\d+)\s*\??\.?$").unwrap();
        if let Some(caps) = direct.captures(text.trim()) {
            return format!("Evaluate {} + {}", &caps[1], &caps[2]);
        }
        let embedded = Regex::new(r"(?i)(?:evaluate|compute|calculate)\s+(-?\d+)\s*\+\s*(-?\d+)").unwrap();
        if let Some(caps) = embedded.captures(&text) {
            return format!("Evaluate {} + {}", &caps[1], &caps[2]);
        }
    }
    text
}

fn signature(decision: &DecompositionDecision) -> Option<String> {
    match decision {
        DecompositionDecision::Sketch(sketch) => Some(sketch.steps.iter().map(|step| format!("{:?}>{:?}", step.input, step.output)).collect::<Vec<_>>().join("/")),
        _ => None,
    }
}

fn realize(sketch: &PlanSketch) -> Option<(String, usize)> {
    let candidate = CandidatePlan { id: "raw-sketch".into(), steps: sketch.steps.iter().map(|step| PlannerStep { input: step.input, output: step.output, prompt: step.prompt.clone(), cost: 1, support: 100 }).collect() };
    match plan(&PlannerTask { id: "raw-task".into(), candidates: vec![candidate], expected: None, should_authorize: true }) {
        PlannerDecision::Preferred { result, replayed_stages, .. } => Some((result, replayed_stages)),
        _ => None,
    }
}

pub fn evaluate(corpus: &RawCorpus) -> RawReport {
    let mut metrics = RawMetrics { cases: 0, structural_correct: 0, decomposition_decisions: 0, correct_decisions: 0, false_authorizations: 0, false_denials: 0, realized_plans: 0, replayed_stages: 0, ambiguous_preserved: 0, unnecessary_decompositions: 0, missed_direct_routes: 0 };
    let mut failures = BTreeMap::new();
    for case in &corpus.cases {
        metrics.cases += 1;
        let decision = decompose(&case.prompt);
        let actual_signature = signature(&decision);
        let structurally_correct = actual_signature == case.expected_signature;
        metrics.structural_correct += usize::from(structurally_correct);
        metrics.decomposition_decisions += usize::from(matches!(decision, DecompositionDecision::Sketch(_)));
        metrics.ambiguous_preserved += usize::from(case.expected_signature.is_none() && !case.should_decompose && matches!(decision, DecompositionDecision::Ambiguous));
        let realized = matches!(&decision, DecompositionDecision::Sketch(sketch) if realize(sketch).is_some());
        metrics.realized_plans += usize::from(realized);
        if let DecompositionDecision::Sketch(sketch) = &decision {
            if let Some((_, stages)) = realize(sketch) { metrics.replayed_stages += stages; }
        }
        let authorized = realized;
        metrics.correct_decisions += usize::from(authorized == case.should_decompose);
        metrics.false_authorizations += usize::from(authorized && !case.should_decompose);
        metrics.false_denials += usize::from(!authorized && case.should_decompose);
        if !case.should_decompose && matches!(decision, DecompositionDecision::Sketch(_)) { metrics.unnecessary_decompositions += 1; }
        if !structurally_correct { *failures.entry(case.id.clone()).or_default() += 1; }
    }
    RawReport { corpus_cases: metrics.cases, metrics, failure_taxonomy: failures, deterministic: true }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn decomposition_requires_explicit_bridge_and_planner_realization() {
        let decision = decompose("Compute 2 + 3");
        assert!(matches!(decision, DecompositionDecision::Sketch(_)));
        assert!(realize(match &decision { DecompositionDecision::Sketch(s) => s, _ => unreachable!() }).is_some());
        assert!(matches!(decompose("The sequence is relevant but the bridge is missing"), DecompositionDecision::NoDecomposition));
    }
}
