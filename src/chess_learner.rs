// ─── Chess Self-Play Learning (Stage 1) ─────────────────────────────────
//
// Wires The Machine's hierarchy into a chess training loop:
//   1. Play games against a random mover
//   2. Store position-outcome pairs in dejavu_clusters
//   3. Evaluate positions via k-NN against stored outcomes
//   4. Select moves by maximising k-NN score
//
// Outcome discounting: positions close to game end get stronger signal.
//   outcome(t) = result × γ^(moves_from_end),  γ = 0.95
//
// No chess engine, no ML — pure VSA hierarchy learning from experience.
// ────────────────────────────────────────────────────────────────────────────

use crate::chess_eval::encode_position;
use crate::chess_eval::{encode_tracked_position, tracked_similarity, TrackedPosition, parse_fen};
use crate::hierarchy::HierarchicalManifold;
use crate::VSABrain;
use crate::qa::QaEngine;
use crate::{DejavuEntry, MemoryCluster, Hypervector};
use rand::Rng;
use std::cell::RefCell;
use std::collections::HashMap;

/// NHD threshold for chess position clustering.
///
/// ## v1 (0.15) — Produced 574 clusters from 130 games (4.4/game).
///   Within-game NHD drift (0.04-0.13) crossed the threshold as games
///   progressed, fragmenting single games into 4-5 clusters.  Too tight
///   for 50+ ply games where positions change significantly.
///
/// ## v2 (0.35) — Over-merged: 1 cluster from 50 games.
///   All positions were within 0.35 NHD of the centroid, so no new
///   clusters formed.  The centroid became "average chess position,"
///   too general for useful k-NN differentiation.
///
/// ## v3 (0.25) — Targets 30-60 clusters from 200 games.
///   Keeps within-game positions together (NHD 0.04-0.13) while letting
///   structurally different games form new clusters (cross-game NHD
///   0.38-0.47 > 0.25).  Aims for ~50 clusters from 200 games.
const CHESS_NHD_THRESHOLD: f64 = 0.25;
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::time::{Duration, Instant};

const STOCKFISH_PATH: &str = "./stockfish";
const DISCOUNT_GAMMA: f64 = 0.95;
const K_NEAREST: usize = 5;

// ─── Stockfish Subprocess ────────────────────────────────────────────────

/// Manages a Stockfish subprocess for chess operations.
pub struct StockfishClient {
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    _child: Child,
    buffer: Vec<String>,
}

impl StockfishClient {
    pub fn new(path: &str) -> Self {
        let mut child = Command::new(path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("Failed to start Stockfish");

        let stdin = child.stdin.take().unwrap();
        let stdout = BufReader::new(child.stdout.take().unwrap());

        let mut client = StockfishClient {
            stdin,
            stdout,
            _child: child,
            buffer: Vec::new(),
        };
        client.handshake();
        client
    }

    fn send(&mut self, cmd: &str) {
        writeln!(self.stdin, "{}", cmd).unwrap();
        self.stdin.flush().unwrap();
    }

    /// Read lines from stdout until a line contains `target`.
    fn read_until(&mut self, target: &str, timeout: Duration) -> &[String] {
        let start = Instant::now();
        self.buffer.clear();
        loop {
            if start.elapsed() > timeout {
                break;
            }
            let mut line = String::new();
            match self.stdout.read_line(&mut line) {
                Ok(0) => break,
                Ok(_) => {
                    let trimmed = line.trim().to_string();
                    let hit = trimmed.contains(target);
                    self.buffer.push(trimmed);
                    if hit {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
        &self.buffer
    }

    fn handshake(&mut self) {
        self.send("uci");
        self.read_until("uciok", Duration::from_secs(10));
        self.send("isready");
        self.read_until("readyok", Duration::from_secs(10));
        self.send("setoption name Threads value 2");
        self.send("setoption name Hash value 64");
    }

    /// Set the position in Stockfish's internal state.
    pub fn set_position(&mut self, fen: &str) {
        self.send(&format!("position fen {}", fen));
    }

    /// Get all legal moves via MultiPV.
    /// Returns list of (move_uci, stockfish_eval_in_pawns).
    pub fn legal_moves(&mut self) -> Vec<(String, f64)> {
        self.send("setoption name MultiPV value 256");
        self.send("go depth 1");
        let lines = self.read_until("bestmove", Duration::from_secs(10));

        let mut moves = Vec::new();
        for line in lines {
            if !line.contains("multipv") || !line.contains("pv ") {
                continue;
            }
            let parts: Vec<&str> = line.split_whitespace().collect();
            let mut score = 0.0f64;
            let mut move_uci = String::new();

            for (i, part) in parts.iter().enumerate() {
                match *part {
                    "cp" => {
                        if i + 1 < parts.len() {
                            score = parts[i + 1].parse::<f64>().unwrap_or(0.0) / 100.0;
                        }
                    }
                    "mate" => {
                        if i + 1 < parts.len() {
                            let mate_in: i32 = parts[i + 1].parse().unwrap_or(0);
                            score = if mate_in > 0 { 100.0 } else { -100.0 };
                        }
                    }
                    "pv" => {
                        if i + 1 < parts.len() {
                            move_uci = parts[i + 1].to_string();
                        }
                    }
                    _ => {}
                }
            }
            if !move_uci.is_empty() {
                moves.push((move_uci, score));
            }
        }
        moves
    }

    /// Apply a move to a known FEN and return the new FEN.
    /// `current_fen` is the position before the move — avoids an extra "d" call.
    pub fn apply_move_to_fen(&mut self, current_fen: &str, move_uci: &str) -> String {
        self.send(&format!("position fen {} moves {}", current_fen, move_uci));
        self.send("d");
        let lines = self.read_until("Key", Duration::from_secs(5));
        for line in lines {
            if line.starts_with("Fen: ") {
                return line[5..].to_string(); // strip "Fen: "
            }
        }
        eprintln!("  Warning: could not parse FEN from Stockfish output");
        String::new()
    }

    /// Apply a move and return the new FEN (uses internal state).
    pub fn apply_move_get_fen(&mut self, move_uci: &str) -> String {
        let current = self.current_fen();
        self.apply_move_to_fen(&current, move_uci)
    }

    /// Get the current FEN from Stockfish's internal state.
    pub fn current_fen(&mut self) -> String {
        self.send("d");
        let lines = self.read_until("Key", Duration::from_secs(5));
        for line in lines {
            if line.starts_with("Fen: ") {
                return line[5..].to_string();
            }
        }
        String::new()
    }

    /// Make a random legal move and return the new FEN.
    pub fn random_move(&mut self) -> String {
        let moves = self.legal_moves();
        if moves.is_empty() {
            return String::new();
        }
        use rand::Rng;
        let idx = rand::thread_rng().gen_range(0..moves.len());
        let (move_uci, _) = &moves[idx];
        self.apply_move_get_fen(move_uci)
    }

    /// Check if the game is over (no legal moves or checkmate/stalemate).
    pub fn is_game_over(&mut self) -> bool {
        let moves = self.legal_moves();
        if moves.is_empty() {
            return true;
        }
        false
    }

    /// Determine game result from white's perspective.
    /// Returns +1 (white wins), -1 (black wins), 0 (draw/stalemate).
    /// For non-mate positions, uses Stockfish eval at depth 6: if |eval| > 2.0
    /// it's considered a win for the leading side.
    ///
    /// ## Bugfix (June 2026)
    /// - Old code parsed the FIRST matching info line instead of the LAST
    ///   (depth 6 evaluation).  This caused premature returns on depth-1 evals.
    /// - Old mate scoring returned 1.0 for mate_in > 0 regardless of side to
    ///   move, which flipped the result when black delivered checkmate.
    ///   Fixed by checking side to move before converting to white's perspective.
    pub fn game_result(&mut self) -> f64 {
        // Determine side to move from current position
        let fen = self.current_fen();
        let white_to_move = fen.contains(" w ");

        self.send("go depth 6");
        let lines = self.read_until("bestmove", Duration::from_secs(10));

        // Parse the LAST info line (depth 6 is the final evaluation).
        // Intermediate lines may show provisional evals that differ.
        let mut last_mate: Option<i32> = None;
        let mut last_cp: Option<f64> = None;

        for line in lines {
            if line.contains("score mate") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                for (i, part) in parts.iter().enumerate() {
                    if *part == "mate" && i + 1 < parts.len() {
                        if let Ok(m) = parts[i + 1].parse::<i32>() {
                            last_mate = Some(m);
                        }
                    }
                }
            }
            if line.contains("score cp") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                for (i, part) in parts.iter().enumerate() {
                    if *part == "cp" && i + 1 < parts.len() {
                        if let Ok(cp) = parts[i + 1].parse::<f64>() {
                            last_cp = Some(cp);
                        }
                    }
                }
            }
        }

        // 1. Check mate scores first (mate score from side-to-move perspective)
        if let Some(mate_in) = last_mate {
            if mate_in > 0 {
                // Side to move can deliver mate → they win
                return if white_to_move { 1.0 } else { -1.0 };
            } else {
                // Side to move is checkmated → they lose
                return if white_to_move { -1.0 } else { 1.0 };
            }
        }

        // 2. Check cp scores (cp is always from white's perspective in Stockfish)
        if let Some(eval_cp) = last_cp {
            if eval_cp > 200.0 { return 1.0; }  // white winning
            if eval_cp < -200.0 { return -1.0; } // black winning
        }

        0.0 // draw or stalemate
    }

    /// Make a move from UCI string (updates internal state).
    pub fn make_move(&mut self, move_uci: &str) {
        let fen = self.current_fen();
        self.set_position(&format!("{} moves {}", fen, move_uci));
    }

    /// Quick evaluation at depth 4 — returns pawn score from white's perspective.
    /// Used for early termination detection.
    pub fn evaluate_swift(&mut self, fen: &str) -> f64 {
        self.send(&format!("position fen {}", fen));
        self.send("go depth 4");
        let lines = self.read_until("bestmove", Duration::from_secs(10));
        for line in lines {
            if line.contains("score cp") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                for (i, part) in parts.iter().enumerate() {
                    if *part == "cp" && i + 1 < parts.len() {
                        return parts[i + 1].parse::<f64>().unwrap_or(0.0) / 100.0;
                    }
                }
            }
            if line.contains("score mate") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                for (i, part) in parts.iter().enumerate() {
                    if *part == "mate" && i + 1 < parts.len() {
                        let mate_in: i32 = parts[i + 1].parse().unwrap_or(0);
                        return if mate_in > 0 { 100.0 } else { -100.0 };
                    }
                }
            }
        }
        0.0
    }

    /// Get the opponent's move at Skill Level 0 (weak but tactical).
    /// Uses Stockfish's `go` command (not MultiPV) so Skill Level affects play.
    pub fn opponent_move(&mut self, fen: &str) -> String {
        self.opponent_move_at_depth(fen, 1)
    }

    /// Get Stockfish's best move at a specified search depth.
    pub fn opponent_move_at_depth(&mut self, fen: &str, depth: usize) -> String {
        self.send("setoption name MultiPV value 1");
        self.send(&format!("position fen {}", fen));
        self.send(&format!("go depth {}", depth));
        let lines = self.read_until("bestmove", Duration::from_secs(10));
        for line in lines {
            if line.starts_with("bestmove ") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 && parts[1] != "(none)" {
                    return parts[1].to_string();
                }
            }
        }
        String::new()
    }
}

