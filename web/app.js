"use strict";

/* The Machine — local chat interface.
 * Talks to the Phase 1 conversational runtime through /api/*.
 * No frameworks, no build step: this file is served verbatim by chat_server. */

const state = {
  sessions: [],
  sessionId: null,
  session: null,
  mode: "ask",
  sending: false,
  turnId: null,
  memoryTimer: null,
};

const els = {
  list: document.getElementById("conversation-list"),
  newConversation: document.getElementById("new-conversation"),
  messages: document.getElementById("messages"),
  emptyState: document.getElementById("empty-state"),
  sessionTitle: document.getElementById("session-title"),
  sessionMeta: document.getElementById("session-meta"),
  exportLink: document.getElementById("export-link"),
  status: document.getElementById("status"),
  statusText: document.getElementById("status-text"),
  input: document.getElementById("composer-input"),
  send: document.getElementById("send-button"),
  cancel: document.getElementById("cancel-button"),
  modes: Array.from(document.querySelectorAll(".mode")),
  memoryToggle: document.getElementById("memory-toggle"),
  memoryPanel: document.getElementById("memory-panel"),
  memoryQuery: document.getElementById("memory-query"),
  memoryCounts: document.getElementById("memory-counts"),
  memoryResults: document.getElementById("memory-results"),
};

/* ---------- helpers ---------- */

function el(tag, className, text) {
  const node = document.createElement(tag);
  if (className) node.className = className;
  if (text !== undefined) node.textContent = text;
  return node;
}

async function api(path, options) {
  const response = await fetch(path, options);
  if (!response.ok) {
    let message = response.status + " " + response.statusText;
    try {
      const body = await response.json();
      if (body && body.error) message = body.error;
    } catch (_) {
      /* not JSON */
    }
    throw new Error(message);
  }
  return response.json();
}

function relativeTime(iso) {
  const then = Date.parse(iso);
  if (Number.isNaN(then)) return "";
  const seconds = Math.max(0, (Date.now() - then) / 1000);
  if (seconds < 45) return "just now";
  if (seconds < 3600) return Math.round(seconds / 60) + "m ago";
  if (seconds < 86400) return Math.round(seconds / 3600) + "h ago";
  return Math.round(seconds / 86400) + "d ago";
}

function plural(count, noun) {
  return count + " " + noun + (count === 1 ? "" : "s");
}

/* ---------- conversation list ---------- */

async function refreshSessions() {
  state.sessions = await api("/api/sessions");
  renderSessionList();
}

function renderSessionList() {
  els.list.replaceChildren();
  if (!state.sessions.length) {
    els.list.append(el("p", "muted", "No conversations yet."));
    return;
  }
  for (const session of state.sessions) {
    const button = el(
      "button",
      "conversation-item" + (session.id === state.sessionId ? " active" : "")
    );
    button.append(el("span", "title", session.title || "New conversation"));
    const parts = [plural(session.turn_count, "turn")];
    if (session.stale_count) parts.push(plural(session.stale_count, "stale answer"));
    if (session.awaiting_clarification) parts.push("awaiting detail");
    else if (session.last_turn_at) parts.push(relativeTime(session.last_turn_at));
    button.append(el("span", "sub", parts.join(" · ")));
    button.addEventListener("click", () => {
      openSession(session.id).catch(showFatal);
    });
    els.list.append(button);
  }
}

async function createSession() {
  const session = await api("/api/sessions", { method: "POST" });
  await refreshSessions();
  await openSession(session.id);
  return session.id;
}

async function openSession(id) {
  state.sessionId = id;
  state.session = await api("/api/sessions/" + encodeURIComponent(id));
  renderSessionList();
  renderMessages();
  updateHeader();
}

