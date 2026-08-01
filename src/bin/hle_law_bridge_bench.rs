//! Phase 31 independent boundary benchmark for the shadow law bridges.

use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::env;
use std::fs;
use the_machine::law_bridge::{
    bind_equation, lookup_law, replay_binding, replay_lookup, BindingStatus,
    EquationBindingRequest, LawLookupRequest, LawRecord, LookupStatus, QuantityBinding,
};

#[derive(Debug, Serialize)]
struct Report {
    schema_version: String,
    corpus_cases: usize,
    lookup_cases: usize,
    binding_cases: usize,
    lookup_unique: usize,
    lookup_ambiguous: usize,
    lookup_unsupported_or_missing: usize,
    binding_complete: usize,
    binding_rejected: usize,
    replay_verified: usize,
    false_authorizations: usize,
    hle_retrieval_ready_cases: usize,
    hle_authorized_answers: usize,
    corpus_sha256: String,
    source_hle_audit_sha256: String,
    registry_mutated: bool,
    method: String,
}

fn hash<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn law(
    id: &str,
    aliases: &[&str],
    domain: &str,
    equation: &str,
    variables: &[&str],
    units: &[&str],
) -> LawRecord {
    LawRecord {
        law_id: id.into(),
        aliases: aliases.iter().map(|value| (*value).into()).collect(),
        domain: domain.into(),
        equation: equation.into(),
        variables: variables.iter().map(|value| (*value).into()).collect(),
        assumptions: vec!["record-specific validity conditions apply".into()],
        validity_domain: "bounded independent benchmark domain".into(),
        unit_constraints: units.iter().map(|value| (*value).into()).collect(),
        provenance: format!("independent-corpus:{id}"),
    }
}