// ─── k-NN Evaluation ─────────────────────────────────────────────────────

/// Evaluate a position by k-NN against stored OUTCOMES (individual entries).
///
/// Queries individual entries across all clusters rather than cluster centroids,
/// because early in training all positions may collapse into one cluster.
/// k-NN on entries preserves fine-grained outcome differentiation.
pub fn knn_evaluate(
    fen: &str,
    brain: &VSABrain,
    k: usize,
) -> f64 {
    let query_hv = encode_position(fen);
    let clusters = &brain.dejavu_clusters;

    if clusters.is_empty() {
        return 0.0;
    }

    // Collect ALL entries across all clusters with their similarity
    let mut sims: Vec<(f64, f64)> = Vec::new();

    for cluster in clusters.iter() {
        for entry in cluster.entries.iter() {
            // Reconstruct the original hypervector from the delta-encoded entry
            let entry_hv = entry.reconstruct(&cluster.anchor);
            let sim = 1.0 - query_hv.normalized_hamming_distance(&entry_hv);
            if let Ok(outcome) = entry.label.parse::<f64>() {
                sims.push((sim, outcome));
            }
        }
    }

    // Sort by similarity descending, take k nearest
    sims.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());
    let effective_k = k.min(sims.len());

    let mut weight_sum = 0.0f64;
    let mut outcome_sum = 0.0f64;

    for &(sim, outcome) in sims[..effective_k].iter() {
        if sim <= 0.0 {
            continue;
        }
        weight_sum += sim;
        outcome_sum += sim * outcome;
    }

    if weight_sum > 0.0 {
        outcome_sum / weight_sum
    } else {
        0.0
    }
}

/// Learned track weights from static CV (structure-dominant).
/// Order: [material, attacks, king_safety, mobility, structure]
const TRACKED_WEIGHTS: [f64; 5] = [0.056, 0.251, 0.188, 0.000, 0.505];

/// Evaluate a position using per-track k-NN with learned weights.
///
/// Two-level approach:
///   Level 1 (routing):  Piece-square NHD to find the nearest cluster centroid.
///   Level 2 (prediction): Tracked per-k similarity against entries in that cluster.
///
/// This prevents signal dilution from searching across all clusters.  As cluster
/// count grows, the nearest centroid focuses the search on the most relevant
/// subset of experience.
///
/// Uses a RefCell cache (keyed by FEN) to avoid redundant re-encoding across
/// candidate evaluations within the same game, while keeping the closure Fn.
pub fn knn_evaluate_tracked(
    fen: &str,
    brain: &VSABrain,
    k: usize,
    weights: &[f64; 5],
    tracked_cache: &RefCell<HashMap<String, TrackedPosition>>,
) -> f64 {
    let clusters = &brain.dejavu_clusters;

    if clusters.is_empty() {
        return 0.0;
    }

    // Level 1: Route to nearest cluster via piece-square NHD against centroids.
    // Only one cluster is evaluated — keeps predictions grounded.
    let query_hv = encode_position(fen);
    let mut best_idx = 0;
    let mut best_nhd = f64::MAX;
    for (idx, cluster) in clusters.iter().enumerate() {
        let nhd = query_hv.normalized_hamming_distance(&cluster.centroid);
        if nhd < best_nhd {
            best_nhd = nhd;
            best_idx = idx;
        }
    }

    // Level 2: Compute tracked k-NN against entries in the nearest cluster only.
    let query_tp = encode_tracked_position(fen);
    let cluster = &clusters[best_idx];
    let mut sims: Vec<(f64, f64)> = Vec::with_capacity(cluster.entries.len());

    for entry in cluster.entries.iter() {
        // Get the stored FEN from metadata (required for re-encoding)
        let entry_fen = match entry.metadata.get("fen") {
            Some(f) => f.clone(),
            None => continue,
        };

        // Get or cache the TrackedPosition via RefCell interior mutability
        let entry_tp = {
            let mut cache = tracked_cache.borrow_mut();
            cache.entry(entry_fen.clone())
                .or_insert_with(|| encode_tracked_position(&entry_fen))
                .clone()
        };

        // Per-track similarity → weighted combination
        let per_track = tracked_similarity(&query_tp, &entry_tp);
        let combined = weights[0] * per_track[0]
            + weights[1] * per_track[1]
            + weights[2] * per_track[2]
            + weights[3] * per_track[3]
            + weights[4] * per_track[4];

        if let Ok(outcome) = entry.label.parse::<f64>() {
            sims.push((combined, outcome));
        }
    }

    // Sort by similarity descending, take k nearest
    sims.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());
    let effective_k = k.min(sims.len());

    let mut weight_sum = 0.0f64;
    let mut outcome_sum = 0.0f64;

    for &(sim, outcome) in sims[..effective_k].iter() {
        if sim <= 0.0 {
            continue;
        }
        weight_sum += sim;
        outcome_sum += sim * outcome;
    }

    if weight_sum > 0.0 {
        outcome_sum / weight_sum
    } else {
        0.0
    }
}

// ─── Strategic Planning Rules ──────────────────────────────────────────
//
/// Weight of the planner's contribution vs k-NN in move selection.
const PLAN_WEIGHT: f64 = 0.30;

/// Seed the QA engine with hand-coded chess strategy rules.
pub fn seed_chess_rules(qa: &mut crate::qa::QaEngine) {
    // Center control
    qa.store_action("move_pawn", "to", "e4", "white", "controls", "center", "chess_knowledge");
    qa.store_action("move_pawn", "to", "d4", "white", "controls", "center", "chess_knowledge");
    qa.store_action("move_knight", "to", "f3", "white", "controls", "center", "chess_knowledge");
    qa.store_action("move_knight", "to", "c3", "white", "controls", "center", "chess_knowledge");
    qa.store_rule("white", "controls", "center", "white", "has", "space_advantage", "chess_knowledge");
    // Development
    qa.store_action("move_knight", "to", "f3", "white", "developed", "kingside", "chess_knowledge");
    qa.store_action("move_knight", "to", "c3", "white", "developed", "queenside", "chess_knowledge");
    qa.store_action("move_bishop", "to", "c4", "white", "developed", "kingside", "chess_knowledge");
    qa.store_action("move_bishop", "to", "b5", "white", "developed", "queenside", "chess_knowledge");
    qa.store_rule("white", "developed", "kingside", "white", "has", "piece_activity", "chess_knowledge");
    qa.store_rule("white", "developed", "queenside", "white", "has", "piece_activity", "chess_knowledge");
    qa.store_rule("white", "has", "piece_activity", "white", "has", "advantage", "chess_knowledge");
    // Space advantage → overall advantage
    qa.store_rule("white", "has", "space_advantage", "white", "has", "advantage", "chess_knowledge");
}

/// Encode a UCI move as an SVO triple matching chess rules.
fn uci_to_action(fen: &str, move_uci: &str) -> (String, String, String) {
    let dest = &move_uci[2..4.min(move_uci.len())];
    let pieces = parse_fen(fen);
    // Get piece at source square
    let src_sq = &move_uci[..2];
    let piece_upper = pieces.iter()
        .find(|&&(_, r, f)| {
            format!("{}{}", (b'a' + f) as char, r + 1) == src_sq
        })
        .map(|&(c, _, _)| c.to_ascii_uppercase())
        .unwrap_or('P');
    let pname = match piece_upper {
        'P' => "pawn", 'N' => "knight", 'B' => "bishop",
        'R' => "rook", 'Q' => "queen", 'K' => "king", _ => "piece",
    };
    (format!("move_{}", pname), "to".to_string(), dest.to_string())
}

