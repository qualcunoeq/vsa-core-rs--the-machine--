# Chess Stockfish self-play run 20260706T143421Z_chess_stockfish_18c_4h

## Purpose

This is a 4-hour auxiliary benchmark to test whether a chess-specific bit encoder plus a simple online student policy can show a measurable learning curve against Stockfish guidance.

## Encoder

The encoder is sparse and chess-specific:

- 12 piece occupancy planes over 64 squares.
- Side-to-move bit.
- Castling rights bits.
- En-passant file bits.
- Material/phase bucket.
- Move origin and destination bits.
- Promotion, capture, check, and moved-piece bits.
- Resulting-board bits after the candidate move.

## Learning Signal

Each worker plays self-play games using its own online linear policy. At every ply, Stockfish supplies a teacher move. If the student move differs, the model receives a pairwise update that raises the Stockfish move features and lowers the chosen student move features.

## Metrics

The JSONL output records:

- cumulative and interval agreement rate against Stockfish moves.
- cumulative and interval pairwise loss.
- games and plies completed.
- learned sparse weight count.
- last game result score.
- exploration epsilon.

## Runtime

- Workers: 18
- CPU affinity: cores 0-17 via taskset
- Duration cap: 4 hours
- Stockfish threads per worker: 1
- Stockfish teacher time per ply: 20 ms

## Artifacts

- Results: `results/chess_stockfish/20260706T143421Z_chess_stockfish_18c_4h/selfplay.jsonl`
- Log: `logs/20260706T143421Z_chess_stockfish_18c_4h/chess_stockfish_selfplay.log`
- PID file: `logs/20260706T143421Z_chess_stockfish_18c_4h/chess_stockfish_selfplay.pid`