function updateHeader() {
  if (!state.session) {
    els.sessionTitle.textContent = "No conversation";
    els.sessionMeta.textContent = "";
    els.exportLink.hidden = true;
    return;
  }
  els.sessionTitle.textContent =
    state.session.turns.length && state.session.turns[0].input
      ? truncate(state.session.turns[0].input, 72)
      : "New conversation";
  const parts = [
    plural(state.session.turns.length, "turn"),
    "created " + relativeTime(state.session.created_at),
  ];
  const stale = state.session.turns.filter((turn) => turn.stale).length;
  if (stale) parts.push(plural(stale, "stale answer"));
  if (state.session.pending_clarification) parts.push("awaiting detail");
  els.sessionMeta.textContent = parts.join(" · ");
  els.exportLink.hidden = false;
  els.exportLink.href =
    "/api/sessions/" + encodeURIComponent(state.session.id) + "/export?format=markdown";
}

function truncate(text, max) {
  const trimmed = text.trim();
  return trimmed.length > max ? trimmed.slice(0, max - 1) + "…" : trimmed;
}

/* ---------- transcript ---------- */

function renderMessages() {
  els.messages.replaceChildren();
  const turns = state.session ? state.session.turns : [];
  if (!turns.length) {
    els.messages.append(els.emptyState);
    els.emptyState.hidden = false;
    return;
  }
  els.emptyState.hidden = true;
  for (const turn of turns) {
    els.messages.append(renderUserMessage(turn.input));
    els.messages.append(renderAssistantTurn(turn.result, turn));
  }
  scrollToBottom();
}

function renderUserMessage(text) {
  const message = el("div", "message user");
  message.append(el("div", "who", "You"));
  message.append(el("div", "bubble", text));
  return message;
}

const OUTCOME_LABELS = {
  answered: "Answered",
  clarification_needed: "Needs detail",
  unsupported: "No answer",
  failed: "Failed",
  cancelled: "Cancelled",
};

const EVIDENCE_LABELS = {
  retrieved_claim: "retrieved claim",
  computed_answer: "computed answer",
  proof_checked_conclusion: "proof-checked conclusion",
};

function outcomeOf(outcome) {
  if (typeof outcome === "string") return outcome;
  return Object.keys(outcome || {})[0] || "failed";
}

function outcomeDetail(outcome) {
  if (typeof outcome !== "object" || outcome === null) return "";
  const value = outcome[Object.keys(outcome)[0]] || {};
  return value.reason || value.error || value.question || "";
}

function renderAssistantTurn(result, turn) {
  const message = el("div", "message assistant");
  message.append(el("div", "who", "Machine"));
  const bubble = el("div", "bubble");
  const outcome = el("div", "outcome");
  const key = outcomeOf(result.outcome);
  outcome.append(el("span", "badge " + key, OUTCOME_LABELS[key] || key));
  const detail = outcomeDetail(result.outcome);
  if (detail) outcome.append(el("span", "muted", detail));
  const capability = capabilityLabel(result.capability);
  if (capability && capability !== "none") {
    outcome.append(el("span", "muted", "via " + capability));
  }
  bubble.append(outcome);
  if (turn && turn.stale) {
    const stale = el("div", "stale-note", "Stale: " + turn.stale.reason);
    bubble.append(stale);
  }
  bubble.append(el("div", "answer", result.answer_text || "Done."));
  message.append(bubble);
  const details = renderDetails(result);
  if (details) message.append(details);
  return message;
}

function capabilityLabel(capability) {
  if (capability === "factual_qa") return "factual QA";
  if (capability === "causal_chain") return "causal chain";
  if (capability === "none") return "none";
  if (capability && capability.structured_solver) {
    return "solver: " + capability.structured_solver.domain;
  }
  if (typeof capability === "string") return capability.replace(/_/g, " ");
  return String(capability);
}