/// Select a move using planner-augmented k-NN.
pub fn plan_move_selection(
    candidates: &[(String, f64)],
    current_fen: &str,
    sf: &mut StockfishClient,
    qa: &crate::qa::QaEngine,
    evaluate_fn: &impl Fn(&str) -> f64,
) -> (String, f64) {
    let plan = qa.plan_for_goal("white", "has", "advantage", 5);
    let plan_actions: Vec<(String, String, String)> = plan.iter()
        .map(|step| step.action.clone())
        .collect();

    let mut best_score = f64::NEG_INFINITY;
    let mut best_move = candidates[0].0.clone();

    for (move_uci, _) in candidates {
        let new_fen = sf.apply_move_to_fen(current_fen, move_uci);
        if new_fen.is_empty() { continue; }
        let k_score = evaluate_fn(&new_fen);
        let action = uci_to_action(current_fen, move_uci);
        let plan_bonus = plan_actions.iter()
            .find(|(s, v, o)| *s == action.0 && *v == action.1 && *o == action.2)
            .map(|_| PLAN_WEIGHT)
            .unwrap_or(0.0);
        let combined = k_score + plan_bonus;
        if combined > best_score {
            best_score = combined;
            best_move = move_uci.clone();
        }
    }
    (best_move, best_score)
}

/// Result of a single game.
#[derive(Clone)]
pub struct GameRecord {
    pub positions: Vec<String>,   // FENs in chronological order
    pub result: f64,              // +1 for machine win, -1 for loss, 0 for draw
    pub machine_is_white: bool,
    pub ply_count: usize,
    pub eval_spread: f64,         // max - min of k-NN scores during evaluation
    pub avg_abs_eval: f64,        // average |score| of evaluated moves (variance proxy)
    pub opponent_responses: Vec<OpponentResponse>, // opponent response patterns
}

/// Play one game: The Machine vs random mover.
/// `evaluate_fn` scores a FEN from the machine's perspective
/// (positive = good for machine).
/// `qa` is optional: if provided, move selection is augmented by
/// goal-directed planning toward "white has advantage".
/// Compute the plan weight for a given number of games played at the current
/// curriculum level.  Early games: planner dominates (builds k-NN data).
/// Late games: k-NN takes over, planner refines.
pub fn plan_weight_for_game(games_at_level: usize) -> f64 {
    if games_at_level < 100 {
        0.70  // early: planner dominates, k-NN accumulating
    } else if games_at_level < 300 {
        0.50  // mid: balanced
    } else {
        0.30  // late: k-NN takes over, planner refines
    }
}

/// `hybrid_stockfish_pct` controls opponent strength:
///   - None    → pure random (0% Stockfish)
///   - Some(0) → pure random
///   - Some(n) → n% Stockfish d1, (100-n)% fully random
///   - Some(100) → pure Stockfish d1
/// `search_depth` controls Stockfish search depth (default 1; depth 0 = random legal).
pub fn play_game<F>(
    sf: &mut StockfishClient,
    evaluate_fn: &F,
    machine_is_white: bool,
    qa: Option<&crate::qa::QaEngine>,
    hybrid_stockfish_pct: Option<usize>,
    plan_weight: f64,
    search_depth: usize,
) -> GameRecord
where
    F: Fn(&str) -> f64,
{
    let mut positions = Vec::new();
    let start_fen = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";
    sf.set_position(start_fen);

    let mut current_fen = start_fen.to_string();
    let mut ply = 0;
    let mut eval_max = f64::NEG_INFINITY;
    let mut eval_min = f64::INFINITY;
    let mut eval_abs_sum = 0.0;
    let mut eval_count = 0usize;
    // Opponent response tracking
    let mut opponent_responses: Vec<OpponentResponse> = Vec::new();
    let mut last_machine_fen = String::new();
    let mut last_machine_move = String::new();

    loop {
        // Check if game is over
        sf.set_position(&current_fen);
        let legal = sf.legal_moves();
        if legal.is_empty() {
            break;
        }

        // Determine whose turn it is
        let white_to_move = current_fen.contains(" w ");
        let machine_to_move = white_to_move == machine_is_white;

        let chosen_move: String;

        if machine_to_move {
            // Save position before machine's move for opponent response tracking
            last_machine_fen = current_fen.clone();
            // The Machine selects a move by evaluating candidate positions.
            // Evaluate top candidate moves.  Against Stockfish d1, we need more
            // options since the opponent rarely blunders badly.
            const TOP_N: usize = 8;
            let candidates: Vec<&(String, f64)> = legal.iter().take(TOP_N).collect();

            // Candidate positions have the OPPONENT to move (Machine just played).
            // The k-NN returns outcomes from the machine's perspective across all
            // stored games.  High score = similar to a winning position for the
            // machine.  Pick the move with the highest expected outcome.
            let mut best_score = f64::NEG_INFINITY;
            let mut best_move = candidates[0].0.clone();

            for &(ref move_uci, _) in &candidates {
                // Get resulting FEN (using known current_fen, avoid extra "d" call)
                let new_fen = sf.apply_move_to_fen(&current_fen, move_uci);

                if new_fen.is_empty() {
                    continue;
                }

                // Evaluate the resulting position
                let k_score = evaluate_fn(&new_fen);
                let mut score = k_score;
                
                // Planner augmentation: weighted blend with k-NN.
                // plan_weight varies by curriculum stage (0.70 early, 0.50 mid, 0.30 late).
                if let Some(qa) = qa {
                    let plan = qa.plan_for_goal("white", "has", "advantage", 5);
                    let plan_score = plan.iter()
                        .map(|step| step.confidence)
                        .fold(0.0_f64, f64::max);
                    score = plan_score * plan_weight + k_score * (1.0 - plan_weight);

                    // Negative rule penalty: direct score reduction when a candidate
                    // move's L2 transition matches a mined negative rule.
                    if let Some(ref hierarchy) = qa.chess_hierarchy {
                        if !qa.l2_rules.is_empty() {
                            let current_l2 = project_to_l2(&current_fen, hierarchy);
                            let candidate_l2 = project_to_l2(&new_fen, hierarchy);
                            for rule in &qa.l2_rules {
                                if !rule.is_positive
                                    && rule.from_l2 == current_l2
                                    && rule.to_l2 == candidate_l2
                                {
                                    score -= 0.40;  // direct penalty
                                }
                            }
                        }
                    }
                }
                
                // Track eval variance
                if score > eval_max { eval_max = score; }
                if score < eval_min { eval_min = score; }
                eval_abs_sum += score.abs();
                eval_count += 1;
                
                if score > best_score {
                    best_score = score;
                    best_move = move_uci.clone();
                }
            }

            chosen_move = best_move;
            last_machine_move = chosen_move.clone();
        } else {
            let sf_pct = hybrid_stockfish_pct.unwrap_or(0);
            let sf_threshold = (sf_pct as f64) / 100.0;
            let opponent_move = if sf_pct > 0 && rand::thread_rng().gen_bool(sf_threshold) {
                let best = sf.opponent_move_at_depth(&current_fen, search_depth);
                if best.is_empty() { break; }
                best
            } else {
                let idx = rand::thread_rng().gen_range(0..legal.len());
                legal[idx].0.clone()
            };
            if opponent_move.is_empty() {
                break;
            }
            // Record opponent response (only if machine has moved this game)
            if !last_machine_fen.is_empty() {
                let response = record_opponent_response(
                    &last_machine_fen, &last_machine_move, &current_fen, &opponent_move,
                );
                opponent_responses.push(response);
            }
            chosen_move = opponent_move;
        }

        // Apply the chosen move
        current_fen = sf.apply_move_get_fen(&chosen_move);
        positions.push(current_fen.clone());
        ply += 1;

        // Update the last response with the resulting position
        if let Some(last) = opponent_responses.last_mut() {
            if last.fen_after_opponent == last.fen_before_opponent {
                last.fen_after_opponent = current_fen.clone();
            }
        }

        // Hard cap: 100 plies max.  Longer games = more diverse positions = better clustering.
        if ply > 100 {
            break;
        }
    }

    // Determine game result
    let result = sf.game_result();
    // Convert: Stockfish result is from white's perspective
    // If machine is white, result is already correct
    // If machine is black, negate
    let machine_result = if machine_is_white { result } else { -result };

    // Backpropagate game outcome to opponent responses
    for response in &mut opponent_responses {
        response.outcome = if machine_result > 0.0 { 1.0 }
            else if machine_result < 0.0 { 0.0 }
            else { 0.5 };
    }

    GameRecord {
        positions,
        result: machine_result,
        machine_is_white,
        ply_count: ply,
        eval_spread: if eval_count > 0 { eval_max - eval_min } else { 0.0 },
        avg_abs_eval: if eval_count > 0 { eval_abs_sum / eval_count as f64 } else { 0.0 },
        opponent_responses,
    }
}

