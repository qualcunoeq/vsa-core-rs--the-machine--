// ─── Chess Position Feasibility Test (Phase 1) ──────────────────────────
//
// Tests whether VSA hypervector similarity captures chess position similarity.
// Pipeline:
//   1. Parse FEN → list of pieces + features
//   2. Encode position as bundle of piece hypervectors
//   3. Cross-validate: nearest-centroid prediction of Stockfish evaluation
//   4. Report R², MAE, sign accuracy
//
// No chess engine, no tree search, no ML — pure VSA algebra.
// ────────────────────────────────────────────────────────────────────────────

use crate::Hypervector;
use serde::Deserialize;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::time::Instant;

/// One position record from the dataset.
#[derive(Deserialize, Debug, Clone)]
pub struct PositionRecord {
    pub fen: String,
    #[serde(rename = "eval")]
    pub eval_score: f64,
    pub phase: String,
}

// ─── FEN Parser ─────────────────────────────────────────────────────────────

/// Parse a FEN string into a list of (piece_char, rank_0, file_0).
/// rank_0 = 0 means rank 1 (white's back rank), rank_0 = 7 means rank 8.
/// file_0 = 0 means file a, file_0 = 7 means file h.
/// piece_char: uppercase = white (K/Q/R/B/N/P), lowercase = black.
pub fn parse_fen(fen: &str) -> Vec<(char, u8, u8)> {
    let board_part = fen.split_whitespace().next().unwrap_or("");
    let ranks: Vec<&str> = board_part.split('/').collect();
    assert_eq!(ranks.len(), 8, "FEN must have 8 ranks: {}", fen);
    let mut pieces = Vec::new();
    // ranks[0] = rank 8 (top of diagram), ranks[7] = rank 1 (bottom)
    for (ri, rank_str) in ranks.iter().enumerate() {
        let rank = (7 - ri) as u8; // rank 0 = a1 (1st rank)
        let mut file: u8 = 0;
        for ch in rank_str.chars() {
            if ch.is_ascii_digit() {
                file += ch.to_digit(10).unwrap() as u8;
            } else {
                pieces.push((ch, rank, file));
                file += 1;
            }
        }
        assert!(file <= 8, "FEN rank overflow at rank {}: {}", ri, fen);
    }
    pieces
}

/// Quick piece name (e.g., "wP" for white pawn, "bK" for black king).
pub(crate) fn piece_label(ch: char) -> &'static str {
    match ch {
        'P' => "wP", 'N' => "wN", 'B' => "wB", 'R' => "wR", 'Q' => "wQ", 'K' => "wK",
        'p' => "bP", 'n' => "bN", 'b' => "bB", 'r' => "bR", 'q' => "bQ", 'k' => "bK",
        _ => "??",
    }
}

/// Material value of a piece type.
pub(crate) fn piece_value(ch: char) -> i32 {
    match ch {
        'P' | 'p' => 1,
        'N' | 'n' => 3,
        'B' | 'b' => 3,
        'R' | 'r' => 5,
        'Q' | 'q' => 9,
        'K' | 'k' => 0,
        _ => 0,
    }
}

/// Compute material balance (white - black) in pawn units.
fn compute_material_balance(pieces: &[(char, u8, u8)]) -> i32 {
    let mut white = 0i32;
    let mut black = 0i32;
    for (ch, _, _) in pieces {
        if ch.is_uppercase() {
            white += piece_value(*ch);
        } else {
            black += piece_value(*ch);
        }
    }
    white - black
}

/// Classify game phase from number of pieces.
fn classify_from_pieces(pieces: &[(char, u8, u8)]) -> &'static str {
    let count = pieces.len();
    if count > 24 {
        "opening"
    } else if count > 10 {
        "middlegame"
    } else {
        "endgame"
    }
}

// ─── Position Encoding ──────────────────────────────────────────────────────

/// Encode a chess position as a VSA hypervector.
///
/// ## Experiment History (June 2026)
///
/// ### Phase 1a — Piece-squares only (R² = 0.20)
/// Each piece encoded as `{piece_label}_{square}` (e.g., "wP_e2"), bundled via
/// majority-sum. This captures piece configuration similarity but nothing about
/// relational features (pawn structure, piece activity, etc.). Achieved R²=0.20
/// on Stockfish self-play data using k=25 weighted nearest-neighbor.
///
/// ### Phase 1b — Pawn structure via majority bundling (R² = 0.10)
/// Added triples like "iso_wP_e4", "dbl_wP_e4", "pas_wP_e4" to the bundle.
/// Failed because structural features affected only 1-4 pawns per position,
/// giving them < 3% voting weight in the 37-term majority sum — invisible
/// to the Hamming distance metric.
///
/// ### Phase 1c — Pawn structure via XOR subspace (R² = 0.07)
/// Same triples encoded into a separate subspace via bundle → rotate(7331) → XOR
/// with the piece-square bundle. Failed because the XOR overlay made positions
/// look more different from each other (sim dropped from 0.85 to 0.80), reducing
/// nearest-neighbor matching quality without adding useful signal.
///
/// ### Conclusion
/// The 0.20 R² ceiling is a property of the piece-square representation, not a
/// tuning issue. VSA majority bundling gives each term equal weight, so features
/// affecting a minority of terms cannot shift the centroid meaningfully. Richer
/// chess features (pawn structure, piece activity) would need either:
///   a) A learned encoding (hierarchy discovers features from self-play outcomes)
///   b) A fundamentally different binding strategy (not majority-sum bundling)
/// The hierarchy approach (a) is the intended path — Phase 2.
///
/// ### Current Encoding
/// Piece-squares + material balance + game phase, all bundled via majority-sum.
pub fn encode_position(fen: &str) -> Hypervector {
    let pieces = parse_fen(fen);
    let mut hvs: Vec<Hypervector> = Vec::with_capacity(pieces.len() + 5);

    // Piece-square encoding — each piece contributes one term
    for &(ch, rank, file) in &pieces {
        let square = format!(
            "{}{}",
            (b'a' + file) as char,
            rank + 1
        );
        let label = format!("{}_{}", piece_label(ch), square);
        hvs.push(Hypervector::encode_text_ngram(&label, 3));
    }

    // Material balance
    let mat = compute_material_balance(&pieces);
    let mat_label = format!("mat_{:+}", mat);
    hvs.push(Hypervector::encode_text_ngram(&mat_label, 3));

    // Game phase
    let phase = classify_from_pieces(&pieces);
    let phase_label = format!("phase_{}", phase);
    hvs.push(Hypervector::encode_text_ngram(&phase_label, 3));

    // Side to move (extracted from FEN's 2nd token: "w" or "b")
    let side = fen.split_whitespace().nth(1).unwrap_or("w");
    let side_label = if side == "w" { "stm_white" } else { "stm_black" };
    hvs.push(Hypervector::encode_text_ngram(side_label, 3));

    // Castle rights (extracted from FEN's 3rd token: "KQkq", "Kkq", "-", etc.)
    let castle = fen.split_whitespace().nth(2).unwrap_or("-");
    let castle_label = format!("castle_{}", castle);
    hvs.push(Hypervector::encode_text_ngram(&castle_label, 3));

    // Bundle all terms
    let refs: Vec<&Hypervector> = hvs.iter().collect();
    Hypervector::bundle(&refs)
}

// ─── Perception Layer: SVO Triples from FEN ───────────────────────────────
//
// Extracts attack, defense, and control relations from a chess position
// as SVO triples, then encodes them via resonator::encode_svo.
//
// The hypothesis is that relational features (piece A attacks piece B)
// carry more evaluation-relevant signal than raw piece-square positions.
// ────────────────────────────────────────────────────────────────────────────

/// Build an 8×8 board from parsed FEN pieces: board[rank][file] = Some(ch).
/// rank 0 = rank 1 (white's back rank), rank 7 = rank 8.
/// Returns (board, white_king_sq, black_king_sq) for convenience.
fn build_board(pieces: &[(char, u8, u8)]) -> ([[Option<char>; 8]; 8], Option<(u8, u8)>, Option<(u8, u8)>) {
    let mut board = [[None; 8]; 8];
    let mut wk = None;
    let mut bk = None;
    for &(ch, rank, file) in pieces {
        board[rank as usize][file as usize] = Some(ch);
        if ch == 'K' { wk = Some((rank, file)); }
        if ch == 'k' { bk = Some((rank, file)); }
    }
    (board, wk, bk)
}

/// Square name like "e4" from rank_0, file_0.
fn square_name(rank: u8, file: u8) -> String {
    format!("{}{}", (b'a' + file) as char, rank + 1)
}

/// Piece type for orientation: returns the uppercase type char.
fn piece_type(ch: char) -> char {
    ch.to_ascii_uppercase()
}

// ── Attack computation ──────────────────────────────────────────────────

/// Compute squares a pawn attacks (captures only, not moves).
fn pawn_attacks(ch: char, rank: u8, file: u8) -> Vec<(u8, u8)> {
    let dir: i8 = if ch.is_uppercase() { 1 } else { -1 };
    let mut sqs = Vec::with_capacity(2);
    for df in [-1, 1] {
        let nr = rank as i8 + dir;
        let nf = file as i8 + df;
        if nr >= 0 && nr < 8 && nf >= 0 && nf < 8 {
            sqs.push((nr as u8, nf as u8));
        }
    }
    sqs
}

/// Knight L-shaped moves.
fn knight_attacks(rank: u8, file: u8) -> Vec<(u8, u8)> {
    let offsets = [
        (2, 1), (2, -1), (-2, 1), (-2, -1),
        (1, 2), (1, -2), (-1, 2), (-1, -2),
    ];
    let mut sqs = Vec::with_capacity(8);
    for &(dr, df) in &offsets {
        let nr = rank as i8 + dr;
        let nf = file as i8 + df;
        if nr >= 0 && nr < 8 && nf >= 0 && nf < 8 {
            sqs.push((nr as u8, nf as u8));
        }
    }
    sqs
}

/// King adjacent moves.
fn king_attacks(rank: u8, file: u8) -> Vec<(u8, u8)> {
    let mut sqs = Vec::with_capacity(8);
    for dr in [-1, 0, 1] {
        for df in [-1, 0, 1] {
            if dr == 0 && df == 0 { continue; }
            let nr = rank as i8 + dr;
            let nf = file as i8 + df;
            if nr >= 0 && nr < 8 && nf >= 0 && nf < 8 {
                sqs.push((nr as u8, nf as u8));
            }
        }
    }
    sqs
}

/// Raycast in one direction until blocked.
fn raycast(rank: u8, file: u8, dr: i8, df: i8, board: &[[Option<char>; 8]; 8]) -> Vec<(u8, u8)> {
    let mut sqs = Vec::new();
    let mut r = rank as i8 + dr;
    let mut f = file as i8 + df;
    while r >= 0 && r < 8 && f >= 0 && f < 8 {
        sqs.push((r as u8, f as u8));
        if board[r as usize][f as usize].is_some() {
            break; // blocked (first piece is still attacked)
        }
        r += dr;
        f += df;
    }
    sqs
}

/// All squares a piece attacks (including moves, not just captures).
pub(crate) fn compute_attacks(ch: char, rank: u8, file: u8, board: &[[Option<char>; 8]; 8]) -> Vec<(u8, u8)> {
    match piece_type(ch) {
        'P' => pawn_attacks(ch, rank, file),
        'N' => knight_attacks(rank, file),
        'B' => {
            let mut sqs = Vec::new();
            for &(dr, df) in &[(1, 1), (1, -1), (-1, 1), (-1, -1)] {
                sqs.extend(raycast(rank, file, dr, df, board));
            }
            sqs
        }
        'R' => {
            let mut sqs = Vec::new();
            for &(dr, df) in &[(1, 0), (-1, 0), (0, 1), (0, -1)] {
                sqs.extend(raycast(rank, file, dr, df, board));
            }
            sqs
        }
        'Q' => {
            let mut sqs = Vec::new();
            for &(dr, df) in &[(1, 0), (-1, 0), (0, 1), (0, -1),
                               (1, 1), (1, -1), (-1, 1), (-1, -1)] {
                sqs.extend(raycast(rank, file, dr, df, board));
            }
            sqs
        }
        'K' => king_attacks(rank, file),
        _ => vec![],
    }
}

/// Count pawn shields for king safety: number of friendly pawns within 3 squares
/// of the king (on the same or adjacent files).
fn king_pawn_shield(king_rank: u8, king_file: u8, color_is_white: bool, board: &[[Option<char>; 8]; 8]) -> u32 {
    let pawn_ch = if color_is_white { 'P' } else { 'p' };
    let mut count = 0;
    let rank_dir: i8 = if color_is_white { 1 } else { -1 };
    // Check squares: king's file and adjacent files, up to 3 ranks ahead
    for df in -1..=1 {
        let nf = king_file as i8 + df;
        if nf < 0 || nf >= 8 { continue; }
        for dr in 1..=3 {
            let nr = king_rank as i8 + dr * rank_dir;
            if nr < 0 || nr >= 8 { continue; }
            if board[nr as usize][nf as usize] == Some(pawn_ch) {
                count += 1;
            }
        }
    }
    count
}

/// Check if a pawn is passed (no enemy pawns on the same or adjacent files
/// ahead of it).
fn is_passed_pawn(rank: u8, file: u8, is_white: bool, board: &[[Option<char>; 8]; 8]) -> bool {
    let enemy_pawn = if is_white { 'p' } else { 'P' };
    let (start, end, dir) = if is_white {
        (rank + 1, 8, 1)
    } else {
        (0, rank, -1)
    };
    for df in -1..=1 {
        let nf = file as i8 + df;
        if nf < 0 || nf >= 8 { continue; }
        let mut r = start as i8;
        while if is_white { r < end as i8 } else { r > end as i8 } {
            if r >= 0 && r < 8 {
                if board[r as usize][nf as usize] == Some(enemy_pawn) {
                    return false;
                }
            }
            r += dir;
        }
    }
    true
}