function renderDetails(result) {
  const hasContent =
    (result.evidence && result.evidence.length) ||
    result.verification !== "not_attempted" ||
    (result.memory_changes && result.memory_changes.length) ||
    result.interpretation ||
    result.timing ||
    result.diagnostics;
  if (!hasContent) return null;

  const details = el("details", "details");
  details.append(el("summary", null, "Evidence & execution details"));

  if (result.evidence && result.evidence.length) {
    const section = detailSection("Evidence");
    for (const item of result.evidence) {
      const row = el("div", "evidence-item");
      row.append(el("div", "content", item.content));
      const meta = [
        EVIDENCE_LABELS[item.kind] || item.kind,
        "source: " + item.provenance,
        "conf " + Number(item.confidence).toFixed(2),
      ];
      if (item.replay_verified === true) meta.push("replay verified");
      if (item.replay_verified === false) meta.push("replay failed");
      row.append(el("div", "kv", meta.join(" · ")));
      section.append(row);
    }
    details.append(section);
  }

  if (result.verification && result.verification !== "not_attempted") {
    const section = detailSection("Verification");
    section.append(el("div", null, verificationText(result.verification)));
    details.append(section);
  }

  if (result.memory_changes && result.memory_changes.length) {
    const section = detailSection("Memory changes");
    for (const change of result.memory_changes) {
      const row = el("div", "memory-change");
      row.append(el("div", null, change.operation + ": " + change.detail));
      row.append(el("div", "kv", "source: " + change.provenance));
      section.append(row);
    }
    details.append(section);
  }

  if (result.interpretation) {
    const section = detailSection("Interpretation");
    section.append(
      el(
        "div",
        null,
        "Read as " + result.interpretation.kind.replace(/_/g, " ") + " · capability " + capabilityLabel(result.capability)
      )
    );
    if (result.interpretation.normalized_input && result.interpretation.normalized_input !== "") {
      section.append(el("div", "kv", "input: " + result.interpretation.normalized_input));
    }
    const followUp = result.interpretation.follow_up;
    if (followUp) {
      const bindings = (followUp.resolved || [])
        .map((pair) => pair[0] + " → " + pair[1])
        .join(", ");
      const suffix = bindings ? " (" + bindings + ")" : "";
      section.append(
        el("div", "kv", "follow-up: " + followUp.kind + " · " + followUp.source + suffix)
      );
    }
    for (const note of result.interpretation.notes || []) {
      section.append(el("div", "note", note));
    }
    details.append(section);
  }

  if (result.timing || result.diagnostics) {
    const section = detailSection("Execution");
    const diag = result.diagnostics || {};
    const bits = [];
    if (result.timing) bits.push(Number(result.timing.elapsed_ms).toFixed(0) + " ms");
    if (diag.turn_id) bits.push("turn " + diag.turn_id);
    if (diag.episode_id) bits.push("episode " + diag.episode_id);
    if (typeof diag.fact_count_before === "number") {
      bits.push(
        "facts " + diag.fact_count_before + "→" + diag.fact_count_after +
        ", rules " + diag.rule_count_before + "→" + diag.rule_count_after
      );
    }
    section.append(el("div", "kv", bits.join(" · ")));
    details.append(section);
  }

  return details;
}

function detailSection(title) {
  const section = el("div", "detail-section");
  section.append(el("h4", null, title));
  return section;
}

function verificationText(verification) {
  if (verification && verification.verified) {
    return "Verified via " + verification.verified.method + " (" + Number(verification.verified.score).toFixed(2) + ")";
  }
  if (verification && verification.unverified) {
    return "Unverified: " + verification.unverified.reason;
  }
  return "Not attempted";
}

function scrollToBottom() {
  els.messages.scrollTop = els.messages.scrollHeight;
}

/* ---------- turn submission ---------- */

function setMode(mode) {
  state.mode = mode;
  for (const button of els.modes) {
    const selected = button.dataset.mode === mode;
    button.classList.toggle("selected", selected);
    button.setAttribute("aria-checked", selected ? "true" : "false");
  }
  const placeholders = {
    ask: "Ask a question…",
    teach: "Teach a fact (“The Fed raises rates”) or a rule (“If The Fed raises rates then treasury yields rise across the curve”)",
    inspect: "Search memory, e.g. the_fed",
  };
  els.input.placeholder = placeholders[mode] || "";
}

function setSending(sending) {
  state.sending = sending;
  els.send.disabled = sending;
  els.input.disabled = sending;
  els.cancel.hidden = !sending;
  els.cancel.disabled = false;
  els.status.hidden = !sending;
}

function setStatus(text) {
  els.statusText.textContent = text;
}

