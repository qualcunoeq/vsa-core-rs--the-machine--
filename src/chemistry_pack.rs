//! Source-derived, bounded chemistry representations.
//!
//! The source contract follows OpenStax Chemistry 2e: molecular formulas use
//! element symbols and subscripts, and balanced-equation coefficients encode
//! stoichiometric relationships.  This module deliberately stops at exact
//! formula parsing, balance validation, and ratios from an already balanced
//! equation.  It does not infer reaction products, oxidation states, charges,
//! phases, molar masses, or reaction mechanisms.

use super::SourceCitation;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

#[path = "chemistry_frontend.rs"]
pub mod chemistry_frontend;

#[path = "chemistry_linear_bridge.rs"]
pub mod chemistry_linear_bridge;

const DOMAIN: &str = "source_derived_bounded_chemistry";
const MAX_TERMS: usize = 8;
const MAX_COEFFICIENT: u32 = 100;
const MAX_ATOMS: u32 = 500;

const ELEMENTS: &[&str] = &[
    "H", "He", "Li", "Be", "B", "C", "N", "O", "F", "Ne", "Na", "Mg", "Al", "Si", "P", "S", "Cl",
    "Ar", "K", "Ca", "Fe", "Co", "Ni", "Cu", "Zn", "Br", "Ag", "I", "Ba", "Au", "Hg", "Pb",
];

