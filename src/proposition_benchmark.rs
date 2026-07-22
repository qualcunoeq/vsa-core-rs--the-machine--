//! Deterministic proof-kernel benchmark for the elementary discrete/proof
//! vertical.
//!
//! Cases are generated as proof objects, not accepted answers.  The trusted
//! `ProofChecker` is the execution authority and is invoked twice so replay
//! agreement is measured independently from initial acceptance.  Negative
//! proof objects are expected to abstain and are classified by the kernel
//! error that caused rejection.

use crate::algebra::SymExpr;
use crate::cognition::ExperimentResult;
use crate::kernel::{Certificate, Proof, ProofChecker};
use crate::proposition::{
    shared_var, Binder, LocalContext, Proposition, Substitution, TheoremEnvironment, TheoremId,
};
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProofCaseKind {
    Reflexivity,
    AddZero,
    MultiplicativeIdentity,
    Commutativity,
    Distributivity,
    SymmetryWithPremise,
    TransitivityWithPremise,
    NonnegativeAbs,
    UniversalIntroduction,
    MissingBinder,
    WrongCertificate,
    MissingPremise,
    UnknownTheorem,
    WrongExpectedConclusion,
}

#[derive(Debug, Clone)]
struct ProofCase {
    id: String,
    proof: Proof,
    expected: Proposition,
    context: LocalContext,
    should_accept: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct PropositionMetrics {
    pub cases: usize,
    pub expected_accepts: usize,
    pub accepted: usize,
    pub replay_verified: usize,
    pub false_acceptances: usize,
    pub false_rejections: usize,
    pub failure_taxonomy: BTreeMap<String, usize>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PropositionBenchmarkReport {
    pub seed: u64,
    pub generated_cases: usize,
    pub total: PropositionMetrics,
    pub development: PropositionMetrics,
    pub holdout: PropositionMetrics,
    pub deterministic: bool,
}

fn number(value: i64) -> SymExpr {
    SymExpr::Num(value as f64)
}

fn binding(name: &str, value: SymExpr) -> Substitution {
    let mut substitution = Substitution::new();
    substitution.insert(name, value);
    substitution
}

fn theorem(id: u64, substitution: Substitution, premise_proofs: Vec<Proof>) -> Proof {
    Proof::Theorem {
        id: TheoremId(id),
        subst: substitution,
        premise_proofs,
    }
}

fn case_at(index: usize, seed: u64) -> ProofCase {
    let value = (((seed.wrapping_add(index as u64 * 17)) % 13) as i64) - 6;
    let positive = value.abs().max(1);
    let a = number(value);
    let b = number(value + 2);
    let c = number(value + 4);
    let mut context = LocalContext::new();
    let (kind, proof, expected, should_accept) = match index % 14 {
        0 => {
            let expected = Proposition::eq(a.clone(), a.clone());
            (ProofCaseKind::Reflexivity, Proof::Refl(a), expected, true)
        }
        1 => {
            let expected = Proposition::eq(
                SymExpr::Add(Box::new(a.clone()), Box::new(number(0))),
                a.clone(),
            );
            (
                ProofCaseKind::AddZero,
                theorem(7, binding("x", a), vec![]),
                expected,
                true,
            )
        }
        2 => {
            let expected = Proposition::eq(
                SymExpr::Mul(Box::new(a.clone()), Box::new(number(1))),
                a.clone(),
            );
            (
                ProofCaseKind::MultiplicativeIdentity,
                theorem(8, binding("x", a), vec![]),
                expected,
                true,
            )
        }
        3 => {
            let expected = Proposition::eq(
                SymExpr::Add(Box::new(a.clone()), Box::new(b.clone())),
                SymExpr::Add(Box::new(b.clone()), Box::new(a.clone())),
            );
            (
                ProofCaseKind::Commutativity,
                theorem(
                    10,
                    {
                        let mut s = Substitution::new();
                        s.insert("a", a);
                        s.insert("b", b);
                        s
                    },
                    vec![],
                ),
                expected,
                true,
            )
        }
        4 => {
            let expected = Proposition::eq(
                SymExpr::Mul(
                    Box::new(a.clone()),
                    Box::new(SymExpr::Add(Box::new(b.clone()), Box::new(c.clone()))),
                ),
                SymExpr::Add(
                    Box::new(SymExpr::Mul(Box::new(a.clone()), Box::new(b.clone()))),
                    Box::new(SymExpr::Mul(Box::new(a), Box::new(c.clone()))),
                ),
            );
            (
                ProofCaseKind::Distributivity,
                theorem(
                    12,
                    {
                        let mut s = Substitution::new();
                        s.insert("a", number(value));
                        s.insert("b", number(value + 2));
                        s.insert("c", number(value + 4));
                        s
                    },
                    vec![],
                ),
                expected,
                true,
            )
        }
        5 => {
            let premise = Proposition::eq(a.clone(), b.clone());
            let hypothesis = context.add_hypothesis(premise);
            let expected = Proposition::eq(b.clone(), a.clone());
            (
                ProofCaseKind::SymmetryWithPremise,
                theorem(
                    2,
                    {
                        let mut s = Substitution::new();
                        s.insert("a", a);
                        s.insert("b", b);
                        s
                    },
                    vec![Proof::Hypothesis(hypothesis)],
                ),
                expected,
                true,
            )
        }
        6 => {
            let premise = Proposition::and(
                Proposition::eq(a.clone(), b.clone()),
                Proposition::eq(b.clone(), c.clone()),
            );
            let hypothesis = context.add_hypothesis(premise);
            let expected = Proposition::eq(a.clone(), c.clone());
            (
                ProofCaseKind::TransitivityWithPremise,
                theorem(
                    3,
                    {
                        let mut s = Substitution::new();
                        s.insert("a", a);
                        s.insert("b", b);
                        s.insert("c", c);
                        s
                    },
                    vec![Proof::Hypothesis(hypothesis)],
                ),
                expected,
                true,
            )
        }
        7 => {
            let x = number(positive);
            let premise = Proposition::ge(x.clone(), number(0));
            let expected = Proposition::eq(SymExpr::Abs(Box::new(x.clone())), x.clone());
            (
                ProofCaseKind::NonnegativeAbs,
                theorem(
                    5,
                    binding("x", x),
                    vec![Proof::Certificate {
                        proposition: premise,
                        certificate: Certificate::ConstantEvaluation,
                    }],
                ),
                expected,
                true,
            )
        }
        8 => {
            let x = shared_var("u");
            let body = Proof::Refl(SymExpr::Var(x.clone()));
            let expected = Proposition::forall(
                &x,
                Proposition::eq(SymExpr::Var(x.clone()), SymExpr::Var(x.clone())),
            );
            (
                ProofCaseKind::UniversalIntroduction,
                Proof::Intro {
                    binder: Binder::ForAll { variable: x },
                    body: Box::new(body),
                },
                expected,
                true,
            )
        }
        9 => {
            let expected =
                Proposition::eq(SymExpr::Add(Box::new(a.clone()), Box::new(number(0))), a);
            (
                ProofCaseKind::MissingBinder,
                theorem(7, Substitution::new(), vec![]),
                expected,
                false,
            )
        }
        10 => {
            let x = number(-positive);
            let expected = Proposition::ge(x.clone(), number(0));
            (
                ProofCaseKind::WrongCertificate,
                Proof::Certificate {
                    proposition: Proposition::ge(x, number(0)),
                    certificate: Certificate::ConstantEvaluation,
                },
                expected,
                false,
            )
        }
        11 => {
            let x = number(positive);
            let expected = Proposition::eq(SymExpr::Abs(Box::new(x.clone())), x.clone());
            (
                ProofCaseKind::MissingPremise,
                theorem(5, binding("x", x), vec![]),
                expected,
                false,
            )
        }
        12 => {
            let expected = Proposition::eq(a.clone(), a);
            (
                ProofCaseKind::UnknownTheorem,
                theorem(999_999, Substitution::new(), vec![]),
                expected,
                false,
            )
        }
        _ => {
            let expected = Proposition::eq(a.clone(), b.clone());
            (
                ProofCaseKind::WrongExpectedConclusion,
                Proof::Refl(a),
                expected,
                false,
            )
        }
    };
    let _ = kind;
    ProofCase {
        id: format!("prop-{index:05}{}", if index % 5 == 0 { "-h" } else { "" }),
        proof,
        expected,
        context,
        should_accept,
    }
}

fn failure_label(error: &crate::kernel::KernelError) -> &'static str {
    use crate::kernel::KernelError;
    match error {
        KernelError::ExpectedMismatch { .. } => "expected_mismatch",
        KernelError::NoSuchTheorem(_) => "unknown_theorem",
        KernelError::NoSuchHypothesis(_) => "unknown_hypothesis",
        KernelError::PremiseMismatch { .. } => "premise_mismatch",
        KernelError::PremiseCountMismatch { .. } => "premise_count_mismatch",
        KernelError::UninstantiatedBinder(_) => "uninstantiated_binder",
        KernelError::TransitivityMismatch { .. } => "transitivity_mismatch",
        KernelError::TransitivityNonEquality { .. } => "transitivity_non_equality",
        KernelError::ConstructorArityMismatch { .. } => "constructor_arity_mismatch",
        KernelError::CongruenceArgNotEquality { .. } => "congruence_argument_not_equality",
        KernelError::CertificateRejected(_) => "certificate_rejected",
        KernelError::IntroArbitraryClash { .. } => "intro_arbitrary_clash",
        KernelError::IntroBodyMismatch { .. } => "intro_body_mismatch",
    }
}

fn evaluate_slice(cases: &[ProofCase], checker: &ProofChecker) -> PropositionMetrics {
    let mut metrics = PropositionMetrics::default();
    for case in cases {
        metrics.cases += 1;
        metrics.expected_accepts += usize::from(case.should_accept);
        let first = checker.check(&case.context, &case.proof, &case.expected);
        match first {
            Ok(()) => {
                metrics.accepted += 1;
                metrics.false_acceptances += usize::from(!case.should_accept);
                let replay = checker
                    .check(&case.context, &case.proof, &case.expected)
                    .is_ok();
                metrics.replay_verified += usize::from(replay);
                metrics.false_rejections += usize::from(case.should_accept && !replay);
            }
            Err(error) => {
                *metrics
                    .failure_taxonomy
                    .entry(failure_label(&error).into())
                    .or_default() += 1;
                metrics.false_rejections += usize::from(case.should_accept);
            }
        }
    }
    metrics
}

pub fn evaluate(count: usize, seed: u64) -> PropositionBenchmarkReport {
    let cases: Vec<_> = (0..count).map(|index| case_at(index, seed)).collect();
    let development: Vec<_> = cases
        .iter()
        .filter(|case| !case.id.ends_with("-h"))
        .cloned()
        .collect();
    let holdout: Vec<_> = cases
        .iter()
        .filter(|case| case.id.ends_with("-h"))
        .cloned()
        .collect();
    let checker = ProofChecker::new(TheoremEnvironment::with_initial_theorems());
    PropositionBenchmarkReport {
        seed,
        generated_cases: count,
        total: evaluate_slice(&cases, &checker),
        development: evaluate_slice(&development, &checker),
        holdout: evaluate_slice(&holdout, &checker),
        deterministic: true,
    }
}

fn result_for(
    name: &str,
    metrics: &PropositionMetrics,
    seed: u64,
    commit: &str,
) -> ExperimentResult {
    let mut values = BTreeMap::new();
    values.insert("cases".into(), metrics.cases as f64);
    values.insert("expected_accepts".into(), metrics.expected_accepts as f64);
    values.insert("accepted".into(), metrics.accepted as f64);
    values.insert(
        "acceptance_rate".into(),
        metrics.accepted as f64 / metrics.cases.max(1) as f64,
    );
    values.insert(
        "replay_rate".into(),
        metrics.replay_verified as f64 / metrics.accepted.max(1) as f64,
    );
    values.insert(
        "false_acceptance_rate".into(),
        metrics.false_acceptances as f64 / metrics.cases.max(1) as f64,
    );
    values.insert(
        "false_rejection_rate".into(),
        metrics.false_rejections as f64 / metrics.cases.max(1) as f64,
    );
    for (label, count) in &metrics.failure_taxonomy {
        values.insert(format!("failure_{label}"), *count as f64);
    }
    ExperimentResult {
        experiment: format!("proposition_{name}"),
        claim: "trusted proposition proofs accept only valid theorem instances and replay deterministically".into(),
        commit: commit.into(),
        seed,
        dataset: Some(name.into()),
        baseline: "trusted proposition proof checker".into(),
        metrics: values.into_iter().collect(),
        passed: metrics.false_acceptances == 0
            && metrics.false_rejections == 0
            && metrics.replay_verified == metrics.accepted,
        notes: format!("failure_taxonomy={:?}", metrics.failure_taxonomy),
    }
}

pub fn experiment_results(
    report: &PropositionBenchmarkReport,
    commit: impl Into<String>,
) -> Vec<ExperimentResult> {
    let commit = commit.into();
    vec![
        result_for("total", &report.total, report.seed, &commit),
        result_for("development", &report.development, report.seed, &commit),
        result_for("holdout", &report.holdout, report.seed, &commit),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn proof_benchmark_is_deterministic_and_fail_closed() {
        let first = evaluate(500, 42);
        assert_eq!(first, evaluate(500, 42));
        assert_eq!(first.total.cases, 500);
        assert_eq!(first.total.expected_accepts, 324);
        assert_eq!(first.total.false_acceptances, 0);
        assert_eq!(first.total.false_rejections, 0);
        assert_eq!(first.total.replay_verified, first.total.accepted);
        assert_eq!(first.holdout.cases, 100);
        assert!(first
            .holdout
            .failure_taxonomy
            .contains_key("certificate_rejected"));
        assert!(first
            .holdout
            .failure_taxonomy
            .contains_key("uninstantiated_binder"));
    }

    #[test]
    fn results_expose_replay_and_refusal_metrics() {
        let report = evaluate(50, 7);
        let results = experiment_results(&report, "test");
        assert_eq!(results.len(), 3);
        assert!(results.iter().all(|result| result.passed));
        assert_eq!(results[0].metric("false_acceptance_rate"), Some(0.0));
    }
}