async function send() {
  const text = els.input.value.trim();
  if (!text || state.sending) return;

  if (state.mode === "inspect") {
    els.input.value = "";
    await inspectMemory(text);
    return;
  }

  if (!state.sessionId) await createSession();

  const userMessage = renderUserMessage(text);
  els.messages.append(userMessage);
  const pending = el("div", "message assistant pending");
  pending.append(el("div", "who", "Machine"));
  pending.append(el("div", "bubble", "…"));
  els.messages.append(pending);
  els.emptyState.hidden = true;
  scrollToBottom();

  els.input.value = "";
  resizeInput();
  setSending(true);
  setStatus(state.mode === "teach" ? "Updating memory…" : "Retrieving facts…");

  let result = null;
  let failure = null;
  try {
    await streamTurn(state.sessionId, text, state.mode, {
      started(data) {
        state.turnId = data.turn_id;
      },
      stage(data) {
        setStatus(data.label + "…");
      },
      turn(data) {
        result = data;
      },
      error(data) {
        failure = data.error || "turn failed";
      },
    });
  } catch (error) {
    failure = error.message;
  }

  pending.remove();
  if (result) {
    els.messages.append(renderAssistantTurn(result));
    if (state.session) {
      state.session.turns.push({
        turn_id: result.diagnostics ? result.diagnostics.turn_id : state.turnId,
        at: new Date().toISOString(),
        input: text,
        result: result,
      });
    }
    setStatus("Done");
  } else {
    const message = el("div", "message assistant");
    message.append(el("div", "who", "Machine"));
    const bubble = el("div", "bubble");
    bubble.append(el("span", "badge failed", "Failed"));
    bubble.append(el("div", "answer", failure || "The request could not be completed."));
    message.append(bubble);
    els.messages.append(message);
  }
  scrollToBottom();
  state.turnId = null;
  setSending(false);
  await refreshSessions().catch(() => {});
  updateHeader();
}

async function streamTurn(sessionId, text, mode, handlers) {
  const response = await fetch(
    "/api/sessions/" + encodeURIComponent(sessionId) + "/turns",
    {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ text: text, mode: mode }),
    }
  );
  if (!response.ok) {
    let message = response.status + " " + response.statusText;
    try {
      const body = await response.json();
      if (body && body.error) message = body.error;
    } catch (_) {
      /* not JSON */
    }
    throw new Error(message);
  }

  const reader = response.body.getReader();
  const decoder = new TextDecoder();
  let buffer = "";
  for (;;) {
    const chunk = await reader.read();
    if (chunk.done) break;
    buffer += decoder.decode(chunk.value, { stream: true });
    let boundary;
    while ((boundary = buffer.indexOf("\n\n")) >= 0) {
      dispatchSse(buffer.slice(0, boundary), handlers);
      buffer = buffer.slice(boundary + 2);
    }
  }
  if (buffer.trim()) dispatchSse(buffer, handlers);
}

function dispatchSse(chunk, handlers) {
  let event = "message";
  const dataLines = [];
  for (const rawLine of chunk.split("\n")) {
    const line = rawLine.replace(/\r$/, "");
    if (line.startsWith(":")) continue;
    if (line.startsWith("event:")) event = line.slice(6).trim();
    else if (line.startsWith("data:")) dataLines.push(line.slice(5).trimStart());
  }
  if (!dataLines.length) return;
  let data;
  try {
    data = JSON.parse(dataLines.join("\n"));
  } catch (_) {
    return;
  }
  const handler = handlers[event];
  if (handler) handler(data);
}

async function cancelTurn() {
  if (!state.turnId) return;
  els.cancel.disabled = true;
  setStatus("Cancelling…");
  try {
    await api("/api/turns/" + encodeURIComponent(state.turnId) + "/cancel", { method: "POST" });
  } catch (_) {
    /* the stream will report the outcome */
  }
}

/* ---------- memory inspector ---------- */

