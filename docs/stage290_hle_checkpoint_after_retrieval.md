# Stage 290 — post-retrieval HLE checkpoint

A clean release-candidate evaluation after Stage 289 retrieval-guided investigation. HLE outcomes are used only by the terminal scorer, never by routing or curriculum selection.

* cases: 2500
* correct authorized: 0
* incorrect authorized / false authorization: 0 / 0
* curriculum signals / pack invocations: 663 / 0
* replay compatibility / not applicable / not recorded: 0 / 2500 / 0
* worktree clean: true
* runtime math cache present / SHA-256: true / Some("0d2ccd02ca9fd0a8d5a963defd89fe3947cf0c73d6dcc878164dc81d6408be12")
* registry / curriculum mutation: false / false
* HLE outcomes used for routing: false
* trace: `/home/shiba/the-machine/docs/stage290_hle_checkpoint_after_retrieval.trace.jsonl`

Dataset SHA-256: `31b26cc8e352af16bedb9a714feb788ae562be38898ab92dc54b4665882bf1c6`
Curriculum manifest SHA-256: `5be43e121500a591b8b380a029a155c8cdafa657b97bbf4756176d39c6560bc8`
Stage 289 retrieval report SHA-256: `cb2471d22776cd2ca3b16837d75eea7e4798c70aeee55fbe7ff8975293a1cf36`

Reproduce with `cargo run --quiet --bin stage290_hle_checkpoint_after_retrieval`.
