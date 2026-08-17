//! Stage 117: shifted technical-language transfer for number theory.
//!
//! The corpus is independently authored from the pack's typed contract. The
//! frontend must preserve ambiguity and missing bindings; only complete typed
//! requests reach the arithmetic pack.

use serde::Serialize;
use sha2::{Digest, Sha256};
use the_machine::number_theory_frontend::{
    formalize_number_theory_text, replay_verified as frontend_replay, NumberTheoryFrontendStatus,
};
use the_machine::number_theory_pack::{
    evaluate_number_theory, NumberTheoryOperation, NumberTheoryStatus,
};

const SUPPORTED: usize = 600;
const AMBIGUOUS: usize = 200;
const MISSING: usize = 200;
const UNSUPPORTED: usize = 200;
const CASES: usize = SUPPORTED + AMBIGUOUS + MISSING + UNSUPPORTED;
const AMBIGUOUS_START: usize = SUPPORTED;
const MISSING_START: usize = SUPPORTED + AMBIGUOUS;
const UNSUPPORTED_START: usize = SUPPORTED + AMBIGUOUS + MISSING;

fn digest<T: Serialize + ?Sized>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn supported_text(operation: usize, variant: usize) -> String {
    match operation {
        0 => match variant % 3 {
            0 => "Compute the greatest common divisor and Bezout coefficients for a=84 and b=30."
                .into(),
            1 => "Use Bézout's identity to find gcd for a = 84 and b = 30.".into(),
            _ => "Find the gcd(a=84,b=30) together with a Bezout witness.".into(),
        },
        1 => match variant % 3 {
            0 => "Find the least nonnegative modular inverse of a=7 modulo m=20.".into(),
            1 => "Determine the inverse modulo m = 20 for the value a = 7.".into(),
            _ => "Compute a=7's modular inverse mod m=20.".into(),
        },
        2 => match variant % 3 {
            0 => "Solve the linear congruence a=6 x congruent to b=9 modulo m=15.".into(),
            1 => "Find x from a = 6 x ≡ b = 9 (mod m = 15).".into(),
            _ => {
                "Resolve the congruence with coefficient a=6, right side b=9, modulus m=15.".into()
            }
        },
        3 => {
            match variant % 3 {
                0 => "Solve the Chinese remainder system x ≡ a=2 (mod m=3) and x ≡ b=3 (mod n=5)."
                    .into(),
                1 => "Find the simultaneous congruences a=2 modulo m=3 and b=3 modulo n=5.".into(),
                _ => "Apply the Chinese remainder theorem to residues a=2,b=3 and moduli m=3,n=5."
                    .into(),
            }
        }
        4 => match variant % 3 {
            0 => "Compute Euler's totient phi(n=36).".into(),
            1 => "Find the value of the Euler totient function at n = 36.".into(),
            _ => "Evaluate φ(n=36) exactly.".into(),
        },
        _ => match variant % 3 {
            0 => "Find a witness for the linear Diophantine equation a=6 x + b=9 y = c=15.".into(),
            1 => "Solve a=6 x plus b=9 y equals c=15 as a bounded Diophantine problem.".into(),
            _ => "Give Bezout-scaled coefficients for a=6, b=9, c=15 in a Diophantine equation."
                .into(),
        },
    }
}

fn ambiguous_text(index: usize) -> String {
    match index % 3 {
        0 => "Compute the gcd or modular inverse for a=7 and b=20.".into(),
        1 => "Either solve the congruence or apply the Chinese remainder theorem with a=2,b=3,m=3,n=5.".into(),
        _ => "The problem may ask for a totient or an inverse; n=36 and m=20 are supplied.".into(),
    }
}

fn missing_text(index: usize) -> String {
    match index % 3 {
        0 => "Compute the modular inverse of a=7, but the modulus is omitted.".into(),
        1 => {
            "Solve the linear congruence with a=6 and modulus m=15, without the right side.".into()
        }
        _ => "Find the Chinese remainder class using a=2 and b=3, but no moduli are stated.".into(),
    }
}

fn unsupported_text(index: usize) -> String {
    match index % 4 {
        0 => "Infer the cryptographic security of modulus m=20 from this inverse.".into(),
        1 => "Prove the asymptotic number-theory behavior of a=7 as the modulus grows.".into(),
        2 => "Give an unbounded prime factorization and analytic number theory conclusion.".into(),
        _ => "Determine the topology of this integer quotient rather than a bounded arithmetic result.".into(),
    }
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    cases: usize,
    supported_cases: usize,
    ambiguous_cases: usize,
    missing_cases: usize,
    unsupported_cases: usize,
    frontend_complete: usize,
    downstream_complete: usize,
    ambiguity_preserved: usize,
    missing_preserved: usize,
    unsupported_refused: usize,
    frontend_replay_verified: usize,
    downstream_replay_verified: usize,
    frontend_tamper_rejections: usize,
    downstream_tamper_rejections: usize,
    false_authorizations: usize,
    false_denials: usize,
    route_mismatches: usize,
    corpus_sha256: String,
}

