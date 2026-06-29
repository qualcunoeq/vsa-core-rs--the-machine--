#!/usr/bin/env python3
"""
Generate 10,000 chess positions with Stockfish evaluations for Phase 1
feasibility test: "Does VSA centroid similarity capture chess position similarity?"
"""

import json
import random
import subprocess
import sys
import time

import chess
import chess.pgn

# ─── Configuration ──────────────────────────────────────────────────────────

STOCKFISH_PATH = "/home/shiba/the-machine/stockfish"
NUM_POSITIONS = 10_000
OUTPUT_PATH = "/home/shiba/the-machine/positions.jsonl"
STOCKFISH_DEPTH = 10
SEED = 42
MIN_PLY = 6
MAX_PLY = 80

# ─── Stockfish Interface ────────────────────────────────────────────────────


class StockfishEval:
    """Thin wrapper around Stockfish UCI protocol for position evaluation."""

    def __init__(self, path: str, depth: int = 10):
        self.depth = depth
        self.proc = subprocess.Popen(
            [path],
            universal_newlines=True,
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.DEVNULL,
            bufsize=1,
        )
        # Read all startup lines until uciok
        self._uci_handshake()

    def _read_until(self, target: str, timeout: float = 30.0) -> str:
        """Read lines until target substring appears, return last line."""
        start = time.time()
        while time.time() - start < timeout:
            line = self.proc.stdout.readline()
            if not line:
                time.sleep(0.01)
                continue
            line = line.strip()
            if target in line:
                return line
        raise TimeoutError(f"Did not see '{target}' within {timeout}s")

    def _uci_handshake(self):
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

    def evaluate(self, fen: str) -> float:
        """Return evaluation in pawn units (positive = white advantage)."""
        self.proc.stdin.write(f"position fen {fen}\n")
        self.proc.stdin.flush()
        self.proc.stdin.write(f"go depth {self.depth}\n")
        self.proc.stdin.flush()

        score = None
        while True:
            line = self.proc.stdout.readline()
            if not line:
                continue
            line = line.strip()
            if line.startswith("bestmove"):
                break
            # Parse "score cp N" or "score mate N"
            if "score cp" in line:
                # Extract the cp value
                parts = line.split()
                for i, p in enumerate(parts):
                    if p == "cp" and i + 1 < len(parts):
                        score = int(parts[i + 1]) / 100.0
                        break
            elif "score mate" in line:
                parts = line.split()
                for i, p in enumerate(parts):
                    if p == "mate" and i + 1 < len(parts):
                        mate_in = int(parts[i + 1])
                        score = 100.0 if mate_in > 0 else -100.0
                        break

        return score if score is not None else 0.0

    def close(self):
        self.proc.stdin.write("quit\n")
        self.proc.stdin.flush()
        self.proc.wait(timeout=5)


# ─── Position Generation ────────────────────────────────────────────────────


def classify_phase(board: chess.Board) -> str:
    total_pieces = sum(1 for _ in board.piece_map())
    if total_pieces > 24:
        return "opening"
    elif total_pieces > 10:
        return "middlegame"
    else:
        return "endgame"


def random_position(depth_range=(6, 80)) -> chess.Board:
    board = chess.Board()
    ply = random.randint(*depth_range)
    for _ in range(ply):
        legal = list(board.legal_moves)
        if not legal:
            break
        move = random.choice(legal)
        board.push(move)
        if board.is_game_over():
            break
    return board


# ─── Main ────────────────────────────────────────────────────────────────────


def main():
    random.seed(SEED)

    print(f"Starting Stockfish (depth={STOCKFISH_DEPTH})...", file=sys.stderr)
    sf = StockfishEval(STOCKFISH_PATH, depth=STOCKFISH_DEPTH)
    print("Stockfish ready.", file=sys.stderr)

    positions_generated = 0
    attempts = 0
    start_time = time.time()

    with open(OUTPUT_PATH, "w") as f:
        while positions_generated < NUM_POSITIONS:
            attempts += 1
            board = random_position(depth_range=(MIN_PLY, MAX_PLY))
            fen = board.fen()
            phase = classify_phase(board)

            try:
                eval_score = sf.evaluate(fen)
            except Exception as e:
                print(f"  Error: {e}", file=sys.stderr)
                continue

            record = {"fen": fen, "eval": eval_score, "phase": phase}
            f.write(json.dumps(record) + "\n")
            positions_generated += 1

            if positions_generated % 500 == 0:
                elapsed = time.time() - start_time
                rate = positions_generated / elapsed
                print(
                    f"  {positions_generated}/{NUM_POSITIONS} "
                    f"({rate:.1f} pos/sec, {elapsed:.0f}s elapsed)",
                    file=sys.stderr,
                )

    sf.close()
    elapsed = time.time() - start_time
    print(f"\nDone: {positions_generated} positions in {elapsed:.0f}s", file=sys.stderr)
    print(f"Output: {OUTPUT_PATH}", file=sys.stderr)

    # Stats
    evals = []
    phases = {"opening": 0, "middlegame": 0, "endgame": 0}
    with open(OUTPUT_PATH) as f:
        for line in f:
            rec = json.loads(line)
            evals.append(rec["eval"])
            phases[rec["phase"]] += 1

    print(f"Eval range: [{min(evals):.2f}, {max(evals):.2f}]", file=sys.stderr)
    print(f"Eval mean:  {sum(evals)/len(evals):.2f}", file=sys.stderr)
    print(f"Phases: {phases}", file=sys.stderr)


if __name__ == "__main__":
    main()
