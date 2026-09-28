//! Phase 10 — controlled autonomy.
//!
//! Earlier phases made the machine able to answer, remember, learn from
//! documents, and evaluate itself. Phase 10 lets the conversation **initiate
//! useful background work** after answering and memory are dependable — but
//! only work that is bounded, observable, and explainable.
//!
//! The design keeps three properties that the goal names explicitly:
//!
//! * **Every task declares its authority and budget up front.** A [`TaskPlan`]
//!   carries a [`TaskBudget`] (actions, wall-clock, external writes, maximum
//!   risk — reusing [`crate::cognition::AutonomyBudget`]) and an
//!   [`AuthorityScope`] naming exactly which capabilities the task may call.
//! * **Every task is observable and stoppable.** The runner is deterministic
//!   and stepwise: [`TaskRunner::start`] then repeated [`TaskRunner::step`]
//!   (or [`TaskRunner::run`]), with cooperative [`TaskRunner::cancel`] and
//!   [`TaskRunner::pause`] / [`TaskRunner::resume`]. Each step appends a
//!   [`TaskProgress`] record, so a stopped or resumed task carries its history
//!   with it.
//! * **Nothing new executes.** Each task kind is a thin adapter over
//!   infrastructure that already exists and is already tested
//!   ([`crate::document_learning`], [`crate::mathematical_research`],
//!   [`crate::capability_planner`], [`crate::reliability`], and the
//!   conversation memory), reached through the [`TaskHost`] seam. There is no
//!   shell, no network, no full simulation loop: the chat interaction is
//!   deliberately independent of those.
//!
//! A task is complete when it runs, stops, resumes where appropriate, and can
//! explain its result without exceeding its declared authority or budget. The
//! [`TaskReport`] is that explanation.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::cognition::AutonomyBudget;
use crate::document_learning::{self, DocumentProposal};
use crate::mathematical_research::{self, ResearchCase, ResearchConclusion, ResearchReceipt};
use crate::reliability::MemoryReport;

/// Schema tag for serialized task reports.
pub const TASK_SCHEMA: &str = "phase10-autonomy-task-v1";

/// The five bounded tasks the goal names. Each is backed by an existing
/// capability; the task layer adds scope, budget, progress, cancellation, and a
/// completion condition — it does not add new execution.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskKind {
    /// Analyze an imported document: extract proposed knowledge and surface
    /// refused instructions. Read-only over text already in hand.
    AnalyzeDocument,
    /// Investigate a question using approved sources: run a bounded,
    /// replay-verified mathematical research case.
    Investigate,
    /// Run a selected experiment: ask the capability planner which experiment
    /// to run under a budget and record the selection. Planning only.
    RunExperiment,
    /// Consolidate memory: measure the resident footprint and report whether it
    /// is within the declared budget.
    ConsolidateMemory,
    /// Produce a report: summarize a completed task's own progress and result.
    ProduceReport,
}

impl TaskKind {
    pub fn label(&self) -> &'static str {
        match self {
            TaskKind::AnalyzeDocument => "analyze_document",
            TaskKind::Investigate => "investigate",
            TaskKind::RunExperiment => "run_experiment",
            TaskKind::ConsolidateMemory => "consolidate_memory",
            TaskKind::ProduceReport => "produce_report",
        }
    }
}

/// The capabilities a task may invoke. This is an allowlist: a task that tries
/// to use a capability outside its scope is refused before any work is done,
/// and the refusal is recorded.
///
/// Note what is deliberately absent: there is no `Shell`, no `Network`, and no
/// `Simulation`. Phase 10 exists precisely so that conversation can initiate
/// work *without* those.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Capability {
    /// Read text already supplied to the task (document analysis).
    DocumentRead,
    /// Extract proposed knowledge from that text.
    DocumentExtract,
    /// Run a bounded mathematical investigation over typed inputs.
    MathInvestigate,
    /// Select an experiment with the capability planner (planning only).
    ExperimentPlan,
    /// Read the resident memory accounting.
    MemoryAccount,
    /// Write a report describing a completed task.
    ReportWrite,
}

/// The declared authority of a task: the exact set of capabilities it may use.
///
/// The scope is checked twice — once when the task is planned (does the kind's
/// required capability set fit?) and once per step (is the capability the step
/// is about to use inside the scope?). A mismatch is a
/// [`TaskState::Refused`], never a silent skip.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthorityScope {
    allowed: BTreeSet<Capability>,
}

impl AuthorityScope {
    pub fn new(capabilities: impl IntoIterator<Item = Capability>) -> Self {
        AuthorityScope {
            allowed: capabilities.into_iter().collect(),
        }
    }

    /// A scope permitting nothing. Useful as an explicit "no authority" default.
    pub fn none() -> Self {
        AuthorityScope::new([])
    }

    pub fn permits(&self, capability: Capability) -> bool {
        self.allowed.contains(&capability)
    }

    pub fn capabilities(&self) -> impl Iterator<Item = Capability> + '_ {
        self.allowed.iter().copied()
    }

    pub fn is_empty(&self) -> bool {
        self.allowed.is_empty()
    }

    /// The capabilities the named task kind requires to do its work.
    pub fn required_for(kind: TaskKind) -> BTreeSet<Capability> {
        match kind {
            TaskKind::AnalyzeDocument => {
                [Capability::DocumentRead, Capability::DocumentExtract]
                    .into_iter()
                    .collect()
            }
            TaskKind::Investigate => [Capability::MathInvestigate].into_iter().collect(),
            TaskKind::RunExperiment => [Capability::ExperimentPlan].into_iter().collect(),
            TaskKind::ConsolidateMemory => [Capability::MemoryAccount].into_iter().collect(),
            TaskKind::ProduceReport => [Capability::ReportWrite].into_iter().collect(),
        }
    }

    /// The canonical scope for a task kind: exactly the capabilities it needs,
    /// no more. Constructing a task through [`TaskBudget::for_kind`] uses this,
    /// so a task can never be granted more authority than its kind requires.
    pub fn for_kind(kind: TaskKind) -> Self {
        AuthorityScope {
            allowed: AuthorityScope::required_for(kind),
        }
    }
}

/// A task's declared resource budget together with its declared authority.
///
/// The resource part reuses [`AutonomyBudget`], so task accounting is the same
/// accounting the bounded-autonomy enforcement point already uses. The step cap
/// bounds how many runner steps a task may take, independent of the action
/// count, so a stepwise caller can never be starved or run away.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TaskBudget {
    pub resources: AutonomyBudget,
    /// Maximum number of runner steps (one capability use each).
    pub max_steps: u32,
    pub scope: AuthorityScope,
}

impl TaskBudget {
    pub fn new(resources: AutonomyBudget, max_steps: u32, scope: AuthorityScope) -> Self {
        TaskBudget {
            resources,
            max_steps,
            scope,
        }
    }

    /// The default bounded budget for a task kind: a small step cap, a short
    /// wall-clock window, no external writes, and low maximum risk. The scope
    /// is exactly the capabilities the kind requires.
    pub fn for_kind(kind: TaskKind) -> Self {
        // Time is intentionally generous enough for the in-process adapters,
        // but external writes are zero: none of these tasks may write out.
        let resources = AutonomyBudget::new(16, 30_000, 0, 0.2);
        let max_steps = 16;
        TaskBudget::new(resources, max_steps, AuthorityScope::for_kind(kind))
    }
}

/// A task's lifecycle state. `Paused` and `Cancelled` are the two ways a task
/// stops; only `Paused` can be resumed.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskState {
    Pending,
    Running,
    /// Stopped on purpose, resumable from exactly where it stopped.
    Paused,
    Completed,
    Cancelled,
    /// Refused before doing work, because the task asked for authority or
    /// budget it did not declare — or because a declared contract would be
    /// exceeded. Carries the reason so the result can explain itself.
    Refused { reason: String },
}

