// ─── First-Class Concept Definitions ──────────────────────────────────────
//
// Bridges the gap between:
//   - QA facts (natural language "X is Y" with concept_definition source tag)
//   - FormulaRegistry (symbolic formula entries with domain tags)
//   - Hierarchy (VSA hypervector-level abstract concepts)
//   - Sensorimotor simulation (physics, text input, simulated experience)
//
// The key insight: formal definitions previously existed only as domain tags
// in formula registries.  This module promotes them to first-class "X is Y"
// ConceptDefinition entities that the reasoning system can reference directly.
//
// ## Architecture
//
//   ConceptRegistry (the central authority for concept knowledge)
//     ├── ConceptDefinition (name + definition + type + domain)
//     ├── Links to QA facts (via qa_subject)
//     ├── Links to FormulaRegistry formulas (via formula_slug)
//     ├── Links to Hierarchy centroids (via hierarchy_index)
//     └── Dependency graph (depends_on, has_parts)
//
// ## Integration Points
//
// - QaEngine.concept_registry: ConceptRegistry (new field, serialized)
// - seed_concept_definitions() also registers in ConceptRegistry
// - FormulaRegistry.sync_to_concept_registry() bridges formulas
// - answer("What is X?") consults ConceptRegistry before scanning facts
//
// ────────────────────────────────────────────────────────────────────────────

use crate::Hypervector;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ═══════════════════════════════════════════════════════════════════════════
// CONCEPT TYPE
// ═══════════════════════════════════════════════════════════════════════════

/// What kind of concept this definition represents.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum ConceptType {
    /// A self-evident starting assumption (no proof required).
    Axiom,
    /// A statement proven from axioms and previously established theorems.
    Theorem,
    /// A formal symbolic definition ("X ≜ Y" in formal notation).
    FormalDefinition,
    /// A natural-language concept definition ("X is Y").
    Definition,
    /// A concept grounded in sensorimotor experience (physics, senses).
    Sensorimotor,
    /// A composite/hierarchical concept built from sub-concepts.
    Composite,
    /// A computational rule (e.g., integration rule, simplification rule).
    ComputationRule,
}

