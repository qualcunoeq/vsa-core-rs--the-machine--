//! Follow-up understanding: reference resolution and conversation-state
//! operations.
//!
//! Conversation state ([`WorkingContext`]) keeps the exact strings and ids
//! that earlier turns introduced. This module uses that state, plus the
//! bounded hypervector context memory in [`crate::context`], to:
//!
//! 1. resolve pronouns and demonstratives to exact entities;
//! 2. recognize follow-up requests (about an entity, earlier state, a
//!    corrected fact, a re-parameterized equation, an explanation);
//! 3. refuse to guess when a reference matches several candidates.
//!
//! Hypervectors are only ever used to *rank* candidates; the binding that
//! comes out is always an exact entity string or turn id taken from the
//! stored context.

use crate::context::HierarchicalContextMemory;
use crate::nlp::{self, SvoTriple};
use crate::Hypervector;

use super::store::{Assumption, ConversationSession, OperationKind, WorkingContext};
use super::types::FollowUpInfo;

/// Minimum similarity for a context-similarity binding.
pub const CONTEXT_SIMILARITY_THRESHOLD: f64 = 0.55;
/// Minimum margin between the best and second-best candidate.
pub const CONTEXT_SIMILARITY_MARGIN: f64 = 0.03;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PronounClass {
    PersonSingular,
    Thing,
    PersonPlural,
}

#[derive(Clone, Debug)]
struct PronounOccurrence {
    token: String,
    class: PronounClass,
    /// True for possessive forms (`his`, `her`, `its`, `their`, ...), which
    /// become `X's` only when they actually determine a following noun.
    possessive_form: bool,
    /// Position in the whitespace-split token list.
    index: usize,
}

/// Tokens that follow a possessive form but are not nouns.
const NON_NOUN_AFTER_POSSESSIVE: &[&str] = &[
    "now", "currently", "today", "yesterday", "tomorrow", "instead", "before", "after",
    "previously", "originally", "then", "already", "still", "yet", "here", "there", "too", "also",
    "again", "please", "the", "a", "an",
];

fn next_token_is_noun_like(tokens: &[&str], index: usize) -> bool {
    let Some(next) = tokens.get(index + 1) else {
        return false;
    };
    let cleaned = next
        .trim_matches(|c: char| !c.is_alphanumeric() && c != '\'')
        .to_lowercase();
    !cleaned.is_empty()
        && cleaned.chars().all(|c| c.is_alphabetic() || c == '\'')
        && !NON_NOUN_AFTER_POSSESSIVE.contains(&cleaned.as_str())
}

/// A follow-up request recognized from the rewritten utterance.
#[derive(Clone, Debug, PartialEq)]
pub enum FollowUp {
    /// "What do you know about her?" -> summarize facts about an entity.
    About { entity: String },
    /// "Who managed it before?" -> read the previous version of a fact.
    Before { verb: String, object: String },
    /// "Actually, Bob manages it now." -> supersede the active fact.
    Correction {
        subject: String,
        verb: String,
        object: String,
    },
    /// "What if the right-hand side is 15?" -> re-solve with a new value.
    ResolveEquation {
        equation: String,
        variable: String,
        side: EquationSide,
        value: String,
        new_equation: String,
    },
    /// "Explain the substitution." -> describe the last operation.
    Explain { focus: String },
    /// A solver follow-up without a stored equation.
    NeedEquation { request: String },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EquationSide {
    Left,
    Right,
}

impl EquationSide {
    pub fn key(self) -> &'static str {
        match self {
            EquationSide::Left => "left_hand_side",
            EquationSide::Right => "right_hand_side",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            EquationSide::Left => "left-hand side",
            EquationSide::Right => "right-hand side",
        }
    }
}

/// The outcome of understanding an incoming turn against conversation state.
#[derive(Clone, Debug, PartialEq)]
pub enum Resolution {
    /// Nothing conversational was detected; use the text as typed.
    Direct { effective: String },
    /// References were bound and/or a follow-up was recognized.
    Resolved {
        effective: String,
        follow_up: Option<FollowUp>,
        info: Option<FollowUpInfo>,
    },
    /// A reference matched several entities; the runtime must ask instead of
    /// guessing.
    Clarification {
        question: String,
        candidates: Vec<String>,
        effective: String,
        info: FollowUpInfo,
    },
}