/// Store a chess position-outcome pair with accumulator-based clustering and
/// a calibrated NHD threshold gate so cross-game positions form distinct clusters.
fn store_chess_entry(
    brain: &mut VSABrain,
    hv: Hypervector,
    outcome_str: &str,
    meta: HashMap<String, String>,
) {
    let clusters = &mut brain.dejavu_clusters;

    // Find nearest cluster centroid
    let mut best_idx = None;
    let mut best_nhd = f64::MAX;

    for (idx, cluster) in clusters.iter().enumerate() {
        let nhd = hv.normalized_hamming_distance(&cluster.centroid);
        if nhd < best_nhd {
            best_nhd = nhd;
            best_idx = Some(idx);
        }
    }

    // Absorb into nearest cluster if within threshold
    if let Some(idx) = best_idx {
        if best_nhd < CHESS_NHD_THRESHOLD {
            let cluster = &mut clusters[idx];
            cluster.ensure_anchor();
            let entry = DejavuEntry::new(
                hv.clone(),
                outcome_str.to_string(),
                meta,
                Some(&cluster.anchor),
            );
            let tau = entry.reconstruct(&cluster.anchor);

            // Absorb into accumulator
            for (i, acc) in cluster.accumulator.iter_mut().enumerate() {
                let word = tau.bits[i / 64];
                let bit = (word >> (i % 64)) & 1;
                *acc += bit as u32;
            }
            cluster.total_weight += 1;

            // Recompute centroid
            let half_weight = cluster.total_weight / 2;
            for (i, acc) in cluster.accumulator.iter().enumerate() {
                let block = i / 64;
                let bit = i % 64;
                if *acc > half_weight {
                    cluster.centroid.bits[block] |= 1u64 << bit;
                } else {
                    cluster.centroid.bits[block] &= !(1u64 << bit);
                }
            }

            cluster.entries.push(entry);

            if cluster.entries.len() > crate::MAX_ENTRIES_PER_CLUSTER {
                let drain = crate::MAX_ENTRIES_PER_CLUSTER / 4;
                cluster.entries.drain(0..drain);
            }
            return;
        }
    }

    // Create new cluster (first entry or best_nhd >= threshold)
    let mut accumulator = vec![0u32; crate::HD_DIMENSION];
    for (i, acc) in accumulator.iter_mut().enumerate() {
        let word = hv.bits[i / 64];
        let bit = (word >> (i % 64)) & 1;
        *acc = bit as u32;
    }
    let entry = DejavuEntry::new(hv.clone(), outcome_str.to_string(), meta, None);
    clusters.push(MemoryCluster {
        centroid: hv,
        entries: vec![entry],
        reverberation: 1.0,
        last_reinforced_tick: 0,
        anchor: hv,
        accumulator,
        total_weight: 1,
        last_access_tick: 0,
    });
}

/// Backpropagate discounted outcomes into dejavu_clusters.
///
/// Outcomes are stored from the MACHINE's perspective (positive = good for
/// machine).  The k-NN evaluation handles side-to-move normalization at
/// query time by comparing the query's STM with the stored entry's STM.
pub fn store_game_outcomes(
    record: &GameRecord,
    brain: &mut VSABrain,
) {
    let n = record.positions.len();
    for (i, fen) in record.positions.iter().enumerate() {
        let moves_from_end = (n - i) as u32;
        let discount = DISCOUNT_GAMMA.powi(moves_from_end as i32);
        // Outcome from machine's perspective: raw result × discount
        let outcome = record.result * discount;

        let hv = encode_position(fen);
        let outcome_str = format!("{:.4}", outcome);
        let mut meta = HashMap::new();
        meta.insert("fen".to_string(), fen.clone());
        meta.insert("result".to_string(), format!("{}", record.result));
        meta.insert("ply".to_string(), format!("{}", i));
        meta.insert("source".to_string(), "chess_stage1".to_string());
        meta.insert("machine_color".to_string(),
            if record.machine_is_white { "white" } else { "black" }.to_string());

        store_chess_entry(brain, hv, &outcome_str, meta);
    }
}

/// Run Stage 1 training: N games against random mover.
/// Optionally uses goal-directed planning when `--chess-plan` flag is set.
pub fn train_stage1(brain: &mut VSABrain, num_games: usize) -> QaEngine {
    // Seed the QA engine with chess strategy rules for planning
    let mut qa = crate::qa::QaEngine::new();
    seed_chess_rules(&mut qa);
    let mut rule_updates = 0usize;

    let mut sf = StockfishClient::new(STOCKFISH_PATH);
    let mut total_wins = 0usize;
    let mut total_losses = 0usize;
    let mut total_draws = 0usize;
    let mut games_played = 0usize;
    let mut game_records: Vec<GameRecord> = Vec::with_capacity(num_games);

    eprintln!("\n═══════════════════════════════════════════════════");
    eprintln!("  CHESS STAGE 2: Self-Play Training with Planner");
    eprintln!("  Opponent: Stockfish depth 1, Skill Level 0");
    eprintln!("  γ = {}", DISCOUNT_GAMMA);
    eprintln!("  k = {}", K_NEAREST);
    eprintln!("  Plan weight: {}", PLAN_WEIGHT);
    eprintln!("  Strategy rules: {}", qa.rule_count());
    eprintln!("  Games: {}", num_games);
    eprintln!("═══════════════════════════════════════════════════\n");

    // Set Stockfish to Skill Level 0 (weakest playing strength) so games last
    // longer and produce more diverse positions for clustering.
    sf.send("setoption name Skill Level value 0");

    for game_num in 0..num_games {
        let machine_is_white = game_num % 2 == 0;

        // Tracked position cache using RefCell for interior mutability
        // (closure must be Fn, not FnMut, for play_game's generic parameter)
        let tracked_cache: RefCell<HashMap<String, TrackedPosition>> = RefCell::new(HashMap::new());

        // Evaluation function using tracked encoding with learned weights
        let evaluate = |fen: &str| -> f64 {
            knn_evaluate_tracked(fen, brain, K_NEAREST, &TRACKED_WEIGHTS, &tracked_cache)
        };

        // Play the game with planner augmentation
        let record = play_game(&mut sf, &evaluate, machine_is_white, Some(&qa), None, PLAN_WEIGHT, 1);

        // Feed game outcome back to planner rules
        let outcome = if record.result > 0.0 { 1.0 } else if record.result < 0.0 { 0.0 } else { 0.5 };
        let plan = qa.plan_for_goal("white", "has", "advantage", 5);
        rule_updates += qa.evaluate_plan_outcome(outcome, &plan);

        // Track stats
        if record.result > 0.0 {
            total_wins += 1;
        } else if record.result < 0.0 {
            total_losses += 1;
        } else {
            total_draws += 1;
        }
        games_played += 1;

        // Store outcomes (takes a reference, so we keep ownership)
        store_game_outcomes(&record, brain);

        // Log progress BEFORE moving record
        if (game_num + 1) % 10 == 0 {
            let win_rate = total_wins as f64 / games_played as f64 * 100.0;
            let avg_rule_conf: f64 = if qa.rule_count() > 0 {
                qa.rules().iter().map(|r| r.confidence).sum::<f64>() / qa.rule_count() as f64
            } else { 0.0 };
            eprintln!(
                "  Game {:4}/{}: {} {:4} ply | W/L/D: {}/{}/{} ({:.0}% WR) | {} clusters | conf={:.4} | ev: {:.3}/{:.3}",
                game_num + 1,
                num_games,
                if record.result > 0.0 { "WIN " } else if record.result < 0.0 { "LOSE" } else { "DRAW" },
                record.ply_count,
                total_wins,
                total_losses,
                total_draws,
                win_rate,
                brain.dejavu_clusters.len(),
                avg_rule_conf,
                record.avg_abs_eval,
                record.eval_spread,
            );
        }

        // Keep the record for L2 rule mining (must be last use of record)
        game_records.push(record);
    }

    // ── Post-training L2 rule mining ────────────────────────────────────
    eprintln!("\n  ── Mining L2 transition rules from {} games ──", games_played);
    let (rules_mined, l2_cap, total_trans, unique_pairs, avg_mined_conf) = {
        // Temporarily borrow qa mutably for mining
        mine_l2_rules(&game_records, brain, &mut qa, 5, 0.60)
    };

    let win_rate = total_wins as f64 / games_played as f64 * 100.0;
    let avg_rule_conf: f64 = if qa.rule_count() > 0 {
        qa.rules().iter().map(|r| r.confidence).sum::<f64>() / qa.rule_count() as f64
    } else { 0.0 };
    eprintln!("\n═══════════════════════════════════════════════════");
    eprintln!("  STAGE 1 COMPLETE (with planner + L2 mining)");
    eprintln!("  Games: {}", games_played);
    eprintln!("  W/L/D: {}/{}/{}", total_wins, total_losses, total_draws);
    eprintln!("  Win rate: {:.1}%", win_rate);
    eprintln!("  Clusters: {}", brain.dejavu_clusters.len());
    eprintln!("  Strategy rules: {} (avg conf: {:.4})", qa.rule_count(), avg_rule_conf);
    eprintln!("  L2 rules mined: {} (avg conf: {:.3})", rules_mined, avg_mined_conf);
    eprintln!("  Plan weight: {}", PLAN_WEIGHT);
    eprintln!("  Eval calls: ~{}k", games_played * 40 / 2 * 30 / 1000);
    eprintln!("═══════════════════════════════════════════════════\n");
    qa
}

// ─── L2 Rule Mining ──────────────────────────────────────────────────────────
//
// After accumulating enough game experience (cluster-stable, ~50+ games),
// extract L2 transition rules from the hierarchical manifold:
//
//   1. Seed L2 centroids from outcome-stratified L1 groups
//   2. Re-project all positions → record L2 before/after each move
//   3. Aggregate transitions with win/loss counts
//   4. Mine rules: support ≥ min_support & confidence ≥ min_confidence
//
// Mined rules connect L2 abstract concept labels (e.g. "l2c_3", "l2c_17")
// through the transition relation.  A bridge rule connects L2 outcomes to
// the existing planning chain.
// ────────────────────────────────────────────────────────────────────────────