async function inspectMemory(query) {
  els.messages.append(renderUserMessage("Inspect memory: " + query));
  els.emptyState.hidden = true;
  const message = el("div", "message assistant");
  message.append(el("div", "who", "Memory inspector"));
  const bubble = el("div", "bubble");
  bubble.append(el("div", "answer", "Searching memory…"));
  message.append(bubble);
  els.messages.append(message);
  scrollToBottom();

  try {
    const snapshot = await fetchMemory(query);
    bubble.replaceChildren();
    bubble.append(el("div", "answer", plural(snapshot.facts, "fact") + " · " + plural(snapshot.rules, "rule") + " shared"));
    bubble.append(el("div", "muted", "Inspection is read-only and is not recorded as a conversation turn."));
    if (snapshot.assertions.length) {
      const details = el("details", "details");
      details.open = true;
      details.append(el("summary", null, "Matches for “" + query + "”"));
      for (const assertion of snapshot.assertions) {
        details.append(renderAssertionItem(assertion, false));
      }
      bubble.append(details);
    } else {
      bubble.append(el("div", "note", "No assertions matched."));
    }
    openMemoryPanel(query);
  } catch (error) {
    bubble.replaceChildren(el("div", "answer", "Memory search failed: " + error.message));
  }
  scrollToBottom();
}

async function fetchMemory(query) {
  const suffix = query ? "?q=" + encodeURIComponent(query) : "";
  return api("/api/memory" + suffix);
}

function factText(fact) {
  return fact ? fact.subject + " " + fact.verb + " " + fact.object : "";
}

function assertionStatement(assertion) {
  const payload = assertion.payload || {};
  if (payload.type === "fact") return factText(payload);
  if (payload.type === "rule") {
    return "if " + factText(payload.antecedent) + " then " + factText(payload.consequent);
  }
  return assertion.id;
}

function renderAssertionItem(assertion, interactive) {
  const row = el("div", "evidence-item assertion " + assertion.status);
  row.append(el("div", "content", assertionStatement(assertion)));
  const source = assertion.source || {};
  const meta = [
    assertion.kind + " · " + assertion.status + " · v" + assertion.version,
    "source: " + (source.kind || "unknown"),
    source.session_id ? "session " + source.session_id : "shared knowledge",
    source.turn_id ? "turn " + source.turn_id : null,
    "added " + relativeTime(assertion.created_at),
  ].filter(Boolean);
  row.append(el("div", "kv", meta.join(" · ")));
  if (interactive) {
    const actions = el("div", "assertion-actions");
    actions.append(actionButton("History", () => showHistory(assertion)));
    if (assertion.status === "active") {
      actions.append(actionButton("Correct", () => correctAssertion(assertion)));
      actions.append(actionButton("Retract", () => retractAssertion(assertion)));
    }
    actions.append(actionButton("Forget", () => forgetAssertion(assertion)));
    row.append(actions);
  }
  return row;
}

function actionButton(label, handler) {
  const button = el("button", "mini", label);
  button.type = "button";
  button.addEventListener("click", () => {
    Promise.resolve(handler()).catch((error) => {
      window.alert("Operation failed: " + error.message);
    });
  });
  return button;
}

async function correctAssertion(assertion) {
  const replacement = window.prompt(
    "Corrected statement:",
    assertionStatement(assertion)
  );
  if (replacement === null || !replacement.trim()) return;
  const note = window.prompt("Note (optional):", "") || "";
  await api(
    "/api/memory/assertions/" + encodeURIComponent(assertion.id) + "/correct",
    {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        text: replacement.trim(),
        note: note.trim(),
        session_id: state.sessionId,
      }),
    }
  );
  await refreshMemoryPanel(els.memoryQuery.value.trim());
}

async function retractAssertion(assertion) {
  if (
    !window.confirm(
      "Retract this assertion? It stays as a tombstone and dependent answers are marked stale."
    )
  ) {
    return;
  }
  const reason = window.prompt("Reason:", "retracted in the interface") || "retracted in the interface";
  await api(
    "/api/memory/assertions/" + encodeURIComponent(assertion.id) + "/retract",
    {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ reason: reason, session_id: state.sessionId }),
    }
  );
  await refreshMemoryPanel(els.memoryQuery.value.trim());
}