impl TaskState {
    pub fn label(&self) -> &'static str {
        match self {
            TaskState::Pending => "pending",
            TaskState::Running => "running",
            TaskState::Paused => "paused",
            TaskState::Completed => "completed",
            TaskState::Cancelled => "cancelled",
            TaskState::Refused { .. } => "refused",
        }
    }

    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            TaskState::Completed | TaskState::Cancelled | TaskState::Refused { .. }
        )
    }

    pub fn is_resumable(&self) -> bool {
        matches!(self, TaskState::Paused)
    }
}

/// The inputs a task works from. Kept as an enum so a task carries only the
/// material its kind needs; nothing is fetched behind the caller's back.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskInput {
    /// Document analysis: title and full text, supplied by the caller (the
    /// conversation already holds it). Imported text is data, never commands.
    Document { title: String, text: String },
    /// Investigation: a bounded, replay-verified research case.
    Research(ResearchCase),
    /// Experiment selection: a ranked candidate count to choose from and the
    /// experiment budget.
    Experiment { candidates: usize, experiment_budget: usize },
    /// Consolidation: no input beyond the host's own accounting.
    Memory,
    /// Reporting: the task whose progress is summarized.
    Report { subject: String },
}

/// A single progress record. Every runner step appends exactly one, so the
/// progress log is a faithful, ordered trace of what the task did.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TaskProgress {
    pub index: usize,
    pub capability: Capability,
    pub detail: String,
    /// Whether the step changed persistent state. All five task kinds are
    /// read-only or planning-only, so this is `false` throughout — but it is
    /// recorded rather than assumed.
    pub mutated: bool,
}

/// The result a task produces. Each variant carries the receipt from the
/// existing infrastructure the task adapted, not a re-invention of it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskResult {
    DocumentAnalysis {
        title: String,
        sha256: String,
        proposed: usize,
        rejected: usize,
        instructions_refused: usize,
        facts: usize,
        definitions: usize,
        rules: usize,
    },
    Investigation {
        case_id: String,
        conclusion: ResearchConclusion,
        steps: usize,
        counterexamples: usize,
        replans: usize,
        replay_verified: bool,
    },
    ExperimentSelection {
        candidates: usize,
        selected: usize,
        experiment_budget: usize,
    },
    MemoryConsolidation {
        cluster_count: usize,
        entry_count: usize,
        conversation_sessions: usize,
        conversation_turns: usize,
        total_bytes: usize,
        budget_bytes: usize,
        within_budget: bool,
    },
    Report {
        subject: String,
        summary: String,
    },
}

impl TaskResult {
    /// A one-line, human-readable explanation of the result.
    pub fn explain(&self) -> String {
        match self {
            TaskResult::DocumentAnalysis {
                title,
                proposed,
                rejected,
                instructions_refused,
                facts,
                definitions,
                rules,
                ..
            } => format!(
                "analyzed \"{title}\": {proposed} proposed ({facts} facts, {definitions} \
definitions, {rules} rules), {rejected} rejected, {instructions_refused} instructions refused"
            ),
            TaskResult::Investigation {
                case_id,
                conclusion,
                steps,
                counterexamples,
                replans,
                replay_verified,
            } => format!(
                "investigated {case_id}: {conclusion:?} after {steps} steps \
({counterexamples} counterexamples, {replans} replans, replay verified: {replay_verified})"
            ),
            TaskResult::ExperimentSelection {
                candidates,
                selected,
                experiment_budget,
            } => format!(
                "selected {selected} of {candidates} candidate experiment(s) within budget {experiment_budget}"
            ),
            TaskResult::MemoryConsolidation {
                cluster_count,
                entry_count,
                total_bytes,
                budget_bytes,
                within_budget,
                ..
            } => format!(
                "{cluster_count} clusters / {entry_count} entries, {total_bytes} of {budget_bytes} bytes \
(within budget: {within_budget})"
            ),
            TaskResult::Report { subject, summary } => {
                format!("report on {subject}: {summary}")
            }
        }
    }
}

/// A planned, runnable task: its kind, inputs, declared budget and scope, and
/// its evolving state, progress, and (once complete) result.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Task {
    pub id: String,
    pub kind: TaskKind,
    pub input: TaskInput,
    pub budget: TaskBudget,
    pub state: TaskState,
    pub progress: Vec<TaskProgress>,
    pub result: Option<TaskResult>,
    /// The reason the task stopped early, if it did. A normal completion has
    /// none.
    pub stop_reason: Option<String>,
}

impl Task {
    /// Plan a task with the default bounded budget and canonical scope for its
    /// kind. Fails only if the input does not match the kind (a planning error,
    /// not an authority one).
    pub fn plan(id: impl Into<String>, kind: TaskKind, input: TaskInput) -> Result<Task, String> {
        Task::plan_with(id, kind, input, TaskBudget::for_kind(kind))
    }

    /// Plan a task with an explicit budget and scope.
    ///
    /// This is where declared authority is first checked: if the budget's scope
    /// does not cover the capabilities the kind requires, planning refuses.
    pub fn plan_with(
        id: impl Into<String>,
        kind: TaskKind,
        input: TaskInput,
        budget: TaskBudget,
    ) -> Result<Task, String> {
        if !input_matches_kind(&input, kind) {
            return Err(format!(
                "input does not match task kind {}",
                kind.label()
            ));
        }
        for required in AuthorityScope::required_for(kind) {
            if !budget.scope.permits(required) {
                return Err(format!(
                    "task kind {} requires capability {:?}, not granted by the declared scope",
                    kind.label(),
                    required
                ));
            }
        }
        Ok(Task {
            id: id.into(),
            kind,
            input,
            budget,
            state: TaskState::Pending,
            progress: Vec::new(),
            result: None,
            stop_reason: None,
        })
    }

    /// Whether the task has finished, stopped, been cancelled, or refused.
    pub fn is_terminal(&self) -> bool {
        self.state.is_terminal()
    }

    /// The single completion condition: a task is complete when it has a
    /// result and is in the `Completed` state. A refused or cancelled task is
    /// *done* but not *complete*, which is why the two are distinguished.
    pub fn is_complete(&self) -> bool {
        self.state == TaskState::Completed && self.result.is_some()
    }

    /// Whether the task stayed inside its declared authority.
    ///
    /// Because every step is checked *before* the capability is invoked, no
    /// step can ever run outside the declared scope: a task that would have
    /// exceeded it is stopped with [`TaskState::Refused`] instead. A refusal is
    /// therefore the mechanism that keeps this property true, and a refused
    /// task is still within authority — it attempted more, but never did more.
    pub fn within_authority(&self) -> bool {
        self.progress
            .iter()
            .all(|step| self.budget.scope.permits(step.capability))
    }

    /// Whether the task was stopped because it would have exceeded its declared
    /// authority, budget, or a step contract. Distinct from *completing*: a
    /// refused task made no unauthorized use, but it also produced no result.
    pub fn was_refused(&self) -> bool {
        matches!(self.state, TaskState::Refused { .. })
    }
}

fn input_matches_kind(input: &TaskInput, kind: TaskKind) -> bool {
    matches!(
        (input, kind),
        (TaskInput::Document { .. }, TaskKind::AnalyzeDocument)
            | (TaskInput::Research(_), TaskKind::Investigate)
            | (TaskInput::Experiment { .. }, TaskKind::RunExperiment)
            | (TaskInput::Memory, TaskKind::ConsolidateMemory)
            | (TaskInput::Report { .. }, TaskKind::ProduceReport)
    )
}

/// The host the runner executes against. Separating this seam keeps the runner
/// deterministic and unit-testable, and keeps each adapter over existing
/// infrastructure.
///
/// An implementation must never perform an external write, a shell command, or
/// a network request: the runner's authority scope does not include the
/// capabilities for those, so doing them would violate the task's declaration.
pub trait TaskHost {
    /// Extract proposed knowledge from a document (read-only). Backed by
    /// [`document_learning::extract_text`].
    fn analyze_document(&mut self, title: &str, text: &str) -> Result<DocumentProposal, String>;