impl Resolution {
    pub fn effective(&self) -> &str {
        match self {
            Resolution::Direct { effective }
            | Resolution::Resolved { effective, .. }
            | Resolution::Clarification { effective, .. } => effective,
        }
    }
}

/// Understand `input` against the session's working context.
pub fn resolve(
    input: &str,
    context: &WorkingContext,
    memory: Option<&HierarchicalContextMemory>,
) -> Resolution {
    let occurrences = pronoun_occurrences(input);
    let (effective, bindings, source, confidence, clarification) =
        rewrite_pronouns(input, &occurrences, context, memory);

    if let Some((question, candidates)) = clarification {
        let info = FollowUpInfo {
            kind: "reference".to_string(),
            resolved: bindings,
            source: source.clone(),
            confidence,
        };
        return Resolution::Clarification {
            question,
            candidates,
            effective,
            info,
        };
    }

    let follow_up = detect_follow_up(&effective, context);
    let kind = match &follow_up {
        Some(FollowUp::About { .. }) => "about",
        Some(FollowUp::Before { .. }) => "before",
        Some(FollowUp::Correction { .. }) => "correction",
        Some(FollowUp::ResolveEquation { .. }) => "resolve_equation",
        Some(FollowUp::Explain { .. }) => "explain",
        Some(FollowUp::NeedEquation { .. }) => "resolve_equation",
        None => "reference",
    };
    let info = if bindings.is_empty() && follow_up.is_none() {
        None
    } else {
        Some(FollowUpInfo {
            kind: kind.to_string(),
            resolved: bindings,
            source,
            confidence,
        })
    };

    if follow_up.is_none() && info.is_none() {
        Resolution::Direct { effective }
    } else {
        Resolution::Resolved {
            effective,
            follow_up,
            info,
        }
    }
}

fn pronoun_occurrences(input: &str) -> Vec<PronounOccurrence> {
    let tokens: Vec<&str> = input.split_whitespace().collect();
    let mut occurrences = Vec::new();
    for (index, token) in tokens.iter().enumerate() {
        let cleaned = token
            .trim_matches(|c: char| !c.is_alphanumeric() && c != '\'')
            .to_lowercase();
        let (class, possessive_form) = match cleaned.as_str() {
            "he" | "him" | "she" => (PronounClass::PersonSingular, false),
            "his" | "her" | "hers" => (PronounClass::PersonSingular, true),
            "it" => (PronounClass::Thing, false),
            "its" => (PronounClass::Thing, true),
            "they" | "them" => (PronounClass::PersonPlural, false),
            "their" | "theirs" => (PronounClass::PersonPlural, true),
            "this" | "that" | "these" | "those" => {
                // Demonstratives are only treated as references when they end
                // the utterance; elsewhere "that" is usually a complementizer.
                let last = index + 1 == tokens.len();
                if !last {
                    continue;
                }
                (PronounClass::Thing, false)
            }
            _ => continue,
        };
        occurrences.push(PronounOccurrence {
            token: cleaned,
            class,
            possessive_form,
            index,
        });
    }
    occurrences
}

fn candidates_for(class: PronounClass, context: &WorkingContext) -> Vec<(usize, &super::store::EntityRef)> {
    context
        .entity_refs
        .iter()
        .enumerate()
        .filter(|(_, reference)| match class {
            PronounClass::PersonSingular | PronounClass::PersonPlural => reference.kind == "person",
            PronounClass::Thing => reference.kind == "thing",
        })
        .collect()
}

