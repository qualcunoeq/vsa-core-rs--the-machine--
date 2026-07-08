#!/usr/bin/env python3
"""Stockfish-guided chess self-play benchmark with a chess-specific bit encoder.

This is intentionally an experiment, not production model code. It measures whether a
simple online bitwise linear policy can improve its move agreement with Stockfish over
long self-play runs.
"""
from __future__ import annotations

import argparse
import json
import math
import multiprocessing as mp
import os
import random
import signal
import time
from dataclasses import dataclass
from pathlib import Path
from typing import Iterable

import chess
import chess.engine

PIECE_ORDER = [
    chess.PAWN,
    chess.KNIGHT,
    chess.BISHOP,
    chess.ROOK,
    chess.QUEEN,
    chess.KING,
]
PIECE_TO_OFFSET = {piece: i for i, piece in enumerate(PIECE_ORDER)}
MAX_PLIES = 160


def board_bits(board: chess.Board) -> set[int]:
    """Encode board state as sparse bit indexes.

    Layout:
    - 12 piece planes * 64 squares: color-major piece occupancy.
    - side-to-move bit.
    - 4 castling-right bits.
    - 8 en-passant file bits.
    - lightweight phase/material buckets.
    """
    bits: set[int] = set()
    for square, piece in board.piece_map().items():
        color_offset = 0 if piece.color == chess.WHITE else 6
        plane = color_offset + PIECE_TO_OFFSET[piece.piece_type]
        bits.add(plane * 64 + square)

    base = 12 * 64
    if board.turn == chess.WHITE:
        bits.add(base)
    if board.has_kingside_castling_rights(chess.WHITE):
        bits.add(base + 1)
    if board.has_queenside_castling_rights(chess.WHITE):
        bits.add(base + 2)
    if board.has_kingside_castling_rights(chess.BLACK):
        bits.add(base + 3)
    if board.has_queenside_castling_rights(chess.BLACK):
        bits.add(base + 4)
    if board.ep_square is not None:
        bits.add(base + 5 + chess.square_file(board.ep_square))

    material = 0
    values = {
        chess.PAWN: 1,
        chess.KNIGHT: 3,
        chess.BISHOP: 3,
        chess.ROOK: 5,
        chess.QUEEN: 9,
    }
    for piece in board.piece_map().values():
        material += values.get(piece.piece_type, 0)
    phase_bucket = min(7, max(0, material // 6))
    bits.add(base + 13 + phase_bucket)
    return bits


def move_bits(board: chess.Board, move: chess.Move) -> set[int]:
    """Encode a candidate move plus the resulting board as sparse bit indexes."""
    bits = board_bits(board)
    base = 12 * 64 + 21
    bits.add(base + move.from_square)
    bits.add(base + 64 + move.to_square)
    if move.promotion:
        bits.add(base + 128 + PIECE_TO_OFFSET[move.promotion])
    if board.is_capture(move):
        bits.add(base + 134)
    if board.gives_check(move):
        bits.add(base + 135)
    moved = board.piece_at(move.from_square)
    if moved:
        color_offset = 0 if moved.color == chess.WHITE else 6
        bits.add(base + 136 + color_offset + PIECE_TO_OFFSET[moved.piece_type])

    board.push(move)
    for b in board_bits(board):
        bits.add(base + 148 + b)
    board.pop()
    return bits


@dataclass
class Student:
    weights: dict[int, float]
    lr: float
    l2: float

    def score(self, features: Iterable[int]) -> float:
        return sum(self.weights.get(i, 0.0) for i in features)

    def update_pairwise(self, good: set[int], bad: set[int]) -> float:
        margin = self.score(good) - self.score(bad)
        loss = math.log1p(math.exp(-max(-50.0, min(50.0, margin))))
        grad = 1.0 / (1.0 + math.exp(max(-50.0, min(50.0, margin))))
        touched = good | bad
        for i in touched:
            delta = 0.0
            if i in good:
                delta += self.lr * grad
            if i in bad:
                delta -= self.lr * grad
            old = self.weights.get(i, 0.0)
            new = old * (1.0 - self.lr * self.l2) + delta
            if abs(new) < 1e-9:
                self.weights.pop(i, None)
            else:
                self.weights[i] = new
        return loss


def choose_student_move(student: Student, board: chess.Board, rng: random.Random, epsilon: float) -> chess.Move:
    legal = list(board.legal_moves)
    if not legal:
        raise RuntimeError('no legal moves')
    if rng.random() < epsilon:
        return rng.choice(legal)
    scored = [(student.score(move_bits(board, m)), rng.random(), m) for m in legal]
    scored.sort(reverse=True, key=lambda x: (x[0], x[1]))
    return scored[0][2]


def result_score(board: chess.Board) -> float:
    outcome = board.outcome(claim_draw=True)
    if outcome is None or outcome.winner is None:
        return 0.5
    return 1.0 if outcome.winner == chess.WHITE else 0.0


def worker_main(worker_id: int, args: argparse.Namespace, stop_at: float, out_q: mp.Queue) -> None:
    rng = random.Random(args.seed + worker_id * 1000003)
    student = Student(weights={}, lr=args.lr, l2=args.l2)
    engine = chess.engine.SimpleEngine.popen_uci(args.stockfish)
    engine.configure({'Threads': 1, 'Hash': args.hash_mb})
    games = 0
    plies = 0
    agreements = 0
    losses = []
    interval_games = 0
    interval_plies = 0
    interval_agreements = 0
    interval_losses = []
    started = time.time()
    try:
        while time.time() < stop_at:
            board = chess.Board()
            game_plies = 0
            while not board.is_game_over(claim_draw=True) and game_plies < args.max_plies and time.time() < stop_at:
                legal = list(board.legal_moves)
                if not legal:
                    break
                teacher = engine.play(board, chess.engine.Limit(time=args.teacher_time)).move
                epsilon = max(args.min_epsilon, args.epsilon * (args.epsilon_decay ** games))
                student_move = choose_student_move(student, board, rng, epsilon)
                if teacher == student_move:
                    agreements += 1
                    interval_agreements += 1
                else:
                    teacher_features = move_bits(board, teacher)
                    student_features = move_bits(board, student_move)
                    loss = student.update_pairwise(teacher_features, student_features)
                    losses.append(loss)
                    interval_losses.append(loss)
                board.push(student_move)
                plies += 1
                interval_plies += 1
                game_plies += 1
            games += 1
            interval_games += 1
            if interval_games >= args.report_games:
                now = time.time()
                out_q.put({
                    'kind': 'progress',
                    'worker': worker_id,
                    'elapsed_sec': now - started,
                    'games': games,
                    'plies': plies,
                    'interval_games': interval_games,
                    'interval_plies': interval_plies,
                    'agreement_rate': agreements / max(1, plies),
                    'interval_agreement_rate': interval_agreements / max(1, interval_plies),
                    'avg_loss': sum(losses) / max(1, len(losses)),
                    'interval_avg_loss': sum(interval_losses) / max(1, len(interval_losses)),
                    'weight_count': len(student.weights),
                    'last_result_score': result_score(board),
                    'epsilon': epsilon,
                })
                interval_games = 0
                interval_plies = 0
                interval_agreements = 0
                interval_losses = []
    except Exception as exc:
        out_q.put({'kind': 'worker_error', 'worker': worker_id, 'error': repr(exc)})
    finally:
        try:
            engine.quit()
        except Exception:
            pass
        out_q.put({
            'kind': 'worker_done',
            'worker': worker_id,
            'games': games,
            'plies': plies,
            'agreement_rate': agreements / max(1, plies),
            'avg_loss': sum(losses) / max(1, len(losses)),
            'weight_count': len(student.weights),
        })


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument('--stockfish', default='/usr/games/stockfish')
    parser.add_argument('--out', required=True)
    parser.add_argument('--workers', type=int, default=18)
    parser.add_argument('--duration-minutes', type=float, default=240.0)
    parser.add_argument('--teacher-time', type=float, default=0.02)
    parser.add_argument('--hash-mb', type=int, default=32)
    parser.add_argument('--max-plies', type=int, default=MAX_PLIES)
    parser.add_argument('--report-games', type=int, default=10)
    parser.add_argument('--seed', type=int, default=20260706)
    parser.add_argument('--lr', type=float, default=0.04)
    parser.add_argument('--l2', type=float, default=0.000001)
    parser.add_argument('--epsilon', type=float, default=0.35)
    parser.add_argument('--epsilon-decay', type=float, default=0.9995)
    parser.add_argument('--min-epsilon', type=float, default=0.05)
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    stop_at = time.time() + args.duration_minutes * 60.0
    out = Path(args.out)
    out.parent.mkdir(parents=True, exist_ok=True)
    q: mp.Queue = mp.Queue()
    procs = [mp.Process(target=worker_main, args=(i, args, stop_at, q)) for i in range(args.workers)]
    for p in procs:
        p.start()
    done = 0
    totals = {'games': 0, 'plies': 0}
    with out.open('a', buffering=1) as f:
        f.write(json.dumps({
            'kind': 'run_start',
            'started_at': time.time(),
            'workers': args.workers,
            'duration_minutes': args.duration_minutes,
            'encoder': 'sparse chess bit encoder: 12x64 occupancy + side/castling/ep/phase + move/resulting-board features',
            'teacher': 'stockfish',
            'teacher_time_sec': args.teacher_time,
        }, sort_keys=True) + '\n')
        while done < len(procs):
            msg = q.get()
            msg['timestamp'] = time.time()
            f.write(json.dumps(msg, sort_keys=True) + '\n')
            if msg.get('kind') == 'worker_done':
                done += 1
                totals['games'] += int(msg.get('games', 0))
                totals['plies'] += int(msg.get('plies', 0))
        f.write(json.dumps({'kind': 'run_done', 'timestamp': time.time(), **totals}, sort_keys=True) + '\n')
    for p in procs:
        p.join(timeout=5)
    return 0


if __name__ == '__main__':
    signal.signal(signal.SIGINT, signal.SIG_DFL)
    raise SystemExit(main())