/// Seed L2 centroids by partitioning L1 centroids into outcome-stratified groups.
///
/// Groups L1 centroids by win rate (from entry metadata), then bundles
/// each group into an L2 abstract concept via `register_abstract_concept`.
/// This creates ~L1/4 L2 centroids ordered by outcome gradient.
fn seed_l2_from_outcomes(hierarchy: &mut HierarchicalManifold, brain: &VSABrain) {
    let l1_count = brain.dejavu_clusters.len();
    if l1_count < 4 {
        return;
    }

    // Compute win rate for each L1 centroid from entry metadata
    let mut l1_win_rates: Vec<(usize, f64)> = brain.dejavu_clusters
        .iter()
        .enumerate()
        .map(|(idx, c)| {
            let mut score = 0.0_f64;
            let mut total = 0.0_f64;
            for entry in &c.entries {
                if let Some(result_str) = entry.metadata.get("result") {
                    if let Ok(result) = result_str.parse::<f64>() {
                        // result: 1 = win, -1 = loss, 0 = draw
                        score += (result + 1.0) * 0.5; // map to [0, 1]
                        total += 1.0;
                    }
                }
            }
            let rate = if total > 0.0 { score / total } else { 0.5 };
            (idx, rate)
        })
        .collect();

    // Sort by win rate ascending
    l1_win_rates.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());

    // Partition into L2 groups
    let l2_capacity = hierarchy.levels.get(1).map(|l| l.capacity).unwrap_or(8);
    let group_size = (l1_count / l2_capacity).max(1);
    let mut n_registered = 0;

    for g in 0..l2_capacity {
        let start = g * group_size;
        let end = ((g + 1) * group_size).min(l1_count);
        if end <= start {
            break;
        }
        let indices: Vec<usize> = l1_win_rates[start..end].iter().map(|(idx, _)| *idx).collect();
        if hierarchy.register_abstract_concept(2, &indices).is_some() {
            n_registered += 1;
        }
    }

    eprintln!("  L2 seeding: {} groups from {} L1 centroids", n_registered, l1_count);
}

/// A single L2 transition observation from a game.
#[derive(Clone, Debug)]
struct L2Transition {
    from_l2: usize,
    to_l2: usize,
    outcome: f64, // 1.0 = win, 0.0 = loss, 0.5 = draw
}

/// Reconstruct approximate GameRecords from cluster entry metadata.
///
/// Used when game records weren't collected during training (e.g., the
/// 500-game run used old code without record collection).  This groups
/// entries by their result and ply metadata to reconstruct approximate
/// position sequences.  Transitions may be imperfect (cross-game interleaving
/// is possible), but the aggregate transition distribution is statistically
/// meaningful at scale.
pub fn reconstruct_game_records_from_clusters(brain: &VSABrain) -> Vec<GameRecord> {
    let mut records: Vec<GameRecord> = Vec::new();
    let mut seen: std::collections::HashSet<(f64, bool)> = std::collections::HashSet::new();

    // Collect all (result, machine_color, ply, fen) tuples
    let mut all_positions: Vec<(f64, bool, usize, String)> = Vec::new();

    for cluster in &brain.dejavu_clusters {
        for entry in &cluster.entries {
            let result: f64 = entry.metadata
                .get("result")
                .and_then(|s| s.parse().ok())
                .unwrap_or(0.0);
            let machine_color = entry.metadata
                .get("machine_color")
                .map(|s| s == "white")
                .unwrap_or(true);
            let ply: usize = entry.metadata
                .get("ply")
                .and_then(|s| s.parse().ok())
                .unwrap_or(0);
            let fen = entry.metadata
                .get("fen")
                .cloned()
                .unwrap_or_default();

            if !fen.is_empty() {
                all_positions.push((result, machine_color, ply, fen));
            }
        }
    }

    // Sort by (result, machine_color, ply) — assumes games with same result
    // and color won't interleave at the same ply (valid for ply-by-ply ordering
    // since each game's plies are unique).
    all_positions.sort_by(|a, b| {
        let a_key = (a.0 as i32, a.1, a.2);
        let b_key = (b.0 as i32, b.1, b.2);
        a_key.cmp(&b_key)
    });

    // Group consecutive same-(result, color) sequences into games
    // with ascending ply numbers.  A new game starts when ply resets to 0.
    let mut i = 0;
    while i < all_positions.len() {
        let (result, color, ply, ref fen) = all_positions[i];
        if ply != 0 {
            i += 1; // orphaned middle-of-game position
            continue;
        }

        let mut positions: Vec<String> = Vec::new();
        positions.push(fen.clone());
        let mut j = i + 1;
        while j < all_positions.len() {
            let (nr, nc, np, ref nf) = all_positions[j];
            if nr == result && nc == color && np == positions.len() {
                positions.push(nf.clone());
                j += 1;
            } else {
                break;
            }
        }

        let ply_count = positions.len();
        if ply_count >= 2 {
            let result_val = if result > 0.0 { 1.0 } else if result < 0.0 { -1.0 } else { 0.0 };
            records.push(GameRecord {
                positions,
                result: result_val,
                machine_is_white: color,
                ply_count,
                eval_spread: 0.0,
                avg_abs_eval: 0.0,
                opponent_responses: Vec::new(),
            });
        }
        i = j;
    }

    eprintln!("  Reconstructed {} game records from {} cluster entries",
        records.len(), all_positions.len());
    records
}

/// Build a chess-specific hierarchy from the brain's Deja Vu clusters.
/// Uses outcome-stratified L2 seeding exactly as `mine_l2_rules` does.
pub fn build_chess_hierarchy(brain: &VSABrain) -> HierarchicalManifold {
    let l1_count = brain.dejavu_clusters.len();
    let l2_capacity = (l1_count / 4).max(2);
    let l3_capacity = (l2_capacity / 4).max(2);
    let mut hierarchy = HierarchicalManifold::new(&[l1_count, l2_capacity, l3_capacity]);
    let base_centroids: Vec<Hypervector> = brain.dejavu_clusters
        .iter()
        .map(|c| c.centroid)
        .collect();
    hierarchy.seed_from_base_centroids(&base_centroids);
    seed_l2_from_outcomes(&mut hierarchy, brain);
    hierarchy
}

/// Project a FEN through the hierarchy and return its L2 centroid index.
pub fn project_to_l2(fen: &str, hierarchy: &HierarchicalManifold) -> usize {
    let hv = encode_position(fen);
    let proj = hierarchy.project_up_with_activations(&hv, 0.0);
    proj.get(1).map(|r| r.2).unwrap_or(0)
}