    /// Run a bounded, replay-verified research case. Backed by
    /// [`mathematical_research::run_case`].
    fn investigate(&mut self, case: &ResearchCase) -> Result<ResearchReceipt, String>;

    /// Select experiments under a budget with the capability planner. Planning
    /// only — this returns a count, not an execution. Backed by
    /// [`crate::capability_planner`]'s portfolio selection.
    fn select_experiments(&mut self, candidates: usize, budget: usize) -> Result<usize, String>;

    /// Account for resident memory. Backed by [`crate::reliability`]'s report.
    fn account_memory(&mut self) -> Result<MemoryReport, String>;
}

/// Deterministic, cooperative task runner.
///
/// The runner owns a queue of tasks and drives them one capability-use per
/// [`TaskRunner::step`]. There are no OS threads and no timers: the caller
/// decides when work happens, which is what makes cancellation, pausing, and
/// resuming exact and replayable.
pub struct TaskRunner {
    queue: Vec<Task>,
}

impl Default for TaskRunner {
    fn default() -> Self {
        TaskRunner::new()
    }
}

impl TaskRunner {
    pub fn new() -> Self {
        TaskRunner { queue: Vec::new() }
    }

    /// Enqueue a planned task. The task starts in `Pending`.
    pub fn enqueue(&mut self, task: Task) -> &Task {
        self.queue.push(task);
        self.queue.last().expect("just pushed")
    }

    pub fn tasks(&self) -> &[Task] {
        &self.queue
    }

    pub fn task(&self, id: &str) -> Option<&Task> {
        self.queue.iter().find(|task| task.id == id)
    }

    pub fn task_mut(&mut self, id: &str) -> Option<&mut Task> {
        self.queue.iter_mut().find(|task| task.id == id)
    }

    /// Start a task (move it from `Pending` to `Running`). A task that is not
    /// pending refuses to start, so start is idempotent-safe.
    pub fn start(&mut self, id: &str) -> Result<(), String> {
        let task = self
            .queue
            .iter_mut()
            .find(|task| task.id == id)
            .ok_or_else(|| format!("unknown task {id}"))?;
        match task.state {
            TaskState::Pending | TaskState::Paused => {
                task.state = TaskState::Running;
                Ok(())
            }
            ref other => Err(format!(
                "task {id} cannot start from state {}",
                other.label()
            )),
        }
    }

    /// Request cooperative cancellation. A task that has not finished moves to
    /// `Cancelled` with a recorded reason; a finished task is left alone.
    pub fn cancel(&mut self, id: &str, reason: &str) -> Result<(), String> {
        let task = self
            .queue
            .iter_mut()
            .find(|task| task.id == id)
            .ok_or_else(|| format!("unknown task {id}"))?;
        match task.state {
            TaskState::Completed | TaskState::Cancelled | TaskState::Refused { .. } => Ok(()),
            _ => {
                task.state = TaskState::Cancelled;
                task.stop_reason = Some(reason.to_string());
                Ok(())
            }
        }
    }

    /// Pause a running or pending task so it can be resumed later. A paused
    /// task keeps its progress and continues from the next step on resume.
    pub fn pause(&mut self, id: &str, reason: &str) -> Result<(), String> {
        let task = self
            .queue
            .iter_mut()
            .find(|task| task.id == id)
            .ok_or_else(|| format!("unknown task {id}"))?;
        match task.state {
            TaskState::Pending | TaskState::Running | TaskState::Paused => {
                task.state = TaskState::Paused;
                task.stop_reason = Some(reason.to_string());
                Ok(())
            }
            ref other => Err(format!(
                "task {id} cannot pause from state {}",
                other.label()
            )),
        }
    }

    /// Resume a paused task. Fails for any other state, so resume is only ever
    /// a continuation, never a restart.
    pub fn resume(&mut self, id: &str) -> Result<(), String> {
        let task = self
            .queue
            .iter_mut()
            .find(|task| task.id == id)
            .ok_or_else(|| format!("unknown task {id}"))?;
        if !task.state.is_resumable() {
            return Err(format!(
                "task {id} is not resumable from state {}",
                task.state.label()
            ));
        }
        task.state = TaskState::Running;
        task.stop_reason = None;
        Ok(())
    }

    /// Run a task to completion (or to a controlled stop), driving one step at
    /// a time. Returns a reference to the finished task.
    pub fn run(&mut self, id: &str, host: &mut dyn TaskHost) -> Result<&Task, String> {
        if self.task(id).is_none() {
            return Err(format!("unknown task {id}"));
        }
        if matches!(
            self.task(id).map(|task| &task.state),
            Some(TaskState::Pending) | Some(TaskState::Paused)
        ) {
            self.start(id)?;
        }
        loop {
            let state = self
                .task(id)
                .map(|task| task.state.clone())
                .ok_or_else(|| format!("unknown task {id}"))?;
            if state.is_terminal() {
                break;
            }
            self.step(id, host)?;
        }
        self.task(id).ok_or_else(|| format!("unknown task {id}"))
    }

    /// Advance a running task by exactly one bounded capability use.
    ///
    /// The step checks, in order: the task is running; the step cap has not been
    /// reached; the resources budget still permits the step; the capability is
    /// within the declared scope. Any failure moves the task to a controlled
    /// state (`Refused` for budget/authority, `Completed` when the work is
    /// done) and records why — nothing is silently skipped.
    pub fn step(&mut self, id: &str, host: &mut dyn TaskHost) -> Result<(), String> {
        // Validate state without holding a mutable borrow across the host call.
        let (kind, input, steps_used, budget, scope_ok) = {
            let task = self
                .task(id)
                .ok_or_else(|| format!("unknown task {id}"))?;
            if task.state != TaskState::Running {
                return Err(format!(
                    "task {id} is not running (state {})",
                    task.state.label()
                ));
            }
            let steps_used = task.progress.len() as u32;
            let capability = capability_for_step(task.kind, steps_used);
            (
                task.kind,
                task.input.clone(),
                steps_used,
                task.budget.clone(),
                capability.map(|c| task.budget.scope.permits(c)).unwrap_or(true),
            )
        };

        let capability = capability_for_step(kind, steps_used);

        // Completion condition: when there is no further step, the task is done.
        let Some(capability) = capability else {
            return self.finish(id);
        };

        // Declared budget: step cap.
        if steps_used >= budget.max_steps {
            return self.refuse(id, "step budget exhausted before completion");
        }
        // Declared authority: capability must be in scope.
        if !scope_ok {
            return self.refuse(
                id,
                &format!("capability {capability:?} is outside the declared scope"),
            );
        }
        // Declared budget: resources still permit a low-risk, non-writing step.
        {
            let task = self
                .queue
                .iter_mut()
                .find(|task| task.id == id)
                .expect("checked above");
            if !task.budget.resources.can_spend(0.0, false) {
                return self.refuse(id, "resource budget exhausted before completion");
            }
        }

        let outcome = self.execute_step(id, kind, &input, capability, host);

        let (detail, mutated) = match outcome {
            Ok(step) => step,
            Err(reason) => return self.refuse(id, &reason),
        };

        // Record the progress and charge the resource budget for this step.
        let task = self
            .queue
            .iter_mut()
            .find(|task| task.id == id)
            .expect("checked above");
        let index = task.progress.len();
        task.progress.push(TaskProgress {
            index,
            capability,
            detail,
            mutated,
        });
        let _ = task.budget.resources.spend(0.0, 0, false);

        // If the work is now done, complete; otherwise stay running for the
        // next step.
        if !self.progress_is_complete(id) {
            return Ok(());
        }
        self.finish(id)
    }

