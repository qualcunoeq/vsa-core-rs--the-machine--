# Controlled autonomy (Phase 10)

Earlier phases made conversation dependable: it answers, remembers, learns from
documents, and evaluates itself. Phase 10 lets the conversation **initiate
useful background work** — but only work that is bounded, observable, and
explainable.

The guiding rule: **a task may do only what it declared, and must be able to
explain what it did.** Autonomy without a declared scope is not allowed; a task
that wants more than it declared is stopped rather than trusted.

## The five tasks

| Task | What it does | Reused infrastructure |
|---|---|---|
| Analyze a document | Extract proposed knowledge and surface refused instructions | `document_learning::extract_text` |
| Investigate a question | Run a bounded, replay-verified research case | `mathematical_research::run_case` |
| Run a selected experiment | Choose a portfolio under a budget (planning only) | `capability_planner` selection |
| Consolidate memory | Account for the conversation's resident footprint | `reliability::account_conversation` |
| Produce a report | Summarize a completed task's progress and result | the task layer itself |

The task layer adds scope, budget, progress, cancellation, and a completion
condition. It does not add new execution: each kind is a thin adapter over code
that already exists and is already tested.

## Scope and budget

Every task declares two things up front:

* an **`AuthorityScope`** — an allowlist of `Capability` values it may use. The
  enum has no `Shell`, no `Network`, and no `Simulation` variant, so those
  authorities cannot even be expressed.
* a **`TaskBudget`** — a `cognition::AutonomyBudget` (actions, wall-clock,
  external writes, maximum risk) plus a step cap.

`AuthorityScope::for_kind` grants exactly what a kind needs and no more. The
scope is checked twice:

1. **at planning** — if the budget does not cover the capabilities the kind
   requires, planning is refused;
2. **at each step** — if the capability about to run is outside the scope, the
   task is refused *before* the capability is invoked.

That second check is what makes "without exceeding its declared authority"
literal: nothing unauthorized ever runs, so the run stays within authority even
when it is stopped for wanting more. The same is true of the budget: a step is
charged only after it is permitted, and a task that would exceed its step or
resource budget is refused, not truncated.

## Lifecycle

```
Pending ──start──▶ Running ──step*──▶ Completed
                     │  ▲
              pause  │  │ resume
                     ▼  │
                   Paused
                     │
              cancel ▼
                 Cancelled          Refused (budget/authority breach)
```

The runner is deterministic and stepwise: `start`, then repeated `step` (one
capability use each) or `run`. There are no OS threads and no timers, so
cancellation, pausing, and resuming are exact and replayable. `pause` and
`resume` continue from the next step — a paused task keeps its progress and
never repeats a step. Every step appends a `TaskProgress` record.

The completion condition is exact: the kind's ordered step plan is exhausted and
a result exists. A **refusal is done but not complete** — it is a controlled
stop, and the two are kept distinct so a stopped task is never reported as a
success.

## Explanation

`TaskReport` explains a run: its state, steps taken, budget spent, whether it
stayed within budget and authority, whether it was refused, the ordered progress
log, and the result. `TaskResult::explain` gives a one-line human summary of
each result. `machine_eval task` runs the canonical suite and writes
`docs/phase10_autonomy_v1.report.json` / `.md`.

## Keeping chat independent

The task layer never reaches a shell, a network, or the simulation loop. Two
hosts implement the `TaskHost` seam:

* `DefaultTaskHost` reaches only the read-only and planning-only library modules;
* `ConversationTaskHost` reads the conversation's own sessions and turns, so a
  consolidation task observes the live chat state through public accessors.

The conversation initiates the work by handing its state to a host; it does not
embed a scheduler, a thread pool, or an executor. A document analysis is
read-only and never commits knowledge: committing remains the explicit,
human-accepted step of the document workflow.

## Result

Measured by `cargo run --bin machine_eval task` on the canonical suite:

* six scenarios, five complete and one refused (the budget-starved scenario),
  all within authority and budget;
* one scenario pauses after a step and resumes to completion, proving the
  stop/resume path;
* every completed run explains its result; the starved run explains its refusal.

## Boundaries

* Tasks are read-only or planning-only; none of the five mutates persistent
  knowledge.
* The suite is deterministic; there is no ambient scheduler and no wall-clock
  dependence in the reported outcomes.
* Resume continues; it never restarts from scratch.
* A host error becomes a controlled refusal with a reason, not a stuck task.