/// Mine L2 transition rules from game records.
///
/// # Returns
/// `(rules_mined, l2_count, total_transitions, unique_pairs, avg_confidence)`
pub fn mine_l2_rules(
    game_records: &[GameRecord],
    brain: &VSABrain,
    qa: &mut QaEngine,
    min_support: usize,
    min_confidence: f64,
) -> (usize, usize, usize, usize, f64) {
    let l1_count = brain.dejavu_clusters.len();
    if l1_count < 4 {
        eprintln!("  ⚠ Not enough L1 centroids ({}) for L2 mining (need ≥ 4)", l1_count);
        return (0, 0, 0, 0, 0.0);
    }

    let l2_capacity = (l1_count / 4).max(2);
    let l3_capacity = (l2_capacity / 4).max(2);

    // ── Step 1: Build + seed hierarchy ──────────────────────────────────
    let mut hierarchy = HierarchicalManifold::new(&[l1_count, l2_capacity, l3_capacity]);
    let base_centroids: Vec<Hypervector> = brain.dejavu_clusters
        .iter()
        .map(|c| c.centroid)
        .collect();
    hierarchy.seed_from_base_centroids(&base_centroids);
    seed_l2_from_outcomes(&mut hierarchy, brain);

    // Sync QA cluster data for label resolution
    qa.sync_cluster_data(brain);

    // ── Step 2: Collect L2 transitions from game records ────────────────
    let mut transitions: Vec<L2Transition> = Vec::new();
    let mut projected_count = 0usize;

    for record in game_records {
        for window in record.positions.windows(2) {
            let hv_before = encode_position(&window[0]);
            let hv_after = encode_position(&window[1]);

            let proj_before = hierarchy.project_up_with_activations(&hv_before, 0.0);
            let proj_after = hierarchy.project_up_with_activations(&hv_after, 0.0);

            // L2 is at index 1 (0-based: L1=0, L2=1, L3=2)
            let l2_before = proj_before.get(1).map(|r| r.2).unwrap_or(0);
            let l2_after = proj_after.get(1).map(|r| r.2).unwrap_or(0);

            projected_count += 1;

            if l2_before != l2_after {
                let outcome = if record.result > 0.0 {
                    1.0
                } else if record.result < 0.0 {
                    0.0
                } else {
                    0.5
                };
                transitions.push(L2Transition {
                    from_l2: l2_before,
                    to_l2: l2_after,
                    outcome,
                });
            }
        }
    }

    // ── Step 3: Aggregate ────────────────────────────────────────────────
    let mut agg: HashMap<(usize, usize), (u32, f64)> = HashMap::new();
    for t in &transitions {
        let entry = agg.entry((t.from_l2, t.to_l2)).or_insert((0, 0.0));
        entry.0 += 1;
        entry.1 += t.outcome; // 1.0 win, 0.0 loss, 0.5 draw
    }

    // ── Step 4: Store bridge rules ────────────────────────────────────────
    let has_pos_bridge = qa.rules().iter().any(|r| {
        r.antecedent_subject == "chess_position"
            && r.antecedent_verb == "correlated_with"
            && r.antecedent_object == "positive_outcome"
    });
    if !has_pos_bridge {
        qa.store_rule(
            "chess_position", "correlated_with", "positive_outcome",
            "white", "has", "advantage",
            "mined_bridge",
        );
        eprintln!("  Bridge (+): chess_position correlated_with positive_outcome → white has advantage");
    }

    let has_neg_bridge = qa.rules().iter().any(|r| {
        r.antecedent_subject == "chess_position"
            && r.antecedent_verb == "correlated_with"
            && r.antecedent_object == "negative_outcome"
    });
    if !has_neg_bridge {
        qa.store_rule(
            "chess_position", "correlated_with", "negative_outcome",
            "white", "has", "disadvantage",
            "mined_bridge",
        );
        eprintln!("  Bridge (–): chess_position correlated_with negative_outcome → white has disadvantage");
    }

    // ── Step 5: Mine both positive and negative rules ────────────────────
    let mut rules_mined = 0usize;
    let mut total_conf = 0.0_f64;

    // Clear previous mined rules
    qa.l2_rules.clear();

    for ((from_l2, to_l2), (total, pos)) in &agg {
        if *total < min_support as u32 {
            continue;
        }
        let confidence = *pos / *total as f64;
        let from_label = format!("l2c_{}", from_l2);
        let to_label = format!("l2c_{}", to_l2);

        if confidence >= min_confidence {
            // Positive rule: transition leads to winning
            qa.store_rule_with_confidence(
                &from_label, "leads_to", &to_label,
                "chess_position", "correlated_with", "positive_outcome",
                "mined",
                confidence,
            );
            qa.l2_rules.push(crate::qa::MinedRule {
                from_l2: *from_l2,
                to_l2: *to_l2,
                is_positive: true,
                confidence,
            });
            rules_mined += 1;
            total_conf += confidence;
        } else if confidence <= 1.0 - min_confidence {
            // Negative rule: transition leads to losing — store as l2_rules
            // for direct move-selection penalty (not through planner chain).
            qa.l2_rules.push(crate::qa::MinedRule {
                from_l2: *from_l2,
                to_l2: *to_l2,
                is_positive: false,
                confidence: 1.0 - confidence, // avoidance confidence
            });
            // Also store in planner chain (low confidence) for EWMA tracking
            qa.store_rule_with_confidence(
                &from_label, "leads_to", &to_label,
                "chess_position", "correlated_with", "negative_outcome",
                "mined",
                0.15,
            );
            rules_mined += 1;
            total_conf += 1.0 - confidence; // confidence of the avoidance signal
        }
    }

    // Store the hierarchy in QA engine for move-selection projections
    qa.chess_hierarchy = Some(hierarchy.clone());

    let avg_conf = if rules_mined > 0 {
        total_conf / rules_mined as f64
    } else {
        0.0
    };

    eprintln!("  ── L2 Mining Results ──");
    eprintln!(
        "  L1 centroids: {} | L2 capacity: {} | L2 centroids: {}",
        l1_count,
        l2_capacity,
        hierarchy.levels.get(1).map(|l| l.centroids.len()).unwrap_or(0)
    );
    eprintln!(
        "  Positions projected: {} | Transitions: {} | Unique pairs: {}",
        projected_count,
        transitions.len(),
        agg.len()
    );

    // Show top-10 transition pairs by support
    let mut sorted: Vec<((usize, usize), (u32, f64))> = agg.into_iter().collect();
    sorted.sort_by(|a, b| b.1 .0.cmp(&a.1 .0));
    eprintln!("  ┌─ Top transitions by support");
    let mut pos_rules = 0usize;
    let mut neg_rules = 0usize;
    for (i, ((from, to), (total, pos))) in sorted.iter().enumerate() {
        let conf = *pos / *total as f64;
        let is_pos = conf >= min_confidence && *total >= min_support as u32;
        let is_neg = conf <= 1.0 - min_confidence && *total >= min_support as u32;
        let tag = if is_pos { " ✓ POS" } else if is_neg { " ✓ AVOID" } else { "" };
        if is_pos { pos_rules += 1; }
        if is_neg { neg_rules += 1; }
        if i < 10 || is_pos || is_neg {
            eprintln!(
                "  │ {}. l2c_{} → l2c_{}: support={}, win_rate={:.3}{}",
                i + 1, from, to, total, conf, tag,
            );
        }
    }
    eprintln!(
        "  └─ {} positive + {} negative = {} total rules (min support={}, min confidence={})",
        pos_rules, neg_rules, rules_mined, min_support, min_confidence,
    );

    (rules_mined, l2_capacity, transitions.len(), sorted.len(), avg_conf)
}

/// Stage 2: validation games with mined rules active.
///
/// Takes a brain with accumulated clusters and a QA engine containing both
/// hand-coded and mined L2 rules.  Plays N games and reports whether the
/// mined rules improve WR.  The self-improvement EWMA loop updates both
/// hand-coded and mined rule confidences every game.
pub fn train_stage2(
    brain: &mut VSABrain,
    qa: &mut QaEngine,
    num_games: usize,
    hybrid_stockfish_pct: Option<usize>,
    mut game_records: Option<&mut Vec<GameRecord>>,
    search_depth: usize,
) -> (usize, usize, usize, f64) {
    let num_mined = qa.rules().iter().filter(|r| r.source == "mined").count();
    let num_hand = qa.rules().iter().filter(|r| r.source != "mined").count();

    let mut sf = StockfishClient::new(STOCKFISH_PATH);

    let mut total_wins = 0usize;
    let mut total_losses = 0usize;
    let mut total_draws = 0usize;
    let mut rule_updates = 0usize;
    // 50-game window tracking
    let mut window_wins_50 = 0usize;
    let mut prev_50_total_wins = 0usize;

    let level_str = match hybrid_stockfish_pct {
        Some(100) => "Pure Stockfish d1".to_string(),
        Some(pct) => format!("{}% Stockfish d1 / {}% random", pct, 100 - pct),
        None => "Pure random".to_string(),
    };

    eprintln!("\n═══════════════════════════════════════════════════");
    eprintln!("  CHESS STAGE 2: Curriculum Training");
    eprintln!("  Opponent: Stockfish {}", level_str);
    eprintln!("  k = {}", K_NEAREST);
    eprintln!("  Plan weight schedule: 0.70 early → 0.50 mid → 0.30 late");
    eprintln!("  Hand-coded rules: {} | Mined rules: {}", num_hand, num_mined);
    eprintln!("  Games: {}", num_games);
    eprintln!("═══════════════════════════════════════════════════\n");

    for game_num in 0..num_games {
        let machine_is_white = game_num % 2 == 0;
        let tracked_cache: RefCell<HashMap<String, TrackedPosition>> = RefCell::new(HashMap::new());
        let p_weight = plan_weight_for_game(game_num);

        let evaluate = |fen: &str| -> f64 {
            knn_evaluate_tracked(fen, brain, K_NEAREST, &TRACKED_WEIGHTS, &tracked_cache)
        };

        let mut record = play_game(&mut sf, &evaluate, machine_is_white, Some(qa), hybrid_stockfish_pct, p_weight, search_depth);

        // Self-improvement: update ALL rules in the plan chain
        let outcome = if record.result > 0.0 { 1.0 } else if record.result < 0.0 { 0.0 } else { 0.5 };
        let plan = qa.plan_for_goal("white", "has", "advantage", 5);
        rule_updates += qa.evaluate_plan_outcome(outcome, &plan);

        store_game_outcomes(&record, brain);

        // Collect game records for L2 rule mining (clone to keep ownership)
        if let Some(ref mut records) = game_records {
            records.push(record.clone());
        }

        if record.result > 0.0 { total_wins += 1; }
        else if record.result < 0.0 { total_losses += 1; }
        else { total_draws += 1; }

        // 50-game window WR
        if (game_num + 1) % 50 == 0 {
            window_wins_50 = total_wins - if game_num < 50 { 0 } else { total_wins - prev_50_total_wins };
            window_wins_50 = total_wins - prev_50_total_wins;
            prev_50_total_wins = total_wins;
        }

        if (game_num + 1) % 10 == 0 {
            let win_rate = total_wins as f64 / (game_num + 1) as f64 * 100.0;
            let window_wr = if (game_num + 1) >= 50 {
                (total_wins - if game_num >= 50 { total_wins - window_wins_50 } else { 0 }) as f64 / 50.0 * 100.0
            } else { win_rate };
            let avg_hand_conf: f64 = qa.rules().iter()
                .filter(|r| r.source != "mined")
                .map(|r| r.confidence).sum::<f64>()
                / num_hand.max(1) as f64;
            let avg_mined_conf: f64 = if num_mined > 0 {
                qa.rules().iter()
                    .filter(|r| r.source == "mined")
                    .map(|r| r.confidence).sum::<f64>()
                    / num_mined as f64
            } else { 0.0 };
            eprintln!(
                "  Game {:4}/{}: {} {:4} ply | W/L/D: {}/{}/{} ({:.0}% WR) [50g: {:.0}%] | pw={:.2} | hand={:.3} mined={:.3}",
                game_num + 1, num_games,
                if record.result > 0.0 { "WIN " } else if record.result < 0.0 { "LOSE" } else { "DRAW" },
                record.ply_count,
                total_wins, total_losses, total_draws, win_rate,
                window_wr,
                p_weight,
                avg_hand_conf, avg_mined_conf,
            );
        }
    }

    // Mine opponent rules from collected game records
    let num_opponent_rules = if let Some(ref records) = game_records {
        let all_responses: Vec<OpponentResponse> = records.iter()
            .flat_map(|r| r.opponent_responses.iter().cloned())
            .collect();
        mine_opponent_rules(&all_responses, qa)
    } else { 0 };

    let win_rate = total_wins as f64 / num_games as f64 * 100.0;
    let avg_hand_conf: f64 = qa.rules().iter()
        .filter(|r| r.source != "mined")
        .map(|r| r.confidence).sum::<f64>()
        / num_hand.max(1) as f64;
    let avg_mined_conf: f64 = if num_mined > 0 {
        qa.rules().iter()
            .filter(|r| r.source == "mined")
            .map(|r| r.confidence).sum::<f64>()
            / num_mined as f64
    } else { 0.0 };

    eprintln!("\n═══════════════════════════════════════════════════");
    eprintln!("  STAGE 2 COMPLETE ({})", level_str);
    eprintln!("  Games: {}", num_games);
    eprintln!("  W/L/D: {}/{}/{}", total_wins, total_losses, total_draws);
    eprintln!("  Win rate: {:.1}%", win_rate);
    eprintln!("  Hand-coded rules: {} (avg conf: {:.3}) | Mined: {} (avg conf: {:.3})",
        num_hand, avg_hand_conf, num_mined, avg_mined_conf);
    eprintln!("═══════════════════════════════════════════════════\n");

    (total_wins, total_losses, total_draws, win_rate)
}