fn rewrite_pronouns(
    input: &str,
    occurrences: &[PronounOccurrence],
    context: &WorkingContext,
    memory: Option<&HierarchicalContextMemory>,
) -> (String, Vec<(String, String)>, String, f64, Option<(String, Vec<String>)>) {
    if occurrences.is_empty() {
        return (input.to_string(), Vec::new(), "explicit".to_string(), 1.0, None);
    }

    let words: Vec<&str> = input.split_whitespace().collect();
    let mut tokens: Vec<String> = words.iter().map(|token| (*token).to_string()).collect();
    let mut bindings: Vec<(String, String)> = Vec::new();
    let mut source = "explicit".to_string();
    let mut confidence = 1.0;

    for occurrence in occurrences {
        let candidates = candidates_for(occurrence.class, context);
        if candidates.is_empty() {
            let question = format!(
                "I do not know what \"{}\" refers to yet. Which entity do you mean?",
                occurrence.token
            );
            return (
                input.to_string(),
                bindings,
                "unresolved".to_string(),
                0.0,
                Some((question, Vec::new())),
            );
        }

        let chosen = if candidates.len() == 1 {
            Some((candidates[0].1.text.clone(), 1.0, "unique_entity".to_string()))
        } else {
            choose_by_similarity(input, &candidates, memory)
        };

        let Some((entity, score, how)) = chosen else {
            let names: Vec<String> = candidates
                .iter()
                .map(|(_, reference)| reference.text.clone())
                .collect();
            let question = format!(
                "\"{}\" could refer to {}; which one do you mean?",
                occurrence.token,
                join_candidates(&names)
            );
            return (
                input.to_string(),
                bindings,
                "ambiguous".to_string(),
                0.0,
                Some((question, names)),
            );
        };

        if how != "explicit" {
            source = how.clone();
            confidence = score;
        }
        let possessive = occurrence.possessive_form
            && (matches!(occurrence.token.as_str(), "hers" | "theirs")
                || next_token_is_noun_like(&words, occurrence.index));
        let replacement = if possessive {
            format!("{entity}'s")
        } else {
            entity.clone()
        };
        if occurrence.index < tokens.len() {
            tokens[occurrence.index] = replace_token(&words[occurrence.index], &replacement);
        }
        bindings.push((occurrence.token.clone(), entity));
    }

    (tokens.join(" "), bindings, source, confidence, None)
}

fn choose_by_similarity(
    utterance: &str,
    candidates: &[(usize, &super::store::EntityRef)],
    memory: Option<&HierarchicalContextMemory>,
) -> Option<(String, f64, String)> {
    let memory = memory?;
    let query = Hypervector::encode_sentence(utterance);
    let hits = memory.candidates(&query, CONTEXT_SIMILARITY_THRESHOLD);
    if hits.is_empty() {
        return None;
    }

    let mut scored: Vec<(String, f64)> = candidates
        .iter()
        .map(|(_, reference)| {
            let entity_label = format!("entity:{}", reference.text);
            let turn_label = format!("turn:{}", reference.turn_id);
            let best = hits
                .iter()
                .filter(|hit| hit.label == entity_label || hit.label == turn_label)
                .map(|hit| hit.similarity)
                .fold(0.0_f64, f64::max);
            (reference.text.clone(), best)
        })
        .collect();
    scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    let (best_entity, best_score) = scored.first()?.clone();
    if best_score < CONTEXT_SIMILARITY_THRESHOLD {
        return None;
    }
    if let Some((_, second)) = scored.get(1) {
        if best_score - second < CONTEXT_SIMILARITY_MARGIN {
            return None;
        }
    }
    Some((best_entity, best_score, "context_similarity".to_string()))
}

fn join_candidates(names: &[String]) -> String {
    match names {
        [] => "nothing".to_string(),
        [only] => only.clone(),
        [first, second] => format!("{first} or {second}"),
        [rest @ .., last] => format!("{}, or {last}", rest.join(", ")),
    }
}

/// Substitute a chosen candidate into the utterance that triggered an
/// ambiguous-reference clarification.
pub fn substitute_candidate(original: &str, candidate: &str) -> String {
    let words: Vec<&str> = original.split_whitespace().collect();
    let occurrences = pronoun_occurrences(original);
    if occurrences.is_empty() {
        return original.to_string();
    }
    let mut tokens: Vec<String> = words.iter().map(|token| (*token).to_string()).collect();
    for occurrence in &occurrences {
        let possessive = occurrence.possessive_form
            && (matches!(occurrence.token.as_str(), "hers" | "theirs")
                || next_token_is_noun_like(&words, occurrence.index));
        if occurrence.index < tokens.len() {
            let replacement = if possessive {
                format!("{candidate}'s")
            } else {
                candidate.to_string()
            };
            tokens[occurrence.index] = replace_token(&words[occurrence.index], &replacement);
        }
    }
    tokens.join(" ")
}