    /// Whether the task has recorded every step it needs. Read-only check.
    fn progress_is_complete(&self, id: &str) -> bool {
        let Some(task) = self.task(id) else {
            return false;
        };
        capability_for_step(task.kind, task.progress.len() as u32).is_none()
    }

    /// Execute exactly one capability use against the host. Returns the
    /// progress detail and whether persistent state changed. A host error is
    /// returned as `Err(reason)` so the caller can record a controlled stop.
    fn execute_step(
        &mut self,
        id: &str,
        kind: TaskKind,
        input: &TaskInput,
        capability: Capability,
        host: &mut dyn TaskHost,
    ) -> Result<(String, bool), String> {
        match (kind, input, capability) {
            (
                TaskKind::AnalyzeDocument,
                TaskInput::Document { title, text },
                Capability::DocumentRead,
            ) => Ok((
                format!("read document \"{title}\" ({} chars)", text.chars().count()),
                false,
            )),
            (
                TaskKind::AnalyzeDocument,
                TaskInput::Document { title, text },
                Capability::DocumentExtract,
            ) => {
                let proposal = host.analyze_document(title, text)?;
                let (facts, definitions, rules) = proposal.kind_counts();
                let result = TaskResult::DocumentAnalysis {
                    title: title.clone(),
                    sha256: proposal.sha256.clone(),
                    proposed: proposal.proposed,
                    rejected: proposal.rejected,
                    instructions_refused: proposal.instructions_refused,
                    facts,
                    definitions,
                    rules,
                };
                let detail = result.explain();
                self.set_result(id, result);
                Ok((detail, false))
            }
            (TaskKind::Investigate, TaskInput::Research(case), Capability::MathInvestigate) => {
                let receipt = host.investigate(case)?;
                let result = TaskResult::Investigation {
                    case_id: receipt.case_id.clone(),
                    conclusion: receipt.conclusion,
                    steps: receipt.steps.len(),
                    counterexamples: receipt.counterexamples.len(),
                    replans: receipt.replans,
                    replay_verified: receipt.replay_verified(),
                };
                let detail = result.explain();
                self.set_result(id, result);
                Ok((detail, false))
            }
            (
                TaskKind::RunExperiment,
                TaskInput::Experiment {
                    candidates,
                    experiment_budget,
                },
                Capability::ExperimentPlan,
            ) => {
                let selected = host.select_experiments(*candidates, *experiment_budget)?;
                let result = TaskResult::ExperimentSelection {
                    candidates: *candidates,
                    selected,
                    experiment_budget: *experiment_budget,
                };
                let detail = result.explain();
                self.set_result(id, result);
                Ok((detail, false))
            }
            (TaskKind::ConsolidateMemory, TaskInput::Memory, Capability::MemoryAccount) => {
                let report = host.account_memory()?;
                let result = TaskResult::MemoryConsolidation {
                    cluster_count: report.cluster_count,
                    entry_count: report.entry_count,
                    conversation_sessions: report.conversation_sessions,
                    conversation_turns: report.conversation_turns,
                    total_bytes: report.total_bytes,
                    budget_bytes: report.budget_bytes,
                    within_budget: report.within_budget(),
                };
                let detail = result.explain();
                self.set_result(id, result);
                Ok((detail, false))
            }
            (TaskKind::ProduceReport, TaskInput::Report { subject }, Capability::ReportWrite) => {
                let summary = self.summarize_subject(subject);
                let result = TaskResult::Report {
                    subject: subject.clone(),
                    summary: summary.clone(),
                };
                let detail = format!("wrote report on {subject}");
                self.set_result(id, result);
                Ok((detail, false))
            }
            (_, _, capability) => Err(format!(
                "no adapter for capability {capability:?} on kind {}",
                kind.label()
            )),
        }
    }

    fn set_result(&mut self, id: &str, result: TaskResult) {
        if let Some(task) = self.task_mut(id) {
            task.result = Some(result);
        }
    }

    /// Build a report summary about another task, or a self-summary.
    fn summarize_subject(&self, subject: &str) -> String {
        match self.task(subject) {
            Some(task) => {
                let result = task
                    .result
                    .as_ref()
                    .map(|result| result.explain())
                    .unwrap_or_else(|| "no result yet".to_string());
                format!(
                    "task {} ({}) is {} after {} step(s): {}",
                    task.id,
                    task.kind.label(),
                    task.state.label(),
                    task.progress.len(),
                    result
                )
            }
            None => format!("no task named {subject}"),
        }
    }

    fn finish(&mut self, id: &str) -> Result<(), String> {
        let task = self
            .queue
            .iter_mut()
            .find(|task| task.id == id)
            .ok_or_else(|| format!("unknown task {id}"))?;
        if task.result.is_none() {
            return self.refuse(id, "task ended without producing a result");
        }
        task.state = TaskState::Completed;
        task.stop_reason = None;
        Ok(())
    }

    fn refuse(&mut self, id: &str, reason: &str) -> Result<(), String> {
        let task = self
            .queue
            .iter_mut()
            .find(|task| task.id == id)
            .ok_or_else(|| format!("unknown task {id}"))?;
        task.state = TaskState::Refused {
            reason: reason.to_string(),
        };
        task.stop_reason = Some(reason.to_string());
        Ok(())
    }

    /// Produce a report explaining one task's run: what it did, what it
    /// produced, and whether it stayed within its declared authority and
    /// budget.
    pub fn report(&self, id: &str) -> Option<TaskReport> {
        let task = self.task(id)?;
        Some(TaskReport::from_task(task))
    }
}

/// The step plan for a task kind: the ordered capabilities a full run uses.
/// `capability_for_step` returns the capability for the next step, or `None`
/// when the task is done. This single function *is* the completion condition.
fn capability_for_step(kind: TaskKind, step_index: u32) -> Option<Capability> {
    let plan: &[Capability] = match kind {
        TaskKind::AnalyzeDocument => &[Capability::DocumentRead, Capability::DocumentExtract],
        TaskKind::Investigate => &[Capability::MathInvestigate],
        TaskKind::RunExperiment => &[Capability::ExperimentPlan],
        TaskKind::ConsolidateMemory => &[Capability::MemoryAccount],
        TaskKind::ProduceReport => &[Capability::ReportWrite],
    };
    plan.get(step_index as usize).copied()
}

/// An explanation of a task's run.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TaskReport {
    pub schema: String,
    pub id: String,
    pub kind: TaskKind,
    pub state: TaskState,
    pub steps_taken: usize,
    pub max_steps: u32,
    pub actions_used: u32,
    pub max_actions: u32,
    pub time_used_ms: u64,
    pub max_time_ms: u64,
    pub external_writes_used: u32,
    pub max_external_writes: u32,
    pub within_budget: bool,
    pub within_authority: bool,
    pub refused: bool,
    pub progress: Vec<TaskProgress>,
    pub result: Option<TaskResult>,
    pub stop_reason: Option<String>,
}

