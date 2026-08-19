//! Stage 363: execute two residual sources through the generic metadata gate.
//!
//! Complex-analysis and finite-character source documents were previously
//! residual evidence. Their explicit provenance/scope is now parsed by the
//! generic metadata layer, then handed to their existing bounded packs. The
//! benchmark tests source identity, boundaries, replay, and tamper behavior;
//! it does not add subject-specific routing or solver code.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::fs;
use the_machine::bounded_complex_analysis_pack::{
    evaluate_complex_analysis, replay_verified as complex_replay_verified,
    ComplexAnalysisOperation, ComplexAnalysisRequest, ComplexAnalysisStatus, ComplexNumber,
};
use the_machine::dirichlet_character_pack::{
    evaluate, CharacterOperation, CharacterStatus, DirichletCharacterRequest,
};
use the_machine::probability_pack::Rational;
use the_machine::source_metadata::{extract_source_metadata, replay_verified};

const COMPLEX_SOURCE: &str = "docs/sources/openstax_bounded_complex_analysis_source.txt";
const CHARACTER_SOURCE: &str = "docs/sources/mit_analytic_number_theory_character_definition.txt";
const REPORT_JSON: &str = "docs/stage363_residual_source_execution.json";
const REPORT_MD: &str = "docs/stage363_residual_source_execution.md";

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    sources: usize,
    metadata_replays: usize,
    metadata_tamper_rejections: usize,
    source_mutations: usize,
    source_mutations_rejected: usize,
    execution_cases: usize,
    exact_decisions: usize,
    execution_replays: usize,
    execution_tamper_rejections: usize,
    complete_cases: usize,
    ambiguous_cases: usize,
    missing_cases: usize,
    unsupported_or_invalid_cases: usize,
    source_matches: usize,
    false_authorizations: usize,
    false_denials: usize,
    production_mutations: usize,
    report_sha256: String,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(value).expect("report serializes"))
    )
}