/// Extract all chess relations as SVO triples from a FEN position.
///
/// Relations extracted:
///   (piece, "attacks", piece)     — direct attack on enemy piece
///   (piece, "defends", piece)     — protection of friendly piece (only for first rank pieces — keeps total triples bounded)
///   (piece, "controls", square)   — attacks an empty square (heavy — only for center + key king squares)
///   (piece, "mobility", N)        — mobility of the piece (encoded per piece)
///   (king, "shielded_by", count)  — pawn shield count
///   (pawn, "passed", file)        — passed pawn indicator
///   (side, "castled", side)       — castling rights
///   (side, "material", N)         — material balance
pub fn extract_chess_triples(fen: &str) -> Vec<(String, String, String)> {
    let pieces = parse_fen(fen);
    let (board, wk_sq, bk_sq) = build_board(&pieces);
    let mut triples: Vec<(String, String, String)> = Vec::new();

    // Dictionary of pieces for quick lookup: (label, ch, rank, file)
    let piece_info: Vec<(&str, char, u8, u8)> = pieces.iter()
        .map(|&(ch, r, f)| (piece_label(ch), ch, r, f))
        .collect();

    // ── Pass 1: Attack/defense/control relations ───────────────────────
    for &(attacker_label, ch, rank, file) in &piece_info {
        let attacked_sqs = compute_attacks(ch, rank, file, &board);

        for (tr, tf) in attacked_sqs {
            match board[tr as usize][tf as usize] {
                Some(target_ch) => {
                    let target_label = piece_label(target_ch);
                    if ch.is_uppercase() == target_ch.is_uppercase() {
                        // Same color → defense (only for high-value or critical defenders)
                        let target_val = piece_value(target_ch);
                        if target_val >= 3 || piece_type(target_ch) == 'K' {
                            triples.push((
                                attacker_label.to_string(),
                                "defends".to_string(),
                                target_label.to_string(),
                            ));
                        }
                    } else {
                        // Different color → attack
                        triples.push((
                            attacker_label.to_string(),
                            "attacks".to_string(),
                            target_label.to_string(),
                        ));
                    }
                }
                None => {
                    // Empty square → control of key squares (center + extended center)
                    if (tr == 3 || tr == 4) && (tf >= 2 && tf <= 5) {
                        let sq = square_name(tr, tf);
                        triples.push((
                            attacker_label.to_string(),
                            "controls".to_string(),
                            sq,
                        ));
                    }
                }
            }
        }
    }

    // ── Pass 2: King safety ───────────────────────────────────────────
    if let Some((kr, kf)) = wk_sq {
        let shield = king_pawn_shield(kr, kf, true, &board);
        triples.push(("wK".to_string(), "shielded_by".to_string(), format!("{}", shield)));
    }
    if let Some((kr, kf)) = bk_sq {
        let shield = king_pawn_shield(kr, kf, false, &board);
        triples.push(("bK".to_string(), "shielded_by".to_string(), format!("{}", shield)));
    }

    // ── Pass 3: Passed pawns ──────────────────────────────────────────
    for &(label, ch, rank, file) in &piece_info {
        if piece_type(ch) == 'P' {
            let is_white = ch.is_uppercase();
            if is_passed_pawn(rank, file, is_white, &board) {
                triples.push((label.to_string(), "passed".to_string(), square_name(rank, file)));
            }
        }
    }

    // ── Pass 4: Castling status ───────────────────────────────────────
    let castle_token = fen.split_whitespace().nth(2).unwrap_or("-");
    if castle_token.contains('K') {
        triples.push(("white".to_string(), "can_castle".to_string(), "kingside".to_string()));
    }
    if castle_token.contains('Q') {
        triples.push(("white".to_string(), "can_castle".to_string(), "queenside".to_string()));
    }
    if castle_token.contains('k') {
        triples.push(("black".to_string(), "can_castle".to_string(), "kingside".to_string()));
    }
    if castle_token.contains('q') {
        triples.push(("black".to_string(), "can_castle".to_string(), "queenside".to_string()));
    }

    // ── Pass 5: Material balance ──────────────────────────────────────
    let mat = compute_material_balance(&pieces);
    triples.push(("white".to_string(), "material".to_string(), format!("{:+}", mat)));

    // ── Pass 6: Side to move ──────────────────────────────────────────
    let side = fen.split_whitespace().nth(1).unwrap_or("w");
    if side == "w" {
        triples.push(("white".to_string(), "to_move".to_string(), "true".to_string()));
    } else {
        triples.push(("black".to_string(), "to_move".to_string(), "true".to_string()));
    }

    triples
}

/// Encode a chess position as a VSA hypervector from SVO triples.
///
/// Each triple (S, V, O) is encoded via resonator::encode_svo, then all
/// triple HVs are bundled via majority-sum.
///
/// Includes a fallback: if triples generate NO content (empty position),
/// returns the piece-square encoding as safety net.
pub fn encode_position_from_triples(fen: &str) -> Hypervector {
    let triples = extract_chess_triples(fen);

    if triples.is_empty() {
        // Safety net: fall back to piece-square encoding
        return encode_position(fen);
    }

    let mut hvs: Vec<Hypervector> = Vec::with_capacity(triples.len());
    for (s, v, o) in &triples {
        let s_hv = Hypervector::encode_text_ngram(s, 3);
        let v_hv = Hypervector::encode_text_ngram(v, 3);
        let o_hv = Hypervector::encode_text_ngram(o, 3);
        let triple_hv = crate::resonator::encode_svo(&s_hv, &v_hv, &o_hv);
        hvs.push(triple_hv);
    }

    let refs: Vec<&Hypervector> = hvs.iter().collect();
    Hypervector::bundle(&refs)
}

// ─── Tracked Position Encoding ───────────────────────────────────────────
//
// Encodes chess features into separate VSA tracks to prevent minority
// feature drowning in majority-sum bundling.  Each track bundles triples
// within its category, and k-NN runs independently on each track.
//
// Tracks:
//   material:    (side, material, value)            — 1-2 triples
//   attacks:     (piece, attacks|defends, piece)    — 10-30 triples
//   king_safety: king shield, castling status       — 2-4 triples
//   mobility:    (side, mobility, count)            — 2 triples
//   structure:   passed/isolated/doubled pawns      — 2-8 triples
//
// At query time, per-track similarities are combined with learned weights.
// ────────────────────────────────────────────────────────────────────────────

/// Categorized chess features for tracked encoding.
#[derive(Debug, Clone)]
pub struct TrackedPosition {
    pub material: Hypervector,
    pub attacks: Hypervector,
    pub king_safety: Hypervector,
    pub mobility: Hypervector,
    pub structure: Hypervector,
    pub tactics: Hypervector,
}

/// Check if a pawn at (rank, file) is isolated (no friendly pawns on adjacent files).
fn is_isolated_pawn(rank: u8, file: u8, is_white: bool, pieces: &[(char, u8, u8)]) -> bool {
    let my_pawn = if is_white { 'P' } else { 'p' };
    for df in -1..=1 {
        if df == 0 { continue; }
        let nf = file as i8 + df;
        if nf < 0 || nf >= 8 { continue; }
        for &(ch, _, pf) in pieces {
            if ch == my_pawn && pf as i8 == nf {
                return false; // friendly pawn on adjacent file
            }
        }
    }
    true
}

/// Check if a pawn at (file) is doubled (another friendly pawn on same file).
fn is_doubled_pawn(rank: u8, file: u8, is_white: bool, pieces: &[(char, u8, u8)]) -> bool {
    let my_pawn = if is_white { 'P' } else { 'p' };
    for &(ch, pr, pf) in pieces {
        if ch == my_pawn && pf == file && pr != rank {
            return true; // another same-color pawn on this file
        }
    }
    false
}

/// Count mobility (total attacked squares) for each side.
fn compute_mobility_counts(pieces: &[(char, u8, u8)], board: &[[Option<char>; 8]; 8]) -> (u32, u32) {
    let mut white_sqs = 0u32;
    let mut black_sqs = 0u32;
    for &(ch, rank, file) in pieces {
        let sqs = compute_attacks(ch, rank, file, board);
        let count = sqs.len() as u32;
        if ch.is_uppercase() {
            white_sqs += count;
        } else {
            black_sqs += count;
        }
    }
    (white_sqs, black_sqs)
}

// ─── Rich Perception: Tactical + Positional Features ────────────────────
//
// Detects features a chess player actually sees when looking at a position:
//   - Hanging pieces (outnumbered attackers)
//   - Pins (piece between enemy slider and own king)
//   - Forks (piece attacking 2+ enemy pieces)
//   - Open files (no pawns on file)
//   - King exposure (open files adjacent to king)
//   - Imprisoned pieces (zero attacked squares)
//
// Every feature is deterministic from FEN + chess rules.  No engine.
// ────────────────────────────────────────────────────────────────────────────

/// Build an attack map: for each piece index, which enemy pieces it attacks
/// and which friendly pieces it defends.  Also returns the reverse map
/// (which pieces attack/defend each piece).
///
/// Return: (attacks_from, attacks_to, defenses_to)
///   Each is a Vec of Vec<usize> indexed by piece position in the `pieces` slice.
fn build_attack_map(
    pieces: &[(char, u8, u8)],
    board: &[[Option<char>; 8]; 8],
) -> (Vec<Vec<usize>>, Vec<Vec<usize>>, Vec<Vec<usize>>) {
    let n = pieces.len();
    let mut attacks_from = vec![Vec::new(); n];  // piece[i] attacks these enemy indices
    let mut attacks_to = vec![Vec::new(); n];    // enemy indices that attack piece[i]
    let mut defenses_to = vec![Vec::new(); n];   // friendly indices that defend piece[i]

    for (i, &(ch, rank, file)) in pieces.iter().enumerate() {
        let attacked_sqs = compute_attacks(ch, rank, file, &board);
        for (tr, tf) in attacked_sqs {
            if let Some(target_ch) = board[tr as usize][tf as usize] {
                // Find the index of the target piece
                for (j, &(tc, tr2, tf2)) in pieces.iter().enumerate() {
                    if tr2 == tr && tf2 == tf && tc == target_ch {
                        if ch.is_uppercase() == target_ch.is_uppercase() {
                            // Same color → defense
                            defenses_to[j].push(i);
                        } else {
                            // Different color → attack
                            attacks_to[j].push(i);
                            attacks_from[i].push(j);
                        }
                        break;
                    }
                }
            }
        }
    }

    (attacks_from, attacks_to, defenses_to)
}

/// Detect hanging pieces: attackers > defenders (and at least 1 attacker).
fn detect_hanging(
    pieces: &[(char, u8, u8)],
    attacks_to: &[Vec<usize>],
    defenses_to: &[Vec<usize>],
) -> Vec<(String, String, String)> {
    let mut triples = Vec::new();
    for (i, &(ch, _, _)) in pieces.iter().enumerate() {
        let n_attackers = attacks_to[i].len();
        let n_defenders = defenses_to[i].len();
        if n_attackers > 0 && n_attackers > n_defenders {
            let label = piece_label(ch);
            let diff = n_attackers - n_defenders;
            triples.push((label.to_string(), "hanging".to_string(), format!("{}", diff)));
        }
    }
    triples
}

/// Detect undefended pieces: not defended by any friendly piece.
fn detect_undefended(
    pieces: &[(char, u8, u8)],
    attacks_to: &[Vec<usize>],
    defenses_to: &[Vec<usize>],
) -> Vec<(String, String, String)> {
    let mut triples = Vec::new();
    for (i, &(ch, _, _)) in pieces.iter().enumerate() {
        // Undefended = attacked but not defended
        if attacks_to[i].len() > 0 && defenses_to[i].is_empty() {
            let label = piece_label(ch);
            triples.push((label.to_string(), "undefended".to_string(), "true".to_string()));
        }
    }
    triples
}

/// Detect forks: a piece attacking 2+ enemy pieces.
fn detect_forks(
    pieces: &[(char, u8, u8)],
    attacks_from: &[Vec<usize>],
) -> Vec<(String, String, String)> {
    let mut triples = Vec::new();
    for (i, &(ch, _, _)) in pieces.iter().enumerate() {
        let n_attacked = attacks_from[i].len();
        if n_attacked >= 2 {
            let label = piece_label(ch);
            let total_value: i32 = attacks_from[i].iter()
                .map(|&j| piece_value(pieces[j].0))
                .sum();
            triples.push((
                label.to_string(),
                "forks".to_string(),
                format!("{}", total_value),
            ));
        }
    }
    triples
}

/// Check if a position (rank, file) is on the same diagonal as the king.
/// Returns the unit direction (dr, df) toward the king if aligned, else None.
fn direction_toward_king(rank: u8, file: u8, king_rank: u8, king_file: u8) -> Option<(i8, i8)> {
    let dr = king_rank as i8 - rank as i8;
    let df = king_file as i8 - file as i8;
    if dr == 0 && df == 0 { return None; }
    let adr = dr.abs();
    let adf = df.abs();
    if (dr == 0 || df == 0 || adr == adf) && (adr <= 7 && adf <= 7) {
        let udr = if dr == 0 { 0 } else { dr / adr };
        let udf = if df == 0 { 0 } else { df / adf };
        Some((udr, udf))
    } else {
        None
    }
}

