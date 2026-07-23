//! Deterministic planner-pressure corpus for the quantity-family graph.
//!
//! The v2 corpus is generated from a reviewed seed and fixed templates rather
//! than loaded from the production router.  It deliberately includes cheaper
//! competing routes, equal-cost ties, invalid handoffs, and bounded multi-step
//! quantity plans.  Generation is deterministic so release-mode results are
//! reproducible without mutating global routing.

use crate::quantity_cross_domain_benchmark::{
    CrossDomainCorpus, CrossDomainTask, RouteCandidate, RouteKind,
};

fn candidate(id: &str, kind: RouteKind, prompt: String, cost: u32, support: u32) -> RouteCandidate {
    RouteCandidate {
        id: id.into(),
        kind,
        prompt,
        cost,
        support,
    }
}

fn task(
    id: String,
    candidates: Vec<RouteCandidate>,
    expected: Option<String>,
    should_authorize: bool,
    pair_id: Option<String>,
) -> CrossDomainTask {
    CrossDomainTask {
        id,
        candidates,
        expected,
        should_authorize,
        pair_id,
    }
}

pub fn corpus() -> CrossDomainCorpus {
    let mut cases = Vec::with_capacity(240);

    // 80 direct quantity-to-algebra cases. Every tenth pair uses two
    // equivalent surface forms to pressure route stability.
    for i in 0..80u32 {
        let paired = i % 10 < 2;
        let count = if paired { 5 } else { 5 + (i % 20) };
        let total = count * 4;
        let prompt = if paired && i % 10 == 1 {
            format!("{count} NOTEBOOKS cost {total} DOLLARS. What is the price per notebook?")
        } else if i % 2 == 0 || paired {
            format!("{count} notebooks cost {total} dollars. What is the price per notebook?")
        } else {
            format!("There are {count} red counters plus {} blue counters in the box. Find the total count.", total - count)
        };
        let expected = if i % 2 == 0 || paired {
            "4".into()
        } else {
            total.to_string()
        };
        let pair_id = paired.then(|| format!("quantity-rewrite-{}", i / 10));
        cases.push(task(
            format!("qa{:03}", i),
            vec![candidate(
                "quantity-algebra",
                RouteKind::QuantityToAlgebra,
                prompt,
                2,
                90,
            )],
            Some(expected),
            true,
            pair_id,
        ));
    }

    // 40 unit-aware algebra routes, alternating explicit conversion and
    // compatible addition/subtraction.
    for i in 0..40u32 {
        let amount = 2 + (i % 10);
        let prompt = if i % 2 == 0 {
            format!("Convert {amount} meters to centimeters using 100 centimeters per meter.")
        } else {
            let centimeters = amount * 100;
            format!("Add {amount} meters and 30 centimeters; express the total in centimeters.")
                .replace(&format!("{amount} meters"), &format!("{} meters", amount))
                .replace("230", &centimeters.to_string())
        };
        let expected = if i % 2 == 0 {
            (amount * 100).to_string()
        } else {
            (amount * 100 + 30).to_string()
        };
        cases.push(task(
            format!("ua{:03}", i),
            vec![candidate(
                "unit-algebra",
                RouteKind::UnitToAlgebra,
                prompt,
                2,
                85,
            )],
            Some(expected),
            true,
            None,
        ));
    }

    // 20 fractional quantity routes.
    for i in 0..20u32 {
        let base = 20 + (i % 10) * 4;
        let prompt = if i % 2 == 0 {
            format!("What is three quarters of {base}?")
        } else {
            format!("What remains after removing 1/4 of {base}?")
        };
        cases.push(task(
            format!("fr{:03}", i),
            vec![candidate(
                "fraction-algebra",
                RouteKind::FractionToAlgebra,
                prompt,
                2,
                80,
            )],
            Some((base * 3 / 4).to_string()),
            true,
            None,
        ));
    }

    // 20 quantity-ratio → linear-system routes.
    for i in 0..20u32 {
        let red = 2 + (i % 8);
        let blue = red + 1;
        let anchor = red * 4;
        let prompt = format!("The ratio of red beads to blue beads is {red}:{blue}. If there are {anchor} red beads, how many blue beads are there?");
        let expected = format!(r#"{{"x": "{anchor}", "y": "{}"}}"#, blue * 4);
        cases.push(task(
            format!("qs{:03}", i),
            vec![candidate(
                "quantity-system",
                RouteKind::QuantityToSystem,
                prompt,
                3,
                70,
            )],
            Some(expected),
            true,
            None,
        ));
    }

    // 20 unit conversion → linear-system routes.
    for i in 0..20u32 {
        let meters = 2 + (i % 8);
        let prompt =
            format!("Convert {meters} meters to centimeters using 100 centimeters per meter.");
        let expected = format!(r#"{{"x": "{meters}", "y": "{}"}}"#, meters * 100);
        cases.push(task(
            format!("us{:03}", i),
            vec![candidate(
                "unit-system",
                RouteKind::UnitToSystem,
                prompt,
                3,
                75,
            )],
            Some(expected),
            true,
            None,
        ));
    }

    // 20 bounded multi-step plans. These are three semantic transformations
    // (fractional quantity, arithmetic handoff, final quantity) from the
    // planner's perspective, with every stage replayed by the executor.
    for i in 0..20u32 {
        let base = 20 + (i % 10) * 4;
        let add = 2 + (i % 5);
        let prompt = format!("Start with {base} items. Remove one quarter of them, then add {add} items. What is the final count?");
        cases.push(task(
            format!("ms{:03}", i),
            vec![candidate(
                "multi-step",
                RouteKind::MultiStepToAlgebra,
                prompt,
                4,
                65,
            )],
            Some((base * 3 / 4 + add).to_string()),
            true,
            None,
        ));
    }

    // 20 competing explanations. The planner should prefer the lower-cost
    // route, while still verifying both candidates when they are eligible.
    for i in 0..20u32 {
        let base = 20 + (i % 10) * 4;
        cases.push(task(
            format!("comp{:03}", i),
            vec![
                candidate("fraction-route", RouteKind::FractionToAlgebra, format!("What is one half of {base}?"), 3, 50),
                candidate("quantity-route", RouteKind::QuantityToAlgebra, format!("A quantity starts at {} units and increases by {} units. What is the final quantity?", base / 2, base / 2), 2, 80),
            ],
            Some(base.to_string()),
            true,
            None,
        ));
    }

    // 10 equal-cost ties with different results. These must remain
    // ambiguous rather than becoming an arbitrary authorization.
    for i in 0..10u32 {
        cases.push(task(
            format!("tie{:03}", i),
            vec![
                candidate("left", RouteKind::QuantityToAlgebra, "5 notebooks cost 20 dollars. What is the price per notebook?".into(), 2, 60),
                candidate("right", RouteKind::QuantityToAlgebra, "There are 8 red counters plus 3 blue counters in the box. Find the total count.".into(), 2, 60),
            ],
            None,
            false,
            None,
        ));
    }

    // 10 deliberate failures: unsupported mathematics and incompatible
    // handoffs must not be rescued by planner optimism.
    for i in 0..10u32 {
        let (kind, prompt) = if i % 2 == 0 {
            (
                RouteKind::UnitToSystem,
                "Add 2 meters and 30 centimeters; express the total in centimeters.".into(),
            )
        } else {
            (RouteKind::FractionToAlgebra, "What is 20% of 50?".into())
        };
        cases.push(task(
            format!("reject{:03}", i),
            vec![candidate("invalid-edge", kind, prompt, 1, 100)],
            None,
            false,
            None,
        ));
    }

    CrossDomainCorpus {
        schema_version: 1,
        oracle: "deterministic quantity-planning-v2 generator seed=42".into(),
        cases,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::quantity_cross_domain_benchmark::evaluate;

    #[test]
    fn generated_corpus_has_fixed_size_and_clean_schema() {
        let corpus = corpus();
        assert_eq!(corpus.cases.len(), 240);
        assert!(corpus.validation_errors().is_empty());
    }

    #[test]
    fn generation_is_deterministic() {
        assert_eq!(corpus(), corpus());
    }

    #[test]
    fn generated_corpus_preserves_safety_boundary() {
        let report = evaluate(&corpus());
        assert_eq!(report.metrics.false_authorizations, 0);
        assert_eq!(report.metrics.false_denials, 0);
        assert_eq!(report.rewrites.regressions, 0);
    }
}
