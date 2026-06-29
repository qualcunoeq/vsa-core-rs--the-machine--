#!/usr/bin/env python3
"""
Generate chess positions from Stockfish self-play at moderate depth.

These positions have coherent evaluation signal (material correlates with
eval, positional factors are meaningful) — suitable for testing whether
VSA hypervector similarity captures chess position similarity.

Pipeline:
  1. Stockfish (d4) plays itself for N games
  2. Every 3 plies, sample the current position
  3. Evaluate each sampled position at depth 10
  4. Save to positions_selfplay.jsonl

Controls:
  - auto_play_depth=4:  play strength ≈ 1600 Elo (coherent but imperfect)
  - eval_depth=10:       ground truth evaluation accuracy
"""

import json
import subprocess
import sys
import time
import random

# ─── Configuration ──────────────────────────────────────────────────────────

STOCKFISH_PATH = "/home/shiba/the-machine/stockfish"
OUTPUT_PATH = "/home/shiba/the-machine/positions_selfplay.jsonl"

NUM_POSITIONS = 10_000
AUTO_PLAY_DEPTH = 4   # playing strength (coherent, not random)
EVAL_DEPTH = 10       # ground truth evaluation precision
MAX_PLIES = 60        # max plies per game (~30 moves each)

SAMPLE_INTERVAL = 2   # sample position every N plies
SEED = 42

# ─── Stockfish Wrapper ──────────────────────────────────────────────────────


class StockfishEngine:
    """UCI interface for both playing and evaluating."""

    def __init__(self, path: str):
        self.proc = subprocess.Popen(
            [path],
            universal_newlines=True,
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.DEVNULL,
            bufsize=1,
        )
        self._handshake()

    def _read_until(self, target: str, timeout: float = 30.0) -> str:
        start = time.time()
        while time.time() - start < timeout:
            line = self.proc.stdout.readline()
            if not line:
                time.sleep(0.01)
                continue
            line = line.strip()
            if target in line:
                return line
        raise TimeoutError(f"No '{target}' within {timeout}s")

    def _handshake(self):
        self.proc.stdin.write("uci\n")
        self.proc.stdin.flush()
        self._read_until("uciok")
        self.proc.stdin.write("isready\n")
        self.proc.stdin.flush()
        self._read_until("readyok")
        self.proc.stdin.write("setoption name Threads value 2\n")
        self.proc.stdin.flush()
        self.proc.stdin.write("setoption name Hash value 64\n")
        self.proc.stdin.flush()

    def evaluate(self, fen: str, depth: int = 10) -> float:
        """Return eval in pawns from white's perspective."""
        self.proc.stdin.write(f"position fen {fen}\n")
        self.proc.stdin.flush()
        self.proc.stdin.write(f"go depth {depth}\n")
        self.proc.stdin.flush()
        score = 0.0
        while True:
            line = self.proc.stdout.readline()
            if "bestmove" in line:
                break
            if "score cp" in line:
                parts = line.split()
                for i, p in enumerate(parts):
                    if p == "cp":
                        score = int(parts[i + 1]) / 100.0
            elif "score mate" in line:
                parts = line.split()
                for i, p in enumerate(parts):
                    if p == "mate":
                        mate_in = int(parts[i + 1])
                        score = 100.0 if mate_in > 0 else -100.0
        return score

    def best_move(self, fen: str, depth: int = 4) -> str:
        """Get Stockfish's best move given a FEN."""
        self.proc.stdin.write(f"position fen {fen}\n")
        self.proc.stdin.flush()
        self.proc.stdin.write(f"go depth {depth}\n")
        self.proc.stdin.flush()
        bestmove = ""
        while True:
            line = self.proc.stdout.readline()
            if "bestmove" in line:
                bestmove = line.strip()
                break
        # Parse "bestmove e2e4 ponder c7c5"
        parts = bestmove.split()
        if len(parts) >= 2:
            return parts[1]
        return ""

    def make_move(self, fen: str, move: str) -> str:
        """Apply a move to a FEN, return new FEN."""
        # Use Stockfish's "position fen ... moves ..." then "d" to get FEN
        self.proc.stdin.write(f"position fen {fen} moves {move}\n")
        self.proc.stdin.flush()
        self.proc.stdin.write("d\n")
        self.proc.stdin.flush()
        fen_line = ""
        while True:
            line = self.proc.stdout.readline()
            if "Fen:" in line:
                fen_line = line.strip()
                break
        # Parse "Fen: rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq - 0 1"
        return fen_line.replace("Fen: ", "")

    def close(self):
        self.proc.stdin.write("quit\n")
        self.proc.stdin.flush()
        self.proc.wait(timeout=5)