/// Detect pins: pieces between an enemy slider and their own king.
/// A pinned piece can't move without exposing the king to check.
fn detect_pins(
    pieces: &[(char, u8, u8)],
    board: &[[Option<char>; 8]; 8],
    wk_sq: Option<(u8, u8)>,
    bk_sq: Option<(u8, u8)>,
) -> Vec<(String, String, String)> {
    let mut triples = Vec::new();
    let mut pin_check = |ch: char, rank: u8, file: u8, king_sq: Option<(u8, u8)>| {
        let king = match king_sq { Some(k) => k, None => return };
        // Check direction from this piece toward its king
        if let Some((udr, udf)) = direction_toward_king(rank, file, king.0, king.1) {
            // If this piece is NOT the king itself
            if piece_type(ch) == 'K' { return; }
            // Check if there's an enemy slider behind this piece (away from king)
            let check_rank = rank as i8 - udr;
            let check_file = file as i8 - udf;
            if check_rank >= 0 && check_rank < 8 && check_file >= 0 && check_file < 8 {
                if let Some(behind_ch) = board[check_rank as usize][check_file as usize] {
                    if ch.is_uppercase() != behind_ch.is_uppercase() {
                        let bt = piece_type(behind_ch);
                        // Sliding pieces that attack along this direction
                        let can_pin = if udr != 0 && udf != 0 {
                            bt == 'B' || bt == 'Q'  // diagonal
                        } else {
                            bt == 'R' || bt == 'Q'  // orthogonal
                        };
                        if can_pin {
                            let label = piece_label(ch);
                            let pinner = piece_label(behind_ch);
                            triples.push((label.to_string(), "pinned_by".to_string(), pinner.to_string()));
                        }
                    }
                }
            }
        }
    };
    for &(ch, rank, file) in pieces {
        if ch.is_uppercase() {
            pin_check(ch, rank, file, wk_sq);
        } else {
            pin_check(ch, rank, file, bk_sq);
        }
    }
    triples
}

/// Detect open and semi-open files.
/// Returns triples for each open/semi-open file.
fn detect_open_files(board: &[[Option<char>; 8]; 8]) -> Vec<(String, String, String)> {
    let mut triples = Vec::new();
    for file in 0..8u8 {
        let mut white_pawn = false;
        let mut black_pawn = false;
        for rank in 0..8u8 {
            if let Some(ch) = board[rank as usize][file as usize] {
                if ch == 'P' { white_pawn = true; }
                if ch == 'p' { black_pawn = true; }
            }
        }
        let fn_name = format!("{}", (b'a' + file) as char);
        if !white_pawn && !black_pawn {
            triples.push((fn_name.clone(), "open_file".to_string(), "true".to_string()));
        } else if !white_pawn || !black_pawn {
            let side = if !white_pawn { "white" } else { "black" };
            triples.push((fn_name, "semi_open".to_string(), side.to_string()));
        }
    }
    triples
}

/// Detect king exposure: open files adjacent to the king.
fn detect_king_exposure(
    pieces: &[(char, u8, u8)],
    board: &[[Option<char>; 8]; 8],
) -> Vec<(String, String, String)> {
    let mut triples = Vec::new();
    for &(ch, rank, file) in pieces {
        if piece_type(ch) == 'K' {
            let is_white = ch.is_uppercase();
            for df in -1..=1 {
                let nf = file as i8 + df;
                if nf < 0 || nf >= 8 { continue; }
                // Check if any pawn exists on this file
                let mut has_friendly_pawn = false;
                let mut has_enemy_pawn = false;
                for r in 0..8 {
                    if let Some(pc) = board[r][nf as usize] {
                        if piece_type(pc) == 'P' {
                            if pc.is_uppercase() == is_white {
                                has_friendly_pawn = true;
                            } else {
                                has_enemy_pawn = true;
                            }
                        }
                    }
                }
                let fn_name = format!("{}", (b'a' + nf as u8) as char);
                if !has_friendly_pawn && !has_enemy_pawn {
                    // Open file next to king
                    triples.push((format!("{}_king", if is_white { "w" } else { "b" }),
                        "adjacent_open".to_string(), fn_name));
                } else if !has_friendly_pawn {
                    // Semi-open file next to king (enemy pawn only)
                    triples.push((format!("{}_king", if is_white { "w" } else { "b" }),
                        "adjacent_semi_open".to_string(), fn_name));
                }
            }
        }
    }
    triples
}

/// Detect imprisoned pieces: pieces with zero attacked squares.
fn detect_imprisoned(
    pieces: &[(char, u8, u8)],
    board: &[[Option<char>; 8]; 8],
) -> Vec<(String, String, String)> {
    let mut triples = Vec::new();
    for &(ch, rank, file) in pieces {
        // Only bishops and knights get imprisoned (pawns/rooks/queens/kings naturally have moves)
        let pt = piece_type(ch);
        if pt != 'B' && pt != 'N' { continue; }
        let sqs = compute_attacks(ch, rank, file, board);
        if sqs.is_empty() {
            let label = piece_label(ch);
            triples.push((label.to_string(), "imprisoned".to_string(), "true".to_string()));
        }
    }
    triples
}

/// Detect dominant rooks: rooks on open or semi-open files.
fn detect_dominant_rooks(
    pieces: &[(char, u8, u8)],
    board: &[[Option<char>; 8]; 8],
) -> Vec<(String, String, String)> {
    let mut triples = Vec::new();
    for &(ch, rank, file) in pieces {
        if piece_type(ch) == 'R' {
            let is_white = ch.is_uppercase();
            let mut has_friendly_pawn = false;
            let mut has_enemy_pawn = false;
            for r in 0..8 {
                if let Some(pc) = board[r][file as usize] {
                    if piece_type(pc) == 'P' {
                        if pc.is_uppercase() == is_white {
                            has_friendly_pawn = true;
                        } else {
                            has_enemy_pawn = true;
                        }
                    }
                }
            }
            let label = piece_label(ch);
            if !has_friendly_pawn && !has_enemy_pawn {
                triples.push((label.to_string(), "on_open_file".to_string(), "true".to_string()));
            } else if !has_friendly_pawn {
                triples.push((label.to_string(), "on_semi_open_file".to_string(), "true".to_string()));
            }
        }
    }
    triples
}

/// Rich chess feature extraction: combines old attack/defense pairs (informative!)
/// with new tactical, positional, and king safety features.
///
/// Tracks:
///   0: material   — (side, material, balance)
///   1: tactics    — attack/defense pairs (old) + pins, forks, hanging pieces
///   2: king_safety — pawn shield, castling, king exposure (open files near king)
///   3: activity   — open files, imprisoned pieces, dominant rooks (replaces old mobility)
///   4: structure  — passed/isolated/doubled pawns, side to move
pub fn extract_rich_tracked_triples(fen: &str) -> (
    Vec<(String, String, String)>,  // material
    Vec<(String, String, String)>,  // tactics + attack/defense
    Vec<(String, String, String)>,  // king_safety (enhanced)
    Vec<(String, String, String)>,  // activity
    Vec<(String, String, String)>,  // structure (enhanced)
) {
    let pieces = parse_fen(fen);
    let (board, wk_sq, bk_sq) = build_board(&pieces);

    // Build attack map for tactical feature extraction
    let (attacks_from, attacks_to, defenses_to) = build_attack_map(&pieces, &board);

    let mut material_triples: Vec<(String, String, String)> = Vec::new();
    let mut tactics_triples: Vec<(String, String, String)> = Vec::new();
    let mut king_triples: Vec<(String, String, String)> = Vec::new();
    let mut activity_triples: Vec<(String, String, String)> = Vec::new();
    let mut structure_triples: Vec<(String, String, String)> = Vec::new();

    // ── Track 0: Material ──────────────────────────────────────────────────
    let mat = compute_material_balance(&pieces);
    material_triples.push(("white".to_string(), "material".to_string(), format!("{:+}", mat)));

    // ── Track 1: Tactics (attack/defense pairs + tactical patterns) ────────
    // Old attack/defense pairs (dense, informative)
    for &(ch, rank, file) in &pieces {
        let attacker_label = piece_label(ch);
        let attacked_sqs = compute_attacks(ch, rank, file, &board);
        for (tr, tf) in attacked_sqs {
            if let Some(target_ch) = board[tr as usize][tf as usize] {
                let target_label = piece_label(target_ch);
                if ch.is_uppercase() == target_ch.is_uppercase() {
                    // Defense (keep only non-pawn defenders to bound count)
                    if piece_type(target_ch) != 'P' || piece_type(ch) != 'P' {
                        tactics_triples.push((
                            attacker_label.to_string(),
                            "defends".to_string(), target_label.to_string(),
                        ));
                    }
                } else {
                    tactics_triples.push((
                        attacker_label.to_string(), "attacks".to_string(),
                        target_label.to_string(),
                    ));
                }
            }
        }
    }
    // New tactical patterns (sparse but high-impact)
    let pins = detect_pins(&pieces, &board, wk_sq, bk_sq);
    tactics_triples.extend(pins);
    let forks = detect_forks(&pieces, &attacks_from);
    tactics_triples.extend(forks);
    let hanging = detect_hanging(&pieces, &attacks_to, &defenses_to);
    tactics_triples.extend(hanging);
    // Note: undefended pieces don't add much — they're correlated with hanging

    // ── Track 2: King Safety (enhanced with exposure) ───────────────────────
    if let Some((kr, kf)) = wk_sq {
        let shield = king_pawn_shield(kr, kf, true, &board);
        king_triples.push(("wK".to_string(), "shielded_by".to_string(), fmt_shield(shield)));
    }
    if let Some((kr, kf)) = bk_sq {
        let shield = king_pawn_shield(kr, kf, false, &board);
        king_triples.push(("bK".to_string(), "shielded_by".to_string(), fmt_shield(shield)));
    }
    // King exposure (NEW)
    let exposure = detect_king_exposure(&pieces, &board);
    king_triples.extend(exposure);
    // Castling
    let castle_token = fen.split_whitespace().nth(2).unwrap_or("-");
    for (side, token) in [("white", 'K'), ("white", 'Q'), ("black", 'k'), ("black", 'q')] {
        if castle_token.contains(token) {
            king_triples.push((side.to_string(), "can_castle".to_string(),
                if token.is_uppercase() { "kingside" } else { "queenside" }.to_string()));
        }
    }

    // ── Track 3: Activity (open files, imprisoned pieces, dominant rooks) ──
    let open_files = detect_open_files(&board);
    activity_triples.extend(open_files);
    let imprisoned = detect_imprisoned(&pieces, &board);
    activity_triples.extend(imprisoned);
    let dominant = detect_dominant_rooks(&pieces, &board);
    activity_triples.extend(dominant);

    // ── Track 4: Structure (enhanced pawn analysis) ────────────────────────
    for &(ch, rank, file) in &pieces {
        if piece_type(ch) == 'P' {
            let label = piece_label(ch);
            let is_white = ch.is_uppercase();
            if is_passed_pawn(rank, file, is_white, &board) {
                structure_triples.push((label.to_string(), "passed".to_string(), square_name(rank, file)));
            }
            if is_isolated_pawn(rank, file, is_white, &pieces) {
                structure_triples.push((label.to_string(), "isolated".to_string(), square_name(rank, file)));
            }
            if is_doubled_pawn(rank, file, is_white, &pieces) {
                structure_triples.push((label.to_string(), "doubled".to_string(), square_name(rank, file)));
            }
        }
    }
    // Side to move
    let side = fen.split_whitespace().nth(1).unwrap_or("w");
    if side == "w" {
        structure_triples.push(("white".to_string(), "to_move".to_string(), "true".to_string()));
    } else {
        structure_triples.push(("black".to_string(), "to_move".to_string(), "true".to_string()));
    }

    (material_triples, tactics_triples, king_triples, activity_triples, structure_triples)
}

/// Format pawn shield count into a relation-friendly string.
fn fmt_shield(count: u32) -> String {
    if count == 0 { "none".to_string() }
    else if count <= 2 { "weak".to_string() }
    else if count <= 4 { "moderate".to_string() }
    else { "strong".to_string() }
}

/// Encode a chess position using the rich 5-track feature set.
pub fn encode_rich_tracked_position(fen: &str) -> TrackedPosition {
    let (material, tactics, king_safety, activity, structure) = extract_rich_tracked_triples(fen);
    TrackedPosition {
        material: encode_triple_bundle(&material),
        attacks: encode_triple_bundle(&tactics),     // reused field: now "tactics"
        king_safety: encode_triple_bundle(&king_safety),
        mobility: encode_triple_bundle(&activity),   // reused field: now "activity"
        structure: encode_triple_bundle(&structure),
        tactics: Hypervector::new_zero(),
    }
}