impl ConceptType {
    /// Human-readable label for this concept type.
    pub fn label(&self) -> &str {
        match self {
            ConceptType::Axiom => "axiom",
            ConceptType::Theorem => "theorem",
            ConceptType::FormalDefinition => "formal definition",
            ConceptType::Definition => "definition",
            ConceptType::Sensorimotor => "sensorimotor",
            ConceptType::Composite => "composite",
            ConceptType::ComputationRule => "computation rule",
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// CONCEPT DEFINITION
// ═══════════════════════════════════════════════════════════════════════════

/// A first-class concept definition linking symbols to formal definitions.
///
/// Each `ConceptDefinition` bridges a named symbol (e.g., "power_rule",
/// "geometry", "newtons_second_law") to its formal definition, natural-language
/// description, VSA hypervector representation, and connections to the QA
/// engine and formula registry.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ConceptDefinition {
    /// Canonical name/slug (e.g., "power_rule", "geometry", "newtons_second_law").
    pub name: String,
    /// The human-readable "X is Y" definition text.
    pub definition: String,
    /// What kind of concept this is.
    pub concept_type: ConceptType,
    /// Domain (e.g., "calculus", "geometry", "physics", "logic", "mathematics").
    pub domain: String,
    /// Optional symbolic formula string (e.g., "d/dx x^n = n*x^(n-1)").
    pub formula: Option<String>,
    /// Link to FormulaRegistry slug (if this concept corresponds to a formula).
    pub formula_slug: Option<String>,
    /// Link to QA subject (for QA fact lookup via "What is X?").
    /// This is the normalized subject used in `store_fact`.
    pub qa_subject: Option<String>,
    /// Tags for categorization and search.
    pub tags: Vec<String>,
    /// Names of concepts this definition depends on (for dependency tracking).
    pub depends_on: Vec<String>,
    /// Names of sub-concepts that are part of this definition.
    pub has_parts: Vec<String>,

    // ── Runtime fields (not serialized) ───────────────────────────────

    /// Canonical VSA hypervector encoding this definition.
    /// Encoded from the definition text using n-gram binding.
    #[serde(skip)]
    pub canonical_hv: Option<Hypervector>,

    /// Hierarchy concept index if registered in the hierarchical manifold.
    #[serde(skip)]
    pub hierarchy_index: Option<usize>,

    /// Hierarchy level if registered (1-based, 1 = L1, 2 = L2, etc.).
    #[serde(skip)]
    pub hierarchy_level: Option<usize>,

    /// Number of times this definition has been referenced (for grounding metrics).
    #[serde(skip)]
    pub access_count: u64,
}

impl ConceptDefinition {
    /// Create a new formal concept definition.
    pub fn new(
        name: &str,
        definition: &str,
        concept_type: ConceptType,
        domain: &str,
    ) -> Self {
        let hv = Some(Hypervector::encode_text_ngram(definition, 3));
        ConceptDefinition {
            name: name.to_string(),
            definition: definition.to_string(),
            concept_type,
            domain: domain.to_string(),
            formula: None,
            formula_slug: None,
            qa_subject: Some(name.to_string()),
            tags: Vec::new(),
            depends_on: Vec::new(),
            has_parts: Vec::new(),
            canonical_hv: hv,
            hierarchy_index: None,
            hierarchy_level: None,
            access_count: 0,
        }
    }

    /// Create an axiom concept definition.
    pub fn axiom(name: &str, definition: &str, domain: &str) -> Self {
        Self::new(name, definition, ConceptType::Axiom, domain)
    }

    /// Create a theorem concept definition.
    pub fn theorem(name: &str, definition: &str, domain: &str) -> Self {
        Self::new(name, definition, ConceptType::Theorem, domain)
    }

    /// Create a formal definition concept definition.
    pub fn formal(name: &str, definition: &str, domain: &str) -> Self {
        Self::new(name, definition, ConceptType::FormalDefinition, domain)
    }

    /// Create a natural-language concept definition.
    pub fn definition(name: &str, definition: &str, domain: &str) -> Self {
        Self::new(name, definition, ConceptType::Definition, domain)
    }

    /// Create a sensorimotor concept definition.
    pub fn sensorimotor(name: &str, definition: &str) -> Self {
        Self::new(name, definition, ConceptType::Sensorimotor, "sensorimotor")
    }

    /// Add a tag to this concept definition (builder pattern).
    pub fn with_tag(mut self, tag: &str) -> Self {
        self.tags.push(tag.to_string());
        self
    }

    /// Set the formula string and slug (builder pattern).
    pub fn with_formula(mut self, formula: &str, slug: &str) -> Self {
        self.formula = Some(formula.to_string());
        self.formula_slug = Some(slug.to_string());
        self
    }

    /// Set QA subject linkage (builder pattern).
    pub fn with_qa_subject(mut self, subject: &str) -> Self {
        self.qa_subject = Some(subject.to_string());
        self
    }

    /// Add a dependency (builder pattern).
    pub fn with_dependency(mut self, dep: &str) -> Self {
        self.depends_on.push(dep.to_string());
        self
    }

    /// Add a sub-part (builder pattern).
    pub fn with_part(mut self, part: &str) -> Self {
        self.has_parts.push(part.to_string());
        self
    }

    /// Record an access to this concept (for grounding metrics).
    pub fn record_access(&mut self) {
        self.access_count = self.access_count.saturating_add(1);
    }

    /// Compute the canonical hypervector (n-gram encoding of the definition text).
    pub fn compute_canonical_hv(&mut self) {
        self.canonical_hv = Some(Hypervector::encode_text_ngram(&self.definition, 3));
    }

    /// Get the canonical hypervector, computing it if necessary.
    pub fn canonical_hv(&self) -> Hypervector {
        match self.canonical_hv {
            Some(hv) => hv,
            None => Hypervector::encode_text_ngram(&self.definition, 3),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// CONCEPT REGISTRY
// ═══════════════════════════════════════════════════════════════════════════

/// Central registry of all concept definitions.
///
/// Provides storage, lookup by name/slug, search by domain/tag,
/// and bridging between QA facts, formula registry entries, and
/// hierarchy concepts.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ConceptRegistry {
    /// name → ConceptDefinition
    concepts: HashMap<String, ConceptDefinition>,
    /// domain → [concept names]
    #[serde(default)]
    domain_index: HashMap<String, Vec<String>>,
    /// tag → [concept names]
    #[serde(default)]
    tag_index: HashMap<String, Vec<String>>,
    /// Total number of registry accesses (grounding metric).
    #[serde(default)]
    total_accesses: u64,
}

impl ConceptRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        ConceptRegistry {
            concepts: HashMap::new(),
            domain_index: HashMap::new(),
            tag_index: HashMap::new(),
            total_accesses: 0,
        }
    }

    /// Number of registered concepts.
    pub fn len(&self) -> usize {
        self.concepts.len()
    }

    /// True if no concepts are registered.
    pub fn is_empty(&self) -> bool {
        self.concepts.is_empty()
    }

    /// Register a concept definition. Returns an error if the name already exists.
    pub fn register(&mut self, concept: ConceptDefinition) -> Result<(), String> {
        let name = concept.name.clone();
        if self.concepts.contains_key(&name) {
            return Err(format!("Concept '{}' is already registered", name));
        }

        let domain = concept.domain.clone();
        let tags = concept.tags.clone();

        self.concepts.insert(name.clone(), concept);

        // Index by domain
        self.domain_index
            .entry(domain)
            .or_default()
            .push(name.clone());

        // Index by tag
        for tag in &tags {
            self.tag_index
                .entry(tag.clone())
                .or_default()
                .push(name.clone());
        }

        Ok(())
    }

    /// Get a concept definition by name.
    pub fn get(&self, name: &str) -> Option<&ConceptDefinition> {
        self.concepts.get(name)
    }

    /// Get a mutable reference to a concept definition by name.
    pub fn get_mut(&mut self, name: &str) -> Option<&mut ConceptDefinition> {
        self.concepts.get_mut(name)
    }

    /// Check if a concept exists.
    pub fn contains(&self, name: &str) -> bool {
        self.concepts.contains_key(name)
    }

    /// Get all concept names.
    pub fn names(&self) -> Vec<String> {
        self.concepts.keys().cloned().collect()
    }

    /// Get all concept definitions.
    pub fn concepts(&self) -> Vec<&ConceptDefinition> {
        self.concepts.values().collect()
    }

    /// Find concepts by domain.
    pub fn by_domain(&self, domain: &str) -> Vec<&ConceptDefinition> {
        self.domain_index
            .get(domain)
            .map(|names| {
                names
                    .iter()
                    .filter_map(|n| self.concepts.get(n))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Find concepts by tag.
    pub fn by_tag(&self, tag: &str) -> Vec<&ConceptDefinition> {
        self.tag_index
            .get(tag)
            .map(|names| {
                names
                    .iter()
                    .filter_map(|n| self.concepts.get(n))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Search concepts by name or definition text (substring match, case-insensitive).
    pub fn search(&self, query: &str) -> Vec<&ConceptDefinition> {
        let q = query.to_lowercase();
        self.concepts
            .values()
            .filter(|c| {
                c.name.to_lowercase().contains(&q)
                    || c.definition.to_lowercase().contains(&q)
                    || c.domain.to_lowercase().contains(&q)
                    || c.tags.iter().any(|t| t.to_lowercase().contains(&q))
                    || c.formula.as_ref().map_or(false, |f| f.to_lowercase().contains(&q))
            })
            .collect()
    }

    /// Find concepts by type.
    pub fn by_type(&self, concept_type: &ConceptType) -> Vec<&ConceptDefinition> {
        self.concepts
            .values()
            .filter(|c| c.concept_type == *concept_type)
            .collect()
    }

    /// Get all axioms.
    pub fn axioms(&self) -> Vec<&ConceptDefinition> {
        self.by_type(&ConceptType::Axiom)
    }

    /// Get all theorems.
    pub fn theorems(&self) -> Vec<&ConceptDefinition> {
        self.by_type(&ConceptType::Theorem)
    }

    /// Get all formal definitions.
    pub fn formal_definitions(&self) -> Vec<&ConceptDefinition> {
        self.by_type(&ConceptType::FormalDefinition)
    }

    /// Remove a concept by name. Returns true if it was removed.
    pub fn remove(&mut self, name: &str) -> bool {
        if let Some(concept) = self.concepts.remove(name) {
            // Clean up domain index
            if let Some(names) = self.domain_index.get_mut(&concept.domain) {
                names.retain(|n| n != name);
                if names.is_empty() {
                    self.domain_index.remove(&concept.domain);
                }
            }
            // Clean up tag index
            for tag in &concept.tags {
                if let Some(names) = self.tag_index.get_mut(tag) {
                    names.retain(|n| n != name);
                    if names.is_empty() {
                        self.tag_index.remove(tag);
                    }
                }
            }
            true
        } else {
            false
        }
    }

    /// Record an access to a named concept (for grounding metrics).
    pub fn record_access(&mut self, name: &str) {
        self.total_accesses = self.total_accesses.saturating_add(1);
        if let Some(concept) = self.concepts.get_mut(name) {
            concept.record_access();
        }
    }

    /// Get total access count across all concepts.
    pub fn total_accesses(&self) -> u64 {
        self.total_accesses
    }

    /// Get the most frequently accessed concepts (for grounding metrics).
    pub fn most_accessed(&self, top_n: usize) -> Vec<(&ConceptDefinition, u64)> {
        let mut results: Vec<(&ConceptDefinition, u64)> = self
            .concepts
            .values()
            .map(|c| (c, c.access_count))
            .collect();
        results.sort_by(|a, b| b.1.cmp(&a.1));
        results.truncate(top_n);
        results
    }

    /// Get concepts by dependency — which concepts depend on `name`.
    pub fn dependents(&self, name: &str) -> Vec<&ConceptDefinition> {
        self.concepts
            .values()
            .filter(|c| c.depends_on.iter().any(|d| d == name))
            .collect()
    }

    /// Get the dependency closure of a concept (all transitive dependencies).
    pub fn dependency_closure(&self, name: &str) -> Vec<&ConceptDefinition> {
        let mut seen = std::collections::HashSet::new();
        let mut result = Vec::new();
        let mut stack = vec![name.to_string()];
        while let Some(current) = stack.pop() {
            if seen.contains(&current) {
                continue;
            }
            seen.insert(current.clone());
            if let Some(concept) = self.concepts.get(&current) {
                for dep in &concept.depends_on {
                    if !seen.contains(dep) {
                        stack.push(dep.clone());
                    }
                }
                result.push(concept);
            }
        }
        result
    }

    /// Remove a concept definition and return it.
    pub fn unregister(&mut self, name: &str) -> Option<ConceptDefinition> {
        let concept = self.concepts.remove(name)?;
        if let Some(names) = self.domain_index.get_mut(&concept.domain) {
            names.retain(|n| n != name);
        }
        for tag in &concept.tags {
            if let Some(names) = self.tag_index.get_mut(tag) {
                names.retain(|n| n != name);
            }
        }
        Some(concept)
    }

    /// Record hierarchy assignment for a concept.
    pub fn set_hierarchy(&mut self, name: &str, level: usize, index: usize) {
        if let Some(concept) = self.concepts.get_mut(name) {
            concept.hierarchy_level = Some(level);
            concept.hierarchy_index = Some(index);
        }
    }

    /// Get a summary of registry contents.
    pub fn summary(&self) -> String {
        let total = self.concepts.len();
        let types: Vec<_> = self
            .concepts
            .values()
            .map(|c| c.concept_type.label())
            .collect();
        let mut type_counts: HashMap<&str, usize> = HashMap::new();
        for t in &types {
            *type_counts.entry(t).or_insert(0) += 1;
        }
        let mut lines: Vec<String> = Vec::new();
        lines.push(format!("Concept Registry: {} definitions", total));
        let mut type_list: Vec<_> = type_counts.into_iter().collect();
        type_list.sort_by(|a, b| b.1.cmp(&a.1));
        for (type_name, count) in &type_list {
            lines.push(format!("  {}: {}", type_name, count));
        }
        lines.push(format!("  Accesses: {}", self.total_accesses));
        lines.join("\n")
    }

    /// Generate a human-readable grounding report.
    pub fn grounding_report(&self) -> String {
        let mut lines: Vec<String> = Vec::new();
        lines.push("═══════════════════════════════════════════".to_string());
        lines.push("  GROUNDING REPORT".to_string());
        lines.push("═══════════════════════════════════════════".to_string());
        lines.push(self.summary());
        lines.push("".to_string());

        // Most accessed concepts
        let top = self.most_accessed(5);
        if !top.is_empty() {
            lines.push("  Most Accessed:".to_string());
            for (concept, count) in &top {
                lines.push(format!("    {} ({} accesses) — {}", concept.name, count, concept.concept_type.label()));
            }
        }

        // Concepts by domain
        let mut domains: Vec<&String> = self.domain_index.keys().collect();
        domains.sort();
        lines.push("".to_string());
        lines.push("  By Domain:".to_string());
        for domain in &domains {
            let count = self.domain_index.get(*domain).map(|v| v.len()).unwrap_or(0);
            lines.push(format!("    {}: {} concepts", domain, count));
        }

        // Concepts without QA linkage
        let unlinked: Vec<&ConceptDefinition> = self
            .concepts
            .values()
            .filter(|c| c.qa_subject.is_none())
            .collect();
        if !unlinked.is_empty() {
            lines.push("".to_string());
            lines.push(format!("  Without QA linkage: {} concepts", unlinked.len()));
        }

        // Concepts without hierarchy linkage
        let unhier: Vec<&ConceptDefinition> = self
            .concepts
            .values()
            .filter(|c| c.hierarchy_index.is_none())
            .collect();
        if !unhier.is_empty() {
            lines.push(format!("  Without hierarchy linkage: {} concepts", unhier.len()));
        }

        lines.push("═══════════════════════════════════════════".to_string());
        lines.join("\n")
    }
}

impl Default for ConceptRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// BOOTSTRAP — Seed foundational concept definitions
// ═══════════════════════════════════════════════════════════════════════════

/// Seed the registry with foundational mathematical and logical concepts.
///
/// These are the bedrock definitions that everything else builds on:
/// axioms, fundamental definitions, and core mathematical concepts.
/// The QA-side seeding (SVO facts) is done separately in
/// `qa::seed_concept_definitions()`; this function seeds the formal
/// concept definitions that link to those QA facts.
pub fn seed_foundational_concepts(registry: &mut ConceptRegistry) {
    // ── Logic ────────────────────────────────────────────────────────
    let _ = registry.register(
        ConceptDefinition::axiom("axiom", "a self-evident or assumed statement that serves as a starting point in an axiomatic system, requiring no proof", "logic")
            .with_tag("foundation")
            .with_tag("logic")
    );

    let _ = registry.register(
        ConceptDefinition::theorem("theorem", "a mathematical statement that has been proven to be true from axioms and previously established theorems", "logic")
            .with_dependency("axiom")
            .with_tag("foundation")
            .with_tag("logic")
    );

    let _ = registry.register(
        ConceptDefinition::definition("proof", "a finite sequence of well-formed statements, each derived from earlier ones by valid rules of inference, starting from axioms or assumptions", "logic")
            .with_dependency("axiom")
            .with_tag("foundation")
            .with_tag("logic")
    );

    let _ = registry.register(
        ConceptDefinition::definition("mathematical_logic", "the study of formal reasoning, inference rules, and the foundations of mathematics", "logic")
            .with_dependency("axiom")
            .with_tag("foundation")
            .with_tag("logic")
    );

    // ── Mathematics Core ──────────────────────────────────────────────
    let _ = registry.register(
        ConceptDefinition::definition("function", "a relation between a set of inputs and a set of permissible outputs with the property that each input is related to exactly one output", "mathematics")
            .with_tag("foundation")
            .with_tag("mathematics")
    );

    // ── Geometry ──────────────────────────────────────────────────────
    let _ = registry.register(
        ConceptDefinition::definition("geometry", "the axiomatic branch of mathematics that studies the properties, measurement, and relationships of points, lines, angles, surfaces, and solids in space", "geometry")
            .with_dependency("axiom")
            .with_tag("mathematics")
            .with_tag("geometry")
    );

    // ── Topology ──────────────────────────────────────────────────────
    let _ = registry.register(
        ConceptDefinition::definition("topology", "the branch of mathematics that studies properties of space that are preserved under continuous deformations such as stretching, bending, and twisting", "topology")
            .with_tag("mathematics")
            .with_tag("topology")
    );

    // ── Trigonometry ──────────────────────────────────────────────────
    let _ = registry.register(
        ConceptDefinition::definition("trigonometry", "the branch of mathematics that studies relationships between the angles and side ratios of triangles, and the periodic functions derived from the unit circle", "trigonometry")
            .with_tag("mathematics")
            .with_tag("trigonometry")
    );

    // ── Physics Core ──────────────────────────────────────────────────
    let _ = registry.register(
        ConceptDefinition::definition("newtons_second_law", "the net force on an object equals its mass times its acceleration, F = ma", "physics")
            .with_formula("F = m*a", "newtons_second_law")
            .with_tag("physics")
            .with_tag("mechanics")
    );

    let _ = registry.register(
        ConceptDefinition::definition("kinetic_energy", "the energy possessed by an object due to its motion, KE = ½mv²", "physics")
            .with_formula("KE = 0.5*m*v^2", "kinetic_energy")
            .with_tag("physics")
            .with_tag("mechanics")
    );

    let _ = registry.register(
        ConceptDefinition::definition("gravitational_potential_energy", "the energy stored in an object due to its height above a reference level, PE = mgh", "physics")
            .with_formula("PE = m*g*h", "gravitational_potential_energy")
            .with_tag("physics")
            .with_tag("mechanics")
    );

    let _ = registry.register(
        ConceptDefinition::definition("elastic_potential_energy", "the energy stored in a deformed spring or elastic material, PE = ½kx²", "physics")
            .with_formula("PE = 0.5*k*x^2", "elastic_potential_energy")
            .with_tag("physics")
            .with_tag("mechanics")
    );

    let _ = registry.register(
        ConceptDefinition::definition("hookes_law", "the force exerted by a spring is proportional to its displacement from rest length and opposes the direction of displacement, F = -kx", "physics")
            .with_formula("F = -k*x", "hookes_law")
            .with_tag("physics")
            .with_tag("mechanics")
    );

    let _ = registry.register(
        ConceptDefinition::definition("momentum", "the product of an object's mass and its velocity, p = mv, describing its quantity of motion", "physics")
            .with_formula("p = m*v", "momentum")
            .with_tag("physics")
            .with_tag("mechanics")
    );

    let _ = registry.register(
        ConceptDefinition::definition("impulse", "the product of force and the time interval over which it acts, equal to the change in momentum, J = FΔt = Δp", "physics")
            .with_formula("J = F*delta_t = delta_p", "impulse")
            .with_tag("physics")
            .with_tag("mechanics")
    );

    let _ = registry.register(
        ConceptDefinition::definition("work", "the transfer of energy by a force acting over a displacement, W = F·d·cos(θ)", "physics")
            .with_formula("W = F*d*cos(theta)", "work")
            .with_tag("physics")
            .with_tag("mechanics")
    );

    let _ = registry.register(
        ConceptDefinition::definition("conservation_of_energy", "energy cannot be created or destroyed, only transformed from one form to another; total energy of an isolated system remains constant", "physics")
            .with_dependency("kinetic_energy")
            .with_dependency("gravitational_potential_energy")
            .with_dependency("elastic_potential_energy")
            .with_tag("physics")
            .with_tag("conservation")
    );

    let _ = registry.register(
        ConceptDefinition::definition("gravity", "the attractive force between objects with mass, proportional to the product of their masses and inversely proportional to the square of the distance between them", "physics")
            .with_tag("physics")
            .with_tag("mechanics")
    );

    let _ = registry.register(
        ConceptDefinition::definition("kinematics", "the motion of objects without considering the forces that cause motion, using position, velocity, acceleration, and time", "physics")
            .with_tag("physics")
            .with_tag("mechanics")
    );

    let _ = registry.register(
        ConceptDefinition::definition("conservation_of_momentum", "the total momentum of an isolated system remains constant regardless of internal interactions", "physics")
            .with_dependency("momentum")
            .with_tag("physics")
            .with_tag("conservation")
    );

    // ── Calculus ──────────────────────────────────────────────────────
    let _ = registry.register(
        ConceptDefinition::formal("derivative", "the instantaneous rate of change of a function with respect to its variable, measuring the slope of the tangent line at any point", "calculus")
            .with_tag("calculus")
            .with_tag("analysis")
    );

    let _ = registry.register(
        ConceptDefinition::formal("integral", "the accumulation of a quantity over an interval, representing the area under a curve or the reverse operation of differentiation", "calculus")
            .with_tag("calculus")
            .with_tag("analysis")
    );

    let _ = registry.register(
        ConceptDefinition::theorem("power_rule", "the derivative of x raised to the power n is n times x raised to the power n-1", "calculus")
            .with_formula("d/dx x^n = n*x^(n-1)", "power_rule")
            .with_dependency("derivative")
            .with_tag("calculus")
            .with_tag("derivative")
    );

    let _ = registry.register(
        ConceptDefinition::theorem("product_rule", "the derivative of the product of two functions is the first function times the derivative of the second plus the second function times the derivative of the first", "calculus")
            .with_formula("d/dx (u*v) = u*dv/dx + v*du/dx", "product_rule")
            .with_dependency("derivative")
            .with_tag("calculus")
            .with_tag("derivative")
    );

    let _ = registry.register(
        ConceptDefinition::theorem("chain_rule", "the derivative of a composite function f(g(x)) is the derivative of f evaluated at g(x) times the derivative of g", "calculus")
            .with_formula("d/dx f(g(x)) = f'(g(x))*g'(x)", "chain_rule")
            .with_dependency("derivative")
            .with_tag("calculus")
            .with_tag("derivative")
    );

    let _ = registry.register(
        ConceptDefinition::theorem("derivative_of_sin", "the derivative of sine of x is cosine of x", "calculus")
            .with_formula("d/dx sin(x) = cos(x)", "derivative_of_sin")
            .with_dependency("derivative")
            .with_tag("calculus")
            .with_tag("trigonometry")
    );

    let _ = registry.register(
        ConceptDefinition::theorem("derivative_of_cos", "the derivative of cosine of x is negative sine of x", "calculus")
            .with_formula("d/dx cos(x) = -sin(x)", "derivative_of_cos")
            .with_dependency("derivative")
            .with_tag("calculus")
            .with_tag("trigonometry")
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// TESTS
// ═══════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper to create a populated registry for testing.
    fn populated_registry() -> ConceptRegistry {
        let mut reg = ConceptRegistry::new();
        seed_foundational_concepts(&mut reg);
        reg
    }

    // ── Construction Tests ───────────────────────────────────────────

    #[test]
    fn test_empty_registry() {
        let reg = ConceptRegistry::new();
        assert!(reg.is_empty());
        assert_eq!(reg.len(), 0);
    }

    #[test]
    fn test_register_concept() {
        let mut reg = ConceptRegistry::new();
        let concept = ConceptDefinition::definition("test_concept", "a test concept", "testing");
        assert!(reg.register(concept).is_ok());
        assert!(!reg.is_empty());
        assert_eq!(reg.len(), 1);
    }

    #[test]
    fn test_register_duplicate_fails() {
        let mut reg = ConceptRegistry::new();
        let c1 = ConceptDefinition::definition("dup", "first", "test");
        let c2 = ConceptDefinition::definition("dup", "second", "test");
        assert!(reg.register(c1).is_ok());
        assert!(reg.register(c2).is_err());
    }

    #[test]
    fn test_get_concept() {
        let mut reg = ConceptRegistry::new();
        let _ = reg.register(
            ConceptDefinition::definition("my_concept", "a test definition", "test")
        );
        let c = reg.get("my_concept");
        assert!(c.is_some());
        assert_eq!(c.unwrap().name, "my_concept");
        assert_eq!(c.unwrap().definition, "a test definition");
    }

    #[test]
    fn test_get_nonexistent() {
        let reg = ConceptRegistry::new();
        assert!(reg.get("nothing").is_none());
    }

    #[test]
    fn test_contains() {
        let mut reg = ConceptRegistry::new();
        let _ = reg.register(
            ConceptDefinition::definition("exists", "something", "test")
        );
        assert!(reg.contains("exists"));
        assert!(!reg.contains("missing"));
    }

    // ── Search Tests ─────────────────────────────────────────────────

    #[test]
    fn test_search_by_name() {
        let reg = populated_registry();
        let results = reg.search("geometry");
        assert!(!results.is_empty());
        assert!(results.iter().any(|c| c.name == "geometry"));
    }

    #[test]
    fn test_search_by_definition_text() {
        let reg = populated_registry();
        let results = reg.search("derivative of a composite function");
        assert!(!results.is_empty());
        assert!(results.iter().any(|c| c.name == "chain_rule"));
    }

    #[test]
    fn test_search_by_tag() {
        let reg = populated_registry();
        let results = reg.search("conservation");
        assert!(!results.is_empty());
        assert!(results.iter().any(|c| c.name == "conservation_of_energy"));
    }

    #[test]
    fn test_search_case_insensitive() {
        let reg = populated_registry();
        let results = reg.search("PHYSICS");
        assert!(!results.is_empty());
        assert!(results.iter().any(|c| c.domain == "physics"));
    }

    #[test]
    fn test_search_no_match() {
        let reg = populated_registry();
        let results = reg.search("xyznonexistent");
        assert!(results.is_empty());
    }

    #[test]
    fn test_search_by_formula() {
        let reg = populated_registry();
        let results = reg.search("d/dx x^n");
        assert!(!results.is_empty());
        assert!(results.iter().any(|c| c.name == "power_rule"));
    }

    // ── Domain/Tag Index Tests ────────────────────────────────────────

    #[test]
    fn test_by_domain() {
        let reg = populated_registry();
        let physics_concepts = reg.by_domain("physics");
        assert!(!physics_concepts.is_empty());
        assert!(physics_concepts.iter().all(|c| c.domain == "physics"));
    }

    #[test]
    fn test_by_domain_nonexistent() {
        let reg = populated_registry();
        let results = reg.by_domain("nonexistent");
        assert!(results.is_empty());
    }

    #[test]
    fn test_by_tag() {
        let reg = populated_registry();
        let mechanics = reg.by_tag("mechanics");
        assert!(!mechanics.is_empty());
        assert!(mechanics.iter().all(|c| c.tags.contains(&"mechanics".to_string())));
    }

    #[test]
    fn test_by_tag_nonexistent() {
        let reg = populated_registry();
        let results = reg.by_tag("void");
        assert!(results.is_empty());
    }

    // ── Type Filter Tests ─────────────────────────────────────────────

    #[test]
    fn test_by_type() {
        let reg = populated_registry();
        let axioms = reg.by_type(&ConceptType::Axiom);
        assert!(!axioms.is_empty());
        assert!(axioms.iter().all(|c| c.concept_type == ConceptType::Axiom));
    }

    #[test]
    fn test_axioms() {
        let reg = populated_registry();
        let axioms = reg.axioms();
        assert!(axioms.iter().any(|c| c.name == "axiom"));
    }

    #[test]
    fn test_theorems() {
        let reg = populated_registry();
        let theorems = reg.theorems();
        assert!(theorems.iter().any(|c| c.name == "power_rule"));
    }

    #[test]
    fn test_formal_definitions() {
        let reg = populated_registry();
        let formals = reg.formal_definitions();
        assert!(formals.iter().any(|c| c.name == "derivative"));
        assert!(formals.iter().any(|c| c.name == "integral"));
    }

    // ── Dependency Tests ──────────────────────────────────────────────

    #[test]
    fn test_dependents() {
        let reg = populated_registry();
        let deps = reg.dependents("derivative");
        assert!(!deps.is_empty());
        // All derivative-based theorems should depend on "derivative"
        assert!(deps.iter().all(|c| c.depends_on.contains(&"derivative".to_string())));
    }

    #[test]
    fn test_dependency_closure() {
        let reg = populated_registry();
        // chain_rule depends on derivative
        // derivative has no explicit dependencies
        let closure = reg.dependency_closure("chain_rule");
        let names: Vec<&str> = closure.iter().map(|c| c.name.as_str()).collect();
        assert!(names.contains(&"chain_rule"));
        assert!(names.contains(&"derivative"));
    }

    // ── Access Tracking Tests ─────────────────────────────────────────

    #[test]
    fn test_record_access() {
        let mut reg = populated_registry();
        assert_eq!(reg.total_accesses(), 0);

        reg.record_access("geometry");
        assert_eq!(reg.total_accesses(), 1);

        let geo = reg.get("geometry").unwrap();
        assert_eq!(geo.access_count, 1);
    }

    #[test]
    fn test_record_access_nonexistent() {
        let mut reg = populated_registry();
        reg.record_access("nothing"); // should not panic
        assert_eq!(reg.total_accesses(), 1);
    }

    #[test]
    fn test_most_accessed() {
        let mut reg = populated_registry();
        reg.record_access("geometry");
        reg.record_access("geometry");
        reg.record_access("topology");
        reg.record_access("topology");
        reg.record_access("topology");

        let top = reg.most_accessed(2);
        assert_eq!(top.len(), 2);
        assert_eq!(top[0].0.name, "topology");
        assert_eq!(top[0].1, 3);
        assert_eq!(top[1].0.name, "geometry");
        assert_eq!(top[1].1, 2);
    }

    // ── Concept Type Tests ────────────────────────────────────────────

    #[test]
    fn test_concept_type_labels() {
        assert_eq!(ConceptType::Axiom.label(), "axiom");
        assert_eq!(ConceptType::Theorem.label(), "theorem");
        assert_eq!(ConceptType::FormalDefinition.label(), "formal definition");
        assert_eq!(ConceptType::Definition.label(), "definition");
        assert_eq!(ConceptType::Sensorimotor.label(), "sensorimotor");
        assert_eq!(ConceptType::Composite.label(), "composite");
        assert_eq!(ConceptType::ComputationRule.label(), "computation rule");
    }

    // ── Builder Pattern Tests ─────────────────────────────────────────

    #[test]
    fn test_concept_builder() {
        let c = ConceptDefinition::theorem("test_rule", "a test rule desc", "math")
            .with_tag("analysis")
            .with_formula("a^2 + b^2 = c^2", "pythagorean")
            .with_dependency("algebra")
            .with_part("sqrt")
            .with_qa_subject("pythagorean_theorem");

        assert_eq!(c.name, "test_rule");
        assert_eq!(c.domain, "math");
        assert!(c.tags.contains(&"analysis".to_string()));
        assert_eq!(c.formula, Some("a^2 + b^2 = c^2".to_string()));
        assert_eq!(c.formula_slug, Some("pythagorean".to_string()));
        assert!(c.depends_on.contains(&"algebra".to_string()));
        assert!(c.has_parts.contains(&"sqrt".to_string()));
        assert_eq!(c.qa_subject, Some("pythagorean_theorem".to_string()));
    }

    // ── Canonicl HV Tests ─────────────────────────────────────────────

    #[test]
    fn test_canonical_hv_auto_generated() {
        let c = ConceptDefinition::definition("test", "a test definition", "test");
        assert!(c.canonical_hv.is_some());
    }

    #[test]
    fn test_canonical_hv_roundtrip() {
        let mut c = ConceptDefinition::definition("eq", "F = ma", "physics");
        let hv1 = c.canonical_hv();
        c.canonical_hv = None; // clear cached
        let hv2 = c.canonical_hv(); // recomputed
        assert_eq!(hv1, hv2);
    }

    // ── Hierarchy Linkage Tests ───────────────────────────────────────

    #[test]
    fn test_set_hierarchy() {
        let mut reg = populated_registry();
        reg.set_hierarchy("geometry", 2, 5);
        let c = reg.get("geometry").unwrap();
        assert_eq!(c.hierarchy_level, Some(2));
        assert_eq!(c.hierarchy_index, Some(5));
    }

    // ── Remove Tests ──────────────────────────────────────────────────

    #[test]
    fn test_remove_concept() {
        let mut reg = populated_registry();
        assert!(reg.contains("geometry"));
        assert!(reg.remove("geometry"));
        assert!(!reg.contains("geometry"));
    }

    #[test]
    fn test_remove_nonexistent() {
        let mut reg = populated_registry();
        assert!(!reg.remove("nothing"));
    }

    #[test]
    fn test_unregister() {
        let mut reg = populated_registry();
        let c = reg.unregister("geometry");
        assert!(c.is_some());
        assert_eq!(c.unwrap().name, "geometry");
        assert!(!reg.contains("geometry"));
    }

    // ── Summary / Report Tests ────────────────────────────────────────

    #[test]
    fn test_summary() {
        let reg = populated_registry();
        let summary = reg.summary();
        assert!(summary.contains("Concept Registry"));
        assert!(summary.contains("axiom"));
        assert!(summary.contains("theorem"));
    }

    #[test]
    fn test_grounding_report() {
        let mut reg = populated_registry();
        reg.record_access("geometry");
        let report = reg.grounding_report();
        assert!(report.contains("GROUNDING REPORT"));
        assert!(report.contains("Most Accessed"));
        assert!(report.contains("geometry"));
        assert!(report.contains("By Domain"));
        assert!(report.contains("physics"));
    }

    // ── Foundational Seed Tests ───────────────────────────────────────

    #[test]
    fn test_seed_foundational_concepts_count() {
        let mut reg = ConceptRegistry::new();
        seed_foundational_concepts(&mut reg);
        // Should have at least 20 concepts
        assert!(reg.len() >= 20, "Expected >=20 concepts, got {}", reg.len());
    }

    #[test]
    fn test_seed_includes_core_concepts() {
        let reg = populated_registry();
        let core_names = ["axiom", "theorem", "proof", "geometry", "topology",
                          "trigonometry", "function", "derivative", "integral",
                          "power_rule", "chain_rule", "product_rule",
                          "newtons_second_law", "kinetic_energy", "momentum",
                          "conservation_of_energy", "hookes_law"];
        for name in &core_names {
            assert!(reg.contains(name), "Missing core concept: {}", name);
        }
    }

    #[test]
    fn test_seed_includes_axiom_and_theorem() {
        let reg = populated_registry();
        assert!(reg.contains("axiom"));
        assert!(reg.contains("theorem"));
        assert_eq!(reg.get("axiom").unwrap().concept_type, ConceptType::Axiom);
        assert_eq!(reg.get("theorem").unwrap().concept_type, ConceptType::Theorem);
    }

    #[test]
    fn test_seed_includes_formulas() {
        let reg = populated_registry();
        // Concepts that have formula strings should have them set
        let pr = reg.get("power_rule").unwrap();
        assert!(pr.formula.is_some());
        assert!(pr.formula.as_ref().unwrap().contains("d/dx"));
    }

    #[test]
    fn test_seed_includes_dependencies() {
        let reg = populated_registry();
        let chain = reg.get("chain_rule").unwrap();
        assert!(chain.depends_on.contains(&"derivative".to_string()));
    }

    // ── Builder Concept Type Construction Tests ───────────────────────

    #[test]
    fn test_axiom_builder() {
        let a = ConceptDefinition::axiom("parallel_postulate", "through a point not on a line, exactly one line can be drawn parallel to the given line", "geometry");
        assert_eq!(a.concept_type, ConceptType::Axiom);
        assert_eq!(a.domain, "geometry");
    }

    #[test]
    fn test_sensorimotor_builder() {
        let s = ConceptDefinition::sensorimotor("spring_oscillation", "the back-and-forth motion of a mass on a spring around its equilibrium position");
        assert_eq!(s.concept_type, ConceptType::Sensorimotor);
        assert_eq!(s.domain, "sensorimotor");
    }

    // ── Names / Listing Tests ─────────────────────────────────────────

    #[test]
    fn test_names() {
        let reg = populated_registry();
        let names = reg.names();
        assert!(names.contains(&"geometry".to_string()));
        assert!(names.contains(&"topology".to_string()));
    }

    #[test]
    fn test_concepts_list() {
        let reg = populated_registry();
        let concepts = reg.concepts();
        assert_eq!(concepts.len(), reg.len());
    }

    // ── Search formula text ───────────────────────────────────────────

    #[test]
    fn test_search_finds_by_formula() {
        let reg = populated_registry();
        // Search for "F = ma" should find newtons_second_law
        let results = reg.search("F = ma");
        assert!(results.iter().any(|c| c.name == "newtons_second_law"),
            "Should find newtons_second_law by formula text");
    }

    // ── Edge Cases ────────────────────────────────────────────────────

    #[test]
    fn test_empty_search() {
        let reg = ConceptRegistry::new();
        let results = reg.search("");
        // Empty query matches everything (since all strings contain "")
        assert_eq!(results.len(), reg.len());
    }

    #[test]
    fn test_register_with_empty_domain_still_indexed() {
        let mut reg = ConceptRegistry::new();
        let c = ConceptDefinition::definition("orphan", "an orphan concept", "");
        assert!(reg.register(c).is_ok());
        let orphans = reg.by_domain("");
        assert_eq!(orphans.len(), 1);
        assert_eq!(orphans[0].name, "orphan");
    }
}