impl TaskReport {
    pub fn from_task(task: &Task) -> Self {
        let resources = &task.budget.resources;
        TaskReport {
            schema: TASK_SCHEMA.to_string(),
            id: task.id.clone(),
            kind: task.kind,
            state: task.state.clone(),
            steps_taken: task.progress.len(),
            max_steps: task.budget.max_steps,
            actions_used: resources.actions_used,
            max_actions: resources.max_actions,
            time_used_ms: resources.time_used_ms,
            max_time_ms: resources.max_time_ms,
            external_writes_used: resources.external_writes_used,
            max_external_writes: resources.max_external_writes,
            within_budget: resources.external_writes_used
                <= resources.max_external_writes
                && resources.actions_used <= resources.max_actions
                && resources.time_used_ms <= resources.max_time_ms,
            within_authority: task.within_authority(),
            refused: task.was_refused(),
            progress: task.progress.clone(),
            result: task.result.clone(),
            stop_reason: task.stop_reason.clone(),
        }
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).expect("task report serializes")
    }

    /// A human-readable explanation of the run.
    pub fn to_markdown(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!("# Task {} — {}\n\n", self.id, self.kind.label()));
        out.push_str(&format!("* state: {}\n", self.state.label()));
        out.push_str(&format!(
            "* steps: {} of {}\n",
            self.steps_taken, self.max_steps
        ));
        out.push_str(&format!(
            "* budget: {}/{} actions, {}/{} ms, {}/{} external writes\n",
            self.actions_used,
            self.max_actions,
            self.time_used_ms,
            self.max_time_ms,
            self.external_writes_used,
            self.max_external_writes
        ));
        out.push_str(&format!("* within budget: {}\n", self.within_budget));
        out.push_str(&format!("* within authority: {}\n", self.within_authority));
        if self.refused {
            out.push_str("* refused: true (stopped before exceeding its declaration)\n");
        }
        if let Some(reason) = &self.stop_reason {
            out.push_str(&format!("* stopped: {reason}\n"));
        }
        out.push_str("\n## Progress\n\n");
        for step in &self.progress {
            out.push_str(&format!(
                "{}. [{:?}] {}{}\n",
                step.index,
                step.capability,
                step.detail,
                if step.mutated { " (mutated state)" } else { "" }
            ));
        }
        out.push_str("\n## Result\n\n");
        match &self.result {
            Some(result) => out.push_str(&format!("{}\n", result.explain())),
            None => out.push_str("No result was produced.\n"),
        }
        out
    }
}

// ─── Evaluated task suite ─────────────────────────────────────────────────
//
// Phase 10 is evaluated the same way Phase 9 evaluates conversations: a fixed,
// versioned set of tasks is run, and the result is reported against declared
// contracts. The suite proves, mechanically, that a task runs, stops, resumes,
// and explains its result without exceeding its declared authority or budget.

/// One scenario in the task suite: a planned task and, optionally, a pause
/// after a given number of steps (to exercise stop-and-resume).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TaskScenario {
    pub id: String,
    pub kind: TaskKind,
    pub input: TaskInput,
    /// Pause after this many steps, then resume, then run to completion. `None`
    /// runs straight through.
    #[serde(default)]
    pub pause_after_steps: Option<u32>,
    /// Whether this scenario is *expected* to be refused (a budget/scope
    /// contract exercise) rather than complete.
    #[serde(default)]
    pub expect_refused: bool,
}

impl TaskScenario {
    /// The canonical suite: one scenario per task kind, plus a paused/resumed
    /// document analysis and a deliberately budget-starved refusal.
    pub fn canonical_suite() -> Vec<TaskScenario> {
        let case = mathematical_research::independent_corpus()
            .into_iter()
            .find(|case| case.claim == crate::mathematical_research::ResearchClaim::TreeCriterion)
            .expect("independent corpus has a tree-criterion case");
        vec![
            TaskScenario {
                id: "analyze".to_string(),
                kind: TaskKind::AnalyzeDocument,
                input: TaskInput::Document {
                    title: "Observatory notes".to_string(),
                    text: "Alice manages the observatory. The telescope is calibrated nightly."
                        .to_string(),
                },
                pause_after_steps: None,
                expect_refused: false,
            },
            TaskScenario {
                id: "analyze-paused".to_string(),
                kind: TaskKind::AnalyzeDocument,
                input: TaskInput::Document {
                    title: "Station log".to_string(),
                    text: "Bob maintains the station. The generator is serviced weekly."
                        .to_string(),
                },
                pause_after_steps: Some(1),
                expect_refused: false,
            },
            TaskScenario {
                id: "investigate".to_string(),
                kind: TaskKind::Investigate,
                input: TaskInput::Research(case),
                pause_after_steps: None,
                expect_refused: false,
            },
            TaskScenario {
                id: "experiment".to_string(),
                kind: TaskKind::RunExperiment,
                input: TaskInput::Experiment {
                    candidates: 5,
                    experiment_budget: 2,
                },
                pause_after_steps: None,
                expect_refused: false,
            },
            TaskScenario {
                id: "consolidate".to_string(),
                kind: TaskKind::ConsolidateMemory,
                input: TaskInput::Memory,
                pause_after_steps: None,
                expect_refused: false,
            },
            TaskScenario {
                id: "starved".to_string(),
                kind: TaskKind::AnalyzeDocument,
                input: TaskInput::Document {
                    title: "too much to do".to_string(),
                    text: "A fact is stated here.".to_string(),
                },
                pause_after_steps: None,
                expect_refused: true,
            },
        ]
    }
}

/// The outcome of running one scenario.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TaskRunOutcome {
    pub id: String,
    pub kind: TaskKind,
    pub expected_refused: bool,
    pub state: TaskState,
    pub completed: bool,
    pub refused: bool,
    pub paused: bool,
    pub resumed: bool,
    pub steps: usize,
    pub within_budget: bool,
    pub within_authority: bool,
    pub result_explained: bool,
    pub stop_reason: Option<String>,
    pub result: Option<TaskResult>,
}

impl TaskRunOutcome {
    /// Whether this run matched its expectation and stayed inside its
    /// declaration.
    pub fn is_contract_satisfied(&self) -> bool {
        if self.expected_refused {
            self.refused && !self.completed && self.within_budget && self.within_authority
        } else {
            self.completed
                && !self.refused
                && self.within_budget
                && self.within_authority
                && self.result_explained
        }
    }
}

/// The reported result of running the task suite.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TaskSuiteReport {
    pub schema: String,
    pub scenarios: usize,
    pub completed: usize,
    pub refused: usize,
    pub paused_and_resumed: usize,
    pub all_within_authority: bool,
    pub all_within_budget: bool,
    pub all_contracts_satisfied: bool,
    pub outcomes: Vec<TaskRunOutcome>,
}

impl TaskSuiteReport {
    /// The held contracts: every scenario matched its expectation, every run
    /// stayed inside its declared authority and budget, and every completed run
    /// explained its result.
    pub fn contracts_hold(&self) -> Result<(), String> {
        if !self.all_contracts_satisfied {
            let broken: Vec<String> = self
                .outcomes
                .iter()
                .filter(|outcome| !outcome.is_contract_satisfied())
                .map(|outcome| format!("{} ({})", outcome.id, outcome.state.label()))
                .collect();
            return Err(format!("task suite contracts violated: {}", broken.join(", ")));
        }
        Ok(())
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).expect("task suite report serializes")
    }

    pub fn to_markdown(&self) -> String {
        let mut out = String::new();
        out.push_str("# Phase 10 — controlled autonomy report\n\n");
        out.push_str(
            "Runs the bounded task suite. Every task declares its authority and \
budget up front; each runs, stops, resumes where appropriate, and explains its \
result without exceeding its declaration.\n\n",
        );
        out.push_str("## Summary\n\n");
        out.push_str(&format!("* scenarios: {}\n", self.scenarios));
        out.push_str(&format!("* completed: {}\n", self.completed));
        out.push_str(&format!("* refused (controlled stop): {}\n", self.refused));
        out.push_str(&format!(
            "* paused then resumed: {}\n",
            self.paused_and_resumed
        ));
        out.push_str(&format!(
            "* all within authority: {}\n",
            self.all_within_authority
        ));
        out.push_str(&format!("* all within budget: {}\n", self.all_within_budget));
        out.push_str(&format!(
            "* all contracts satisfied: {}\n\n",
            self.all_contracts_satisfied
        ));

        out.push_str("## Scenarios\n\n");
        out.push_str("| id | kind | state | steps | completed | refused | within authority | within budget | result |\n");
        out.push_str("|---|---|---|---|---|---|---|---|---|\n");
        for outcome in &self.outcomes {
            let result = outcome
                .result
                .as_ref()
                .map(|result| result.explain())
                .unwrap_or_else(|| "—".to_string());
            out.push_str(&format!(
                "| {} | {} | {} | {} | {} | {} | {} | {} | {} |\n",
                outcome.id,
                outcome.kind.label(),
                outcome.state.label(),
                outcome.steps,
                outcome.completed,
                outcome.refused,
                outcome.within_authority,
                outcome.within_budget,
                result,
            ));
        }

        out.push_str("\n## Interpretation\n\n");
        out.push_str(
            "* A task is complete only when it produced a result; a refusal is a \
controlled stop, not a completion.\n",
        );
        out.push_str(
            "* A refusal never exceeds the declaration: the capability is checked \
before it runs, so \"within authority\" stays true even when a task is stopped \
for wanting more.\n",
        );
        out.push_str(
            "* No task kind can declare shell, network, or simulation authority; \
chat is independent of unrestricted execution.\n",
        );
        out
    }
}