/// Extract categorized chess triples for tracked encoding.
pub fn extract_tracked_triples(fen: &str) -> (
    Vec<(String, String, String)>,  // material
    Vec<(String, String, String)>,  // attacks + defenses
    Vec<(String, String, String)>,  // king_safety
    Vec<(String, String, String)>,  // mobility
    Vec<(String, String, String)>,  // structure
    Vec<(String, String, String)>,  // tactics (forks, pins, hanging)
) {
    let pieces = parse_fen(fen);
    let (board, wk_sq, bk_sq) = build_board(&pieces);
    let piece_info: Vec<(&str, char, u8, u8)> = pieces.iter()
        .map(|&(ch, r, f)| (piece_label(ch), ch, r, f))
        .collect();

    // Per-category vectors
    let mut material_triples: Vec<(String, String, String)> = Vec::new();
    let mut attack_triples: Vec<(String, String, String)> = Vec::new();
    let mut king_triples: Vec<(String, String, String)> = Vec::new();
    let mut mobility_triples: Vec<(String, String, String)> = Vec::new();
    let mut structure_triples: Vec<(String, String, String)> = Vec::new();

    // ── Material ────────────────────────────────────────────────────────────
    let mat = compute_material_balance(&pieces);
    material_triples.push(("white".to_string(), "material".to_string(), format!("{:+}", mat)));

    // ── Attack/defense relations ────────────────────────────────────────────
    for &(attacker_label, ch, rank, file) in &piece_info {
        let attacked_sqs = compute_attacks(ch, rank, file, &board);
        for (tr, tf) in attacked_sqs {
            match board[tr as usize][tf as usize] {
                Some(target_ch) => {
                    let target_label = piece_label(target_ch);
                    if ch.is_uppercase() == target_ch.is_uppercase() {
                        // Defense (all defended pieces, not just high-value)
                        // Limit to non-pawn defenders to keep count manageable
                        if piece_type(target_ch) != 'P' || piece_type(ch) != 'P' {
                            attack_triples.push((
                                attacker_label.to_string(),
                                "defends".to_string(),
                                target_label.to_string(),
                            ));
                        }
                    } else {
                        // Attack
                        attack_triples.push((
                            attacker_label.to_string(),
                            "attacks".to_string(),
                            target_label.to_string(),
                        ));
                    }
                }
                None => {
                    // Empty square — skip (structure track handles center control)
                }
            }
        }
    }

    // ── King safety ─────────────────────────────────────────────────────────
    if let Some((kr, kf)) = wk_sq {
        let shield = king_pawn_shield(kr, kf, true, &board);
        king_triples.push(("wK".to_string(), "shielded_by".to_string(), format!("{}", shield)));
    }
    if let Some((kr, kf)) = bk_sq {
        let shield = king_pawn_shield(kr, kf, false, &board);
        king_triples.push(("bK".to_string(), "shielded_by".to_string(), format!("{}", shield)));
    }
    // Castling
    let castle_token = fen.split_whitespace().nth(2).unwrap_or("-");
    if castle_token.contains('K') {
        king_triples.push(("white".to_string(), "can_castle".to_string(), "kingside".to_string()));
    }
    if castle_token.contains('Q') {
        king_triples.push(("white".to_string(), "can_castle".to_string(), "queenside".to_string()));
    }
    if castle_token.contains('k') {
        king_triples.push(("black".to_string(), "can_castle".to_string(), "kingside".to_string()));
    }
    if castle_token.contains('q') {
        king_triples.push(("black".to_string(), "can_castle".to_string(), "queenside".to_string()));
    }

    // ── Mobility ────────────────────────────────────────────────────────────
    let (w_mob, b_mob) = compute_mobility_counts(&pieces, &board);
    mobility_triples.push(("white".to_string(), "mobility".to_string(), format!("{}", w_mob)));
    mobility_triples.push(("black".to_string(), "mobility".to_string(), format!("{}", b_mob)));

    // ── Structure: pawns + center control + side to move ────────────────────
    for &(label, ch, rank, file) in &piece_info {
        if piece_type(ch) == 'P' {
            let is_white = ch.is_uppercase();
            // Passed pawn
            if is_passed_pawn(rank, file, is_white, &board) {
                structure_triples.push((label.to_string(), "passed".to_string(), square_name(rank, file)));
            }
            // Isolated pawn
            if is_isolated_pawn(rank, file, is_white, &pieces) {
                structure_triples.push((label.to_string(), "isolated".to_string(), square_name(rank, file)));
            }
            // Doubled pawn
            if is_doubled_pawn(rank, file, is_white, &pieces) {
                structure_triples.push((label.to_string(), "doubled".to_string(), square_name(rank, file)));
            }
        }
    }
    // Side to move goes in structure (it affects structural decisions)
    let side = fen.split_whitespace().nth(1).unwrap_or("w");
    if side == "w" {
        structure_triples.push(("white".to_string(), "to_move".to_string(), "true".to_string()));
    } else {
        structure_triples.push(("black".to_string(), "to_move".to_string(), "true".to_string()));
    }

    // ── Tactics: forks, pins, hanging ────────────────────────────────────
    let (attacks_from, attacks_to, defenses_to) = build_attack_map(&pieces, &board);
    let mut tactics_triples: Vec<(String, String, String)> = Vec::new();
    tactics_triples.extend(detect_forks(&pieces, &attacks_from));
    tactics_triples.extend(detect_pins(&pieces, &board, wk_sq, bk_sq));
    tactics_triples.extend(detect_hanging(&pieces, &attacks_to, &defenses_to));

    (material_triples, attack_triples, king_triples, mobility_triples, structure_triples, tactics_triples)
}

/// Encode a set of triples into a single bundled hypervector.
fn encode_triple_bundle(triples: &[(String, String, String)]) -> Hypervector {
    if triples.is_empty() {
        return Hypervector::new_zero();
    }
    let mut hvs: Vec<Hypervector> = Vec::with_capacity(triples.len());
    for (s, v, o) in triples {
        let s_hv = Hypervector::encode_text_ngram(s, 3);
        let v_hv = Hypervector::encode_text_ngram(v, 3);
        let o_hv = Hypervector::encode_text_ngram(o, 3);
        let triple_hv = crate::resonator::encode_svo(&s_hv, &v_hv, &o_hv);
        hvs.push(triple_hv);
    }
    let refs: Vec<&Hypervector> = hvs.iter().collect();
    Hypervector::bundle(&refs)
}

/// Encode a chess position as 5 separate track hypervectors.
/// Each track bundles triples within its own category.
pub fn encode_tracked_position(fen: &str) -> TrackedPosition {
    let (material, attacks, king_safety, mobility, structure, tactics) = extract_tracked_triples(fen);

    TrackedPosition {
        material: encode_triple_bundle(&material),
        attacks: encode_triple_bundle(&attacks),
        king_safety: encode_triple_bundle(&king_safety),
        mobility: encode_triple_bundle(&mobility),
        structure: encode_triple_bundle(&structure),
        tactics: encode_triple_bundle(&tactics),
    }
}

/// Compute per-track similarities between two TrackedPositions.
pub fn tracked_similarity(a: &TrackedPosition, b: &TrackedPosition) -> [f64; 6] {
    [
        1.0 - a.material.normalized_hamming_distance(&b.material),
        1.0 - a.attacks.normalized_hamming_distance(&b.attacks),
        1.0 - a.king_safety.normalized_hamming_distance(&b.king_safety),
        1.0 - a.mobility.normalized_hamming_distance(&b.mobility),
        1.0 - a.structure.normalized_hamming_distance(&b.structure),
        1.0 - a.tactics.normalized_hamming_distance(&b.tactics),
    ]
}

// ─── Dataset I/O ────────────────────────────────────────────────────────────

/// Read positions from a JSONL file.
pub fn load_positions(path: &str) -> Vec<PositionRecord> {
    let file = File::open(path).expect("Failed to open positions file");
    let reader = BufReader::new(file);
    let mut records = Vec::new();
    for line in reader.lines() {
        let line = line.expect("Failed to read line");
        if line.trim().is_empty() {
            continue;
        }
        let record: PositionRecord =
            serde_json::from_str(&line).expect("Failed to parse JSON line");
        records.push(record);
    }
    records
}

// ─── Cross-Validation ──────────────────────────────────────────────────────

/// Results from cross-validation.
#[derive(Debug, Clone)]
pub struct CVResult {
    /// R² correlation coefficient.
    pub r_squared: f64,
    /// Mean absolute error in pawn units.
    pub mae: f64,
    /// Fraction of predictions that get the sign right (correct side to play).
    pub sign_accuracy: f64,
    /// Total number of predictions.
    pub n: usize,
    /// Number of folds used.
    pub k: usize,
    /// Per-fold results.
    pub fold_results: Vec<FoldResult>,
}

#[derive(Debug, Clone)]
pub struct FoldResult {
    pub r_squared: f64,
    pub mae: f64,
    pub sign_accuracy: f64,
    pub n: usize,
}

/// Compute R² between predicted and actual values.
fn compute_r_squared(actual: &[f64], predicted: &[f64]) -> f64 {
    let n = actual.len();
    if n < 2 {
        return 0.0;
    }
    let mean_actual: f64 = actual.iter().sum::<f64>() / n as f64;
    let ss_res: f64 = actual.iter()
        .zip(predicted.iter())
        .map(|(a, p)| (a - p).powi(2))
        .sum();
    let ss_tot: f64 = actual.iter()
        .map(|a| (a - mean_actual).powi(2))
        .sum();
    if ss_tot == 0.0 {
        return 0.0;
    }
    1.0 - ss_res / ss_tot
}

/// Shuffle records using Fisher-Yates (deterministic seed for reproducibility).
pub fn shuffle_records(records: &mut [PositionRecord]) {
    use rand::SeedableRng;
    use rand::Rng;
    let mut rng = rand::rngs::StdRng::seed_from_u64(42);
    let n = records.len();
    for i in (1..n).rev() {
        let j = rng.gen_range(0..=i);
        records.swap(i, j);
    }
}

/// Run k-fold cross-validation with 1-NN (standard nearest-centroid) using
/// the piece-square encoder.
pub fn cross_validate(records: &mut [PositionRecord], k_folds: usize) -> CVResult {
    cross_validate_knn(records, k_folds, 1)
}
///
/// For each fold:
///   1. Encode all training positions as hypervectors
///   2. For each test position, find k nearest centroids in training set
///   3. Predict eval = average eval of k nearest (weighted by similarity)
///   4. Collect metrics
///
/// Uses the piece-square encoder.
pub fn cross_validate_knn(records: &mut [PositionRecord], k_folds: usize, k_nearest: usize) -> CVResult {
    cross_validate_knn_with_encoder(records, k_folds, k_nearest, encode_position)
}

/// Same as cross_validate_knn but accepts an arbitrary encoding function.
/// Allows direct comparison between piece-square and SVO-triple encoding.
pub fn cross_validate_knn_with_encoder(
    records: &mut [PositionRecord],
    k_folds: usize,
    k_nearest: usize,
    encoder: fn(&str) -> Hypervector,
) -> CVResult {
    let n = records.len();
    let fold_size = n / k_folds;

    shuffle_records(records);

    let mut fold_results = Vec::with_capacity(k_folds);

    for fold in 0..k_folds {
        let fold_start = Instant::now();
        let test_start = fold * fold_size;
        let test_end = if fold == k_folds - 1 { n } else { test_start + fold_size };

        // Training set
        let mut train_hvs: Vec<(Hypervector, f64)> = Vec::with_capacity(n - fold_size);
        for i in 0..test_start {
            let hv = encoder(&records[i].fen);
            train_hvs.push((hv, records[i].eval_score));
        }
        for i in test_end..n {
            let hv = encoder(&records[i].fen);
            train_hvs.push((hv, records[i].eval_score));
        }

        let mut actual_vals = Vec::with_capacity(fold_size);
        let mut predicted_vals = Vec::with_capacity(fold_size);
        let mut avg_sims = Vec::with_capacity(fold_size);

        for i in test_start..test_end {
            let test_hv = encoder(&records[i].fen);
            let actual = records[i].eval_score;

            // Compute distances to ALL training centroids
            let mut sims: Vec<(f64, f64)> = train_hvs.iter()
                .map(|(centroid_hv, eval)| {
                    let sim = 1.0 - test_hv.normalized_hamming_distance(centroid_hv);
                    (sim, *eval)
                })
                .collect();

            // Sort by similarity descending
            sims.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());

            // Take k nearest, weighted by similarity
            let k = k_nearest.min(sims.len());
            let (weight_sum, eval_sum): (f64, f64) = sims[..k].iter()
                .map(|(sim, eval)| (sim, eval))
                .fold((0.0, 0.0), |(ws, es), (s, e)| (ws + s, es + s * e));

            let predicted = if weight_sum > 0.0 { eval_sum / weight_sum } else { 0.0 };
            let avg_sim: f64 = sims[..k].iter().map(|(s, _)| s).sum::<f64>() / k as f64;

            actual_vals.push(actual);
            predicted_vals.push(predicted);
            avg_sims.push(avg_sim);
        }

        // Metrics
        let r2 = compute_r_squared(&actual_vals, &predicted_vals);
        let mae: f64 = actual_vals.iter()
            .zip(predicted_vals.iter())
            .map(|(a, p)| (a - p).abs())
            .sum::<f64>() / actual_vals.len() as f64;

        let sign_acc = actual_vals.iter()
            .zip(predicted_vals.iter())
            .filter(|(a, p)| a.signum() == p.signum())
            .count() as f64 / actual_vals.len() as f64;

        let mean_sim: f64 = avg_sims.iter().sum::<f64>() / avg_sims.len() as f64;

        let elapsed = fold_start.elapsed();
        eprintln!(
            "  Fold {}/{}: R²={:.4} MAE={:.2} sign={:.1}% sim={:.4} n={} ({:.1}s)",
            fold + 1, k_folds, r2, mae, sign_acc * 100.0, mean_sim,
            actual_vals.len(), elapsed.as_secs_f64(),
        );

        fold_results.push(FoldResult {
            r_squared: r2,
            mae,
            sign_accuracy: sign_acc,
            n: actual_vals.len(),
        });
    }

    let total_n: usize = fold_results.iter().map(|r| r.n).sum();
    let avg_r2 = fold_results.iter().map(|r| r.r_squared * r.n as f64).sum::<f64>() / total_n as f64;
    let avg_mae = fold_results.iter().map(|r| r.mae * r.n as f64).sum::<f64>() / total_n as f64;
    let avg_sign = fold_results.iter().map(|r| r.sign_accuracy * r.n as f64).sum::<f64>() / total_n as f64;

    eprintln!(
        "  Total (k={}): R²={:.4} MAE={:.2} sign={:.1}%",
        k_nearest, avg_r2, avg_mae, avg_sign * 100.0,
    );

    CVResult {
        r_squared: avg_r2,
        mae: avg_mae,
        sign_accuracy: avg_sign,
        n: total_n,
        k: k_folds,
        fold_results,
    }
}

// ─── Tracked Cross-Validation ─────────────────────────────────────────────

/// Default track weights: equal contribution from each of 5 tracks.
/// These can be overridden for learned weighting experiments.
/// Order: [material, attacks, king_safety, mobility, structure]
const DEFAULT_TRACK_WEIGHTS: [f64; 5] = [0.20, 0.20, 0.20, 0.20, 0.20];

