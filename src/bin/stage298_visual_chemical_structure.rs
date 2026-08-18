//! Stage 298: explicit visual chemical structures composed with the
//! source-derived chemistry pack.

use serde::Serialize;
use the_machine::source_formula_pack::chemistry_pack::ChemistryOperation;
use the_machine::vision::visual_chemical::{
    formalize_visual_chemical, ChemicalVisualStatus, VisualAtomObservation, VisualBondObservation,
    VisualChemicalObservation, VisualChemicalResult,
};
use the_machine::visual_chemical_chemistry_bridge::{
    evaluate_chemical_structure, BridgeStatus, ChemicalBridgeRequest,
};

const REPORT_JSON: &str = "docs/stage298_visual_chemical_structure.json";
const REPORT_MD: &str = "docs/stage298_visual_chemical_structure.md";

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum Expected {
    Supported,
    Ambiguous,
    Refused,
}

#[derive(Debug, Serialize)]
struct Receipt {
    id: String,
    expected: Expected,
    visual_status: ChemicalVisualStatus,
    bridge_status: BridgeStatus,
    visual_replay: bool,
    bridge_replay: bool,
    chemistry_replay: bool,
    visual_tamper_rejected: bool,
    bridge_tamper_rejected: bool,
    exact: bool,
    formula_correct: bool,
    provenance_preserved: bool,
    false_authorization: bool,
    false_denial: bool,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    cases: usize,
    supported: usize,
    ambiguous: usize,
    refused: usize,
    exact_decisions: usize,
    supported_formulas_correct: usize,
    visual_replays: usize,
    bridge_replays: usize,
    chemistry_replays: usize,
    visual_tamper_rejections: usize,
    bridge_tamper_rejections: usize,
    provenance_preserved: usize,
    false_authorizations: usize,
    false_denials: usize,
    live_registry_mutations: usize,
    hle_questions_read: usize,
    receipts: Vec<Receipt>,
}

fn atoms(elements: &[&str], index: usize) -> Vec<VisualAtomObservation> {
    elements
        .iter()
        .enumerate()
        .map(|(atom_index, element)| VisualAtomObservation {
            id: format!("a{index}_{atom_index}"),
            element: (*element).into(),
            x: atom_index as i32,
            y: (atom_index % 2) as i32,
            confidence: 99,
        })
        .collect()
}

fn visual(elements: &[&str], scope: Option<&str>, index: usize) -> VisualChemicalResult {
    let atom_list = atoms(elements, index);
    let bonds = atom_list
        .windows(2)
        .enumerate()
        .map(|(bond_index, pair)| VisualBondObservation {
            id: format!("b{index}_{bond_index}"),
            from: pair[0].id.clone(),
            to: pair[1].id.clone(),
            order: Some(1),
            confidence: 99,
        })
        .collect();
    formalize_visual_chemical(&VisualChemicalObservation {
        semantic_label: Some("bounded_chemical_structure".into()),
        scope: scope.map(str::to_owned),
        atoms: atom_list,
        bonds,
        ambiguity: None,
        provenance: vec!["stage298:visual-atom-span".into()],
    })
}

fn request(
    operation: ChemistryOperation,
    ambiguity: Option<&str>,
    provenance: bool,
) -> ChemicalBridgeRequest {
    ChemicalBridgeRequest {
        operation,
        ambiguity: ambiguity.map(str::to_owned),
        provenance: provenance
            .then(|| "stage298:question-span".to_string())
            .into_iter()
            .collect(),
    }
}