fn main() {
    let corpus: Vec<String> = (0..SUPPORTED)
        .map(|i| supported_text(i / 100, i))
        .chain((0..AMBIGUOUS).map(|i| ambiguous_text(i)))
        .chain((0..MISSING).map(|i| missing_text(i)))
        .chain((0..UNSUPPORTED).map(|i| unsupported_text(i)))
        .collect();
    assert_eq!(corpus.len(), CASES);

    let mut frontend_complete = 0;
    let mut downstream_complete = 0;
    let mut ambiguity_preserved = 0;
    let mut missing_preserved = 0;
    let mut unsupported_refused = 0;
    let mut frontend_replay_verified = 0;
    let mut downstream_replay_verified = 0;
    let mut frontend_tamper_rejections = 0;
    let mut downstream_tamper_rejections = 0;
    let mut false_authorizations = 0;
    let mut false_denials = 0;
    let mut route_mismatches = 0;

    for (index, text) in corpus.iter().enumerate() {
        let frontend = formalize_number_theory_text(text, &format!("stage117-{index:04}"));
        if frontend_replay(&frontend) {
            frontend_replay_verified += 1;
        }
        let mut frontend_tampered = frontend.clone();
        frontend_tampered.replay_hash.push('x');
        if !frontend_replay(&frontend_tampered) {
            frontend_tamper_rejections += 1;
        }
        match index {
            0..SUPPORTED => {
                if frontend.status != NumberTheoryFrontendStatus::Complete {
                    route_mismatches += 1;
                    continue;
                }
                frontend_complete += 1;
                let result = evaluate_number_theory(frontend.request.as_ref().unwrap());
                if result.status == NumberTheoryStatus::Complete {
                    downstream_complete += 1;
                } else {
                    route_mismatches += 1;
                }
                if result.replay_verified() {
                    downstream_replay_verified += 1;
                }
                let mut tampered = result.clone();
                tampered.replay_hash.push('x');
                if !tampered.replay_verified() {
                    downstream_tamper_rejections += 1;
                }
                if result.status == NumberTheoryStatus::Complete && result.artifact.is_none() {
                    false_authorizations += 1;
                }
            }
            AMBIGUOUS_START..MISSING_START => {
                if frontend.status == NumberTheoryFrontendStatus::Ambiguous {
                    ambiguity_preserved += 1;
                } else {
                    false_denials += 1;
                }
            }
            MISSING_START..UNSUPPORTED_START => {
                if frontend.status == NumberTheoryFrontendStatus::Missing {
                    missing_preserved += 1;
                } else {
                    false_denials += 1;
                }
            }
            _ => {
                if frontend.status == NumberTheoryFrontendStatus::Unsupported {
                    unsupported_refused += 1;
                } else {
                    false_authorizations += 1;
                }
            }
        }
    }

    assert_eq!(frontend_complete, SUPPORTED);
    assert_eq!(downstream_complete, SUPPORTED);
    assert_eq!(ambiguity_preserved, AMBIGUOUS);
    assert_eq!(missing_preserved, MISSING);
    assert_eq!(unsupported_refused, UNSUPPORTED);
    assert_eq!(frontend_replay_verified, CASES);
    assert_eq!(downstream_replay_verified, SUPPORTED);
    assert_eq!(frontend_tamper_rejections, CASES);
    assert_eq!(downstream_tamper_rejections, SUPPORTED);
    assert_eq!(false_authorizations, 0);
    assert_eq!(false_denials, 0);
    assert_eq!(route_mismatches, 0);

    let report = Report {
        schema: "stage117-number-theory-language-transfer-v1",
        cases: CASES,
        supported_cases: SUPPORTED,
        ambiguous_cases: AMBIGUOUS,
        missing_cases: MISSING,
        unsupported_cases: UNSUPPORTED,
        frontend_complete,
        downstream_complete,
        ambiguity_preserved,
        missing_preserved,
        unsupported_refused,
        frontend_replay_verified,
        downstream_replay_verified,
        frontend_tamper_rejections,
        downstream_tamper_rejections,
        false_authorizations,
        false_denials,
        route_mismatches,
        corpus_sha256: digest(&corpus),
    };
    println!("{}", serde_json::to_string_pretty(&report).unwrap());
}
