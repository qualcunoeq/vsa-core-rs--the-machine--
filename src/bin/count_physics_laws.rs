use the_machine::math_ingest::FormulaRegistry;
use the_machine::physics::formula_entry_to_law;

fn main() {
    let registry = FormulaRegistry::load_from_file("data/formula_registry.json")
        .expect("Failed to load registry");

    let mut convert_count = 0usize;
    let mut last_slug = String::new();

    let formulas = registry.formulas();
    for entry in formulas {
        if let Some(law) = formula_entry_to_law(entry) {
            // Deduplicate by formula string
            if law.formula == last_slug {
                continue;
            }
            last_slug = law.formula.clone();
            convert_count += 1;
            if convert_count <= 30 {
                println!(
                    "  ✓ {}: {} (target: {}) [{}]",
                    entry.slug,
                    law.formula,
                    law.target_var,
                    law.tags.join(", ")
                );
            }
        }
    }

    println!("\n======= Physics Knowledge Summary =======");
    println!("Formulas in registry:         {}", formulas.len());
    println!("Converted to PhysicsLaw:      {}", convert_count);
    println!("Seeded (extended):            {}", 38);
    println!("Total PhysicsKnowledge:       ~{}", 38 + convert_count);
}