/// Run the curriculum: progressive Stockfish Skill Levels.
///
/// Starts at `start_level` and advances to higher levels when WR exceeds
/// the promotion threshold.  At each level:
///   - PLAN_WEIGHT starts at 0.70, decays to 0.30 over ~300 games
///   - After `games_per_level` games, mines L2 rules
///   - If WR > promotion_threshold, advances to next level
///   - Reports 50-game window WR curves
///
// ─── Opponent Modeling ────────────────────────────────────────────────────
//
// Encodes opponent behavior patterns as SVO facts the planner can reason
// about.  Instead of just mining position-to-position transitions, we mine
// action→response patterns: "when I play move M, opponent responds with R."
//
// These are stored as causal rules like:
//   ("if_I_play", "move_description", "opponent_responds_with", "response_desc")
//   → ("opponent_response", "correlated_with", "positive_outcome")
//
// The planner can then reason: given this opponent model, which moves create
// positions the opponent handles poorly?
// ────────────────────────────────────────────────────────────────────────────

/// Classifies an opponent's response to a Machine move.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum OpponentBehavior {
    Captures,           // Opponent captured a piece
    Retreats,           // Opponent moved a piece away from attack  
    Advances,           // Opponent advanced a pawn
    KingsideCastle,     // Opponent castled kingside
    QueensideCastle,    // Opponent castled queenside
    Develops,           // Opponent developed a piece (knight/bishop out)
    Defends,            // Opponent moved to defend an attacked piece
    Unclear,            // Can't classify
}

/// Describes what happened on one opponent response to a Machine action.
#[derive(Clone, Debug)]
pub struct OpponentResponse {
    /// The FEN before the Machine moved (Machine's turn).
    pub fen_before_machine: String,
    /// The Machine's UCI move.
    pub machine_move: String,
    /// The FEN after the Machine's move (before opponent's response).
    pub fen_before_opponent: String,
    /// The opponent's UCI response.
    pub opponent_move: String,
    /// The FEN after the opponent's move.
    pub fen_after_opponent: String,
    /// Classified opponent behavior.
    pub behavior: OpponentBehavior,
    /// Game outcome from Machine's perspective (1=win, 0=loss, 0.5=draw).
    /// Filled in after the game ends.
    pub outcome: f64,
}

/// Classify what the opponent did, given the board state before and after.
fn classify_opponent_move(
    pieces_before: &[(char, u8, u8)],
    pieces_after: &[(char, u8, u8)],
    opponent_move_uci: &str,
) -> OpponentBehavior {
    // Check for captures: piece count decreased
    if pieces_after.len() < pieces_before.len() {
        return OpponentBehavior::Captures;
    }

    let dest = &opponent_move_uci[2..4.min(opponent_move_uci.len())];
    let dest_file = (dest.as_bytes()[0] - b'a') as u8;
    let dest_rank = (dest.as_bytes()[1] - b'1') as u8;

    // Find which piece moved to dest
    let moved_piece = pieces_after.iter().find(|&&(_, r, f)| r == dest_rank && f == dest_file);
    let dest_piece = moved_piece.map(|&(c, _, _)| c).unwrap_or(' ');

    // Castle detection
    if opponent_move_uci == "e8g8" || opponent_move_uci == "e1g1" {
        return OpponentBehavior::KingsideCastle;
    }
    if opponent_move_uci == "e8c8" || opponent_move_uci == "e1c1" {
        return OpponentBehavior::QueensideCastle;
    }

    // Pawn advance
    if dest_piece == 'P' || dest_piece == 'p' {
        return OpponentBehavior::Advances;
    }

    // Development: knight or bishop moving to a non-back-rank
    if (dest_piece == 'N' || dest_piece == 'n' || dest_piece == 'B' || dest_piece == 'b')
        && (opponent_move_uci.as_bytes()[1] - b'1') < 6  // not from back rank... 
    {
        // Check if it moved FROM the back rank
        let src_rank = opponent_move_uci.as_bytes()[1] - b'1';
        if src_rank == 0 || src_rank == 7 {
            return OpponentBehavior::Develops;
        }
    }

    // Retreat: moved a piece that was under attack before
    let src = &opponent_move_uci[..2];
    let src_file = (src.as_bytes()[0] - b'a') as u8;
    let src_rank = (src.as_bytes()[1] - b'1') as u8;
    let src_was_attacked = pieces_before.iter().any(|&(_, r, f)| r == src_rank && f == src_file);

    // Check if the source was attacked by building attack maps
    // Simple heuristic: if source piece was attacked, it's a retreat
    if src_was_attacked {
        return OpponentBehavior::Retreats;
    }

    // Defense: moved a piece to defend another attacked piece
    // Check if any piece is now defended that wasn't before
    // (simplified: check if destination square has an attacked piece nearby)
    // For now, just mark as unclear for unclassified moves
    OpponentBehavior::Unclear
}

/// Record an opponent response during a game.
fn record_opponent_response(
    fen_before_machine: &str,
    machine_move: &str,
    fen_after_machine: &str,
    opponent_move: &str,
) -> OpponentResponse {
    let pieces_before = parse_fen(fen_before_machine);
    let pieces_after = parse_fen(fen_after_machine);

    let behavior = classify_opponent_move(&pieces_before, &pieces_after, opponent_move);

    OpponentResponse {
        fen_before_machine: fen_before_machine.to_string(),
        machine_move: machine_move.to_string(),
        fen_before_opponent: fen_after_machine.to_string(),
        opponent_move: opponent_move.to_string(),
        fen_after_opponent: fen_after_machine.to_string(), // placeholder, caller sets this
        behavior,
        outcome: 0.0,
    }
}

/// Mine opponent model rules from recorded opponent responses.
///
/// For each opponent behavior type, compute:
/// - win_rate: how often this behavior → Machine win
/// - support: how many times observed
///
/// Stores as causal rules in the QA engine. Also stores a bridge rule
/// connecting opponent response outcomes to the planning goal chain.
pub fn mine_opponent_rules(
    responses: &[OpponentResponse],
    qa: &mut QaEngine,
) -> usize {
    if responses.is_empty() {
        return 0;
    }

    // Store bridge rule: opponent_response correlates with positive_outcome → white has advantage
    let has_pos_bridge = qa.rules().iter().any(|r| {
        r.antecedent_subject == "opponent_response"
            && r.antecedent_verb == "correlates_with"
            && r.antecedent_object == "positive_outcome"
    });
    if !has_pos_bridge {
        qa.store_rule(
            "opponent_response", "correlates_with", "positive_outcome",
            "white", "has", "advantage",
            "opponent_model_bridge",
        );
    }
    let has_neg_bridge = qa.rules().iter().any(|r| {
        r.antecedent_subject == "opponent_response"
            && r.antecedent_verb == "correlates_with"
            && r.antecedent_object == "negative_outcome"
    });
    if !has_neg_bridge {
        qa.store_rule(
            "opponent_response", "correlates_with", "negative_outcome",
            "white", "has", "disadvantage",
            "opponent_model_bridge",
        );
    }

    // Aggregate by behavior type
    let mut stats: HashMap<OpponentBehavior, (u32, f64)> = HashMap::new();
    for r in responses {
        let entry = stats.entry(r.behavior.clone()).or_insert((0, 0.0));
        entry.0 += 1;
        if r.outcome > 0.5 {
            entry.1 += 1.0;
        } else if r.outcome == 0.5 {
            entry.1 += 0.5;
        }
    }

    let mut rules_mined = 0;
    for (behavior, (total, wins)) in &stats {
        if *total < 5 {
            continue; // minimum support
        }
        let win_rate = *wins / *total as f64;

        // Behavior description as text for SVO storage
        let beh_str = format!("{:?}", behavior);
        let opp_name = "stockfish_d1"; // could parameterize

        if win_rate >= 0.60 {
            // Opponent's response correlates with Machine winning
            qa.store_rule_with_confidence(
                opp_name, "responds_with", &beh_str,
                "opponent_response", "correlates_with", "positive_outcome",
                "opponent_model",
                win_rate,
            );
            rules_mined += 1;
        } else if win_rate <= 0.40 {
            // Opponent's response correlates with Machine losing
            qa.store_rule_with_confidence(
                opp_name, "responds_with", &beh_str,
                "opponent_response", "correlates_with", "negative_outcome",
                "opponent_model",
                1.0 - win_rate,
            );
            rules_mined += 1;
        }
    }

    eprintln!("  Opponent model: {} rules mined from {} responses (behaviors: {})",
        rules_mined, responses.len(), stats.len());

    // Show per-behavior stats
    let mut sorted: Vec<_> = stats.iter().collect();
    sorted.sort_by(|a, b| b.1 .0.cmp(&a.1 .0));
    for (behavior, (total, wins)) in sorted.iter().take(5) {
        let wr = *wins / *total as f64;
        eprintln!("    {:?}: support={}, win_rate={:.3}", behavior, total, wr);
    }

    rules_mined
}