/// Detect the conversational operation an utterance performs.
pub fn detect_follow_up(text: &str, context: &WorkingContext) -> Option<FollowUp> {
    let trimmed = text.trim();
    let lower = trimmed.to_lowercase();

    if let Some(entity) = detect_about(trimmed, &lower) {
        return Some(FollowUp::About { entity });
    }
    if let Some((verb, object)) = detect_before(trimmed, &lower) {
        return Some(FollowUp::Before { verb, object });
    }
    if lower.starts_with("what if ") {
        return Some(detect_resolve_equation(&lower, context));
    }
    if is_explanation_request(&lower) {
        return Some(FollowUp::Explain {
            focus: explain_focus(&lower),
        });
    }
    if let Some((subject, verb, object)) = detect_correction(trimmed, &lower) {
        return Some(FollowUp::Correction {
            subject,
            verb,
            object,
        });
    }
    None
}

/// Replace a token with `replacement` while preserving its surrounding
/// punctuation (`her?` -> `Alice?`).
fn replace_token(original: &str, replacement: &str) -> String {
    let is_punctuation =
        |c: char| !c.is_alphanumeric() && c != '\'' && c != '_';
    let leading: String = original.chars().take_while(|c| is_punctuation(*c)).collect();
    let trailing: String = original
        .chars()
        .rev()
        .take_while(|c| is_punctuation(*c))
        .collect::<Vec<char>>()
        .into_iter()
        .rev()
        .collect();
    format!("{leading}{replacement}{trailing}")
}

/// Remove every case-insensitive occurrence of an ASCII phrase.
fn remove_phrase(text: &str, phrase: &str) -> String {
    let mut output = text.to_string();
    loop {
        let lower = output.to_ascii_lowercase();
        let Some(position) = lower.find(phrase) else {
            break;
        };
        output.replace_range(position..position + phrase.len(), "");
    }
    output
}

fn strip_prefix_ci<'a>(text: &'a str, prefix: &str) -> Option<&'a str> {
    let lower = text.to_ascii_lowercase();
    if lower.starts_with(prefix) {
        text.get(prefix.len()..)
    } else {
        None
    }
}

fn detect_about(trimmed: &str, lower: &str) -> Option<String> {
    let triggers = [
        "what do you know about ",
        "what can you tell me about ",
        "what about ",
        "tell me about ",
        "anything about ",
    ];
    for trigger in triggers {
        if lower.starts_with(trigger) {
            let entity = trimmed
                .get(trigger.len()..)
                .unwrap_or("")
                .trim()
                .trim_end_matches(['?', '.', '!'])
                .trim()
                .to_string();
            if !entity.is_empty() {
                return Some(entity);
            }
        }
    }
    None
}

fn detect_before(text: &str, lower: &str) -> Option<(String, String)> {
    let markers = [" before", " previously", " originally", " used to"];
    let has_marker = markers.iter().any(|marker| lower.contains(marker));
    if !has_marker {
        return None;
    }
    let mut stripped = text.to_string();
    for marker in markers {
        stripped = remove_phrase(&stripped, marker);
    }
    stripped = stripped
        .trim_end_matches(['?', '.', '!'])
        .trim()
        .to_string();
    let triple = clean_triple(nlp::extract_svo(&stripped).first()?);
    let subject = triple.subject.to_lowercase();
    if subject != "who" && subject != "what" {
        return None;
    }
    if triple.verb.trim().is_empty() || triple.object.trim().is_empty() {
        return None;
    }
    Some((triple.verb, triple.object))
}

fn detect_resolve_equation(lower: &str, context: &WorkingContext) -> FollowUp {
    let sides: [(&str, EquationSide); 8] = [
        ("the right-hand side", EquationSide::Right),
        ("right-hand side", EquationSide::Right),
        ("the right hand side", EquationSide::Right),
        ("rhs", EquationSide::Right),
        ("the left-hand side", EquationSide::Left),
        ("left-hand side", EquationSide::Left),
        ("the left hand side", EquationSide::Left),
        ("lhs", EquationSide::Left),
    ];
    let mut parsed: Option<(EquationSide, String)> = None;
    for (phrase, side) in sides {
        if let Some(rest) = lower.split_once(phrase).map(|(_, rest)| rest.trim()) {
            if let Some(value) = trailing_number(rest) {
                parsed = Some((side, value));
                break;
            }
        }
    }
    let Some((side, value)) = parsed else {
        return FollowUp::NeedEquation {
            request: lower.to_string(),
        };
    };
    let Some((equation, variable)) = current_equation(context) else {
        return FollowUp::NeedEquation {
            request: lower.to_string(),
        };
    };
    let new_equation = substitute_equation_side(&equation, side, &value);
    FollowUp::ResolveEquation {
        equation,
        variable,
        side,
        value,
        new_equation,
    }
}