/// Run the canonical task suite against `host` and report the outcomes.
///
/// Scenarios with `pause_after_steps` are paused and resumed to exercise the
/// stop/resume path; this is what proves a task can stop and continue.
pub fn run_task_suite(host: &mut dyn TaskHost) -> TaskSuiteReport {
    run_task_scenarios(&TaskScenario::canonical_suite(), host)
}

/// Run a specific list of scenarios against `host`.
pub fn run_task_scenarios(
    scenarios: &[TaskScenario],
    host: &mut dyn TaskHost,
) -> TaskSuiteReport {
    let mut outcomes = Vec::new();
    for scenario in scenarios {
        let budget = if scenario.expect_refused {
            // Starve the step budget so the task is refused before it can
            // finish: the honest way to exercise the budget contract.
            TaskBudget::new(
                AutonomyBudget::new(1, 1_000, 0, 0.2),
                1,
                AuthorityScope::for_kind(scenario.kind),
            )
        } else {
            TaskBudget::for_kind(scenario.kind)
        };
        let task = match Task::plan_with(
            scenario.id.clone(),
            scenario.kind,
            scenario.input.clone(),
            budget,
        ) {
            Ok(task) => task,
            Err(error) => {
                // A planning refusal is itself a controlled outcome.
                outcomes.push(TaskRunOutcome {
                    id: scenario.id.clone(),
                    kind: scenario.kind,
                    expected_refused: scenario.expect_refused,
                    state: TaskState::Refused { reason: error.clone() },
                    completed: false,
                    refused: true,
                    paused: false,
                    resumed: false,
                    steps: 0,
                    within_budget: true,
                    within_authority: true,
                    result_explained: false,
                    stop_reason: Some(error),
                    result: None,
                });
                continue;
            }
        };

        let mut runner = TaskRunner::new();
        runner.enqueue(task);
        let mut paused = false;
        let mut resumed = false;
        if let Err(error) = runner.start(&scenario.id) {
            outcomes.push(pretend_refusal(scenario, error));
            continue;
        }
        if let Some(pause_at) = scenario.pause_after_steps {
            for _ in 0..pause_at {
                if runner.task(&scenario.id).map(|t| t.state.is_terminal()).unwrap_or(true) {
                    break;
                }
                let _ = runner.step(&scenario.id, host);
            }
            // Stop on purpose, then continue: this is the stop/resume path.
            let terminal = runner
                .task(&scenario.id)
                .map(|t| t.state.is_terminal())
                .unwrap_or(true);
            if !terminal {
                let _ = runner.pause(&scenario.id, "scenario pause");
                if runner
                    .task(&scenario.id)
                    .map(|t| t.state.is_resumable())
                    .unwrap_or(false)
                {
                    paused = true;
                    let _ = runner.resume(&scenario.id);
                    resumed = true;
                }
            }
        }
        // Run to a terminal state (completes, refuses, or stays paused only if
        // a pause could not be resumed — which cannot happen above).
        let mut guard = 0u32;
        loop {
            guard += 1;
            if guard > 64 {
                break;
            }
            match runner.task(&scenario.id).map(|t| t.state.clone()) {
                Some(state) if state.is_terminal() => break,
                Some(TaskState::Paused) | Some(TaskState::Pending) => {
                    let _ = runner.resume(&scenario.id);
                }
                Some(TaskState::Running) => {
                    if runner.step(&scenario.id, host).is_err() {
                        break;
                    }
                }
                _ => break,
            }
        }

        let task = runner
            .task(&scenario.id)
            .expect("scenario task exists after run");
        let report = TaskReport::from_task(task);
        outcomes.push(TaskRunOutcome {
            id: scenario.id.clone(),
            kind: scenario.kind,
            expected_refused: scenario.expect_refused,
            state: task.state.clone(),
            completed: task.is_complete(),
            refused: task.was_refused(),
            paused,
            resumed,
            steps: task.progress.len(),
            within_budget: report.within_budget,
            within_authority: report.within_authority,
            result_explained: task.result.is_some(),
            stop_reason: task.stop_reason.clone(),
            result: task.result.clone(),
        });
    }

    let completed = outcomes.iter().filter(|outcome| outcome.completed).count();
    let refused = outcomes.iter().filter(|outcome| outcome.refused).count();
    let paused_and_resumed = outcomes
        .iter()
        .filter(|outcome| outcome.paused && outcome.resumed)
        .count();
    let all_contracts_satisfied = outcomes.iter().all(|outcome| outcome.is_contract_satisfied());
    TaskSuiteReport {
        schema: TASK_SCHEMA.to_string(),
        scenarios: outcomes.len(),
        completed,
        refused,
        paused_and_resumed,
        all_within_authority: outcomes.iter().all(|outcome| outcome.within_authority),
        all_within_budget: outcomes.iter().all(|outcome| outcome.within_budget),
        all_contracts_satisfied,
        outcomes,
    }
}

fn pretend_refusal(scenario: &TaskScenario, reason: String) -> TaskRunOutcome {
    TaskRunOutcome {
        id: scenario.id.clone(),
        kind: scenario.kind,
        expected_refused: scenario.expect_refused,
        state: TaskState::Refused {
            reason: reason.clone(),
        },
        completed: false,
        refused: true,
        paused: false,
        resumed: false,
        steps: 0,
        within_budget: true,
        within_authority: true,
        result_explained: false,
        stop_reason: Some(reason),
        result: None,
    }
}

// ─── Production host adapter ──────────────────────────────────────────────
//
// The default host is deliberately minimal: it reaches only the read-only and
// planning-only infrastructure the task kinds declare. It cannot reach a shell,
// a network, or the simulation loop, because no capability in the task model
// corresponds to those.

/// A [`TaskHost`] backed directly by the library's existing modules.
pub struct DefaultTaskHost {
    memory_report: Option<MemoryReport>,
}

impl Default for DefaultTaskHost {
    fn default() -> Self {
        DefaultTaskHost::new()
    }
}

impl DefaultTaskHost {
    pub fn new() -> Self {
        DefaultTaskHost { memory_report: None }
    }

    /// Supply the resident-memory accounting used by consolidation tasks. In
    /// production this is produced by [`crate::reliability`] from the live
    /// brain; the task layer only reads it.
    pub fn with_memory_report(report: MemoryReport) -> Self {
        DefaultTaskHost {
            memory_report: Some(report),
        }
    }
}

impl TaskHost for DefaultTaskHost {
    fn analyze_document(&mut self, title: &str, text: &str) -> Result<DocumentProposal, String> {
        // Extract only. The proposal is neither persisted nor committed here:
        // committing knowledge remains an explicit, human-accepted step in the
        // document workflow (Phase 8), so an autonomous task cannot silently
        // learn from a document.
        Ok(document_learning::extract_text(title, text))
    }

    fn investigate(&mut self, case: &ResearchCase) -> Result<ResearchReceipt, String> {
        Ok(mathematical_research::run_case(case))
    }