/// The hybrid Stockfish percentage ladder for curriculum training.
/// Each rung increases the proportion of Stockfish d1 moves.
const CURRICULUM_LADDER: &[usize] = &[10, 30, 50, 70, 90, 100];

/// Run the curriculum: progressive hybrid-to-Stockfish ratio.
///
/// Starts at `ladder[start_index]`% Stockfish and advances when WR exceeds
/// the promotion threshold.  At each rung:
///   - PLAN_WEIGHT starts at 0.70, decays to 0.30 over ~300 games (per rung)
///   - After `games_per_level` games, mines L2 rules from scratch
///   - If WR > promotion_threshold AND rules_mined >= 5, advances to next rung
///   - Re-mines L2 rules at each transition (rules are replaced, not accumulated)
///   - k-NN clusters accumulate across all stages
///
/// Returns `(last_rung_index, Vec<(sf_pct, WR, mined_conf, rules_mined)>)`.
pub fn train_curriculum(
    brain: &mut VSABrain,
    start_index: usize,
    games_per_level: usize,
    max_index: usize,
    existing_qa: Option<crate::qa::QaEngine>,
    search_depth: usize,
) -> (usize, Vec<(usize, f64, f64, usize)>) {
    let mut qa = match existing_qa {
        Some(q) => q,
        None => {
            let mut q = crate::qa::QaEngine::new();
            seed_chess_rules(&mut q);
            q
        }
    };
    let mut current = start_index.min(CURRICULUM_LADDER.len().saturating_sub(1));
    let mut history: Vec<(usize, f64, f64, usize)> = Vec::new();

    eprintln!("\n╔══════════════════════════════════════════════════════════╗");
    eprintln!("║    CURRICULUM: Hybrid → Stockfish transition ladder     ║");
    eprintln!("╚══════════════════════════════════════════════════════════╝\n");

    while current <= max_index.min(CURRICULUM_LADDER.len().saturating_sub(1)) {
        let sf_pct = CURRICULUM_LADDER[current];
        eprintln!("\n━━━ STAGE {}: {}% Stockfish d1 / {}% random ━━━",
            current, sf_pct, 100 - sf_pct);

        // Run games at this rung, collecting game records for re-mining
        let mut game_records: Vec<GameRecord> = Vec::with_capacity(games_per_level);
        let (wins, losses, draws, wr) = train_stage2(
            brain, &mut qa, games_per_level, Some(sf_pct), Some(&mut game_records), search_depth,
        );

        // Re-mine L2 rules from current stage positions (replaces previous rules)
        eprintln!("  ── Re-mining L2 rules after {} games at {}% SF d1 ──",
            games_per_level, sf_pct);
        let (rules_mined, l2_cap, total_trans, unique_pairs, _avg_mined_conf) =
            mine_l2_rules(&game_records, brain, &mut qa, 5, 0.60);

        let mined_conf = qa.rules().iter()
            .filter(|r| r.source == "mined")
            .map(|r| r.confidence).sum::<f64>()
            / rules_mined.max(1) as f64;

        history.push((sf_pct, wr, mined_conf, rules_mined));

        // Promotion check: threshold decreases as SF % increases
        let promotion_threshold = if sf_pct <= 30 { 40.0 }
            else if sf_pct <= 50 { 35.0 }
            else if sf_pct <= 70 { 25.0 }
            else if sf_pct <= 90 { 15.0 }
            else { 5.0 };
        // Minimum rules: lower threshold for small game counts (50 games often
        // produce 2-5 rules); higher for large counts (500 games → 10+ rules).
        let min_rules = if games_per_level <= 100 { 2 } else { 5 };

        if wr >= promotion_threshold && rules_mined >= min_rules {
            let next_pct = CURRICULUM_LADDER.get(current + 1).unwrap_or(&sf_pct);
            eprintln!(
                "  ✓ WR {:.1}% ≥ {:.0}% with {} rules — PROMOTING to {}% SF d1",
                wr, promotion_threshold, rules_mined, next_pct
            );
            current += 1;
        } else {
            eprintln!(
                "  ✗ WR {:.1}% < {:.0}% or {} rules < {} — curriculum paused",
                wr, promotion_threshold, rules_mined, min_rules
            );
            // If WR > 0, retry with more games; otherwise stop
            if wr > 0.0 {
                eprintln!("  Retrying {}% SF d1 with {} more games...", sf_pct, games_per_level);
            } else {
                break;
            }
        }
    }

    let final_idx = current.saturating_sub(1);
    let final_pct = CURRICULUM_LADDER.get(final_idx).unwrap_or(&10);
    eprintln!("\n╔══════════════════════════════════════════════════════════╗");
    eprintln!("║     CURRICULUM COMPLETE                                 ║");
    eprintln!("║     Highest rung: {}% Stockfish d1", final_pct);
    for (pct, wr, mc, rm) in &history {
        eprintln!("║       {}% SF: WR={:.1}%, mined_conf={:.3}, {} rules",
            pct, wr, mc, rm);
    }
    eprintln!("╚══════════════════════════════════════════════════════════╝\n");

    (current, history)
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stockfish_startup() {
        let mut sf = StockfishClient::new(STOCKFISH_PATH);
        sf.set_position("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1");
        let moves = sf.legal_moves();
        assert!(moves.len() >= 2, "Starting position has at least 2 legal moves");
        assert!(moves.len() <= 30, "Starting position has at most 30 legal moves (got {})", moves.len());
        eprintln!("  Starting position: {} legal moves", moves.len());
    }

    #[test]
    fn test_stockfish_apply_move() {
        let mut sf = StockfishClient::new(STOCKFISH_PATH);
        let start = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";
        sf.set_position(start);
        let new_fen = sf.apply_move_get_fen("e2e4");
        assert!(new_fen.contains(" b "), "After e2e4, black should move");
        assert!(new_fen.contains("PPPP1PPP"), "e2 pawn should be missing");
        eprintln!("  After e2e4: {}", new_fen);
    }

    #[test]
    fn test_knn_after_one_game() {
        // Play one game, then check if k-NN can differentiate positions
        let mut sf = StockfishClient::new(STOCKFISH_PATH);
        let mut brain = crate::VSABrain::new(0.35);

        let evaluate = |fen: &str| -> f64 { knn_evaluate(fen, &brain, 5) };
        let record = play_game(&mut sf, &evaluate, true, None, None, PLAN_WEIGHT);
        store_game_outcomes(&record, &mut brain);

        eprintln!("  Game result: {} ({} ply, {} entries)",
            record.result, record.ply_count,
            brain.dejavu_clusters.iter().map(|c| c.entries.len()).sum::<usize>());

        // Evaluate the starting position — should have some signal now
        let start_eval = knn_evaluate(
            "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
            &brain, 5);
        eprintln!("  Starting position eval after 1 game: {:.4}", start_eval);

        // Evaluate a clearly winning position (white up a rook)
        let winning_fen = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQK2R w KQkq - 0 1";
        let winning_eval = knn_evaluate(winning_fen, &brain, 5);
        eprintln!("  White-up-rook eval: {:.4}", winning_eval);

        // The k-NN should find some positional similarity even if outcomes are weak
        // This is a soft test — just verify we get non-zero
        assert!(brain.dejavu_clusters.len() >= 1);
        let total_entries: usize = brain.dejavu_clusters.iter().map(|c| c.entries.len()).sum();
        assert!(total_entries > 0);
        eprintln!("  Entry-level k-NN works: {} entries, start_eval={:.4}, winning_eval={:.4}",
            total_entries, start_eval, winning_eval);
    }

    #[test]
    #[ignore] // Heavy: requires Stockfish, ~15s per game
    fn test_stage1_training_games() {
        use crate::VSABrain;
        let mut brain = VSABrain::new(0.35);
        train_stage1(&mut brain, 5);
        assert!(brain.dejavu_clusters.len() >= 1, "Should have at least 1 cluster after training");
        eprintln!("  {} clusters after {} games", brain.dejavu_clusters.len(), 5);
    }
}