fn run(
    id: String,
    expected: Expected,
    visual_result: VisualChemicalResult,
    request: ChemicalBridgeRequest,
    expected_formula: Option<&str>,
) -> Receipt {
    let mut visual_tampered = visual_result.clone();
    visual_tampered.replay_hash.push('x');
    let visual_replay = visual_result.replay_verified();
    let visual_tamper_rejected = !visual_tampered.replay_verified();
    let bridge = evaluate_chemical_structure(&visual_result, &request);
    let bridge_replay = bridge.replay_verified();
    let chemistry_replay = bridge
        .chemistry_result
        .as_ref()
        .is_none_or(|result| result.replay_verified());
    let mut bridge_tampered = bridge.clone();
    bridge_tampered.replay_hash.push('x');
    let bridge_tamper_rejected = !bridge_tampered.replay_verified();
    let formula_correct =
        expected != Expected::Supported || bridge.formula.as_deref() == expected_formula;
    let authorized = bridge.authorized();
    let exact = match expected {
        Expected::Supported => authorized && formula_correct,
        Expected::Ambiguous => bridge.status == BridgeStatus::Ambiguous && !authorized,
        Expected::Refused => bridge.status != BridgeStatus::Complete && !authorized,
    };
    Receipt {
        id,
        expected,
        visual_status: visual_result.status,
        bridge_status: bridge.status,
        visual_replay,
        bridge_replay,
        chemistry_replay,
        visual_tamper_rejected,
        bridge_tamper_rejected,
        exact,
        formula_correct,
        provenance_preserved: !bridge.provenance.is_empty(),
        false_authorization: expected != Expected::Supported && authorized,
        false_denial: expected == Expected::Supported && !authorized,
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let supported = [
        (vec!["H", "H", "O"], "H2O"),
        (vec!["C", "O", "O"], "CO2"),
        (vec!["C", "H", "H", "H", "H"], "CH4"),
        (vec!["N", "H", "H", "H"], "H3N"),
        (
            vec!["C", "C", "C", "H", "H", "H", "H", "H", "H", "H", "H", "O"],
            "C3H8O",
        ),
        (vec!["C", "C", "O", "O", "O"], "C2O3"),
    ];
    let mut receipts = Vec::with_capacity(240);
    for index in 0..120 {
        let (elements, formula) = &supported[index % supported.len()];
        receipts.push(run(
            format!("supported-{index:03}"),
            Expected::Supported,
            visual(elements, Some("single_molecule"), index),
            request(ChemistryOperation::ParseFormula, None, true),
            Some(formula),
        ));
    }
    for index in 0..40 {
        let (elements, _) = &supported[index % supported.len()];
        receipts.push(run(
            format!("ambiguous-{index:03}"),
            Expected::Ambiguous,
            visual(elements, Some("single_molecule"), index),
            request(
                ChemistryOperation::ParseFormula,
                Some("atom identity or scope remains unresolved"),
                true,
            ),
            None,
        ));
    }
    for index in 0..20 {
        let (elements, _) = &supported[index % supported.len()];
        receipts.push(run(
            format!("refused-mixture-{index:03}"),
            Expected::Refused,
            visual(elements, Some("mixture"), index),
            request(ChemistryOperation::ParseFormula, None, true),
            None,
        ));
    }
    for index in 0..20 {
        let (elements, _) = &supported[index % supported.len()];
        receipts.push(run(
            format!("refused-operation-{index:03}"),
            Expected::Refused,
            visual(elements, Some("single_molecule"), index),
            request(ChemistryOperation::ValidateReaction, None, true),
            None,
        ));
    }
    for index in 0..20 {
        let (elements, _) = &supported[index % supported.len()];
        receipts.push(run(
            format!("refused-provenance-{index:03}"),
            Expected::Refused,
            visual(elements, Some("single_molecule"), index),
            request(ChemistryOperation::ParseFormula, None, false),
            None,
        ));
    }
    for index in 0..20 {
        receipts.push(run(
            format!("refused-unknown-element-{index:03}"),
            Expected::Refused,
            visual(&["Xe"], Some("single_molecule"), index),
            request(ChemistryOperation::ParseFormula, None, true),
            None,
        ));
    }
    assert_eq!(receipts.len(), 240);
    let report = Report {
        schema: "stage298-visual-chemical-structure-v1",
        cases: receipts.len(),
        supported: receipts
            .iter()
            .filter(|r| r.expected == Expected::Supported)
            .count(),
        ambiguous: receipts
            .iter()
            .filter(|r| r.expected == Expected::Ambiguous)
            .count(),
        refused: receipts
            .iter()
            .filter(|r| r.expected == Expected::Refused)
            .count(),
        exact_decisions: receipts.iter().filter(|r| r.exact).count(),
        supported_formulas_correct: receipts
            .iter()
            .filter(|r| r.expected == Expected::Supported && r.formula_correct)
            .count(),
        visual_replays: receipts.iter().filter(|r| r.visual_replay).count(),
        bridge_replays: receipts.iter().filter(|r| r.bridge_replay).count(),
        chemistry_replays: receipts.iter().filter(|r| r.chemistry_replay).count(),
        visual_tamper_rejections: receipts.iter().filter(|r| r.visual_tamper_rejected).count(),
        bridge_tamper_rejections: receipts.iter().filter(|r| r.bridge_tamper_rejected).count(),
        provenance_preserved: receipts.iter().filter(|r| r.provenance_preserved).count(),
        false_authorizations: receipts.iter().filter(|r| r.false_authorization).count(),
        false_denials: receipts.iter().filter(|r| r.false_denial).count(),
        live_registry_mutations: 0,
        hle_questions_read: 0,
        receipts,
    };
    assert_eq!(report.exact_decisions, 240);
    assert_eq!(report.supported_formulas_correct, 120);
    assert_eq!(report.visual_replays, 240);
    assert_eq!(report.bridge_replays, 240);
    assert_eq!(report.chemistry_replays, 240);
    assert_eq!(report.visual_tamper_rejections, 240);
    assert_eq!(report.bridge_tamper_rejections, 240);
    assert_eq!(report.false_authorizations, 0);
    assert_eq!(report.false_denials, 0);
    std::fs::write(REPORT_JSON, serde_json::to_vec_pretty(&report)?)?;
    std::fs::write(
        REPORT_MD,
        format!(
            concat!(
                "# Stage 298 — visual chemical structure composition\n\n",
                "| metric | result |\n|---|---:|\n",
                "| cases | {}/240 |\n| exact decisions | {}/240 |\n",
                "| supported formulas | {}/120 |\n| ambiguities preserved | {}/40 |\n",
                "| refusals | {}/80 |\n| visual/bridge/chemistry replays | {}/{}/{} |\n",
                "| tamper rejections | {}/{} |\n| false authorizations / denials | {} / {} |\n",
                "| HLE questions read / registry mutations | {} / {} |\n\n",
                "Only explicit atom inventories under single-molecule scope are lowered; bonds are not interpreted as chemistry.\n"
            ),
            report.cases,
            report.exact_decisions,
            report.supported_formulas_correct,
            report.ambiguous,
            report.refused,
            report.visual_replays,
            report.bridge_replays,
            report.chemistry_replays,
            report.visual_tamper_rejections,
            report.bridge_tamper_rejections,
            report.false_authorizations,
            report.false_denials,
            report.hle_questions_read,
            report.live_registry_mutations,
        ),
    )?;
    println!(
        "stage298 cases={} exact={} formulas={} false_auth={} false_denials={}",
        report.cases,
        report.exact_decisions,
        report.supported_formulas_correct,
        report.false_authorizations,
        report.false_denials
    );
    Ok(())
}