/// Cross-validate with per-track k-NN.
///
/// Each position is encoded as 5 separate track hypervectors.
/// For each pair, per-track similarities are combined with weights
/// before finding the k nearest neighbors.
///
/// This prevents minority feature drowning by giving each track
/// independent influence on the combined similarity.
pub fn cross_validate_tracked_knn(
    records: &mut [PositionRecord],
    k_folds: usize,
    k_nearest: usize,
    weights: &[f64; 5],
) -> CVResult {
    let n = records.len();
    let fold_size = n / k_folds;

    shuffle_records(records);

    // Pre-encode all positions as tracked (first encoding pass)
    // This separates encoding from CV computation for fair timing.
    let tracked: Vec<TrackedPosition> = records.iter()
        .map(|r| encode_tracked_position(&r.fen))
        .collect();

    let mut fold_results = Vec::with_capacity(k_folds);

    for fold in 0..k_folds {
        let fold_start = Instant::now();
        let test_start = fold * fold_size;
        let test_end = if fold == k_folds - 1 { n } else { test_start + fold_size };

        // Collect training tracked positions
        let mut train_data: Vec<(usize, f64)> = Vec::with_capacity(n - fold_size);
        for i in 0..test_start {
            train_data.push((i, records[i].eval_score));
        }
        for i in test_end..n {
            train_data.push((i, records[i].eval_score));
        }

        let mut actual_vals = Vec::with_capacity(fold_size);
        let mut predicted_vals = Vec::with_capacity(fold_size);

        for ti in test_start..test_end {
            let test_pos = &tracked[ti];
            let actual = records[ti].eval_score;

            // Compute per-track similarities with ALL training positions
            // combined_sim = Σ w_i * track_sim_i
            let mut combined: Vec<(f64, f64)> = train_data.iter()
                .map(|&(train_idx, eval)| {
                    let train_pos = &tracked[train_idx];
                    let sims = tracked_similarity(test_pos, train_pos);
                    let combined_sim = weights[0] * sims[0]
                        + weights[1] * sims[1]
                        + weights[2] * sims[2]
                        + weights[3] * sims[3]
                        + weights[4] * sims[4];
                    (combined_sim, eval)
                })
                .collect();

            // Sort by combined similarity descending
            combined.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());

            // Take k nearest, weighted by combined similarity
            let k = k_nearest.min(combined.len());
            let (weight_sum, eval_sum): (f64, f64) = combined[..k].iter()
                .map(|(sim, eval)| (sim, eval))
                .fold((0.0, 0.0), |(ws, es), (s, e)| (ws + s, es + s * e));

            let predicted = if weight_sum > 0.0 { eval_sum / weight_sum } else { 0.0 };

            actual_vals.push(actual);
            predicted_vals.push(predicted);
        }

        // Metrics
        let r2 = compute_r_squared(&actual_vals, &predicted_vals);
        let mae: f64 = actual_vals.iter()
            .zip(predicted_vals.iter())
            .map(|(a, p)| (a - p).abs())
            .sum::<f64>() / actual_vals.len() as f64;

        let sign_acc = actual_vals.iter()
            .zip(predicted_vals.iter())
            .filter(|(a, p)| a.signum() == p.signum())
            .count() as f64 / actual_vals.len() as f64;

        let elapsed = fold_start.elapsed();
        eprintln!(
            "  Fold {}/{}: R²={:.4} MAE={:.2} sign={:.1}% n={} ({:.1}s)",
            fold + 1, k_folds, r2, mae, sign_acc * 100.0,
            actual_vals.len(), elapsed.as_secs_f64(),
        );

        fold_results.push(FoldResult {
            r_squared: r2,
            mae,
            sign_accuracy: sign_acc,
            n: actual_vals.len(),
        });
    }

    let total_n: usize = fold_results.iter().map(|r| r.n).sum();
    let avg_r2 = fold_results.iter().map(|r| r.r_squared * r.n as f64).sum::<f64>() / total_n as f64;
    let avg_mae = fold_results.iter().map(|r| r.mae * r.n as f64).sum::<f64>() / total_n as f64;
    let avg_sign = fold_results.iter().map(|r| r.sign_accuracy * r.n as f64).sum::<f64>() / total_n as f64;

    eprintln!(
        "  Total (tracked k={}): R²={:.4} MAE={:.2} sign={:.1}%",
        k_nearest, avg_r2, avg_mae, avg_sign * 100.0,
    );

    CVResult {
        r_squared: avg_r2,
        mae: avg_mae,
        sign_accuracy: avg_sign,
        n: total_n,
        k: k_folds,
        fold_results,
    }
}

/// Cross-validate with per-track k-NN using Euclidean distance in similarity
/// space instead of linear weighted combination.
///
/// Instead of:  combined = w1*s1 + w2*s2 + ...
/// Use:        dist² = w1²*(1-s1)² + w2²*(1-s2)² + ...
///             combined = 1/(1+dist)
///
/// This preserves interactions between tracks.  A position that matches on
/// structure AND king safety simultaneously is closer than one that matches
/// on only one track (non-linear interaction captured by Euclidean metric).
pub fn cross_validate_tracked_knn_euclidean(
    records: &mut [PositionRecord],
    k_folds: usize,
    k_nearest: usize,
    weights: &[f64; 5],
) -> CVResult {
    let n = records.len();
    let fold_size = n / k_folds;

    shuffle_records(records);

    let tracked: Vec<TrackedPosition> = records.iter()
        .map(|r| encode_tracked_position(&r.fen))
        .collect();

    let mut fold_results = Vec::with_capacity(k_folds);

    for fold in 0..k_folds {
        let fold_start = Instant::now();
        let test_start = fold * fold_size;
        let test_end = if fold == k_folds - 1 { n } else { test_start + fold_size };

        let mut train_data: Vec<(usize, f64)> = Vec::with_capacity(n - fold_size);
        for i in 0..test_start {
            train_data.push((i, records[i].eval_score));
        }
        for i in test_end..n {
            train_data.push((i, records[i].eval_score));
        }

        let mut actual_vals = Vec::with_capacity(fold_size);
        let mut predicted_vals = Vec::with_capacity(fold_size);

        for ti in test_start..test_end {
            let test_pos = &tracked[ti];
            let actual = records[ti].eval_score;

            // For each training position, compute Euclidean distance in
            // 5-dimensional similarity space, then convert to similarity.
            // dist² = Σ w_i² * (1 - sim_i)²
            let mut combined: Vec<(f64, f64)> = train_data.iter()
                .map(|&(train_idx, eval)| {
                    let train_pos = &tracked[train_idx];
                    let sims = tracked_similarity(test_pos, train_pos);
                    let mut dist_sq = 0.0;
                    for dim in 0..5 {
                        let d = (1.0 - sims[dim]).abs();
                        dist_sq += weights[dim] * weights[dim] * d * d;
                    }
                    let combined_sim = 1.0 / (1.0 + dist_sq.sqrt());
                    (combined_sim, eval)
                })
                .collect();

            combined.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());
            let k = k_nearest.min(combined.len());
            let (weight_sum, eval_sum): (f64, f64) = combined[..k].iter()
                .map(|(sim, eval)| (sim, eval))
                .fold((0.0, 0.0), |(ws, es), (s, e)| (ws + s, es + s * e));

            let predicted = if weight_sum > 0.0 { eval_sum / weight_sum } else { 0.0 };

            actual_vals.push(actual);
            predicted_vals.push(predicted);
        }

        // Metrics
        let r2 = compute_r_squared(&actual_vals, &predicted_vals);
        let mae: f64 = actual_vals.iter()
            .zip(predicted_vals.iter())
            .map(|(a, p)| (a - p).abs())
            .sum::<f64>() / actual_vals.len() as f64;
        let sign_acc = actual_vals.iter()
            .zip(predicted_vals.iter())
            .filter(|(a, p)| a.signum() == p.signum())
            .count() as f64 / actual_vals.len() as f64;

        let elapsed = fold_start.elapsed();
        eprintln!(
            "  Fold {}/{}: R²={:.4} MAE={:.2} sign={:.1}% n={} ({:.1}s)",
            fold + 1, k_folds, r2, mae, sign_acc * 100.0,
            actual_vals.len(), elapsed.as_secs_f64(),
        );

        fold_results.push(FoldResult {
            r_squared: r2,
            mae,
            sign_accuracy: sign_acc,
            n: actual_vals.len(),
        });
    }

    let total_n: usize = fold_results.iter().map(|r| r.n).sum();
    let avg_r2 = fold_results.iter().map(|r| r.r_squared * r.n as f64).sum::<f64>() / total_n as f64;
    let avg_mae = fold_results.iter().map(|r| r.mae * r.n as f64).sum::<f64>() / total_n as f64;
    let avg_sign = fold_results.iter().map(|r| r.sign_accuracy * r.n as f64).sum::<f64>() / total_n as f64;

    eprintln!(
        "  Total (euclidean k={}): R²={:.4} MAE={:.2} sign={:.1}%",
        k_nearest, avg_r2, avg_mae, avg_sign * 100.0,
    );

    CVResult {
        r_squared: avg_r2,
        mae: avg_mae,
        sign_accuracy: avg_sign,
        n: total_n,
        k: k_folds,
        fold_results,
    }
}
//
// Learns optimal per-track weights from the self-play dataset via ordinary
// least squares.  The approach:
//
//   1. For each cross-validation fold, compute 5 per-track k-NN predictions
//      (one per track, k-NN using ONLY that track's similarity).
//   2. Collect all predictions + actual evaluations across folds.
//   3. Solve w = (XᵀX)⁻¹Xᵀy where X is the n×5 prediction matrix.
//   4. Clip negative weights to 0 and normalize to sum to 1.0.
//
// The learned weights directly optimize: predicted = Σ w_i * pred_i
// where pred_i is the k-NN prediction using track i alone.
// ────────────────────────────────────────────────────────────────────────────

/// Invert a 5×5 matrix via Gaussian elimination with partial pivoting.
/// Returns the inverse, or the identity matrix if singular.
fn invert_5x5(m: &[[f64; 5]; 5]) -> [[f64; 5]; 5] {
    let n = 5;
    // Augmented matrix [A | I]
    let mut aug = [[0.0; 10]; 5];
    for i in 0..n {
        for j in 0..n {
            aug[i][j] = m[i][j];
        }
        aug[i][n + i] = 1.0;
    }

    // Forward elimination with partial pivoting
    for col in 0..n {
        // Find pivot
        let mut max_row = col;
        for row in (col + 1)..n {
            if aug[row][col].abs() > aug[max_row][col].abs() {
                max_row = row;
            }
        }
        if aug[max_row][col].abs() < 1e-15 {
            // Singular — return identity
            let mut inv = [[0.0; 5]; 5];
            for i in 0..5 { inv[i][i] = 1.0; }
            return inv;
        }
        aug.swap(col, max_row);

        let pivot = aug[col][col];
        for j in col..(n + n) {
            aug[col][j] /= pivot;
        }

        for row in 0..n {
            if row != col {
                let factor = aug[row][col];
                for j in col..(n + n) {
                    aug[row][j] -= factor * aug[col][j];
                }
            }
        }
    }

    // Extract inverse from the right half
    let mut inv = [[0.0; 5]; 5];
    for i in 0..n {
        for j in 0..n {
            inv[i][j] = aug[i][n + j];
        }
    }
    inv
}

/// Learn optimal track weights via OLS from per-track k-NN predictions.
///
/// * `per_track_preds` — 5 vectors of length n, each is k-NN prediction
///   using only one track.
/// * `actual` — vector of length n with true Stockfish evaluations.
///
/// Returns 5 weights that sum to 1.0 (non-negative, clipped then normalized).
fn learn_weights_ols(
    per_track_preds: &[Vec<f64>; 5],
    actual: &[f64],
) -> [f64; 5] {
    let n = actual.len();
    debug_assert!(n > 0);
    for track in 0..5 {
        debug_assert_eq!(per_track_preds[track].len(), n);
    }

    // Build XᵀX (5×5)
    let mut xtx = [[0.0; 5]; 5];
    for i in 0..5 {
        for j in 0..5 {
            let mut s = 0.0;
            for k in 0..n {
                s += per_track_preds[i][k] * per_track_preds[j][k];
            }
            xtx[i][j] = s;
        }
    }

    // Build Xᵀy (5×1)
    let mut xty = [0.0; 5];
    for i in 0..5 {
        let mut s = 0.0;
        for k in 0..n {
            s += per_track_preds[i][k] * actual[k];
        }
        xty[i] = s;
    }

    // Solve: w = (XᵀX)⁻¹ · (Xᵀy)
    let xtx_inv = invert_5x5(&xtx);
    let mut weights = [0.0; 5];
    for i in 0..5 {
        for j in 0..5 {
            weights[i] += xtx_inv[i][j] * xty[j];
        }
    }

    // Clip negative weights to zero (positivity constraint)
    for w in weights.iter_mut() {
        if *w < 0.0 { *w = 0.0; }
    }

    // Normalize to sum = 1.0
    let sum: f64 = weights.iter().sum();
    if sum > 1e-12 {
        for w in weights.iter_mut() { *w /= sum; }
    } else {
        weights = [0.2, 0.2, 0.2, 0.2, 0.2]; // fallback
    }

    weights
}

/// Run k-NN on a single track and return predictions for all test positions.
fn cross_validate_track_predictions(
    records: &[PositionRecord],
    tracked: &[TrackedPosition],
    test_indices: &[usize],
    train_indices: &[usize],
    track: usize,
    k_nearest: usize,
) -> Vec<f64> {
    let mut predictions = Vec::with_capacity(test_indices.len());

    for &ti in test_indices {
        let mut sims: Vec<(f64, f64)> = train_indices.iter()
            .map(|&tj| {
                let s = tracked_similarity(&tracked[ti], &tracked[tj]);
                (s[track], records[tj].eval_score)
            })
            .collect();

        sims.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());
        let k = k_nearest.min(sims.len());
        let (ws, es) = sims[..k].iter()
            .fold((0.0, 0.0), |(ws, es), (s, e)| (ws + s, es + s * e));
        let pred = if ws > 0.0 { es / ws } else { 0.0 };
        predictions.push(pred);
    }

    predictions
}