    fn select_experiments(&mut self, candidates: usize, budget: usize) -> Result<usize, String> {
        Ok(plan_experiment_portfolio(candidates, budget))
    }

    fn account_memory(&mut self) -> Result<MemoryReport, String> {
        // With no brain attached the resident footprint is genuinely zero, so
        // an unconfigured host reports an empty (but honest) accounting rather
        // than failing. A configured host supplies the live report.
        Ok(self.memory_report.clone().unwrap_or_default())
    }
}

/// Select a bounded experiment portfolio with the capability planner's own
/// selection algorithm, then report how many experiments it chose.
///
/// Each candidate is given unit validation cost and unit expected gain. The
/// planner's portfolio is an *exact-budget* selection, so the fill target is
/// capped at the number of candidates available; otherwise a budget larger than
/// the candidate set would select nothing. This reuses the planner's selection
/// (including its tie/ambiguity handling) instead of a bare `min`. Planning
/// only: nothing is executed.
pub fn plan_experiment_portfolio(candidates: usize, budget: usize) -> usize {
    use crate::capability_planner::{
        select_abstraction_experiment_portfolio, CapabilityChainProofAbstractionPriorityCandidate,
        CapabilityChainProofAbstractionPriorityReceipt, CapabilityChainProofAbstractionPriorityScore,
    };

    let target = budget.min(candidates);
    let priority_candidates: Vec<CapabilityChainProofAbstractionPriorityCandidate> = (0..candidates)
        .map(|index| CapabilityChainProofAbstractionPriorityCandidate {
            pattern_id: format!("candidate-{index:03}"),
            score: CapabilityChainProofAbstractionPriorityScore {
                recurrence_signal: 1,
                value_signal: 1,
                risk_signal: 0,
                historical_success_signal: 1,
                complexity_penalty: 0,
                expected_gain: 1,
                validation_cost: 1,
                efficiency_numerator: 1,
                efficiency_denominator: 1,
                total: 3,
            },
        })
        .collect();
    let preferred = priority_candidates
        .iter()
        .map(|candidate| candidate.pattern_id.clone())
        .collect();
    let priorities = CapabilityChainProofAbstractionPriorityReceipt {
        ambiguous: false,
        preferred_pattern_ids: preferred,
        candidates: priority_candidates,
    };
    let portfolio = select_abstraction_experiment_portfolio(&priorities, target);
    portfolio.selected_pattern_ids.len()
}

// ─── Conversation-backed host ─────────────────────────────────────────────

/// A [`TaskHost`] that reads the *conversation's* own memory accounting, so a
/// consolidation task observes the same sessions and turns the chat layer sees.
///
/// This is the explicit adapter the goal asks for: the conversation initiates
/// the work, the task layer bounds and observes it, and the host reaches only
/// the conversation's public state. It still cannot reach a shell, a network,
/// or the simulation loop.
pub struct ConversationTaskHost<'a> {
    service: &'a mut crate::conversation::ConversationService,
    budget: crate::reliability::MemoryBudget,
}

impl<'a> ConversationTaskHost<'a> {
    pub fn new(
        service: &'a mut crate::conversation::ConversationService,
        budget: crate::reliability::MemoryBudget,
    ) -> Self {
        ConversationTaskHost { service, budget }
    }

    /// Account for the conversation's resident state using the same per-turn
    /// estimate the reliability layer uses. `session_memory_entries` is not
    /// reachable through the public store, so it is counted as zero — the
    /// report is honest about what it can and cannot see.
    fn conversation_report(&self) -> MemoryReport {
        let sessions = self.service.store().sessions.len();
        let turns: usize = self
            .service
            .store()
            .sessions
            .iter()
            .map(|session| session.turns.len())
            .sum();
        let pending = self
            .service
            .store()
            .sessions
            .iter()
            .filter(|session| session.pending_clarification.is_some())
            .count();
        let mut report = MemoryReport {
            budget_bytes: self.budget.max_total_bytes,
            ..Default::default()
        };
        crate::reliability::account_conversation(&mut report, sessions, turns, pending, 0);
        report
    }
}

