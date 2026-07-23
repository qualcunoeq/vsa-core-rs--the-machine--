use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{env, fs};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Case {
    id: String,
    prompt: String,
    outcome: String,
    family: String,
    signature: Option<String>,
    target: Option<String>,
    reason: Option<String>,
    pair_id: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct Corpus {
    schema_version: u32,
    release_id: String,
    status: String,
    oracle: String,
    cases: Vec<Case>,
}

fn supported(id: String, prompt: String, family: &str, signature: &str, target: &str, pair_id: Option<String>) -> Case {
    Case { id, prompt, outcome: "supported".into(), family: family.into(), signature: Some(signature.into()), target: Some(target.into()), reason: None, pair_id }
}

fn negative(id: String, prompt: String, outcome: &str, family: &str, reason: &str) -> Case {
    Case { id, prompt, outcome: outcome.into(), family: family.into(), signature: None, target: None, reason: Some(reason.into()), pair_id: None }
}

fn positive_cases() -> Vec<Case> {
    let mut cases = Vec::new();
    for i in 0..30 {
        let count = 2 + i % 9;
        let unit = 3 + i % 8;
        let total = count * unit;
        let pair = (i < 10).then(|| format!("qr.exp.pair.{:03}", i + 1));
        let prompt = if i % 2 == 0 {
            format!("{count} notebooks cost {total} dollars. What is the price per notebook?")
        } else {
            format!("A total of {total} dollars buys {count} notebooks; find dollars per notebook.")
        };
        cases.push(supported(format!("qr.exp.pos.{:03}", i + 1), prompt, "unit_rate", "[count:count,cost:currency]>currency/count>unit_rate", "unit_rate", pair.clone()));
        if i < 10 {
            let rewrite = format!("The total price is {total} dollars for {count} notebooks. Determine the dollars per notebook.");
            cases.push(supported(format!("qr.exp.pos.{:03}", i + 201), rewrite, "unit_rate", "[count:count,cost:currency]>currency/count>unit_rate", "unit_rate", pair));
        }
    }
    for i in 0..30 {
        let left = 2 + i % 5;
        let right = 3 + i % 6;
        let scale = 2 + i % 5;
        let left_count = left * scale;
        let right_count = right * scale;
        let pair = (i < 10).then(|| format!("qr.exp.pair.{:03}", i + 11));
        let prompt = if i % 2 == 0 {
            format!("The ratio of red beads to blue beads is {left}:{right}. If there are {left_count} red beads, how many blue beads are there?")
        } else {
            format!("For every {left} red beads there are {right} blue beads; the collection has {left_count} red beads. Find the blue count.")
        };
        cases.push(supported(format!("qr.exp.pos.{:03}", i + 41), prompt, "ratio", "[left:count,right:count,ratio]>count>ratio_target", "ratio_target", pair.clone()));
        if i < 10 {
            let rewrite = format!("There are {right_count} blue beads for {left_count} red beads under a {left}:{right} ratio. Find blue beads.");
            cases.push(supported(format!("qr.exp.pos.{:03}", i + 211), rewrite, "ratio", "[left:count,right:count,ratio]>count>ratio_target", "ratio_target", pair));
        }
    }
    for i in 0..30 {
        let source_items = 3 + i % 8;
        let source_units = 2 + i % 7;
        let target_items = 5 + i % 9;
        let pair = (i < 10).then(|| format!("qr.exp.pair.{:03}", i + 21));
        let prompt = if i % 2 == 0 {
            format!("{source_items} identical batches require {source_units} liters. How many liters are required for {target_items} batches at the same rate?")
        } else {
            format!("At a constant proportion, {source_units} liters serve {source_items} batches. Determine liters for {target_items} batches.")
        };
        cases.push(supported(format!("qr.exp.pos.{:03}", i + 81), prompt, "proportion", "[source:quantity,source_count,target_count]>quantity>scaled_quantity", "scaled_quantity", pair.clone()));
        if i < 10 {
            let rewrite = format!("Scale {source_units} liters for {source_items} batches to {target_items} batches at the same rate.");
            cases.push(supported(format!("qr.exp.pos.{:03}", i + 221), rewrite, "proportion", "[source:quantity,source_count,target_count]>quantity>scaled_quantity", "scaled_quantity", pair));
        }
    }
    let conversions = [(100, "centimeters", "meters", "length"), (1000, "meters", "kilometers", "length"), (60, "minutes", "hours", "time"), (12, "inches", "feet", "length"), (16, "ounces", "pounds", "mass"), (1000, "milliliters", "liters", "volume"), (24, "hours", "days", "time"), (7, "days", "weeks", "time")];
    for i in 0..20 {
        let (factor, small, large, kind) = conversions[i % conversions.len()];
        let amount = 2 + i % 9;
        let pair = (i < 10).then(|| format!("qr.exp.pair.{:03}", i + 31));
        let prompt = if i % 2 == 0 {
            format!("Using the stated conversion of {factor} {small} per {large}, convert {amount} {large} to {small}.")
        } else {
            format!("One {large} contains {factor} {small}. Express {amount} {large} in {small}.")
        };
        let signature = format!("[{kind}:{large},factor:{factor}{small}/{large}]>{small}>{kind}_converted");
        cases.push(supported(format!("qr.exp.pos.{:03}", i + 121), prompt, "unit_conversion", &signature, "converted_quantity", pair.clone()));
        if i < 10 {
            let rewrite = format!("Express {amount} {large} as {small}, given {factor} {small} per {large}.");
            cases.push(supported(format!("qr.exp.pos.{:03}", i + 231), rewrite, "unit_conversion", &signature, "converted_quantity", pair));
        }
    }
    for i in 0..20 {
        let first = 8 + i % 23;
        let second = 3 + i % 17;
        let verb = if i % 2 == 0 { "altogether" } else { "remain" };
        let pair = (i < 10).then(|| format!("qr.exp.pair.{:03}", i + 41));
        let prompt = if verb == "altogether" {
            format!("A box contains {first} red counters and {second} blue counters. How many counters are there altogether?")
        } else {
            format!("A container has {first} liters and {second} liters are removed. How many liters remain?")
        };
        let target = if verb == "altogether" { "total" } else { "remaining" };
        cases.push(supported(format!("qr.exp.pos.{:03}", i + 151), prompt, "sum_difference", "[first:quantity,second:quantity]>quantity>target", target, pair.clone()));
        if i < 10 {
            let rewrite = if verb == "altogether" {
                format!("There are {first} red counters plus {second} blue counters in the box. Find the total count.")
            } else {
                format!("After taking {second} liters from a container holding {first} liters, state the remaining volume.")
            };
            cases.push(supported(format!("qr.exp.pos.{:03}", i + 241), rewrite, "sum_difference", "[first:quantity,second:quantity]>quantity>target", target, pair));
        }
    }
    for i in 0..20 {
        let base = 10 + i;
        let increment = 2 + i % 7;
        let total = base + increment;
        let prompt = if i % 2 == 0 {
            format!("A quantity starts at {base} units and increases by {increment} units. What is the final quantity?")
        } else {
            format!("The final amount is {total} units after adding {increment} units. What was the starting amount?")
        };
        cases.push(supported(format!("qr.exp.pos.{:03}", i + 181), prompt, "linear_quantity", "[base:quantity,change:quantity]>quantity>linear_target", "linear_target", None));
    }
    cases
}

fn negative_cases() -> Vec<Case> {
    let templates = [
        ("percentage", "percentage_without_explicit_linear_relation", "A price changes by {n}% each year. What is the final price?"),
        ("compound_interest", "compound_interest", "A bank compounds {n}% interest monthly. What is the balance after a year?"),
        ("missing_anchor", "missing_numeric_anchor", "The ratio of apples to oranges is {n}:{m}, but neither fruit count is given. Find the apples."),
        ("nonlinear", "nonlinear_relation", "A quantity follows a nonlinear square-law relation. Find its value when the input is {n}."),
        ("geometry", "geometry_out_of_scope", "A circle has radius {n} meters. Find its area."),
        ("probability", "probability_out_of_scope", "A fair die is rolled twice. What is the probability of two sixes?"),
        ("unstated_conversion", "conversion_factor_not_stated", "Convert {n} miles to kilometers using the usual conversion."),
        ("incompatible_units", "incompatible_units", "Add {n} liters to {m} kilograms and report the total."),
        ("missing_interval", "missing_time_interval", "A vehicle travels at {n} km/h, but the travel time is missing. Find the distance."),
        ("multi_stage", "unsupported_multi_stage_narrative", "A machine changes rates three times, pauses, restarts, and then reports a final total."),
    ];
    let mut cases = Vec::new();
    for i in 0..100 {
        let (family, reason, template) = templates[i % templates.len()];
        let n = 2 + i % 19;
        let m = 3 + i % 13;
        let prompt = template.replace("{n}", &n.to_string()).replace("{m}", &m.to_string());
        let outcome = if matches!(family, "missing_anchor" | "incompatible_units" | "missing_interval") { "ambiguous" } else { "unsupported" };
        cases.push(negative(format!("qr.exp.neg.{:03}", i + 1), prompt, outcome, family, reason));
    }
    cases
}

fn generate() -> Corpus {
    let mut cases = positive_cases();
    cases.extend(negative_cases());
    Corpus {
        schema_version: 1,
        release_id: "quantity-relation-v1-expanded".into(),
        status: "pre_implementation_contract_corpus_pending_independent_review".into(),
        oracle: "deterministic typed relation contract oracle; no executor results".into(),
        cases,
    }
}

fn main() {
    let mut args = env::args().skip(1);
    let emit = match args.next().as_deref() {
        Some("--emit") => args.next(),
        None => None,
        Some(other) => panic!("unknown argument: {other}"),
    };
    let corpus = generate();
    let bytes = serde_json::to_vec_pretty(&corpus).expect("serialize corpus");
    if let Some(path) = emit {
        fs::write(path, &bytes).expect("write corpus release");
    }
    let hash = Sha256::digest(&bytes);
    let supported = corpus.cases.iter().filter(|case| case.outcome == "supported").count();
    let negative = corpus.cases.iter().filter(|case| case.outcome != "supported").count();
    let pairs = corpus.cases.iter().filter_map(|case| case.pair_id.as_ref()).collect::<std::collections::BTreeSet<_>>().len();
    println!("quantity-relation-expanded: cases={} supported={} negative_or_ambiguous={} rewrite_families={} sha256={:x}", corpus.cases.len(), supported, negative, pairs, hash);
}
