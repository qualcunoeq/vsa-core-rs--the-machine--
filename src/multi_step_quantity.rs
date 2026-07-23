//! Bounded multi-step quantity composition.
//!
//! This planner composes already-governed quantity primitives.  It emits no
//! executable action until every intermediate stage has been independently
//! formalized, executed, and replay-verified.

use crate::algebra_island;
use crate::fractional_quantity::{
    bridge_to_algebra as bridge_fraction_to_algebra, formalize as formalize_fraction,
    FractionalQuantityDecision,
};
use crate::unit_aware_quantity::{
    bridge_to_algebra as bridge_unit_to_algebra, formalize as formalize_unit, UnitQuantityDecision,
};
use regex::Regex;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MultiStepDecisionKind {
    Accepted,
    Ambiguous,
    Unsupported,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultiStepPlan {
    pub family: String,
    pub stage_prompts: Vec<String>,
    pub final_target: String,
    pub signature: String,
}

impl MultiStepPlan {
    pub fn replay_shape_verified(&self) -> bool {
        !self.family.is_empty()
            && !self.final_target.is_empty()
            && !self.signature.is_empty()
            && self.stage_prompts.len() >= 2
            && self.stage_prompts.iter().all(|prompt| !prompt.is_empty())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MultiStepDecision {
    Accepted(MultiStepPlan),
    Ambiguous,
    Unsupported,
}

impl MultiStepDecision {
    pub fn kind(&self) -> MultiStepDecisionKind {
        match self {
            Self::Accepted(_) => MultiStepDecisionKind::Accepted,
            Self::Ambiguous => MultiStepDecisionKind::Ambiguous,
            Self::Unsupported => MultiStepDecisionKind::Unsupported,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MultiStepStageReceipt {
    pub prompt: String,
    pub result: String,
    pub replay_verified: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MultiStepExecutionReceipt {
    pub family: String,
    pub stages: Vec<MultiStepStageReceipt>,
    pub final_result: String,
    pub replay_verified: bool,
}

fn execute_expression(prompt: &str) -> Option<MultiStepStageReceipt> {
    let answer = algebra_island::try_answer(prompt)?;
    if !answer.receipt.verification.passed {
        return None;
    }
    let replay = algebra_island::try_answer(prompt)?;
    let replay_verified = replay.answer == answer.answer && replay.receipt.verification.passed;
    replay_verified.then_some(MultiStepStageReceipt {
        prompt: prompt.into(),
        result: answer.answer,
        replay_verified,
    })
}

/// Execute every stage after validating the plan shape.  The next prompt is
/// built only from the replay-verified previous result.
pub fn execute(plan: &MultiStepPlan) -> Option<MultiStepExecutionReceipt> {
    if !plan.replay_shape_verified() {
        return None;
    }
    let mut stages: Vec<MultiStepStageReceipt> = Vec::new();
    let first = match plan.family.as_str() {
        "fraction_then_arithmetic" => {
            let relation = match formalize_fraction(&plan.stage_prompts[0]) {
                FractionalQuantityDecision::Accepted(artifact) => artifact,
                _ => return None,
            };
            bridge_fraction_to_algebra(&relation)?
        }
        "unit_then_arithmetic" => {
            let relation = match formalize_unit(&plan.stage_prompts[0]) {
                UnitQuantityDecision::Accepted(artifact) => artifact,
                _ => return None,
            };
            bridge_unit_to_algebra(&relation)?
        }
        "arithmetic_chain" => return execute_arithmetic_chain(plan),
        _ => return None,
    };
    stages.push(MultiStepStageReceipt {
        prompt: first.prompt.clone(),
        result: first.result.clone(),
        replay_verified: first.algebra_replay_verified,
    });
    let second_prompt = plan.stage_prompts[1].replace("{intermediate}", &first.result);
    let second = execute_expression(&second_prompt)?;
    stages.push(second.clone());
    Some(MultiStepExecutionReceipt {
        family: plan.family.clone(),
        final_result: second.result,
        replay_verified: stages.iter().all(|stage| stage.replay_verified),
        stages,
    })
}

fn execute_arithmetic_chain(plan: &MultiStepPlan) -> Option<MultiStepExecutionReceipt> {
    let mut stages: Vec<MultiStepStageReceipt> = Vec::new();
    for prompt in &plan.stage_prompts {
        let rendered = if let Some(previous) = stages.last() {
            prompt.replace("{intermediate}", &previous.result)
        } else {
            prompt.clone()
        };
        stages.push(execute_expression(&rendered)?);
    }
    let final_result = stages.last()?.result.clone();
    Some(MultiStepExecutionReceipt {
        family: plan.family.clone(),
        replay_verified: stages.iter().all(|stage| stage.replay_verified),
        stages,
        final_result,
    })
}

/// Parse three deliberately bounded multi-step families.
pub fn formalize(prompt: &str) -> MultiStepDecision {
    let text = prompt.to_ascii_lowercase().replace(['\n', '\r'], " ");
    let text = text.trim();
    if text.contains("either") || text.contains("not specified") || text.contains("unknown") {
        return MultiStepDecision::Ambiguous;
    }
    if text.contains('%')
        || text.contains("percent")
        || text.contains("probability")
        || text.contains("compound")
        || text.contains("nonlinear")
        || text.contains("three or more stages")
    {
        return MultiStepDecision::Unsupported;
    }

    let fraction = Regex::new(
        r"^start with (\d+) items?\. remove (one quarter|a quarter|one half|a half|1/4|1/2) of them, then (add|subtract) (\d+) items?\. what is the final count\??$",
    )
    .unwrap();
    if let Some(caps) = fraction.captures(text) {
        let fraction_prompt = format!("What remains after removing {} of {}?", &caps[2], &caps[1]);
        let operator = if &caps[3] == "add" { "+" } else { "-" };
        let second = format!("Evaluate {{intermediate}} {} {}", operator, &caps[4]);
        return match formalize_fraction(&fraction_prompt) {
            FractionalQuantityDecision::Accepted(_) => MultiStepDecision::Accepted(MultiStepPlan {
                family: "fraction_then_arithmetic".into(),
                stage_prompts: vec![fraction_prompt, second],
                final_target: "final_count".into(),
                signature: "fraction>quantity>arithmetic>quantity".into(),
            }),
            _ => MultiStepDecision::Unsupported,
        };
    }

    let unit = Regex::new(
        r"^convert (\d+) meters to centimeters using (\d+) centimeters per meter, then add (\d+) centimeters\. what is the total in centimeters\??$",
    )
    .unwrap();
    if let Some(caps) = unit.captures(text) {
        let conversion = format!(
            "Convert {} meters to centimeters using {} centimeters per meter.",
            &caps[1], &caps[2]
        );
        let second = format!("Evaluate {{intermediate}} + {}", &caps[3]);
        return match formalize_unit(&conversion) {
            UnitQuantityDecision::Accepted(_) => MultiStepDecision::Accepted(MultiStepPlan {
                family: "unit_then_arithmetic".into(),
                stage_prompts: vec![conversion, second],
                final_target: "total_centimeters".into(),
                signature: "unit_conversion>quantity>arithmetic>quantity".into(),
            }),
            _ => MultiStepDecision::Unsupported,
        };
    }

    let arithmetic = Regex::new(
        r"^start with (\d+) items?; add (\d+) items?, then subtract (\d+) items?\. what is the final count\??$",
    )
    .unwrap();
    if let Some(caps) = arithmetic.captures(text) {
        return MultiStepDecision::Accepted(MultiStepPlan {
            family: "arithmetic_chain".into(),
            stage_prompts: vec![
                format!("Evaluate {} + {}", &caps[1], &caps[2]),
                format!("Evaluate {{intermediate}} - {}", &caps[3]),
            ],
            final_target: "final_count".into(),
            signature: "quantity>arithmetic>quantity>arithmetic>quantity".into(),
        });
    }

    MultiStepDecision::Unsupported
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fraction_chain_replays_each_stage() {
        let MultiStepDecision::Accepted(plan) = formalize(
            "Start with 20 items. Remove one quarter of them, then add 3 items. What is the final count?",
        ) else { panic!("not accepted"); };
        let receipt = execute(&plan).expect("execution");
        assert_eq!(receipt.final_result, "18");
        assert!(
            receipt.replay_verified && receipt.stages.iter().all(|stage| stage.replay_verified)
        );
    }

    #[test]
    fn unit_chain_and_arithmetic_chain_replay() {
        for prompt in [
            "Convert 2 meters to centimeters using 100 centimeters per meter, then add 30 centimeters. What is the total in centimeters?",
            "Start with 10 items; add 5 items, then subtract 2 items. What is the final count?",
        ] {
            let MultiStepDecision::Accepted(plan) = formalize(prompt) else { panic!("not accepted: {prompt}"); };
            assert!(execute(&plan).is_some());
        }
    }
}
