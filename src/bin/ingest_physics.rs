//! Populate the PhysicsKnowledge cache from Wikipedia.
//! Run once: cargo run --release --bin ingest_physics
//! Subsequent runs are instant from cache.
fn main() {
    let pk = the_machine::physics::load_or_fetch_physics_knowledge(true);
    eprintln!("\n═══════════════════════════════════════════════");
    eprintln!("  PhysicsKnowledge now has {} laws", pk.laws.len());
    eprintln!("  Cached to data/wikipedia_physics_cache.json");
    eprintln!("  Next startup will load instantly. ✨");
    eprintln!("═══════════════════════════════════════════════");
}
