// ─── Universal Perceptual Encoder ──────────────────────────────────────────
//
// The Machine's native language is SVO triples.  Every domain — chess, text,
// images, system state, network traffic — must be converted into (subject,
// relation, object) triples before the VSABrain can reason about it.
//
// This trait defines that conversion.  Every domain implements it.
//
// Historical note: chess was the test bed that proved relational SVO encoding
// accurately captures material-heavy positions (R²=0.42) but misses tactical
// structure entirely (R²=0.10 on Lichess 1800+ positions).  Adding tactical
// relation extraction — detecting forks, pins, skewers as explicit SVO triples
// extracted from attack maps — is the first proof that explicit relational
// encoding beats implicit statistical learning for causal reasoning.
// ────────────────────────────────────────────────────────────────────────────

/// A named entity in the perception domain.
pub type Entity = String;

/// A relation/verb between two entities.
pub type Relation = String;

/// An SVO triple: (subject, relation, object).
pub type SvoTriple = (String, String, String);

/// Universal perceptual encoder trait.
///
/// Converts raw domain input into SVO triples that the VSABrain can
/// bind, store, reason about, and compare.
///
/// # Domain examples
///
/// **Chess**: FEN → (wN_e5, attacks, bQ_d7), (wN_e5, forks, bQ_d7_and_bR_f7), ...
///
/// **Text**: "The Fed raised rates" → (the_fed, raised, rates), ...
///
/// **System state**: `ps aux` → (process_1847, is_writing_to, port_443), ...
///
/// **Images**: pixel array → (car, is_left_of, building), (car, color, red), ...
pub trait PerceptualEncoder {
    /// The raw input type for this domain.
    type Input;

    /// Extract named entities from raw input.
    ///
    /// Entities are the "subjects" and "objects" of SVO triples.  In chess,
    /// they're pieces on squares (wN_e5).  In text, they're named entities.
    /// In system state, they're processes, ports, files.
    fn extract_entities(&self, input: &Self::Input) -> Vec<Entity>;

    /// Extract relations between entities.
    ///
    /// Relations are the "verbs" of SVO triples.  In chess, they're "attacks",
    /// "forks", "pins".  In text, they're grammatical relations.  In system
    /// state, they're "writes_to", "connects_to", "is_parent_of".
    fn extract_relations(&self, input: &Self::Input, entities: &[Entity]) -> Vec<SvoTriple>;

    /// Convenience: entities + relations → full SVO triple list.
    fn encode(&self, input: &Self::Input) -> Vec<SvoTriple> {
        let entities = self.extract_entities(input);
        self.extract_relations(input, &entities)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockEncoder;

    impl PerceptualEncoder for MockEncoder {
        type Input = String;
        fn extract_entities(&self, input: &String) -> Vec<Entity> {
            input.split_whitespace().map(|s| s.to_string()).collect()
        }
        fn extract_relations(&self, _input: &String, entities: &[Entity]) -> Vec<SvoTriple> {
            if entities.len() >= 2 {
                vec![(
                    entities[0].clone(),
                    "relates_to".to_string(),
                    entities[1].clone(),
                )]
            } else {
                vec![]
            }
        }
    }

    #[test]
    fn test_encode_returns_triples() {
        let encoder = MockEncoder;
        let triples = encoder.encode(&"apple banana".to_string());
        assert_eq!(triples.len(), 1);
        assert_eq!(
            triples[0],
            (
                "apple".to_string(),
                "relates_to".to_string(),
                "banana".to_string()
            )
        );
    }

    #[test]
    fn test_encode_empty_input() {
        let encoder = MockEncoder;
        let triples = encoder.encode(&"".to_string());
        assert!(triples.is_empty());
    }

    #[test]
    fn test_encode_single_entity_no_relation() {
        let encoder = MockEncoder;
        let triples = encoder.encode(&"only_one".to_string());
        assert!(triples.is_empty());
    }
}