fn trailing_number(text: &str) -> Option<String> {
    for token in text.split_whitespace() {
        let cleaned = token.trim_matches(|c: char| !c.is_ascii_digit() && c != '.' && c != '-');
        if cleaned.is_empty() || !cleaned.chars().any(|c| c.is_ascii_digit()) {
            continue;
        }
        if cleaned.parse::<f64>().is_ok() {
            return Some(cleaned.to_string());
        }
    }
    None
}

fn is_explanation_request(lower: &str) -> bool {
    [
        "explain",
        "how did you",
        "how do you",
        "why did you",
        "why do you",
        "show your work",
        "walk me through",
        "where did you",
    ]
    .iter()
    .any(|trigger| lower.starts_with(trigger))
}

fn explain_focus(lower: &str) -> String {
    if lower.contains("substitut") {
        "substitution".to_string()
    } else if lower.contains("check") || lower.contains("verif") {
        "verification".to_string()
    } else {
        "result".to_string()
    }
}

fn detect_correction(text: &str, lower: &str) -> Option<(String, String, String)> {
    let markers = [
        "actually",
        "no,",
        "no ",
        "wait,",
        "wait ",
        "correction",
        " instead",
        " from now on",
        " these days",
        " now",
        " currently",
        " at the moment",
    ];
    let has_marker = markers.iter().any(|marker| lower.contains(marker));
    if !has_marker {
        return None;
    }

    let mut stripped = text.to_string();
    for prefix in ["actually,", "actually ", "no,", "wait,", "correction:"] {
        if let Some(rest) = strip_prefix_ci(&stripped, prefix) {
            stripped = rest.trim().to_string();
            break;
        }
    }
    for marker in [
        " from now on",
        " these days",
        " at the moment",
        " right now",
        " currently",
        " instead",
        " now",
    ] {
        stripped = remove_phrase(&stripped, marker);
    }
    stripped = stripped
        .trim_end_matches(['?', '.', '!'])
        .trim()
        .to_string();
    let triple = clean_triple(nlp::extract_svo(&stripped).first()?);
    if triple.subject.is_empty() || triple.verb.is_empty() || triple.object.is_empty() {
        return None;
    }
    let subject = triple.subject.to_lowercase();
    if subject == "who" || subject == "what" || subject == "i" || subject == "you" {
        return None;
    }
    Some((triple.subject, triple.verb, triple.object))
}

/// Normalize an extracted triple: collapse whitespace and drop sentence
/// punctuation that the extractor leaves on the last token.
pub fn clean_triple(triple: &SvoTriple) -> SvoTriple {
    SvoTriple {
        subject: clean_token(&triple.subject),
        verb: clean_token(&triple.verb),
        object: clean_token(&triple.object),
        confidence: triple.confidence,
        construction: triple.construction.clone(),
    }
}