def classify_phase(fen: str) -> str:
    """Count pieces to classify phase."""
    board = fen.split()[0]
    count = sum(1 for ch in board if ch.isalpha())
    if count > 24:
        return "opening"
    elif count > 10:
        return "middlegame"
    else:
        return "endgame"


# ─── Self-Play Game ─────────────────────────────────────────────────────────


def play_game(engine: StockfishEngine, max_plies: int = 80,
              sample_interval: int = 3) -> list:
    """Play a self-play game and return sampled (fen, white_move) pairs."""
    samples = []
    fen = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1"

    side_to_move = 0  # 0=white, 1=black
    ply = 0

    while ply < max_plies:
        # Sample this position
        samples.append(fen)

        # Get move
        move = engine.best_move(fen, depth=AUTO_PLAY_DEPTH)
        if not move or move == "(none)":
            break

        # Make move
        fen = engine.make_move(fen, move)
        ply += 1
        side_to_move = 1 - side_to_move

        # Skip future samples by interval
        # We add all samples above and deduplicate later

    return samples


# ─── Main ────────────────────────────────────────────────────────────────────


def main():
    random.seed(SEED)

    print("Starting Stockfish...", file=sys.stderr)
    engine = StockfishEngine(STOCKFISH_PATH)
    print(f"Stockfish ready. Generating {NUM_POSITIONS} positions from "
          f"self-play at depth {AUTO_PLAY_DEPTH}...", file=sys.stderr)

    all_positions = []  # (fen, white_move_flag)
    games_played = 0
    start_time = time.time()

    while len(all_positions) < NUM_POSITIONS:
        # Play a game
        game_positions = play_game(engine, max_plies=MAX_PLIES,
                                    sample_interval=SAMPLE_INTERVAL)
        games_played += 1

        # Sample at regular intervals
        sampled = game_positions[0::SAMPLE_INTERVAL]  # every Nth position

        # Add moves played after each sample
        for fen in sampled:
            if len(all_positions) >= NUM_POSITIONS:
                break
            all_positions.append(fen)

        if games_played % 100 == 0:
            elapsed = time.time() - start_time
            rate = len(all_positions) / elapsed if elapsed > 0 else 0
            print(
                f"  {len(all_positions)}/{NUM_POSITIONS} positions "
                f"from {games_played} games ({rate:.1f} pos/sec)",
                file=sys.stderr,
            )

    # Deduplicate
    all_positions = list(dict.fromkeys(all_positions))  # preserve order, unique
    print(f"\n{len(all_positions)} unique positions from {games_played} games",
          file=sys.stderr)

    # Evaluate each position
    print(f"Evaluating at depth {EVAL_DEPTH}...", file=sys.stderr)
    eval_start = time.time()

    with open(OUTPUT_PATH, "w") as f:
        for i, fen in enumerate(all_positions):
            phase = classify_phase(fen)

            try:
                eval_score = engine.evaluate(fen, depth=EVAL_DEPTH)
            except Exception as e:
                print(f"  Eval error: {e}", file=sys.stderr)
                continue

            record = {"fen": fen, "eval": eval_score, "phase": phase}
            f.write(json.dumps(record) + "\n")

            if (i + 1) % 1000 == 0:
                print(f"  Evaluated {i+1}/{len(all_positions)}", file=sys.stderr)

    engine.close()
    elapsed = time.time() - start_time
    print(f"\nDone in {elapsed:.0f}s:", file=sys.stderr)
    print(f"  {len(all_positions)} positions", file=sys.stderr)
    print(f"  {games_played} self-play games", file=sys.stderr)
    print(f"  Output: {OUTPUT_PATH}", file=sys.stderr)


if __name__ == "__main__":
    main()