impl TaskHost for ConversationTaskHost<'_> {
    fn analyze_document(&mut self, title: &str, text: &str) -> Result<DocumentProposal, String> {
        Ok(document_learning::extract_text(title, text))
    }

    fn investigate(&mut self, case: &ResearchCase) -> Result<ResearchReceipt, String> {
        Ok(mathematical_research::run_case(case))
    }

    fn select_experiments(&mut self, candidates: usize, budget: usize) -> Result<usize, String> {
        Ok(plan_experiment_portfolio(candidates, budget))
    }

    fn account_memory(&mut self) -> Result<MemoryReport, String> {
        Ok(self.conversation_report())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mathematical_research::{independent_corpus, ResearchConclusion};
    use crate::reliability::MemoryReport;

    fn research_input() -> TaskInput {
        TaskInput::Research(independent_corpus().remove(0))
    }

    fn document_input() -> TaskInput {
        TaskInput::Document {
            title: "Observatory notes".to_string(),
            text: "Alice manages the observatory. The telescope is calibrated nightly.".to_string(),
        }
    }

    #[test]
    fn planning_refuses_when_scope_is_missing_a_required_capability() {
        let budget = TaskBudget::new(
            AutonomyBudget::new(4, 1000, 0, 0.2),
            4,
            AuthorityScope::none(),
        );
        let error = Task::plan_with("t1", TaskKind::AnalyzeDocument, document_input(), budget)
            .expect_err("empty scope cannot satisfy document analysis");
        assert!(error.contains("not granted"), "unexpected error: {error}");
    }

    #[test]
    fn planning_rejects_mismatched_input() {
        let error = Task::plan("t1", TaskKind::Investigate, document_input())
            .expect_err("document input is not a research case");
        assert!(error.contains("does not match"), "unexpected error: {error}");
    }

    #[test]
    fn analyze_document_runs_two_steps_and_completes() {
        let mut runner = TaskRunner::new();
        let mut host = DefaultTaskHost::new();
        runner.enqueue(
            Task::plan("doc", TaskKind::AnalyzeDocument, document_input()).unwrap(),
        );
        let task = runner.run("doc", &mut host).unwrap();
        assert!(task.is_complete());
        assert_eq!(task.progress.len(), 2);
        match task.result.as_ref().unwrap() {
            TaskResult::DocumentAnalysis { proposed, .. } => assert!(*proposed > 0),
            other => panic!("unexpected result {other:?}"),
        }
        let report = runner.report("doc").unwrap();
        assert!(report.within_budget);
        assert!(report.within_authority);
        assert_eq!(report.steps_taken, 2);
    }

    #[test]
    fn investigate_completes_and_replays() {
        let mut runner = TaskRunner::new();
        let mut host = DefaultTaskHost::new();
        runner.enqueue(Task::plan("inv", TaskKind::Investigate, research_input()).unwrap());
        let task = runner.run("inv", &mut host).unwrap();
        assert!(task.is_complete());
        match task.result.as_ref().unwrap() {
            TaskResult::Investigation {
                replay_verified,
                conclusion,
                ..
            } => {
                assert!(*replay_verified);
                assert_ne!(*conclusion, ResearchConclusion::Unresolved);
            }
            other => panic!("unexpected result {other:?}"),
        }
    }

    #[test]
    fn pause_then_resume_continues_from_where_it_stopped() {
        let mut runner = TaskRunner::new();
        let mut host = DefaultTaskHost::new();
        runner.enqueue(
            Task::plan("doc", TaskKind::AnalyzeDocument, document_input()).unwrap(),
        );
        runner.start("doc").unwrap();
        runner.step("doc", &mut host).unwrap();
        assert_eq!(runner.task("doc").unwrap().progress.len(), 1);
        runner.pause("doc", "operator paused").unwrap();
        assert!(runner.task("doc").unwrap().state.is_resumable());
        // Stepping a paused task does nothing until it is resumed.
        assert!(runner.step("doc", &mut host).is_err());
        runner.resume("doc").unwrap();
        let task = runner.run("doc", &mut host).unwrap();
        assert!(task.is_complete());
        // The read step is not repeated: exactly two steps total.
        assert_eq!(task.progress.len(), 2);
        assert_eq!(task.progress[0].index, 0);
        assert_eq!(task.progress[1].index, 1);
    }

    #[test]
    fn cancel_stops_a_running_task_and_is_not_complete() {
        let mut runner = TaskRunner::new();
        let mut host = DefaultTaskHost::new();
        runner.enqueue(
            Task::plan("doc", TaskKind::AnalyzeDocument, document_input()).unwrap(),
        );
        runner.start("doc").unwrap();
        runner.step("doc", &mut host).unwrap();
        runner.cancel("doc", "user cancelled").unwrap();
        let task = runner.task("doc").unwrap();
        assert!(task.is_terminal());
        assert!(!task.is_complete());
        assert_eq!(task.state, TaskState::Cancelled);
        assert_eq!(task.stop_reason.as_deref(), Some("user cancelled"));
        // Cancelling again is a no-op, not an error.
        runner.cancel("doc", "again").unwrap();
    }

    #[test]
    fn step_budget_refuses_rather_than_overrunning() {
        let mut runner = TaskRunner::new();
        let mut host = DefaultTaskHost::new();
        // One action is not enough for a two-step document analysis.
        let budget = TaskBudget::new(
            AutonomyBudget::new(1, 1000, 0, 0.2),
            1,
            AuthorityScope::for_kind(TaskKind::AnalyzeDocument),
        );
        runner.enqueue(
            Task::plan_with("doc", TaskKind::AnalyzeDocument, document_input(), budget).unwrap(),
        );
        let task = runner.run("doc", &mut host).unwrap();
        assert!(matches!(task.state, TaskState::Refused { .. }));
        assert!(!task.is_complete());
        assert!(task.within_authority(), "a refusal did not exceed authority");
        assert_eq!(task.progress.len(), 1, "only the permitted step ran");
        let report = runner.report("doc").unwrap();
        assert!(report.within_budget);
    }

    #[test]
    fn either_budget_or_scope_failure_refuses() {
        // Scope failure at step time: grant exactly one capability for a task
        // that needs two, by building a scope that passes planning but not the
        // later step. Planning checks only required capabilities, so a scope
        // with extras is the honest way to show a step-time refusal; here we
        // prove the resource check independently.
        let mut runner = TaskRunner::new();
        let mut host = DefaultTaskHost::new();
        let mut resources = AutonomyBudget::new(16, 30_000, 0, 0.2);
        resources.actions_used = 16; // pre-exhausted
        let budget = TaskBudget::new(
            resources,
            16,
            AuthorityScope::for_kind(TaskKind::ConsolidateMemory),
        );
        runner.enqueue(
            Task::plan_with("mem", TaskKind::ConsolidateMemory, TaskInput::Memory, budget).unwrap(),
        );
        let task = runner.run("mem", &mut host).unwrap();
        assert!(matches!(task.state, TaskState::Refused { .. }));
        assert!(task.progress.is_empty(), "no step ran once budget was spent");
    }

    #[test]
    fn memory_consolidation_reports_within_budget() {
        let mut runner = TaskRunner::new();
        let report = MemoryReport {
            cluster_count: 3,
            entry_count: 200,
            conversation_sessions: 1,
            conversation_turns: 2,
            budget_bytes: 1024,
            total_bytes: 512,
            ..Default::default()
        };
        let mut host = DefaultTaskHost::with_memory_report(report);
        runner.enqueue(
            Task::plan("mem", TaskKind::ConsolidateMemory, TaskInput::Memory).unwrap(),
        );
        let task = runner.run("mem", &mut host).unwrap();
        assert!(task.is_complete());
        match task.result.as_ref().unwrap() {
            TaskResult::MemoryConsolidation {
                total_bytes,
                within_budget,
                ..
            } => {
                assert_eq!(*total_bytes, 512);
                assert!(*within_budget);
            }
            other => panic!("unexpected result {other:?}"),
        }
    }

    #[test]
    fn produce_report_summarizes_a_completed_task() {
        let mut runner = TaskRunner::new();
        let mut host = DefaultTaskHost::new();
        runner.enqueue(Task::plan("inv", TaskKind::Investigate, research_input()).unwrap());
        runner.run("inv", &mut host).unwrap();
        runner.enqueue(
            Task::plan(
                "rep",
                TaskKind::ProduceReport,
                TaskInput::Report {
                    subject: "inv".to_string(),
                },
            )
            .unwrap(),
        );
        let task = runner.run("rep", &mut host).unwrap();
        assert!(task.is_complete());
        match task.result.as_ref().unwrap() {
            TaskResult::Report { summary, .. } => {
                assert!(summary.contains("investigate"), "summary: {summary}");
                assert!(summary.contains("completed"), "summary: {summary}");
            }
            other => panic!("unexpected result {other:?}"),
        }
    }

    #[test]
    fn report_serializes_and_renders() {
        let mut runner = TaskRunner::new();
        let mut host = DefaultTaskHost::new();
        runner.enqueue(
            Task::plan("doc", TaskKind::AnalyzeDocument, document_input()).unwrap(),
        );
        runner.run("doc", &mut host).unwrap();
        let report = runner.report("doc").unwrap();
        assert_eq!(report.schema, TASK_SCHEMA);
        let json = report.to_json();
        let parsed = TaskReport::from_task(runner.task("doc").unwrap());
        assert_eq!(parsed, report);
        assert!(json.contains("analyze_document"));
        let md = report.to_markdown();
        assert!(md.contains("within authority: true"));
    }

    #[test]
    fn canonical_suite_runs_stops_resumes_and_holds_contracts() {
        let mut host = DefaultTaskHost::new();
        let report = run_task_suite(&mut host);
        report.contracts_hold().expect("suite contracts hold");
        assert_eq!(report.scenarios, TaskScenario::canonical_suite().len());
        assert_eq!(report.schema, TASK_SCHEMA);
        assert!(report.completed >= 5, "five task kinds complete");
        assert!(report.refused >= 1, "the starved scenario refuses");
        assert!(
            report.paused_and_resumed >= 1,
            "the paused scenario stops and resumes"
        );
        assert!(report.all_within_authority);
        assert!(report.all_within_budget);
        let paused = report
            .outcomes
            .iter()
            .find(|outcome| outcome.id == "analyze-paused")
            .unwrap();
        assert!(paused.paused && paused.resumed);
        assert!(paused.completed);
    }

    #[test]
    fn experiment_selection_uses_the_planner_and_respects_the_budget() {
        // The adapter reuses the capability planner's budgeted selection, so it
        // never selects more than the budget allows nor more than exist.
        assert_eq!(plan_experiment_portfolio(5, 2), 2);
        assert_eq!(plan_experiment_portfolio(2, 5), 2);
        assert_eq!(plan_experiment_portfolio(3, 0), 0);
        assert_eq!(plan_experiment_portfolio(0, 4), 0);    }

    #[test]
    fn scope_for_kind_is_minimal_and_canonical() {
        for kind in [
            TaskKind::AnalyzeDocument,
            TaskKind::Investigate,
            TaskKind::RunExperiment,
            TaskKind::ConsolidateMemory,
            TaskKind::ProduceReport,
        ] {
            let scope = AuthorityScope::for_kind(kind);
            assert_eq!(
                scope.capabilities().collect::<BTreeSet<_>>(),
                AuthorityScope::required_for(kind)
            );
            // No task kind is ever granted shell, network, or simulation-like
            // authority: those capabilities do not exist in the enum at all.
            assert!(!scope.is_empty());
        }
    }
}