fn without_key(document: &str, key: &str) -> String {
    let prefix = format!("{key}:");
    document
        .lines()
        .filter(|line| {
            let upper = line.trim_start().to_ascii_uppercase();
            let evidence_alias = key == "EVIDENCE"
                && (upper.starts_with("EVIDENCE:") || upper.starts_with("EVIDENCE_SPAN:"));
            !upper.starts_with(&prefix) && !evidence_alias
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn rational(numerator: i128, denominator: i128) -> Rational {
    Rational::new(numerator, denominator).unwrap()
}

fn complex(real: i128, imag: i128) -> ComplexNumber {
    ComplexNumber::new(rational(real, 1), rational(imag, 1))
}

fn complex_requests(provenance: &[String]) -> Vec<(ComplexAnalysisRequest, ComplexAnalysisStatus)> {
    let base = |operation| ComplexAnalysisRequest {
        operation,
        coefficients: vec![complex(1, 0), complex(2, 1), complex(1, 0)],
        point: Some(complex(1, 1)),
        ux: Some(rational(1, 1)),
        uy: Some(rational(0, 1)),
        vx: Some(rational(0, 1)),
        vy: Some(rational(1, 1)),
        domain: "bounded_exact_complex_analysis".into(),
        ambiguity: None,
        provenance: provenance.to_vec(),
    };
    let mut cases = vec![
        (
            base(ComplexAnalysisOperation::PolynomialValue),
            ComplexAnalysisStatus::Complete,
        ),
        (
            base(ComplexAnalysisOperation::PolynomialDerivative),
            ComplexAnalysisStatus::Complete,
        ),
        (
            base(ComplexAnalysisOperation::CauchyRiemannCheck),
            ComplexAnalysisStatus::Complete,
        ),
        (
            base(ComplexAnalysisOperation::AffineHolomorphicDerivative),
            ComplexAnalysisStatus::Complete,
        ),
    ];
    let mut polar = base(ComplexAnalysisOperation::PolarConversion);
    cases.push((polar, ComplexAnalysisStatus::Unsupported));
    let mut missing = base(ComplexAnalysisOperation::PolynomialValue);
    missing.point = None;
    cases.push((missing, ComplexAnalysisStatus::Missing));
    let mut ambiguous = base(ComplexAnalysisOperation::PolynomialValue);
    ambiguous.ambiguity = Some("coordinate convention unresolved".into());
    cases.push((ambiguous, ComplexAnalysisStatus::Ambiguous));
    cases
}

fn character_requests(provenance: &[String]) -> Vec<(DirichletCharacterRequest, CharacterStatus)> {
    let base = |operation| DirichletCharacterRequest {
        operation,
        modulus: Some(5),
        exponent: Some(1),
        value: Some(2),
        sum_limit: Some(16),
        domain: "bounded_dirichlet_character".into(),
        ambiguity: None,
        provenance: provenance.to_vec(),
    };
    let mut cases = vec![
        (
            base(CharacterOperation::ValidateCharacter),
            CharacterStatus::Complete,
        ),
        (
            base(CharacterOperation::Evaluate),
            CharacterStatus::Complete,
        ),
        (
            base(CharacterOperation::PartialSum),
            CharacterStatus::Complete,
        ),
        (
            base(CharacterOperation::Orthogonality),
            CharacterStatus::Complete,
        ),
    ];
    let mut composite = base(CharacterOperation::ValidateCharacter);
    composite.modulus = Some(6);
    cases.push((composite, CharacterStatus::Unsupported));
    let mut missing = base(CharacterOperation::Evaluate);
    missing.value = None;
    cases.push((missing, CharacterStatus::Missing));
    let mut ambiguous = base(CharacterOperation::Evaluate);
    ambiguous.ambiguity = Some("character generator convention unresolved".into());
    cases.push((ambiguous, CharacterStatus::Ambiguous));
    cases
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let source_paths = [COMPLEX_SOURCE, CHARACTER_SOURCE];
    let mut metadata_replays = 0;
    let mut metadata_tamper_rejections = 0;
    let mut source_mutations = 0;
    let mut source_mutations_rejected = 0;
    let mut source_matches = 0;
    let mut exact = 0;
    let mut execution_replays = 0;
    let mut execution_tamper = 0;
    let mut complete = 0;
    let mut ambiguous = 0;
    let mut missing = 0;
    let mut unsupported_or_invalid = 0;
    let mut false_auth = 0;
    let mut false_denial = 0;
    let mut execution_cases = 0;
    for path in source_paths {
        let bytes = fs::read(path)?;
        let text = std::str::from_utf8(&bytes)?;
        let metadata = extract_source_metadata(text).map_err(|errors| errors.join("; "))?;
        metadata_replays += usize::from(replay_verified(&metadata));
        let mut metadata_tampered = metadata.clone();
        metadata_tampered.document_sha256.push('x');
        metadata_tamper_rejections += usize::from(!replay_verified(&metadata_tampered));
        let mutations = [
            without_key(text, "SOURCE_ID"),
            without_key(text, "URL"),
            without_key(text, "EVIDENCE"),
        ];
        source_mutations += mutations.len();
        source_mutations_rejected += mutations
            .iter()
            .filter(|mutation| extract_source_metadata(mutation).is_err())
            .count();
        let provenance = vec![
            format!("source-id:{}", metadata.citation.source_id),
            format!("source-document-sha256:{}", metadata.document_sha256),
            "stage363-residual-source-execution".into(),
        ];
        if path == COMPLEX_SOURCE {
            for (request, expected) in complex_requests(&provenance) {
                let result = evaluate_complex_analysis(&request);
                source_matches += usize::from(result.source == metadata.citation);
                exact += usize::from(result.status == expected);
                execution_replays += usize::from(complex_replay_verified(&result));
                let mut tampered = result.clone();
                tampered.replay_hash.push('x');
                execution_tamper += usize::from(!complex_replay_verified(&tampered));
                false_auth += usize::from(
                    expected != ComplexAnalysisStatus::Complete
                        && result.status == ComplexAnalysisStatus::Complete,
                );
                false_denial += usize::from(
                    expected == ComplexAnalysisStatus::Complete
                        && result.status != ComplexAnalysisStatus::Complete,
                );
                complete += usize::from(result.status == ComplexAnalysisStatus::Complete);
                ambiguous += usize::from(result.status == ComplexAnalysisStatus::Ambiguous);
                missing += usize::from(result.status == ComplexAnalysisStatus::Missing);
                unsupported_or_invalid += usize::from(matches!(
                    result.status,
                    ComplexAnalysisStatus::Unsupported
                        | ComplexAnalysisStatus::InvalidDomain
                        | ComplexAnalysisStatus::Inconsistent
                ));
                execution_cases += 1;
            }
        } else {
            for (request, expected) in character_requests(&provenance) {
                let result = evaluate(&request);
                source_matches += usize::from(result.source == metadata.citation);
                exact += usize::from(result.status == expected);
                execution_replays += usize::from(result.replay_verified());
                let mut tampered = result.clone();
                tampered.replay_hash.push('x');
                execution_tamper += usize::from(!tampered.replay_verified());
                false_auth += usize::from(
                    expected != CharacterStatus::Complete
                        && result.status == CharacterStatus::Complete,
                );
                false_denial += usize::from(
                    expected == CharacterStatus::Complete
                        && result.status != CharacterStatus::Complete,
                );
                complete += usize::from(result.status == CharacterStatus::Complete);
                ambiguous += usize::from(result.status == CharacterStatus::Ambiguous);
                missing += usize::from(result.status == CharacterStatus::Missing);
                unsupported_or_invalid += usize::from(matches!(
                    result.status,
                    CharacterStatus::Unsupported
                        | CharacterStatus::InvalidDomain
                        | CharacterStatus::Inconsistent
                ));
                execution_cases += 1;
            }
        }
    }
    assert_eq!(metadata_replays, source_paths.len());
    assert_eq!(metadata_tamper_rejections, source_paths.len());
    assert_eq!(source_mutations, source_mutations_rejected);
    assert_eq!(execution_cases, 14);
    assert_eq!(exact, execution_cases);
    assert_eq!(execution_replays, execution_cases);
    assert_eq!(execution_tamper, execution_cases);
    assert_eq!(source_matches, execution_cases);
    assert_eq!(false_auth, 0);
    assert_eq!(false_denial, 0);
    let mut report = Report {
        schema: "stage363-residual-source-execution-v1",
        sources: source_paths.len(),
        metadata_replays,
        metadata_tamper_rejections,
        source_mutations,
        source_mutations_rejected,
        execution_cases,
        exact_decisions: exact,
        execution_replays,
        execution_tamper_rejections: execution_tamper,
        complete_cases: complete,
        ambiguous_cases: ambiguous,
        missing_cases: missing,
        unsupported_or_invalid_cases: unsupported_or_invalid,
        source_matches,
        false_authorizations: false_auth,
        false_denials: false_denial,
        production_mutations: 0,
        report_sha256: String::new(),
    };
    report.report_sha256 = digest(&report);
    fs::write(
        REPORT_JSON,
        format!("{}\n", serde_json::to_string_pretty(&report)?),
    )?;
    fs::write(
        REPORT_MD,
        format!(
            "# Stage 363 — residual source metadata execution\n\n\
* source documents / metadata replay / tamper: {} / {} / {}\n\
* source mutations / rejected: {} / {}\n\
* execution cases / exact decisions: {} / {}\n\
* execution replay / tamper rejection: {} / {}\n\
* complete / ambiguous / missing / unsupported-or-invalid: {} / {} / {} / {}\n\
* source citation matches: {}\n\
* false authorizations / denials: {} / {}\n\
* production mutations: {}\n\n\
Two previously residual source documents now pass the generic metadata gate and feed their existing bounded packs with exact source-linked provenance. The packs retain their declared finite boundaries; no subject-specific source parser, live route, or registry mutation was added.\n",
            report.sources,
            report.metadata_replays,
            report.metadata_tamper_rejections,
            report.source_mutations,
            report.source_mutations_rejected,
            report.execution_cases,
            report.exact_decisions,
            report.execution_replays,
            report.execution_tamper_rejections,
            report.complete_cases,
            report.ambiguous_cases,
            report.missing_cases,
            report.unsupported_or_invalid_cases,
            report.source_matches,
            report.false_authorizations,
            report.false_denials,
            report.production_mutations,
        ),
    )?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