fn catalog() -> Vec<LawRecord> {
    let ohm = law(
        "ohms_law",
        &["ohm law", "resistance law"],
        "physics",
        "V = I * R",
        &["V", "I", "R"],
        &["V=volt", "I=ampere", "R=ohm"],
    );
    let newton = law(
        "newtons_second_law",
        &["newton second law", "force law"],
        "physics",
        "F = m * a",
        &["F", "m", "a"],
        &["F=newton", "m=kilogram", "a=m/s^2"],
    );
    let ideal_gas = law(
        "ideal_gas_law",
        &["ideal gas", "gas law"],
        "chemistry",
        "P * V = n * R * T",
        &["P", "V", "n", "R", "T"],
        &["P=pascal", "V=m^3", "T=kelvin"],
    );
    let energy_a = law(
        "energy_a",
        &["energy law"],
        "physics",
        "E = m * c^2",
        &["E", "m", "c"],
        &["E=joule", "m=kilogram"],
    );
    let energy_b = law(
        "energy_b",
        &["energy law"],
        "physics",
        "E = h * f",
        &["E", "h", "f"],
        &["E=joule"],
    );
    vec![ohm, newton, ideal_gas, energy_a, energy_b]
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let catalog = catalog();
    let mut replay_verified = 0;
    let mut lookup_cases = 0;
    let mut binding_cases = 0;
    let mut lookup_unique = 0;
    let mut lookup_ambiguous = 0;
    let mut lookup_unsupported_or_missing = 0;
    let mut binding_complete = 0;
    let mut binding_rejected = 0;
    let mut false_authorizations = 0;
    let mut corpus_specs: Vec<String> = Vec::new();

    for (name, domain, vars) in [
        ("ohm law", Some("physics"), vec!["V", "I", "R"]),
        ("resistance law", Some("physics"), vec!["V", "I", "R"]),
        ("newton second law", Some("physics"), vec!["F", "m", "a"]),
        ("force law", Some("physics"), vec!["F", "m", "a"]),
        (
            "ideal gas",
            Some("chemistry"),
            vec!["P", "V", "n", "R", "T"],
        ),
        ("gas law", Some("chemistry"), vec!["P", "V", "n", "R", "T"]),
    ] {
        lookup_cases += 1;
        let result = lookup_law(
            &LawLookupRequest {
                name_or_alias: name.into(),
                domain: domain.map(str::to_string),
                requested_variables: vars.into_iter().map(String::from).collect(),
                context: "independent positive case".into(),
            },
            &catalog,
        );
        if result.status == LookupStatus::Unique {
            lookup_unique += 1;
        }
        replay_verified += usize::from(replay_lookup(&result));
        corpus_specs.push(format!("lookup-positive:{:?}", result.status));
    }
    for _ in 0..4 {
        lookup_cases += 1;
        let result = lookup_law(
            &LawLookupRequest {
                name_or_alias: "energy law".into(),
                domain: Some("physics".into()),
                requested_variables: vec!["E".into()],
                context: "ambiguous alias".into(),
            },
            &catalog,
        );
        lookup_ambiguous += usize::from(result.status == LookupStatus::Ambiguous);
        false_authorizations += usize::from(result.status == LookupStatus::Unique);
        replay_verified += usize::from(replay_lookup(&result));
        corpus_specs.push(format!("lookup-ambiguous:{:?}", result.status));
    }
    for name in [
        "unknown law",
        "",
        "unsupported convention",
        "law from another domain",
    ] {
        lookup_cases += 1;
        let result = lookup_law(
            &LawLookupRequest {
                name_or_alias: name.into(),
                domain: None,
                requested_variables: vec![],
                context: "unsupported boundary".into(),
            },
            &catalog,
        );
        lookup_unsupported_or_missing += usize::from(matches!(
            result.status,
            LookupStatus::Unsupported | LookupStatus::Missing
        ));
        false_authorizations += usize::from(result.status == LookupStatus::Unique);
        replay_verified += usize::from(replay_lookup(&result));
        corpus_specs.push(format!("lookup-boundary:{:?}", result.status));
    }

    let ohm = catalog[0].clone();
    for complete in [true, true, true, true] {
        binding_cases += 1;
        let bindings = if complete {
            vec![
                QuantityBinding {
                    symbol: "I".into(),
                    value: "2".into(),
                    unit: Some("ampere".into()),
                    provenance: "prompt:I".into(),
                },
                QuantityBinding {
                    symbol: "R".into(),
                    value: "5".into(),
                    unit: Some("ohm".into()),
                    provenance: "prompt:R".into(),
                },
            ]
        } else {
            Vec::new()
        };
        let result = bind_equation(&EquationBindingRequest {
            law: ohm.clone(),
            bindings,
            requested_output: "V".into(),
        });
        binding_complete += usize::from(result.status == BindingStatus::Complete);
        binding_rejected += usize::from(result.status != BindingStatus::Complete);
        false_authorizations +=
            usize::from(result.status != BindingStatus::Complete && result.artifact.is_some());
        replay_verified += usize::from(replay_binding(&result));
        corpus_specs.push(format!("binding-positive:{:?}", result.status));
    }
    for (bindings, output) in [
        (
            vec![QuantityBinding {
                symbol: "I".into(),
                value: "2".into(),
                unit: None,
                provenance: "prompt:I".into(),
            }],
            "V",
        ),
        (
            vec![QuantityBinding {
                symbol: "I".into(),
                value: "2".into(),
                unit: Some("ampere".into()),
                provenance: "prompt:I".into(),
            }],
            "V",
        ),
        (
            vec![
                QuantityBinding {
                    symbol: "I".into(),
                    value: "2".into(),
                    unit: Some("ampere".into()),
                    provenance: "prompt:I".into(),
                },
                QuantityBinding {
                    symbol: "I".into(),
                    value: "3".into(),
                    unit: Some("ampere".into()),
                    provenance: "prompt:I2".into(),
                },
            ],
            "V",
        ),
        (
            vec![
                QuantityBinding {
                    symbol: "I".into(),
                    value: "2".into(),
                    unit: Some("ampere".into()),
                    provenance: "prompt:I".into(),
                },
                QuantityBinding {
                    symbol: "R".into(),
                    value: "5".into(),
                    unit: Some("ohm".into()),
                    provenance: "prompt:R".into(),
                },
            ],
            "unknown",
        ),
        (
            vec![
                QuantityBinding {
                    symbol: "I".into(),
                    value: "2".into(),
                    unit: Some("volt".into()),
                    provenance: "prompt:I-wrong-unit".into(),
                },
                QuantityBinding {
                    symbol: "R".into(),
                    value: "5".into(),
                    unit: Some("ohm".into()),
                    provenance: "prompt:R".into(),
                },
            ],
            "V",
        ),
    ] {
        binding_cases += 1;
        let result = bind_equation(&EquationBindingRequest {
            law: ohm.clone(),
            bindings,
            requested_output: output.into(),
        });
        binding_complete += usize::from(result.status == BindingStatus::Complete);
        binding_rejected += usize::from(result.status != BindingStatus::Complete);
        false_authorizations +=
            usize::from(result.status != BindingStatus::Complete && result.artifact.is_some());
        replay_verified += usize::from(replay_binding(&result));
        corpus_specs.push(format!("binding-boundary:{:?}", result.status));
    }
    let hle_bytes = fs::read("docs/phase30_hle_law_audit.json")?;
    let source_hle_audit_sha256 = format!("{:x}", Sha256::digest(&hle_bytes));
    let hle_report: Value = serde_json::from_slice(&hle_bytes)?;
    let hle_retrieval_ready_cases = hle_report["cases"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|case| case["outcome"] == "retrieval_ready_equation")
        .count();
    let report = Report {
        schema_version: "phase31.law_bridge.v1".into(),
        corpus_cases: corpus_specs.len(), lookup_cases, binding_cases, lookup_unique,
        lookup_ambiguous, lookup_unsupported_or_missing, binding_complete, binding_rejected,
        replay_verified, false_authorizations, hle_retrieval_ready_cases,
        hle_authorized_answers: 0, corpus_sha256: hash(&corpus_specs),
        source_hle_audit_sha256,
        registry_mutated: false,
        method: "shadow-only law lookup and equation binding benchmark; HLE holdouts remain non-authorizing".into(),
    };
    let output = serde_json::to_string_pretty(&report)?;
    let output_path = env::args()
        .nth(1)
        .unwrap_or_else(|| "/tmp/hle_law_bridge_bench.json".into());
    fs::write(output_path, &output)?;
    println!("{}", output);
    Ok(())
}
