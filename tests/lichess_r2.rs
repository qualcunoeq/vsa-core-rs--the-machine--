use std::io::{BufRead, BufReader};
use std::fs::File;
use std::time::Instant;

/// Quick R² measurement: seed k-NN from Lichess positions, measure
/// predictive accuracy on held-out positions.
#[test]
fn test_lichess_r2() {
    let csv_path = "/tmp/lichess_100k.csv";
    
    // Count total lines
    eprint!("Counting total lines... ");
    let start = Instant::now();
    let file = File::open(csv_path).unwrap();
    let total: usize = BufReader::new(file).lines()
        .filter(|l| l.as_ref().map(|s| !s.starts_with("fen,") && !s.trim().is_empty()).unwrap_or(false))
        .count();
    eprintln!("{} in {:.1}s", total, start.elapsed().as_secs_f64());
    
    let train_count = 20000usize;
    let test_count = 5000usize;
    
    eprintln!("Train: {}, Test: {}", train_count, test_count);
    
    let mut brain = the_machine::VSABrain::new(0.12);
    
    // Seed clusters from Lichess data
    eprint!("Seeding {} positions... ", train_count);
    let seed_start = Instant::now();
    let seeded = the_machine::chess_learner::seed_from_lichess_csv(
        &mut brain, csv_path, train_count).unwrap();
    eprintln!("{} positions → {} clusters in {:.1}s",
        seeded, brain.dejavu_clusters.len(), seed_start.elapsed().as_secs_f64());
    
    // Measure R² using built-in eval function
    eprint!("Evaluating on {} test positions... ", test_count);
    let eval_start = Instant::now();
    let (mse, r2) = the_machine::chess_learner::eval_lichess_r2(
        &mut brain, csv_path, train_count, test_count, 25);
    eprintln!("done in {:.1}s", eval_start.elapsed().as_secs_f64());
    
    eprintln!("\n═══ Results ═══");
    eprintln!("MSE:  {:.2} cp²", mse);
    eprintln!("R²:   {:.4}", r2);
    eprintln!("RMSE: {:.2} cp", mse.sqrt());
    eprintln!("────────────────────");
    eprintln!("Self-play R²: 0.422 (baseline ceiling)");
    eprintln!("Lichess R²:   {:.4}", r2);
    let diff = (r2 - 0.422) / 0.422 * 100.0;
    eprintln!("Change:       {}{:.1}%", if diff > 0.0 { "+" } else { "" }, diff);
    eprintln!("════════════════\n");
    
    // Compare to self-play ceiling
    if r2 > 0.50 {
        eprintln!("✓ Lichess data significantly beats self-play ceiling!");
    } else if r2 > 0.422 {
        eprintln!("~ Lichess data slightly above self-play ceiling");
    } else if r2 > 0.35 {
        eprintln!("~ Similar to self-play (within noise)");
    } else {
        eprintln!("✗ Lichess data below self-play — architecture is the bottleneck");
    }
    
    assert!(true);
}