async function forgetAssertion(assertion) {
  if (
    !window.confirm(
      "Forget this assertion permanently? The record and its version history are deleted."
    )
  ) {
    return;
  }
  await api(
    "/api/memory/assertions/" + encodeURIComponent(assertion.id) + "/forget",
    {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ confirm: true }),
    }
  );
  await refreshMemoryPanel(els.memoryQuery.value.trim());
}

async function showHistory(assertion) {
  const versions = await api(
    "/api/memory/assertions/" + encodeURIComponent(assertion.id) + "/history"
  );
  els.memoryResults.replaceChildren();
  els.memoryResults.append(
    actionButton("← Back", () => refreshMemoryPanel(els.memoryQuery.value.trim()))
  );
  els.memoryResults.append(
    el("div", "memory-group-title", "History — " + assertionStatement(assertion))
  );
  for (const version of versions) {
    const item = el("div", "memory-item");
    item.append(
      el(
        "div",
        null,
        "v" + version.version + " · " + version.status + " · " + relativeTime(version.recorded_at)
      )
    );
    if (version.note) item.append(el("div", "meta", version.note));
    els.memoryResults.append(item);
  }
}

function openMemoryPanel(query) {
  els.memoryPanel.hidden = false;
  els.memoryToggle.setAttribute("aria-expanded", "true");
  els.memoryQuery.value = query;
  refreshMemoryPanel(query).catch(() => {});
}

async function refreshMemoryPanel(query) {
  const snapshot = await fetchMemory(query);
  els.memoryCounts.textContent =
    plural(snapshot.facts, "fact") + " · " + plural(snapshot.rules, "rule") + " shared";
  els.memoryResults.replaceChildren();
  els.memoryResults.append(
    el("div", "memory-group-title", "Shared knowledge (" + snapshot.assertions.length + ")")
  );
  if (!snapshot.assertions.length) {
    els.memoryResults.append(el("p", "muted", "No matching assertions."));
  } else {
    for (const assertion of snapshot.assertions) {
      els.memoryResults.append(renderAssertionItem(assertion, true));
    }
  }
}

/* ---------- wiring ---------- */

function resizeInput() {
  els.input.style.height = "auto";
  els.input.style.height = Math.min(els.input.scrollHeight, 200) + "px";
}

function showFatal(error) {
  els.messages.replaceChildren();
  const message = el("div", "message assistant");
  message.append(el("div", "who", "The Machine"));
  const bubble = el("div", "bubble");
  bubble.append(el("span", "badge failed", "Failed"));
  bubble.append(el("div", "answer", "Could not reach the local server: " + error.message));
  message.append(bubble);
  els.messages.append(message);
}

function wire() {
  els.newConversation.addEventListener("click", () => createSession().catch(showFatal));
  els.send.addEventListener("click", () => send().catch(showFatal));
  els.cancel.addEventListener("click", () => cancelTurn());
  els.input.addEventListener("input", resizeInput);
  els.input.addEventListener("keydown", (event) => {
    if (event.key === "Enter" && !event.shiftKey) {
      event.preventDefault();
      send().catch(showFatal);
    }
  });
  for (const button of els.modes) {
    button.addEventListener("click", () => setMode(button.dataset.mode));
  }
  els.memoryToggle.addEventListener("click", () => {
    const open = els.memoryPanel.hidden;
    els.memoryPanel.hidden = !open;
    els.memoryToggle.setAttribute("aria-expanded", open ? "true" : "false");
    if (open) refreshMemoryPanel(els.memoryQuery.value.trim()).catch(() => {});
  });
  els.memoryQuery.addEventListener("input", () => {
    clearTimeout(state.memoryTimer);
    state.memoryTimer = setTimeout(() => {
      refreshMemoryPanel(els.memoryQuery.value.trim()).catch(() => {});
    }, 200);
  });
}

async function init() {
  wire();
  setMode("ask");
  try {
    await refreshSessions();
    if (state.sessions.length) await openSession(state.sessions[0].id);
    else await createSession();
  } catch (error) {
    showFatal(error);
  }
}

init();
