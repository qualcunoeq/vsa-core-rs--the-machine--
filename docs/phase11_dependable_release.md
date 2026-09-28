# Dependable release (Phase 11)

Ten phases built a runtime that answers, remembers, learns from documents,
evaluates itself, and can initiate bounded work. Phase 11 makes that runtime
**operable**: someone who has never read a research document can install it, use
it daily, upgrade it, and recover from a bad upgrade without memorizing
research-specific commands.

The guiding rule: **one command for everyday use, and an honest answer for every
question an operator asks.** Nothing here moves reasoning logic; it is a thin
operator layer (`src/operator.rs` + `src/bin/machine.rs`) over the existing
service, and the repository stays a single crate because build-time and
dependency boundaries do not yet justify a split.

## One command

```bash
cargo run --release --bin machine -- setup   # once
cargo run --release --bin machine -- start   # daily
```

Every operator task is a subcommand of `machine`: `setup`, `start`, `doctor`,
`config check|show`, `version`, `capabilities`, `release notes|inventory|write`,
`backup`, `restore`, `upgrade`, `recover`. `machine_chat` remains for
research-only flag parity, but it is no longer the documented path.

## First-run setup

`setup` creates the data and backup directories, opens the database once (which
runs migrations), prints the resulting schema and integrity, runs the doctor, and
prints the exact next command. It is idempotent and safe to re-run.

## Configuration validation

Configuration is built from `MACHINE_*` environment variables and flags.
`validate` returns a `ConfigIssue` per problem — severity, field, message, and a
**remedy** — so a misconfiguration is a fixable message rather than a confusing
failure later. It catches, among others: a zero port, empty paths, a
non-loopback bind without `--allow-remote` and a token, the `gpu` profile on a
CPU-only binary, the `semantic_worker` profile with the worker disabled, a tiny
memory budget, and a database placed outside the data directory.

## Health and dependency status

`machine doctor` reports the release and schema, profile, active projection
path, bundled SQLite and pdf-extract backends, data-directory writability,
database integrity and schema compatibility (inspected read-only), available
legacy imports, CUDA/profile match, semantic-worker state, port availability, and
every configuration issue. Each check is `ok` / `warn` / `fail` with a detail and
an optional remedy; the report is healthy only when nothing fails.

## Versioned capability inventory

`CAPABILITIES` (`phase11-capability-inventory-v1`, release `v3.4`) records for
each capability its id, title, status (`stable` / `active` / `experimental` /
`shadow` / `deferred`), the phase that introduced it, the claim id in
`docs/CLAIMS.md` when one exists, the committed evaluation artifacts that back
it, the command that exercises it, and its surfaces. `machine capabilities` and
`docs/phase11_capability_inventory_v1.json` expose it.

## Release notes tied to evaluation results

`release_notes()` reads the committed evaluation reports and states what was
actually measured — guarantee counts and projection path, conversation-eval
regression/holdout coverage and ablation count, the semantic-fidelity
wrong-answer limit, and the autonomy suite's scenario counts. A missing artifact
is reported as missing, with the command to regenerate it, rather than replaced
by a claim. `machine release write` regenerates the notes, the inventory, and the
doctor report.

## Optional GPU and semantic-worker profiles

Profiles are documented configurations, not separate builds. `standard` is the
default; `minimal` omits the web interface; `gpu` expects a `cargo build
--features cuda` binary; `semantic_worker` enables the shadow worker. `config
check` and `doctor` report when the running binary does not match the requested
profile, so a profile can never quietly overstate what is present.

## Backup, restore, upgrade, recovery

`machine upgrade` refuses a database newer than the build *before touching it*,
takes a timestamped pre-upgrade backup, opens the database (migrating), and
verifies integrity and the resulting schema. `machine recover` restores the most
recent backup and verifies it. Backups use SQLite's online backup API without
opening or migrating the source, so they are safe on a live database. This is the
enforced guarantee `G-RELEASE-SCHEMA`.

## Result

Verified by `release_packaging_validates_health_inventory_and_upgrade_recovery`
and the `operator` unit tests:

* the default configuration validates and a non-loopback bind without opt-in and
  a token is rejected, each with a remedy;
* the inventory is versioned and the release notes reference committed
  evaluation artifacts;
* setup migrates a fresh database, and the doctor is healthy;
* an upgrade takes a backup first, restores via `recover`, and refuses a forged
  newer database without modifying it;
* an upgrade failure leaves both the pre-upgrade backup and the recovery command
  available.

## Boundaries

* The operator layer never starts a server except in `start`, and never performs
  reasoning; it validates, reports, and moves files.
* `doctor` inspects the database read-only; it never migrates or repairs.
* Profiles do not change the build at runtime; they are validated against what
  the binary actually is.
* A recovery needs a backup to exist; if none does, `recover` says so and asks
  for an explicit `restore <file>` rather than inventing a path.