fn source() -> SourceCitation {
    SourceCitation {
        source_id: "openstax-chemistry-2e:formulas-stoichiometry".into(),
        title: "Chemistry 2e".into(),
        section: "2.4 Chemical Formulas; 4.1 Writing and Balancing Chemical Equations; 4.3 Reaction Stoichiometry".into(),
        url: "https://openstax.org/books/chemistry-2e/pages/4-3-reaction-stoichiometry".into(),
        license: "CC BY 4.0; OpenStax attribution required".into(),
        retrieved_utc: "2026-08-16".into(),
        evidence_span: "chemical formulas identify substances; balanced-equation coefficients encode relative amounts and stoichiometric factors".into(),
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ChemistryOperation {
    ParseFormula,
    ValidateReaction,
    StoichiometricRatio,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ChemistryStatus {
    Complete,
    Missing,
    Ambiguous,
    Unsupported,
    InvalidDomain,
    Inconsistent,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ChemistryArtifact {
    MolecularFormula {
        atoms: BTreeMap<String, u32>,
    },
    BalancedReaction {
        reactants: BTreeMap<String, u32>,
        products: BTreeMap<String, u32>,
        atom_totals: BTreeMap<String, u32>,
    },
    StoichiometricRatio {
        from: String,
        to: String,
        from_coefficient: u32,
        to_coefficient: u32,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChemistryRequest {
    pub operation: ChemistryOperation,
    pub formula: Option<String>,
    pub reaction: Option<String>,
    pub from_species: Option<String>,
    pub to_species: Option<String>,
    pub domain: String,
    pub ambiguity: Option<String>,
    pub provenance: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChemistryResult {
    pub status: ChemistryStatus,
    pub artifact: Option<ChemistryArtifact>,
    pub operation: ChemistryOperation,
    pub assumptions: Vec<String>,
    pub source: Option<SourceCitation>,
    pub reasons: Vec<String>,
    pub provenance: Vec<String>,
    pub replay_hash: String,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(value).expect("chemistry serializes"))
    )
}

fn payload(result: &ChemistryResult) -> impl Serialize + '_ {
    (
        result.status,
        result.artifact.as_ref(),
        result.operation,
        &result.assumptions,
        &result.source,
        &result.reasons,
        &result.provenance,
    )
}

fn result(
    request: &ChemistryRequest,
    status: ChemistryStatus,
    artifact: Option<ChemistryArtifact>,
    assumptions: Vec<String>,
    source: Option<SourceCitation>,
    reasons: Vec<String>,
) -> ChemistryResult {
    let mut output = ChemistryResult {
        status,
        artifact,
        operation: request.operation,
        assumptions,
        source,
        reasons,
        provenance: request.provenance.clone(),
        replay_hash: String::new(),
    };
    let replay_hash = digest(&payload(&output));
    output.replay_hash = replay_hash;
    output
}

fn assumptions() -> Vec<String> {
    vec![
        "element symbols and explicit integer subscripts only".into(),
        "balanced-equation coefficients are positive integers".into(),
        "stoichiometric ratios come only from validated balanced equations".into(),
        "charges, phases, hydrates, and molar-mass calculations are outside scope".into(),
    ]
}

fn element(name: &str) -> bool {
    ELEMENTS.contains(&name)
}

fn add_atoms(target: &mut BTreeMap<String, u32>, name: &str, amount: u32) -> Result<(), String> {
    let entry = target.entry(name.to_string()).or_default();
    *entry = entry
        .checked_add(amount)
        .ok_or_else(|| "atom count overflow".to_string())?;
    if target.values().sum::<u32>() > MAX_ATOMS {
        return Err("formula exceeds the bounded atom budget".into());
    }
    Ok(())
}

struct FormulaParser {
    chars: Vec<char>,
    position: usize,
    depth: usize,
}

impl FormulaParser {
    fn new(input: &str) -> Self {
        Self {
            chars: input.chars().collect(),
            position: 0,
            depth: 0,
        }
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.position).copied()
    }

    fn consume(&mut self) -> Option<char> {
        let value = self.peek();
        if value.is_some() {
            self.position += 1;
        }
        value
    }

    fn parse_number(&mut self) -> Result<u32, String> {
        let start = self.position;
        while self.peek().is_some_and(|value| value.is_ascii_digit()) {
            self.position += 1;
        }
        if start == self.position {
            return Ok(1);
        }
        let value: String = self.chars[start..self.position].iter().collect();
        let value = value
            .parse::<u32>()
            .map_err(|_| "invalid subscript".to_string())?;
        if value == 0 || value > MAX_COEFFICIENT {
            return Err("subscripts must be positive and bounded".into());
        }
        Ok(value)
    }

    fn parse_group(&mut self, nested: bool) -> Result<BTreeMap<String, u32>, String> {
        let mut atoms = BTreeMap::new();
        loop {
            match self.peek() {
                None => {
                    if nested {
                        return Err("unclosed parenthesized group".into());
                    }
                    return Ok(atoms);
                }
                Some(')') if nested => {
                    self.consume();
                    return Ok(atoms);
                }
                Some(')') => return Err("unexpected closing parenthesis".into()),
                Some('(') => {
                    if self.depth >= 4 {
                        return Err("nested formula depth exceeds the bounded limit".into());
                    }
                    self.consume();
                    self.depth += 1;
                    let group = self.parse_group(true)?;
                    self.depth -= 1;
                    let multiplier = self.parse_number()?;
                    for (name, count) in group {
                        add_atoms(&mut atoms, &name, count * multiplier)?;
                    }
                }
                Some(value) if value.is_ascii_uppercase() => {
                    let mut name = String::new();
                    name.push(self.consume().expect("peeked uppercase"));
                    if self.peek().is_some_and(|next| next.is_ascii_lowercase()) {
                        name.push(self.consume().expect("peeked lowercase"));
                    }
                    if !element(&name) {
                        return Err(format!("unknown element symbol {name}"));
                    }
                    let multiplier = self.parse_number()?;
                    add_atoms(&mut atoms, &name, multiplier)?;
                }
                Some(value) if value.is_ascii_digit() => {
                    return Err(format!(
                        "leading coefficient {value} is not part of a formula"
                    ));
                }
                Some(value) => return Err(format!("unsupported formula character {value}")),
            }
        }
    }

    fn parse(mut self) -> Result<BTreeMap<String, u32>, String> {
        if self.chars.is_empty() {
            return Err("formula is empty".into());
        }
        let atoms = self.parse_group(false)?;
        if atoms.is_empty() || self.position != self.chars.len() {
            return Err("formula has unparsed content".into());
        }
        Ok(atoms)
    }
}

fn reject_unsupported_notation(value: &str) -> Option<String> {
    if value.contains('·') || value.contains('.') {
        Some("hydrates and dot-separated formula notation are outside scope".into())
    } else if value.contains('+') || value.contains('-') {
        Some("ionic charge notation is outside scope".into())
    } else if value.contains('(') && value.contains("aq") {
        Some("phase labels are outside scope".into())
    } else {
        None
    }
}

fn parse_formula(value: &str) -> Result<BTreeMap<String, u32>, String> {
    if let Some(reason) = reject_unsupported_notation(value) {
        return Err(reason);
    }
    FormulaParser::new(value.trim()).parse()
}

#[derive(Debug, Clone)]
struct ReactionTerm {
    coefficient: u32,
    formula: String,
    atoms: BTreeMap<String, u32>,
}

fn parse_term(raw: &str) -> Result<ReactionTerm, String> {
    let raw = raw.trim();
    let mut split = 0;
    for (index, value) in raw.char_indices() {
        if value.is_ascii_digit() {
            split = index + value.len_utf8();
        } else {
            break;
        }
    }
    let (coefficient, formula) = if split == 0 {
        (1, raw)
    } else {
        let coefficient = raw[..split]
            .parse::<u32>()
            .map_err(|_| "invalid reaction coefficient".to_string())?;
        (coefficient, raw[split..].trim())
    };
    if coefficient == 0 || coefficient > MAX_COEFFICIENT || formula.is_empty() {
        return Err("reaction coefficients must be positive and bounded".into());
    }
    Ok(ReactionTerm {
        coefficient,
        formula: formula.to_string(),
        atoms: parse_formula(formula)?,
    })
}

fn parse_side(side: &str) -> Result<Vec<ReactionTerm>, String> {
    let terms = side
        .split('+')
        .map(parse_term)
        .collect::<Result<Vec<_>, _>>()?;
    if terms.is_empty() || terms.len() > MAX_TERMS {
        return Err("reaction side has an unsupported number of species".into());
    }
    let mut seen = BTreeMap::new();
    for term in &terms {
        if seen.insert(term.formula.clone(), ()).is_some() {
            return Err("duplicate species requires explicit aggregation".into());
        }
    }
    Ok(terms)
}

fn parse_reaction(reaction: &str) -> Result<(Vec<ReactionTerm>, Vec<ReactionTerm>), String> {
    let (left, right) = reaction
        .split_once("->")
        .or_else(|| reaction.split_once('→'))
        .ok_or_else(|| "reaction needs one explicit arrow".to_string())?;
    if reaction.matches("->").count() + reaction.matches('→').count() != 1 {
        return Err("reaction must contain exactly one arrow".into());
    }
    Ok((parse_side(left)?, parse_side(right)?))
}

fn totals(terms: &[ReactionTerm]) -> Result<BTreeMap<String, u32>, String> {
    let mut output = BTreeMap::new();
    for term in terms {
        for (element, count) in &term.atoms {
            add_atoms(&mut output, element, count * term.coefficient)?;
        }
    }
    Ok(output)
}

fn canonical_formula(atoms: &BTreeMap<String, u32>) -> String {
    atoms
        .iter()
        .map(|(element, count)| {
            if *count == 1 {
                element.clone()
            } else {
                format!("{element}{count}")
            }
        })
        .collect::<Vec<_>>()
        .join("")
}

fn gcd(mut left: u32, mut right: u32) -> u32 {
    while right != 0 {
        let remainder = left % right;
        left = right;
        right = remainder;
    }
    left
}

/// Evaluate a bounded source-derived chemistry request.
pub fn evaluate_chemistry(request: &ChemistryRequest) -> ChemistryResult {
    let cited = source();
    if request.domain != DOMAIN {
        return result(
            request,
            ChemistryStatus::InvalidDomain,
            None,
            Vec::new(),
            None,
            vec!["domain is outside source-derived bounded chemistry".into()],
        );
    }
    if let Some(ambiguity) = &request.ambiguity {
        return result(
            request,
            ChemistryStatus::Ambiguous,
            None,
            assumptions(),
            None,
            vec![ambiguity.clone()],
        );
    }
    match request.operation {
        ChemistryOperation::ParseFormula => {
            let Some(formula) = request.formula.as_deref() else {
                return result(
                    request,
                    ChemistryStatus::Missing,
                    None,
                    assumptions(),
                    None,
                    vec!["one molecular formula is required".into()],
                );
            };
            match parse_formula(formula) {
                Ok(atoms) => result(
                    request,
                    ChemistryStatus::Complete,
                    Some(ChemistryArtifact::MolecularFormula { atoms }),
                    assumptions(),
                    Some(cited),
                    Vec::new(),
                ),
                Err(reason) if reason.contains("outside scope") => result(
                    request,
                    ChemistryStatus::Unsupported,
                    None,
                    assumptions(),
                    Some(cited),
                    vec![reason],
                ),
                Err(reason) => result(
                    request,
                    ChemistryStatus::Inconsistent,
                    None,
                    assumptions(),
                    Some(cited),
                    vec![reason],
                ),
            }
        }
        ChemistryOperation::ValidateReaction | ChemistryOperation::StoichiometricRatio => {
            let Some(reaction) = request.reaction.as_deref() else {
                return result(
                    request,
                    ChemistryStatus::Missing,
                    None,
                    assumptions(),
                    None,
                    vec!["one explicit reaction is required".into()],
                );
            };
            let (reactants, products) = match parse_reaction(reaction) {
                Ok(value) => value,
                Err(reason) if reason.contains("outside scope") => {
                    return result(
                        request,
                        ChemistryStatus::Unsupported,
                        None,
                        assumptions(),
                        Some(cited),
                        vec![reason],
                    )
                }
                Err(reason) => {
                    return result(
                        request,
                        ChemistryStatus::Inconsistent,
                        None,
                        assumptions(),
                        Some(cited),
                        vec![reason],
                    )
                }
            };
            let reactant_totals = totals(&reactants).expect("bounded reaction totals");
            let product_totals = totals(&products).expect("bounded reaction totals");
            if reactant_totals != product_totals {
                return result(
                    request,
                    ChemistryStatus::Inconsistent,
                    None,
                    assumptions(),
                    Some(cited),
                    vec!["atom totals differ across the reaction arrow".into()],
                );
            }
            if request.operation == ChemistryOperation::ValidateReaction {
                let reactants = reactants
                    .iter()
                    .map(|term| (canonical_formula(&term.atoms), term.coefficient))
                    .collect();
                let products = products
                    .iter()
                    .map(|term| (canonical_formula(&term.atoms), term.coefficient))
                    .collect();
                return result(
                    request,
                    ChemistryStatus::Complete,
                    Some(ChemistryArtifact::BalancedReaction {
                        reactants,
                        products,
                        atom_totals: reactant_totals,
                    }),
                    assumptions(),
                    Some(cited),
                    Vec::new(),
                );
            }
            let (Some(from), Some(to)) = (
                request.from_species.as_deref(),
                request.to_species.as_deref(),
            ) else {
                return result(
                    request,
                    ChemistryStatus::Missing,
                    None,
                    assumptions(),
                    Some(cited),
                    vec!["source and target species are required for a ratio".into()],
                );
            };
            let from_atoms = parse_formula(from).ok();
            let to_atoms = parse_formula(to).ok();
            let from_matches = reactants
                .iter()
                .chain(products.iter())
                .filter(|term| from_atoms.as_ref() == Some(&term.atoms))
                .collect::<Vec<_>>();
            let to_matches = reactants
                .iter()
                .chain(products.iter())
                .filter(|term| to_atoms.as_ref() == Some(&term.atoms))
                .collect::<Vec<_>>();
            if from_matches.len() != 1 || to_matches.len() != 1 {
                return result(
                    request,
                    ChemistryStatus::Ambiguous,
                    None,
                    assumptions(),
                    Some(cited),
                    vec!["each ratio endpoint must identify exactly one species".into()],
                );
            }
            let from_coefficient = from_matches[0].coefficient;
            let to_coefficient = to_matches[0].coefficient;
            let divisor = gcd(from_coefficient, to_coefficient);
            result(
                request,
                ChemistryStatus::Complete,
                Some(ChemistryArtifact::StoichiometricRatio {
                    from: canonical_formula(&from_matches[0].atoms),
                    to: canonical_formula(&to_matches[0].atoms),
                    from_coefficient: from_coefficient / divisor,
                    to_coefficient: to_coefficient / divisor,
                }),
                assumptions(),
                Some(cited),
                Vec::new(),
            )
        }
    }
}

impl ChemistryResult {
    pub fn replay_verified(&self) -> bool {
        self.replay_hash == digest(&payload(self))
            && !self.provenance.is_empty()
            && (self.status != ChemistryStatus::Complete || self.artifact.is_some())
            && (self.status != ChemistryStatus::Complete || self.source.is_some())
    }

    pub fn authorized(&self) -> bool {
        self.status == ChemistryStatus::Complete && self.replay_verified()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(operation: ChemistryOperation) -> ChemistryRequest {
        ChemistryRequest {
            operation,
            formula: Some("Al2(SO4)3".into()),
            reaction: None,
            from_species: None,
            to_species: None,
            domain: DOMAIN.into(),
            ambiguity: None,
            provenance: vec!["chemistry-test".into()],
        }
    }

    #[test]
    fn formula_and_reaction_are_replayable() {
        let formula = evaluate_chemistry(&request(ChemistryOperation::ParseFormula));
        assert!(formula.authorized());
        let reaction = evaluate_chemistry(&ChemistryRequest {
            operation: ChemistryOperation::ValidateReaction,
            formula: None,
            reaction: Some("N2 + 3H2 -> 2NH3".into()),
            from_species: None,
            to_species: None,
            domain: DOMAIN.into(),
            ambiguity: None,
            provenance: vec!["chemistry-test".into()],
        });
        assert!(reaction.authorized());
    }

    #[test]
    fn imbalance_and_charge_fail_closed() {
        let mut request = request(ChemistryOperation::ParseFormula);
        request.formula = Some("Na+".into());
        assert_eq!(
            evaluate_chemistry(&request).status,
            ChemistryStatus::Unsupported
        );
        let result = evaluate_chemistry(&ChemistryRequest {
            operation: ChemistryOperation::ValidateReaction,
            formula: None,
            reaction: Some("H2 + O2 -> H2O".into()),
            from_species: None,
            to_species: None,
            domain: DOMAIN.into(),
            ambiguity: None,
            provenance: vec!["chemistry-test".into()],
        });
        assert_eq!(result.status, ChemistryStatus::Inconsistent);
    }
}
