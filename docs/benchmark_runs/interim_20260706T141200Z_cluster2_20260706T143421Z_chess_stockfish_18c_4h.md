# Interim results 2026-07-06T14:42:31Z

## Cognition Cluster 2 Completed Outputs

- Cluster: `20260706T141200Z_cluster2`
- JSONL files completed: 40
- Parsed rows: 72
- Passed rows: 72
- Failed rows: 0

| Experiment | Rows | Accuracy avg | Avg latency avg | Max latency avg |
| --- | ---: | ---: | ---: | ---: |
| memory-pressure | 32 | 1 | 4772.13 ms | 477212.74 ms |
| qa-depth-10 | 8 | 1 | 94.04 ms | 94.04 ms |
| qa-depth-100 | 8 | 1 | 4795.06 ms | 4795.06 ms |
| qa-depth-25 | 8 | 1 | 419.13 ms | 419.13 ms |
| qa-depth-250 | 8 | 1 | 28443.56 ms | 28443.56 ms |
| qa-depth-50 | 8 | 1 | 1345.82 ms | 1345.82 ms |

## Chess Stockfish Interim

- Run: `20260706T143421Z_chess_stockfish_18c_4h`
- JSONL rows: 237
- Workers reporting: 18
- Latest cumulative games: 2360
- Latest cumulative plies: 299534
- First progress-window interval agreement: 0.2478
- Latest progress-window interval agreement: 0.2580
- Latest average cumulative agreement: 0.2563

## Interpretation

- Completed cognition rows still show perfect correctness, but depth and memory latency degrade sharply under saturation.
- Cluster 2 has not yet produced adaptation JSONL outputs; those are still the important long-running scaling signal.
- The chess run shows a weak but measurable early improvement in Stockfish move agreement; the full 4-hour curve is needed before treating it as learning rather than noise.