pub fn clean_token(value: &str) -> String {
    value
        .trim()
        .trim_matches(|c: char| matches!(c, '.' | '?' | '!' | ',' | ';' | ':'))
        .trim()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// The equation currently under discussion, taken from the last operation
/// (or reconstructed from the last solver turn's input).
pub fn current_equation(context: &WorkingContext) -> Option<(String, String)> {
    let operation = context.last_operation.as_ref()?;
    if operation.kind != OperationKind::Solve {
        return None;
    }
    if let (Some(equation), Some(variable)) =
        (operation.argument("equation"), operation.argument("variable"))
    {
        return Some((equation.to_string(), variable.to_string()));
    }
    let input = operation.argument("input")?;
    parse_equation_text(input)
}

/// Extract `(equation, variable)` from solver-style input such as
/// `Solve 2x + 3 = 11.` or `solve for y: 5 = y + 2`.
pub fn parse_equation_text(text: &str) -> Option<(String, String)> {
    let trimmed = text.trim().trim_end_matches(['.', '?', ' ']).trim();
    let lower = trimmed.to_lowercase();
    let body = if let Some(rest) = lower.strip_prefix("solve for ") {
        rest.split_once(':').map(|(_, expr)| expr.trim().to_string())?
    } else if let Some(rest) = lower.strip_prefix("solve ") {
        rest.rsplit_once(" for ")
            .map(|(expr, _)| expr.trim().to_string())
            .unwrap_or_else(|| rest.trim().to_string())
    } else {
        trimmed.to_string()
    };
    if !body.contains('=') {
        return None;
    }
    let question = format!("Solve {body}");
    let problem = crate::algebra_island::parse_problem(&question)?;
    let variable = match &problem.target {
        crate::algebra_island::AlgebraTarget::SolveFor(variable) => variable.clone(),
        _ => return None,
    };
    Some((body, variable))
}

fn substitute_equation_side(equation: &str, side: EquationSide, value: &str) -> String {
    match equation.split_once('=') {
        Some((left, right)) => match side {
            EquationSide::Left => format!("{value} = {}", right.trim()),
            EquationSide::Right => format!("{} = {value}", left.trim()),
        },
        None => equation.to_string(),
    }
}

/// A human-readable statement for an assumption.
pub fn assumption_for(
    key: &str,
    value: &str,
    statement: &str,
    turn_id: &str,
) -> Assumption {
    Assumption {
        key: key.to_string(),
        value: value.to_string(),
        statement: statement.to_string(),
        turn_id: turn_id.to_string(),
        at: chrono::Utc::now().to_rfc3339(),
    }
}

/// Entities observed in a finished turn, in the order they appeared.
pub fn entities_from_turn(result: &super::types::TurnResult) -> Vec<String> {
    let mut entities: Vec<String> = Vec::new();
    let mut candidates: Vec<String> = Vec::new();
    for evidence in &result.evidence {
        candidates.push(evidence.content.clone());
    }
    for change in &result.memory_changes {
        candidates.push(change.detail.clone());
    }
    for candidate in candidates {
        if let Some((subject, object)) = WorkingContext::salient_terms(&candidate) {
            if !entities.contains(&subject) {
                entities.push(subject);
            }
            if !entities.contains(&object) {
                entities.push(object);
            }
        }
    }
    entities
}

/// Session memory rebuild uses the same derivation as a live turn.
pub fn session_memory_from(
    session: &ConversationSession,
    window: usize,
) -> HierarchicalContextMemory {
    let mut memory = HierarchicalContextMemory::new();
    let start = session.turns.len().saturating_sub(window);
    for turn in &session.turns[start..] {
        memory.push(
            Hypervector::encode_sentence(&turn.input),
            &format!("turn:{}", turn.turn_id),
        );
        for entity in entities_from_turn(&turn.result) {
            memory.push(
                Hypervector::encode_sentence(&turn.input),
                &format!("entity:{entity}"),
            );
        }
        memory.tick();
    }
    memory
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::conversation::store::{
        Assumption, ClarificationKind, ConversationSession, ConversationTurn, EntityRef,
        LastOperation, LastResult, OperationKind, PendingClarification, WorkingContext,
    };
    use crate::conversation::types::{
        Capability, Interpretation, RequestKind, TurnDiagnostics, TurnOutcome, TurnResult,
        TurnTiming, VerificationStatus,
    };

    fn context_with(entities: &[(&str, &str)]) -> WorkingContext {
        let mut context = WorkingContext::default();
        for (text, turn_id) in entities {
            context.remember_entity(text, turn_id);
        }
        context
    }

    #[test]
    fn possessive_forms_only_attach_to_following_nouns() {
        let context = context_with(&[("Alice", "turn-1")]);
        let resolution = resolve("What do you know about her?", &context, None);
        match resolution {
            Resolution::Resolved { effective, .. } => {
                assert_eq!(effective, "What do you know about Alice?");
            }
            other => panic!("expected a resolved reference, got {other:?}"),
        }

        let resolution = resolve("Bob manages her observatory.", &context, None);
        match resolution {
            Resolution::Resolved { effective, .. } => {
                assert_eq!(effective, "Bob manages Alice's observatory.");
            }
            other => panic!("expected a resolved reference, got {other:?}"),
        }
    }

    #[test]
    fn pronouns_without_candidates_ask_for_the_referent() {
        let context = WorkingContext::default();
        let resolution = resolve("What do you know about her?", &context, None);
        match resolution {
            Resolution::Clarification {
                question,
                candidates,
                ..
            } => {
                assert!(question.contains("her"));
                assert!(candidates.is_empty());
            }
            other => panic!("expected clarification, got {other:?}"),
        }
    }

    #[test]
    fn correction_detection_preserves_subject_case_and_drops_temporal_words() {
        let context = context_with(&[("the observatory", "turn-1")]);
        let resolution = resolve("Actually, Bob manages it now.", &context, None);
        match resolution {
            Resolution::Resolved {
                follow_up: Some(FollowUp::Correction { subject, verb, object }),
                ..
            } => {
                assert_eq!(subject, "Bob");
                assert_eq!(verb, "manage");
                assert_eq!(object, "the observatory");
            }
            other => panic!("expected a correction, got {other:?}"),
        }
    }

    #[test]
    fn right_hand_side_follow_up_parses_value_and_equation() {
        let mut context = WorkingContext::default();
        context.record_operation(
            OperationKind::Solve,
            vec![
                ("input".to_string(), "Solve 2x + 3 = 11".to_string()),
                ("equation".to_string(), "2x + 3 = 11".to_string()),
                ("variable".to_string(), "x".to_string()),
            ],
            "turn-1",
        );
        let follow_up = detect_follow_up("What if the right-hand side is 15?", &context);
        match follow_up {
            Some(FollowUp::ResolveEquation {
                equation,
                variable,
                side,
                value,
                new_equation,
            }) => {
                assert_eq!(equation, "2x + 3 = 11");
                assert_eq!(variable, "x");
                assert_eq!(side, EquationSide::Right);
                assert_eq!(value, "15");
                assert_eq!(new_equation, "2x + 3 = 15");
            }
            other => panic!("expected a re-parameterized equation, got {other:?}"),
        }
    }

    #[test]
    fn equation_parsing_handles_both_solve_forms() {
        assert_eq!(
            parse_equation_text("Solve 2x + 3 = 11."),
            Some(("2x + 3 = 11".to_string(), "x".to_string()))
        );
        assert_eq!(
            parse_equation_text("solve for y: 5 = y + 2"),
            Some(("5 = y + 2".to_string(), "y".to_string()))
        );
        assert_eq!(parse_equation_text("Tell me a story"), None);
    }

    #[test]
    fn pending_choice_selects_exactly_one_candidate() {
        let pending = PendingClarification {
            turn_id: "turn-1".to_string(),
            question: "which?".to_string(),
            missing: "choose one".to_string(),
            kind: ClarificationKind::AmbiguousReference,
            candidates: vec!["Alice".to_string(), "Bob".to_string()],
            original_input: "What do you know about her?".to_string(),
        };
        assert_eq!(pending.selected_candidate("Alice"), Some("Alice"));
        assert_eq!(pending.selected_candidate("alice"), Some("Alice"));
        assert_eq!(pending.selected_candidate("Alice and Bob"), None);
        assert_eq!(pending.selected_candidate("nobody"), None);
    }

    #[test]
    fn substitute_candidate_rewrites_the_original_pronoun() {
        assert_eq!(
            substitute_candidate("What about it?", "the observatory"),
            "What about the observatory?"
        );
        assert_eq!(
            substitute_candidate("What do you know about her?", "Alice"),
            "What do you know about Alice?"
        );
    }

    #[test]
    fn clean_token_strips_sentence_punctuation() {
        assert_eq!(clean_token("the observatory ."), "the observatory");
        assert_eq!(clean_token("  the   telescope ? "), "the telescope");
        assert_eq!(clean_token("Bob"), "Bob");
    }

    #[test]
    fn assumptions_replace_by_key_and_stay_bounded() {
        let mut context = WorkingContext::default();
        for index in 0..6 {
            context.add_assumption(Assumption {
                key: format!("key-{index}"),
                value: index.to_string(),
                statement: "s".to_string(),
                turn_id: "turn-1".to_string(),
                at: "now".to_string(),
            });
        }
        assert_eq!(context.assumptions.len(), WorkingContext::MAX_ASSUMPTIONS);
        context.add_assumption(Assumption {
            key: "key-5".to_string(),
            value: "updated".to_string(),
            statement: "s".to_string(),
            turn_id: "turn-2".to_string(),
            at: "now".to_string(),
        });
        assert_eq!(
            context.assumption("key-5").map(|item| item.value.as_str()),
            Some("updated")
        );
    }

    #[test]
    fn explain_request_is_recognized_with_focus() {
        let context = WorkingContext::default();
        assert_eq!(
            detect_follow_up("Explain the substitution.", &context),
            Some(FollowUp::Explain {
                focus: "substitution".to_string()
            })
        );
        assert_eq!(
            detect_follow_up("How did you get that?", &context),
            Some(FollowUp::Explain {
                focus: "result".to_string()
            })
        );
    }

    #[test]
    fn before_request_is_recognized_only_with_temporal_marker() {
        let context = WorkingContext::default();
        assert_eq!(
            detect_follow_up("Who managed it before?", &context),
            Some(FollowUp::Before {
                verb: "manage".to_string(),
                object: "it".to_string()
            })
        );
        assert_eq!(detect_follow_up("Who managed it?", &context), None);
    }

    fn dummy_result(turn_id: &str, evidence: &str) -> TurnResult {
        TurnResult {
            outcome: TurnOutcome::Answered,
            answer_text: evidence.to_string(),
            answer: Some(evidence.to_string()),
            interpretation: Interpretation {
                kind: RequestKind::Question,
                normalized_input: evidence.to_string(),
                capability: Capability::FactualQa,
                notes: Vec::new(),
                follow_up: None,
            },
            evidence: vec![crate::conversation::types::EvidenceRef {
                kind: crate::conversation::types::EvidenceKind::RetrievedClaim,
                content: evidence.to_string(),
                provenance: "test".to_string(),
                confidence: 1.0,
                replay_verified: None,
                assertion_id: None,
            }],
            capability: Capability::FactualQa,
            verification: VerificationStatus::NotAttempted,
            memory_changes: Vec::new(),
            timing: TurnTiming {
                started_at: "now".to_string(),
                elapsed_ms: 1.0,
            },
            diagnostics: TurnDiagnostics {
                session_id: "session".to_string(),
                turn_id: turn_id.to_string(),
                episode_id: None,
                fact_count_before: 0,
                fact_count_after: 0,
                rule_count_before: 0,
                rule_count_after: 0,
                storage_error: None,
            },
        }
    }

    #[test]
    fn session_memory_rebuilds_turn_and_entity_labels() {
        let mut session = ConversationSession::new("session");
        session.turns.push(ConversationTurn::new(
            "turn-1",
            "Alice manages the observatory.",
            dummy_result("turn-1", "Alice manage the observatory"),
        ));
        let memory = session_memory_from(&session, 32);
        assert!(memory.recall_by_label("turn:turn-1").is_some());
        assert!(memory.recall_by_label("entity:Alice").is_some());
        assert!(memory.recall_by_label("entity:the observatory").is_some());
    }

    #[test]
    fn last_result_records_provenance_and_answer() {
        let mut context = WorkingContext::default();
        context.observe_turn(&dummy_result("turn-7", "Alice manage the observatory"));
        let last = context.last_result.as_ref().expect("last result");
        assert_eq!(last.turn_id, "turn-7");
        assert_eq!(last.provenance, vec!["test".to_string()]);
        assert_eq!(context.references.len(), 1);
        assert_eq!(context.references[0].kind, "ask");
    }

    #[test]
    fn reconstruction_derives_the_last_operation() {
        let mut session = ConversationSession::new("session");
        session.turns.push(ConversationTurn::new(
            "turn-1",
            "Who manages the observatory?",
            dummy_result("turn-1", "Alice manage the observatory"),
        ));
        let context = WorkingContext::reconstruct(&session);
        assert_eq!(
            context.last_operation.as_ref().map(|operation| operation.kind),
            Some(OperationKind::Ask)
        );
        assert!(matches!(
            context.last_result.as_ref().map(|result| result.capability.as_str()),
            Some("factual_qa")
        ));
    }

    #[test]
    fn reconstruct_without_context_keeps_entities() {
        let mut session = ConversationSession::new("session");
        session.turns.push(ConversationTurn::new(
            "turn-1",
            "Alice manages the observatory.",
            dummy_result("turn-1", "Alice manage the observatory"),
        ));
        let context = WorkingContext::reconstruct(&session);
        assert!(context.entities.iter().any(|entity| entity == "Alice"));
        assert!(context
            .entity_refs
            .iter()
            .any(|reference| reference.text == "the observatory"));
        let _ = EntityRef {
            text: "x".to_string(),
            turn_id: "y".to_string(),
            kind: "thing".to_string(),
        };
        let _ = LastResult {
            turn_id: "y".to_string(),
            outcome: "answered".to_string(),
            answer: None,
            provenance: Vec::new(),
            capability: "none".to_string(),
            at: "now".to_string(),
        };
    }
}