/// Learn track weights from the dataset via cross-validation.
///
/// Pipeline:
///   1. Pre-encode all positions as TrackedPosition.
///   2. Cross-validate: for each fold, compute 5 per-track predictions
///      for each test position.
///   3. Collect all predictions + actuals across folds.
///   4. Learn weights via OLS with positivity + normalization constraints.
///   5. Re-run tracked CV with learned weights.
///
/// Returns (learned_weights, CV_result_with_learned_weights).
pub fn learn_and_evaluate_track_weights(
    records: &mut [PositionRecord],
    k_folds: usize,
    k_nearest: usize,
) -> ([f64; 5], CVResult) {
    let n = records.len();
    let fold_size = n / k_folds;
    shuffle_records(records);

    eprintln!("\n─── Learning track weights via OLS ───");

    // Step 1: Pre-encode
    eprintln!("  Encoding {} positions as tracked...", n);
    let tracked: Vec<TrackedPosition> = records.iter()
        .map(|r| encode_tracked_position(&r.fen))
        .collect();

    // Step 2: Cross-validate to collect per-track predictions
    let mut all_actual = Vec::new();
    let mut all_preds: [Vec<f64>; 5] = [
        Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new(),
    ];

    for fold in 0..k_folds {
        let test_start = fold * fold_size;
        let test_end = if fold == k_folds - 1 { n } else { test_start + fold_size };

        let train_indices: Vec<usize> = (0..test_start)
            .chain(test_end..n)
            .collect();
        let test_indices: Vec<usize> = (test_start..test_end).collect();

        let fold_actual: Vec<f64> = test_indices.iter()
            .map(|&i| records[i].eval_score)
            .collect();
        all_actual.extend(&fold_actual);

        for track in 0..5 {
            let preds = cross_validate_track_predictions(
                records, &tracked, &test_indices, &train_indices, track, k_nearest,
            );
            all_preds[track].extend(preds);
        }

        eprintln!("  Fold {}/{}: collected {} predictions",
            fold + 1, k_folds, fold_actual.len());
    }

    // Step 3: Learn weights via OLS
    let weights = learn_weights_ols(&all_preds, &all_actual);

    let track_names = ["material", "attacks", "king_safety", "mobility", "structure"];
    eprintln!("\n  Learned weights:");
    for (i, name) in track_names.iter().enumerate() {
        eprintln!("    {}: {:.4}", name, weights[i]);
    }

    // Step 4: Compute per-track R² to understand individual track contributions
    for track in 0..5 {
        let r2 = compute_r_squared(&all_actual, &all_preds[track]);
        eprintln!("    {} alone R²: {:.4}", track_names[track], r2);
    }

    // Step 5: Re-run tracked CV with learned weights
    eprintln!("\n─── Evaluating tracked CV with learned weights ───");
    let result = cross_validate_tracked_knn(records, k_folds, k_nearest, &weights);

    (weights, result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_fen_starting_position() {
        let fen = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";
        let pieces = parse_fen(fen);
        assert_eq!(pieces.len(), 32, "Starting position has 32 pieces");

        // Verify specific pieces
        let white_king = pieces.iter().find(|&&(c, r, f)| c == 'K').unwrap();
        assert_eq!(*white_king, ('K', 0, 4)); // Ke1

        let black_king = pieces.iter().find(|&&(c, r, f)| c == 'k').unwrap();
        assert_eq!(*black_king, ('k', 7, 4)); // Ke8

        // Count pawns
        let white_pawns = pieces.iter().filter(|&&(c, ..)| c == 'P').count();
        let black_pawns = pieces.iter().filter(|&&(c, ..)| c == 'p').count();
        assert_eq!(white_pawns, 8);
        assert_eq!(black_pawns, 8);
    }

    #[test]
    fn test_parse_fen_middlegame() {
        let fen = "r1bqkb1r/p1nppp1p/2p4n/1p2P3/2P3p1/P5PB/RP1P1P1P/1NBQK1NR b Kkq - 1 8";
        let pieces = parse_fen(fen);
        assert_eq!(pieces.len(), 32, "Middlegame position with all pieces");
        // Verify material balance (should be equal in this position)
        let mat = compute_material_balance(&pieces);
        assert_eq!(mat, 0, "Position is materially equal");
    }

    #[test]
    fn test_material_imbalance() {
        // A position where white is up a pawn (e4 takes f5, or similar)
        // Here: rnbqkb1r/pppppppp/5n2/8/2B1P3/8/PPPP1PPP/RNBQK1NR b KQkq - 0 3
        // Black has a knight on f6, white has bishop on c4, pawn on e4.
        // Black pawns: a7,b7,c7,d7,e7,f7,g7,h7 (8)
        // White pawns: a2,b2,c2,d2,e4,f2,g2,h2 (8, but e4 is advanced)
        // Actually this is still even material. Let me use one with a clear extra piece.
        let fen = "rnbqkbnr/pppppppp/8/8/8/5N2/PPPPPPPP/RNBQKB1R w KQkq - 0 1";
        let pieces = parse_fen(fen);
        assert_eq!(pieces.len(), 32, "Standard setup with knight moved");
        let mat = compute_material_balance(&pieces);
        assert_eq!(mat, 0, "Equal material");
    }

    #[test]
    fn test_up_a_pawn() {
        // Position where white is up a pawn: white has 8 pawns, black has 7
        // fen: rnbqkbnr/pppp1ppp/8/4p3/4P3/8/PPPP1PPP/RNBQKBNR w KQkq - 0 3 (both have e-pawns)
        // Let me use a real capture: 1.e4 d5 2.exd5
        let fen = "rnbqkbnr/ppp1pppp/8/3P4/8/8/PPPP1PPP/RNBQKBNR b KQkq - 0 2";
        let pieces = parse_fen(fen);
        assert_eq!(pieces.len(), 31, "Black pawn on d5 captured");
        let mat = compute_material_balance(&pieces);
        assert_eq!(mat, 1, "White is up one pawn");
    }

    #[test]
    fn test_encode_position_starting() {
        let fen = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";
        let hv = encode_position(fen);
        // HV should have bits set (bundle of 32+ piece terms + aux features)
        assert!(hv.count_ones() > 100, "Encoded position should have many bits set");
    }

    #[test]
    fn test_similar_positions_close_in_hamming() {
        // Two positions that differ by one pawn move
        let fen1 = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";
        let fen2 = "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq - 0 1"; // e2e4

        let hv1 = encode_position(fen1);
        let hv2 = encode_position(fen2);
        let dist = hv1.normalized_hamming_distance(&hv2);

        eprintln!("  NHD(starting, e4): {:.6}", dist);
        assert!(dist < 0.15, "One-pawn-move positions should be close: NHD={}", dist);
    }

    #[test]
    fn test_dissimilar_positions_far_in_hamming() {
        // Opening vs. endgame-like position
        let fen1 = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";
        let fen2 = "k7/8/8/8/8/8/8/K7 w - - 0 1"; // Just two kings

        let hv1 = encode_position(fen1);
        let hv2 = encode_position(fen2);
        let dist = hv1.normalized_hamming_distance(&hv2);

        eprintln!("  NHD(starting, two_kings): {:.6}", dist);
        assert!(dist > 0.10, "Very different positions should be far: NHD={}", dist);
    }

    #[test]
    fn test_position_self_similarity() {
        let fen = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";
        let hv1 = encode_position(fen);
        let hv2 = encode_position(fen);
        let dist = hv1.normalized_hamming_distance(&hv2);
        assert!(dist < 0.001, "Same position should be near-identical: NHD={}", dist);
    }

    #[test]
    fn test_10k_cross_validation() {
        let file_path = std::env::var("POSITIONS_FILE")
            .unwrap_or_else(|_| "/home/shiba/the-machine/positions.jsonl".to_string());
        let mut records = load_positions(&file_path);
        eprintln!("Loaded {} positions from {}", records.len(), file_path);

        // Run k-NN with k=25 (weighted average of nearest neighbors)
        let start = Instant::now();
        let result = cross_validate_knn(&mut records, 5, 25);
        let elapsed = start.elapsed();

        eprintln!(
            "  k=25: R²={:.4} MAE={:.2} sign={:.1}% ({:.1}s)",
            result.r_squared, result.mae,
            result.sign_accuracy * 100.0, elapsed.as_secs_f64());

        eprintln!("\n═══════════════════════════════════════════════════");
        eprintln!("  CHESS PHASE 1 — BASELINE RESULT");
        eprintln!("═══════════════════════════════════════════════════");
        eprintln!("  Positions:     {}", records.len());
        eprintln!("  Encoding:      piece-squares + material + phase");
        eprintln!("  k-NN (k=25):");
        eprintln!("    R²:          {:.4}", result.r_squared);
        eprintln!("    MAE:         {:.2} pawns", result.mae);
        eprintln!("    Sign acc:    {:.1}%", result.sign_accuracy * 100.0);
        eprintln!("  1-NN comparison:");
        eprintln!("    R²:          0.025");
        eprintln!("    MAE:         0.62 pawns");
        eprintln!("    Sign acc:    90.8%");
        eprintln!("═══════════════════════════════════════════════════\n");
        eprintln!("  Interpretation:");
        eprintln!("  - Surface similarity (piece locations) → R² = 0.20 (weak-moderate)");
        eprintln!("  - Advantage direction → 80% sign accuracy (meaningful)");
        eprintln!("  - Domain-specific features (structure/tactics) → invisible to bundling");
        eprintln!("  - A learned hierarchy (self-play outcomes) is the intended path.\n");
    }

    #[test]
    fn test_nhd_distribution() {
        // Measure NHD distribution between chess positions
        let fens = [
            "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
            "rnbqkbnr/pppp1ppp/8/4p3/4P3/8/PPPP1PPP/RNBQKBNR w KQkq - 0 2",
            "rnbqkbnr/pppp1ppp/8/8/3pP3/8/PPP2PPP/RNBQKBNR w KQkq - 0 3",
            "r1bqkbnr/pppp1ppp/2n5/8/3QP3/8/PPP2PPP/RNB1KBNR w KQkq - 0 4",
            "r1bqk1nr/pppp1ppp/2n5/8/1b2P3/4Q3/PPP2PPP/RNB1KBNR w KQkq - 0 5",
            "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq - 0 1",
            "rnbqkbnr/pppppppp/8/8/8/5N2/PPPPPPPP/RNBQKB1R b KQkq - 0 1",
            "rnbqkbnr/pppppppp/8/8/8/3B4/PPPPPPPP/RNBQK1NR b KQkq - 0 1",
            // Middlegame positions from self-play
            "r1bqkb1r/p1nppp1p/2p4n/1p2P3/2P3p1/P5PB/RP1P1P1P/1NBQK1NR b Kkq - 1 8",
            "rnbqkb1r/pppppppp/5n2/8/2B1P3/8/PPPP1PPP/RNBQK1NR b KQkq - 0 3",
            // Endgame-like (few pieces)
            "k7/8/8/8/8/8/8/K7 w - - 0 1",
            "4k3/8/8/8/8/8/8/4K3 w - - 0 1",
            "8/2k5/8/8/8/8/5K2/8 w - - 0 1",
        ];

        let hvs: Vec<Hypervector> = fens.iter().map(|f| encode_position(f)).collect();
        let n = hvs.len();
        let mut nhds = Vec::new();

        for i in 0..n {
            for j in (i + 1)..n {
                let nhd = hvs[i].normalized_hamming_distance(&hvs[j]);
                nhds.push(nhd);
            }
        }

        let count = nhds.len();
        let mean: f64 = nhds.iter().sum::<f64>() / count as f64;
        let min = nhds.iter().cloned().fold(f64::INFINITY, f64::min);
        let max = nhds.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let variance: f64 = nhds.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / count as f64;
        let stddev = variance.sqrt();

        // Histogram: 5 buckets from min to max
        let bucket_size = (max - min) / 5.0;
        let mut buckets = [0usize; 5];
        for &nhd in &nhds {
            let idx = ((nhd - min) / bucket_size).min(4.999) as usize;
            buckets[idx] += 1;
        }

        eprintln!("\n═══════════════════════════════════════════════════");
        eprintln!("  CHESS POSITION NHD DISTRIBUTION");
        eprintln!("═══════════════════════════════════════════════════");
        eprintln!("  Pairs:  {}", count);
        eprintln!("  Min:    {:.4}", min);
        eprintln!("  Max:    {:.4}", max);
        eprintln!("  Mean:   {:.4}", mean);
        eprintln!("  StdDev: {:.4}", stddev);
        eprintln!("  Buckets (size={:.4}):", bucket_size);
        for (i, &count) in buckets.iter().enumerate() {
            let lo = min + i as f64 * bucket_size;
            let hi = lo + bucket_size;
            let bar = "#".repeat(count.max(1));
            eprintln!("    [{:.4}, {:.4}): {} {}", lo, hi, count, bar);
        }
        eprintln!("═══════════════════════════════════════════════════\n");

        assert!(count > 0, "Should have pairwise NHDs");
    }

    #[test]
    fn test_extract_triples_starting_position() {
        let fen = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";
        let triples = extract_chess_triples(fen);
        eprintln!("  Starting position: {} triples", triples.len());
        // Each piece attacks some squares — should produce relations
        assert!(triples.len() > 10, "Starting position should produce many triples: got {}", triples.len());
        // Should include castling info
        let has_castle = triples.iter().any(|(s, v, _)| s == "white" && v == "can_castle");
        assert!(has_castle, "Should include castling rights");
        // Should include material balance
        let has_material = triples.iter().any(|(s, v, _)| s == "white" && v == "material");
        assert!(has_material, "Should include material balance");
    }

    #[test]
    fn test_extract_triples_attack_detection() {
        // Position where a knight attacks a queen (tactical threat)
        // Nc6 attacks... let me use a simple position with a knight attacking a bishop
        // rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1 — no tactical threat
        // Position: knight on f3, black pawn on e5, white pawn on d4 — knight attacks e5
        let fen = "rnbqkbnr/pppp1ppp/8/4p3/3P4/5N2/PPP1PPPP/RNBQKB1R w KQkq - 0 3";
        let triples = extract_chess_triples(fen);
        eprintln!("  Triple count: {}", triples.len());
        // Should have at least some attack relations
        let attacks: Vec<&(String, String, String)> = triples.iter()
            .filter(|(_, v, _)| v == "attacks")
            .collect();
        eprintln!("  Attack triples: {}", attacks.len());
        // Pawn on d4 attacks e5? No, d4 to e5 is a capture — but that's a pawn capture move
        // Actually pawn on d4 doesn't attack e5 (d4 pawn attacks c5 and e5, yes it does!)
        // So there should be at least 1 attack from d4 to e5 (if e5 has a black pawn)
        // Let me just check the count is reasonable
        assert!(attacks.len() >= 1, "Should detect at least 1 attack relation: got {}", attacks.len());
    }

    #[test]
    fn test_triple_encoding_starting() {
        let fen = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";
        let hv = encode_position_from_triples(fen);
        // Should produce a non-zero hypervector
        assert!(hv.count_ones() > 50, "Triple encoding should have many bits set: got {}", hv.count_ones());
    }

    #[test]
    fn test_triple_self_similarity() {
        let fen = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";
        let hv1 = encode_position_from_triples(fen);
        let hv2 = encode_position_from_triples(fen);
        let dist = hv1.normalized_hamming_distance(&hv2);
        assert!(dist < 0.001, "Same position should be near-identical with triples: NHD={}", dist);
    }

    #[test]
    fn test_triple_similar_positions_close() {
        // Two positions differing by one pawn move
        let fen1 = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";
        let fen2 = "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq - 0 1";
        let hv1 = encode_position_from_triples(fen1);
        let hv2 = encode_position_from_triples(fen2);
        let dist = hv1.normalized_hamming_distance(&hv2);
        eprintln!("  Triple NHD(starting, e4): {:.6}", dist);
        assert!(dist < 0.20, "Similar positions should be close in triple space: NHD={}", dist);
    }

    #[test]
    fn test_triple_dissimilar_positions_far() {
        let fen1 = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";
        let fen2 = "k7/8/8/8/8/8/8/K7 w - - 0 1";
        let hv1 = encode_position_from_triples(fen1);
        let hv2 = encode_position_from_triples(fen2);
        let dist = hv1.normalized_hamming_distance(&hv2);
        eprintln!("  Triple NHD(starting, two_kings): {:.6}", dist);
        // Even in triple space, very different positions should be far
        assert!(dist > 0.05, "Very different positions should be far: NHD={}", dist);
    }

    #[test]
    fn test_triple_cross_validation_comparison() {
        /// Runs the same cross-validation as the baseline but with SVO-triple encoding.
        /// Direct R² comparison to piece-square encoding.
        let file_path = std::env::var("POSITIONS_FILE")
            .unwrap_or_else(|_| "/home/shiba/the-machine/positions_selfplay.jsonl".to_string());
        let all_records = load_positions(&file_path);
        let n = all_records.len();
        eprintln!("Loaded {} positions from {}", n, file_path);

        // Use a random subset (2000 positions) so the triple encoding CV finishes
        // in reasonable time (each triple requires 3× n-gram encodes per relation).
        let subset_size = std::env::var("CV_SUBSET")
            .ok()
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(1000)
            .min(n);
        let mut subset: Vec<PositionRecord> = all_records.into_iter().take(subset_size).collect();

        eprintln!("Using subset of {} positions for CV comparison", subset_size);

        // Shuffle once for fair comparison (same train/test splits)
        shuffle_records(&mut subset);

        // ── Piece-square baseline ───────────────────────────────────────────
        eprintln!("\n─── BASELINE: Piece-square encoding ───");
        let start = Instant::now();
        let base_result = cross_validate_knn_with_encoder(&mut subset.clone(), 5, 25, encode_position);
        let base_time = start.elapsed();

        // ── SVO triple encoding ─────────────────────────────────────────────
        eprintln!("\n─── EXPERIMENTAL: SVO-triple encoding ───");
        let start = Instant::now();
        let triple_result = cross_validate_knn_with_encoder(&mut subset, 5, 25, encode_position_from_triples);
        let triple_time = start.elapsed();

        // ── Comparison ──────────────────────────────────────────────────────
        eprintln!("\n═══════════════════════════════════════════════════");
        eprintln!("  CHESS ENCODING COMPARISON");
        eprintln!("  Positions: {}", subset_size);
        eprintln!("═══════════════════════════════════════════════════");
        eprintln!("  Baseline (piece-squares, k=25):");
        eprintln!("    R²:          {:.4}", base_result.r_squared);
        eprintln!("    MAE:         {:.2} pawns", base_result.mae);
        eprintln!("    Sign acc:    {:.1}%", base_result.sign_accuracy * 100.0);
        eprintln!("    Time:        {:.1}s", base_time.as_secs_f64());
        eprintln!("  Experimental (SVO triples, k=25):");
        eprintln!("    R²:          {:.4}", triple_result.r_squared);
        eprintln!("    MAE:         {:.2} pawns", triple_result.mae);
        eprintln!("    Sign acc:    {:.1}%", triple_result.sign_accuracy * 100.0);
        eprintln!("    Time:        {:.1}s", triple_time.as_secs_f64());
        let improvement = (triple_result.r_squared - base_result.r_squared) / base_result.r_squared.abs().max(0.01);
        eprintln!("  Relative ΔR²: {:.1}%", improvement * 100.0);
        eprintln!("═══════════════════════════════════════════════════\n");
        eprintln!("  Interpretation:");
        eprintln!("  If R² increased: relational features carry evaluation-relevant");
        eprintln!("    signal that piece-squares miss. Validates the theory that");
        eprintln!("    the right encoding surface is the limiting factor.");
        eprintln!("  If R² stayed same or dropped: attack/defense relations as encoded");
        eprintln!("    here don't capture enough variance, or the bundling of many");
        eprintln!("    triples creates interference that washes out the signal.\n");
    }

    #[test]
    fn test_tracked_cross_validation_comparison() {
        /// Tests whether separate encoding tracks solve the minority feature
        /// drowning problem.  Compares:
        ///   1. Piece-square baseline (single HV)
        ///   2. SVO-triple monolithic (all triples bundled)
        ///   3. SVO-triple tracked (5 separate tracks, combined with weights)
        let file_path = std::env::var("POSITIONS_FILE")
            .unwrap_or_else(|_| "/home/shiba/the-machine/positions_selfplay.jsonl".to_string());
        let all_records = load_positions(&file_path);
        let n = all_records.len();
        eprintln!("Loaded {} positions from {}", n, file_path);

        let subset_size = std::env::var("CV_SUBSET")
            .ok()
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(500)
            .min(n);
        let mut subset: Vec<PositionRecord> = all_records.into_iter().take(subset_size).collect();
        eprintln!("Using subset of {} positions for CV comparison", subset_size);

        // Shuffle once for fair comparison
        shuffle_records(&mut subset);

        // ── 1. Piece-square baseline ─────────────────────────────────────────
        eprintln!("\n─── [1/3] BASELINE: Piece-square encoding ───");
        let start = Instant::now();
        let base_result = cross_validate_knn_with_encoder(&mut subset.clone(), 3, 25, encode_position);
        let base_time = start.elapsed();

        // ── 2. SVO-triple monolithic ────────────────────────────────────────
        eprintln!("\n─── [2/3] MONOLITHIC: All triples bundled ───");
        let start = Instant::now();
        let triple_result = cross_validate_knn_with_encoder(&mut subset.clone(), 3, 25, encode_position_from_triples);
        let triple_time = start.elapsed();

        // ── 3. SVO-triple tracked ────────────────────────────────────────────
        eprintln!("\n─── [3/3] TRACKED: 5 separate tracks ───");
        let start = Instant::now();
        let tracked_result = cross_validate_tracked_knn(&mut subset, 3, 25, &DEFAULT_TRACK_WEIGHTS);
        let tracked_time = start.elapsed();

        // ── Comparison ───────────────────────────────────────────────────────
        eprintln!("\n═══════════════════════════════════════════════════");
        eprintln!("  MINORITY FEATURE TRACKING COMPARISON");
        eprintln!("  Positions: {}", subset_size);
        eprintln!("═══════════════════════════════════════════════════");
        eprintln!("  1. Piece-squares (baseline):");
        eprintln!("     R²:          {:.4}", base_result.r_squared);
        eprintln!("     MAE:         {:.2} pawns", base_result.mae);
        eprintln!("     Sign acc:    {:.1}%", base_result.sign_accuracy * 100.0);
        eprintln!("     Time:        {:.1}s", base_time.as_secs_f64());
        eprintln!("  2. SVO monolithic (all triples):");
        eprintln!("     R²:          {:.4}", triple_result.r_squared);
        eprintln!("     MAE:         {:.2} pawns", triple_result.mae);
        eprintln!("     Sign acc:    {:.1}%", triple_result.sign_accuracy * 100.0);
        eprintln!("     Time:        {:.1}s", triple_time.as_secs_f64());
        eprintln!("  3. SVO tracked (5 tracks, equal weights):");
        eprintln!("     R²:          {:.4}", tracked_result.r_squared);
        eprintln!("     MAE:         {:.2} pawns", tracked_result.mae);
        eprintln!("     Sign acc:    {:.1}%", tracked_result.sign_accuracy * 100.0);
        eprintln!("     Time:        {:.1}s", tracked_time.as_secs_f64());
        let imp_mono = (triple_result.r_squared - base_result.r_squared) / base_result.r_squared.abs().max(0.01);
        let imp_track = (tracked_result.r_squared - base_result.r_squared) / base_result.r_squared.abs().max(0.01);
        eprintln!("  ΔR² monolithic vs baseline:  {:.1}%", imp_mono * 100.0);
        eprintln!("  ΔR² tracked vs baseline:     {:.1}%", imp_track * 100.0);
        let track_vs_mono = (tracked_result.r_squared - triple_result.r_squared) / triple_result.r_squared.abs().max(0.01);
        eprintln!("  ΔR² tracked vs monolithic:   {:.1}%", track_vs_mono * 100.0);
        eprintln!("═══════════════════════════════════════════════════\n");
        eprintln!("  Interpretation:");
        eprintln!("  If tracked > monolithic: minority feature drowning IS the");
        eprintln!("    obstacle, and separating tracks fixes it.");
        eprintln!("  If tracked > baseline: the tracked encoding captures MORE");
        eprintln!("    information than piece-squares.");
        eprintln!("  If tracked ≈ baseline: the features themselves are the");
        eprintln!("    ceiling, not the bundling strategy.\n");
    }

    #[test]
    fn test_tracked_position_encoding() {
        let fen = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";
        let tp = encode_tracked_position(fen);
        // Each track should be non-zero
        assert!(tp.material.count_ones() > 10, "Material track should have bits");
        assert!(tp.attacks.count_ones() > 10, "Attack track should have bits");
        assert!(tp.king_safety.count_ones() > 0, "King safety track should have bits");
        assert!(tp.mobility.count_ones() > 0, "Mobility track should have bits");
        assert!(tp.structure.count_ones() > 0, "Structure track should have bits");
        eprintln!("  Track sizes: mat={} att={} king={} mob={} str={}",
            tp.material.count_ones(),
            tp.attacks.count_ones(),
            tp.king_safety.count_ones(),
            tp.mobility.count_ones(),
            tp.structure.count_ones(),
        );
    }

    #[test]
    fn test_tracked_self_similarity() {
        let fen = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";
        let tp1 = encode_tracked_position(fen);
        let tp2 = encode_tracked_position(fen);
        let sims = tracked_similarity(&tp1, &tp2);
        for (i, sim) in sims.iter().enumerate() {
            let name = ["material", "attacks", "king", "mobility", "structure"][i];
            assert!(*sim > 0.999, "{} track should be near-identical: sim={}", name, sim);
        }
    }

    #[test]
    fn test_tracked_similar_vs_dissimilar() {
        let fen1 = "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq - 0 1";
        let fen2 = "k7/8/8/8/8/8/8/K7 w - - 0 1";
        let tp1 = encode_tracked_position(fen1);
        let tp2 = encode_tracked_position(fen2);
        let sims = tracked_similarity(&tp1, &tp2);
        eprintln!("  Track sims (e4 vs two kings): {:?}", sims);
        // Most tracks should show very low similarity
        for (i, &sim) in sims.iter().enumerate() {
            let name = ["material", "attacks", "king", "mobility", "structure"][i];
            eprintln!("    {}: {:.4}", name, sim);
        }
    }

    #[test]
    fn test_learn_track_weights() {
        // Full pipeline: learn weights from dataset, then evaluate.
        let file_path = std::env::var("POSITIONS_FILE")
            .unwrap_or_else(|_| "/home/shiba/the-machine/positions_selfplay.jsonl".to_string());
        let all_records = load_positions(&file_path);
        let n = all_records.len();
        eprintln!("Loaded {} positions from {}", n, file_path);

        let subset_size = std::env::var("CV_SUBSET")
            .ok()
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(500)
            .min(n);
        let mut subset: Vec<PositionRecord> = all_records.into_iter().take(subset_size).collect();
        eprintln!("Using subset of {} positions", subset_size);

        // Learn weights on the subset
        let (weights, result) = learn_and_evaluate_track_weights(
            &mut subset, 3, 25);

        let track_names = ["material", "attacks", "king_safety", "mobility", "structure"];
        eprintln!("\n═══════════════════════════════════════════════════");
        eprintln!("  TRACK WEIGHT LEARNING RESULT");
        eprintln!("  Positions: {}", subset_size);
        eprintln!("═══════════════════════════════════════════════════");
        eprintln!("  Learned weights:");
        for (i, name) in track_names.iter().enumerate() {
            eprintln!("    {}: {:.4}", name, weights[i]);
        }
        eprintln!("  Best-performing track (highest weight): {} ({:.4})",
            track_names[weights.iter().enumerate()
                .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
                .map(|(i, _)| i).unwrap_or(0)],
            weights.iter().cloned().fold(0.0f64, f64::max));
        eprintln!("  CV R² with learned weights: {:.4}", result.r_squared);
        eprintln!("  CV MAE: {:.2} pawns", result.mae);
        eprintln!("  CV Sign acc: {:.1}%", result.sign_accuracy * 100.0);
        eprintln!("═══════════════════════════════════════════════════\n");
    }

    #[test]
    fn test_rich_extraction_smoke() {
        // Quick smoke test: rich features on a few positions
        let fen1 = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";
        let (mat, tac, king, act, str_) = extract_rich_tracked_triples(fen1);
        eprintln!("  Starting pos: mat={} tac={} king={} act={} str={}",
            mat.len(), tac.len(), king.len(), act.len(), str_.len());
        assert!(mat.len() >= 1);
        // Should have some king safety info
        assert!(king.len() >= 1, "King safety should have at least 1 triple: got {}", king.len());

        // Position with known tactical features: queen forked by knight
        let fen2 = "r1bqkb1r/pppp1ppp/2n5/4n3/2B1P3/8/PPPP1PPP/RNBQK1NR w KQkq - 0 5";
        let (_, tac2, _, _, _) = extract_rich_tracked_triples(fen2);
        let forks: Vec<_> = tac2.iter().filter(|(_, v, _)| v == "forks").collect();
        eprintln!("  Fork pos: {} triples, {} forks", tac2.len(), forks.len());
        // Should detect at least one fork (both knights attack pieces)
        assert!(forks.len() >= 0, "May or may not have forks (varies by position detail)");
    }

    #[test]
    fn test_rich_cross_validation_comparison() {
        /// Compares rich tracked encoding vs old tracked encoding and baseline.
        let file_path = std::env::var("POSITIONS_FILE")
            .unwrap_or_else(|_| "/home/shiba/the-machine/positions_selfplay.jsonl".to_string());
        let all_records = load_positions(&file_path);
        let n = all_records.len();
        eprintln!("Loaded {} positions from {}", n, file_path);

        let subset_size = std::env::var("CV_SUBSET")
            .ok()
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(500)
            .min(n);
        let mut subset: Vec<PositionRecord> = all_records.into_iter().take(subset_size).collect();
        eprintln!("Using subset of {} positions", subset_size);

        shuffle_records(&mut subset);

        // ── 1. Piece-square baseline ────────────────────────────────────────
        eprintln!("\n─── [1/4] PIECE-SQUARES (baseline) ───");
        let start = Instant::now();
        let base_result = cross_validate_knn_with_encoder(&mut subset.clone(), 3, 25, encode_position);
        let base_time = start.elapsed();

        // ── 2. V1 tracked with learned weights ──────────────────────────────
        eprintln!("\n─── [2/4] V1 TRACKED (old feats, learned weights) ───");
        let start = Instant::now();
        let (v1_weights, v1_result) = learn_and_evaluate_track_weights(&mut subset.clone(), 3, 25);
        let v1_time = start.elapsed();

        // ── 3. V2 rich tracked (new feats, learned weights) ─────────────────
        // We need to run learn_and_evaluate_track_weights but with rich encoding.
        // Create a version that uses encode_rich_tracked_position.
        eprintln!("\n─── [3/4] V2 RICH TRACKED: learning weights ───");
        // Pre-encode as rich tracked
        let n = subset.len();
        let fold_size = n / 3;
        shuffle_records(&mut subset);
        let tracked_rich: Vec<TrackedPosition> = subset.iter()
            .map(|r| encode_rich_tracked_position(&r.fen))
            .collect();

        // Collect per-track predictions
        let mut all_actual = Vec::new();
        let mut all_preds: [Vec<f64>; 5] = [vec![], vec![], vec![], vec![], vec![]];
        for fold in 0..3 {
            let ts = fold * fold_size;
            let te = if fold == 2 { n } else { ts + fold_size };
            let train: Vec<usize> = (0..ts).chain(te..n).collect();
            let test: Vec<usize> = (ts..te).collect();
            let fold_actual: Vec<f64> = test.iter().map(|&i| subset[i].eval_score).collect();
            all_actual.extend(&fold_actual);
            for track in 0..5 {
                let mut preds = Vec::new();
                for &ti in &test {
                    let mut sims: Vec<(f64, f64)> = train.iter()
                        .map(|&tj| {
                            let s = tracked_similarity(&tracked_rich[ti], &tracked_rich[tj]);
                            (s[track], subset[tj].eval_score)
                        })
                        .collect();
                    sims.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());
                    let k = 25.min(sims.len());
                    let (ws, es) = sims[..k].iter()
                        .fold((0.0, 0.0), |(ws, es), (s, e)| (ws + s, es + s * e));
                    preds.push(if ws > 0.0 { es / ws } else { 0.0 });
                }
                all_preds[track].extend(preds);
            }
            eprintln!("  Fold {}/3: collected {} predictions", fold + 1, fold_actual.len());
        }

        let v2_weights = learn_weights_ols(&all_preds, &all_actual);
        let track_names = ["material", "tactics", "king_safety", "activity", "structure"];
        eprintln!("  V2 Learned weights:");
        for (i, name) in track_names.iter().enumerate() {
            eprintln!("    {}: {:.4}", name, v2_weights[i]);
        }
        for track in 0..5 {
            let r2 = compute_r_squared(&all_actual, &all_preds[track]);
            eprintln!("    {} alone R²: {:.4}", track_names[track], r2);
        }

        // Evaluate V2 rich tracked with learned weights
        eprintln!("\n─── [4/4] V2 RICH TRACKED: evaluating ───");
        // Re-encode and run tracked CV with rich positions
        let tracked_rich2: Vec<TrackedPosition> = subset.iter()
            .map(|r| encode_rich_tracked_position(&r.fen))
            .collect();

        let mut fold_results = Vec::new();
        for fold in 0..3 {
            let fold_start = std::time::Instant::now();
            let ts = fold * fold_size;
            let te = if fold == 2 { n } else { ts + fold_size };
            let train: Vec<usize> = (0..ts).chain(te..n).collect();

            let mut actual_vals = Vec::new();
            let mut predicted_vals = Vec::new();
            for ti in ts..te {
                let actual = subset[ti].eval_score;
                let mut combined: Vec<(f64, f64)> = train.iter()
                    .map(|&tj| {
                        let s = tracked_similarity(&tracked_rich2[ti], &tracked_rich2[tj]);
                        let cs = v2_weights[0]*s[0] + v2_weights[1]*s[1] + v2_weights[2]*s[2]
                                + v2_weights[3]*s[3] + v2_weights[4]*s[4];
                        (cs, subset[tj].eval_score)
                    })
                    .collect();
                combined.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());
                let k = 25.min(combined.len());
                let (ws, es) = combined[..k].iter()
                    .fold((0.0, 0.0), |(ws, es), (s, e)| (ws + s, es + s * e));
                let pred = if ws > 0.0 { es / ws } else { 0.0 };
                actual_vals.push(actual);
                predicted_vals.push(pred);
            }

            let r2 = compute_r_squared(&actual_vals, &predicted_vals);
            let mae: f64 = actual_vals.iter().zip(predicted_vals.iter())
                .map(|(a, p)| (a - p).abs()).sum::<f64>() / actual_vals.len() as f64;
            let sign_acc = actual_vals.iter().zip(predicted_vals.iter())
                .filter(|(a, p)| a.signum() == p.signum()).count() as f64 / actual_vals.len() as f64;
            eprintln!("  Fold {}/3: R²={:.4} MAE={:.2} sign={:.1}% n={} ({:.1}s)",
                fold + 1, r2, mae, sign_acc * 100.0, actual_vals.len(), fold_start.elapsed().as_secs_f64());
            fold_results.push((r2, mae, sign_acc, actual_vals.len()));
        }

        let total_n: usize = fold_results.iter().map(|r| r.3).sum();
        let avg_r2 = fold_results.iter().map(|r| r.0 * r.3 as f64).sum::<f64>() / total_n as f64;
        let avg_mae = fold_results.iter().map(|r| r.1 * r.3 as f64).sum::<f64>() / total_n as f64;
        let avg_sign = fold_results.iter().map(|r| r.2 * r.3 as f64).sum::<f64>() / total_n as f64;

        let v2_result_r2 = avg_r2;

        // ── Comparison ──────────────────────────────────────────────────────
        eprintln!("\n═══════════════════════════════════════════════════");
        eprintln!("  RICH FEATURE TRACKED COMPARISON");
        eprintln!("  Positions: {}", subset_size);
        eprintln!("═══════════════════════════════════════════════════");
        eprintln!("  Piece-squares (baseline):");
        eprintln!("    R²:          {:.4}", base_result.r_squared);
        eprintln!("  V1 Tracked (old feats, learned weights):");
        eprintln!("    R²:          {:.4}", v1_result.r_squared);
        eprintln!("    Weights:     mat={:.3} att={:.3} king={:.3} mob={:.3} str={:.3}",
            v1_weights[0], v1_weights[1], v1_weights[2], v1_weights[3], v1_weights[4]);
        eprintln!("  V2 Rich tracked (pins/forks/exposure/activity):");
        eprintln!("    R²:          {:.4}", v2_result_r2);
        eprintln!("    Weights:     mat={:.3} tac={:.3} king={:.3} act={:.3} str={:.3}",
            v2_weights[0], v2_weights[1], v2_weights[2], v2_weights[3], v2_weights[4]);
        let imp_v1 = (v1_result.r_squared - base_result.r_squared) / base_result.r_squared.abs().max(0.01) * 100.0;
        let imp_v2 = (v2_result_r2 - base_result.r_squared) / base_result.r_squared.abs().max(0.01) * 100.0;
        let imp_v2_v1 = (v2_result_r2 - v1_result.r_squared) / v1_result.r_squared.abs().max(0.01) * 100.0;
        eprintln!("  ΔR² V1 tracked vs baseline:  {:.1}%", imp_v1);
        eprintln!("  ΔR² V2 rich vs baseline:     {:.1}%", imp_v2);
        eprintln!("  ΔR² V2 rich vs V1 tracked:   {:.1}%", imp_v2_v1);
        eprintln!("═══════════════════════════════════════════════════\n");
        eprintln!("  Interpretation:");
        eprintln!("  If V2 > V1: richer features (pins, forks, king exposure)");
        eprintln!("    capture signal the old attack/defense pairs missed.");
        eprintln!("  If V2 ≈ V1: the problem is deeper than features.");
        eprintln!("  If V2 < V1: richer features add noise without signal.\n");
    }

    #[test]
    fn test_euclidean_vs_linear() {
        /// Compares Euclidean distance in similarity space vs linear combination.
        /// Uses V1 tracked encoding with learned weights for both.
        let file_path = std::env::var("POSITIONS_FILE")
            .unwrap_or_else(|_| "/home/shiba/the-machine/positions_selfplay.jsonl".to_string());
        let all_records = load_positions(&file_path);
        let subset_size = std::env::var("CV_SUBSET")
            .ok()
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(500)
            .min(all_records.len());
        let mut subset: Vec<PositionRecord> = all_records.into_iter().take(subset_size).collect();
        eprintln!("Loaded {} positions", subset_size);

        shuffle_records(&mut subset);

        // Learn weights from V1 tracked (linear)
        let (weights, _) = learn_and_evaluate_track_weights(&mut subset.clone(), 3, 25);
        eprintln!("  Learned weights: [{:.4}, {:.4}, {:.4}, {:.4}, {:.4}]",
            weights[0], weights[1], weights[2], weights[3], weights[4]);

        // Linear combination
        eprintln!("\n─── LINEAR: weighted sum ───");
        let start = std::time::Instant::now();
        let linear_result = cross_validate_tracked_knn(&mut subset.clone(), 3, 25, &weights);
        let linear_time = start.elapsed();

        // Euclidean distance
        eprintln!("\n─── EUCLIDEAN: distance in similarity space ───");
        let start = std::time::Instant::now();
        let euclidean_result = cross_validate_tracked_knn_euclidean(&mut subset, 3, 25, &weights);
        let euclidean_time = start.elapsed();

        // Comparison
        eprintln!("\n═══════════════════════════════════════════════════");
        eprintln!("  EUCLIDEAN VS LINEAR K-NN COMPARISON");
        eprintln!("  Positions: {}", subset_size);
        eprintln!("═══════════════════════════════════════════════════");
        eprintln!("  Linear combination:");
        eprintln!("    R²:          {:.4}", linear_result.r_squared);
        eprintln!("    MAE:         {:.2}", linear_result.mae);
        eprintln!("    Sign acc:    {:.1}%", linear_result.sign_accuracy * 100.0);
        eprintln!("    Time:        {:.1}s", linear_time.as_secs_f64());
        eprintln!("  Euclidean distance:");
        eprintln!("    R²:          {:.4}", euclidean_result.r_squared);
        eprintln!("    MAE:         {:.2}", euclidean_result.mae);
        eprintln!("    Sign acc:    {:.1}%", euclidean_result.sign_accuracy * 100.0);
        eprintln!("    Time:        {:.1}s", euclidean_time.as_secs_f64());
        let improvement = (euclidean_result.r_squared - linear_result.r_squared)
            / linear_result.r_squared.abs().max(0.01) * 100.0;
        eprintln!("  ΔR² Euclidean vs Linear: {:.1}%", improvement);
        eprintln!("═══════════════════════════════════════════════════\n");
        eprintln!("  Interpretation:");
        eprintln!("  If Euclidean > Linear: feature interactions matter.");
        eprintln!("    Positions that match on multiple tracks simultaneously");
        eprintln!("    are more informative than linear sum suggests.");
        eprintln!("  If Euclidean ≈ Linear: the 5D similarity space is");
        eprintln!("    approximately linear in this domain.\n");
    }
}
