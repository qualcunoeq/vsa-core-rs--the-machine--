use std::sync::Arc;
use tokio::sync::RwLock;

// ─── General Threat Detection ──────────────────────────────────────────────
//
// Domain-independent threat perception layer.  The idea is from Person of
// Interest — Harold Finch's Machine doesn't just plan, it MONITORS.  It
// detects state changes and classifies them as threats, opportunities, or
// neutral events before triggering a response.
//
// The chess instantiation is the first concrete use: after an opponent's move,
// compare attack maps before/after.  Any machine piece that is newly under
// attack is a threat.  This feeds into the planner as a defensive subgoal.

/// A domain-independent classification of a state change.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ThreatClass {
    /// Something actively harmful is happening (piece attacked, port probed)
    Threat,
    /// Something beneficial is happening (opponent blunders, opportunity opens)
    Opportunity,
    /// Change with no immediate action required
    Neutral,
}

/// A single detected event — the output of the perception layer.
#[derive(Debug, Clone)]
pub struct ThreatEvent {
    /// Domain identifier: "chess", "network", "market", etc.
    pub domain: String,
    /// How severe is this?  0.0 (negligible) to 1.0 (critical)
    pub severity: f64,
    /// What entity is affected?  "queen", "port_443", "position_401k"
    pub entity: String,
    /// Human-readable description of what happened
    pub description: String,
    /// Classification
    pub class: ThreatClass,
}

/// A domain-independent threat detector trait.
///
/// Implementations compare two snapshots of a domain state and return
/// everything that changed, classified by type.
pub trait ThreatDetector<State> {
    /// Compare two states and return all changes, each classified.
    fn detect(&self, before: &State, after: &State) -> Vec<ThreatEvent>;
}

// ─── Chess Threat Detector ─────────────────────────────────────────────────
//
/// Chess instantiation of the threat detector.
///
/// Compares attack maps before and after the opponent's move.  Any friendly
/// piece that is under attack after but was NOT under attack before is a
/// threat event.
pub struct ChessThreatDetector {
    pub machine_is_white: bool,
}

/// Intermediate representation for building a square-indexed attack map.
/// `opponent_attacks[sq]` = true if an opponent piece attacks that square.
/// `our_attacks[sq]` = true if one of our pieces attacks that square.
#[allow(dead_code)]
pub(crate) struct SquareAttackMap {
    pub opponent_attacks: [[bool; 8]; 8],
    pub our_attacks: [[bool; 8]; 8],
}

pub(crate) fn build_square_attack_map(
    pieces: &[(char, u8, u8)],
    board: &[[Option<char>; 8]; 8],
    machine_is_white: bool,
) -> SquareAttackMap {
    let mut opp = [[false; 8]; 8];
    let mut ours = [[false; 8]; 8];

    for &(ch, rank, file) in pieces {
        let is_machine_piece = ch.is_uppercase() == machine_is_white;
        let attacked = crate::chess_eval::compute_attacks(ch, rank, file, board);
        for (tr, tf) in attacked {
            let r = tr as usize;
            let f = tf as usize;
            if is_machine_piece {
                ours[r][f] = true;
            } else {
                opp[r][f] = true;
            }
        }
    }

    SquareAttackMap {
        opponent_attacks: opp,
        our_attacks: ours,
    }
}

impl ChessThreatDetector {
    pub fn new(machine_is_white: bool) -> Self {
        ChessThreatDetector { machine_is_white }
    }

    /// Parse a FEN into the piece list and board matrix needed by
    /// the attack detection functions.
    pub fn parse_state(fen: &str) -> (Vec<(char, u8, u8)>, [[Option<char>; 8]; 8]) {
        let pieces = crate::chess_eval::parse_fen(fen);
        let mut board = [[None; 8]; 8];
        for &(ch, rank, file) in &pieces {
            board[rank as usize][file as usize] = Some(ch);
        }
        (pieces, board)
    }
}

impl ThreatDetector<(Vec<(char, u8, u8)>, [[Option<char>; 8]; 8])> for ChessThreatDetector {
    fn detect(
        &self,
        before: &(Vec<(char, u8, u8)>, [[Option<char>; 8]; 8]),
        after: &(Vec<(char, u8, u8)>, [[Option<char>; 8]; 8]),
    ) -> Vec<ThreatEvent> {
        let (before_pieces, before_board) = before;
        let (after_pieces, _after_board) = after;

        let before_map =
            build_square_attack_map(before_pieces, before_board, self.machine_is_white);
        let after_map = build_square_attack_map(after_pieces, _after_board, self.machine_is_white);

        let mut events = Vec::new();

        // For each of OUR pieces in the after state, check if it's newly attacked.
        let is_white = self.machine_is_white;
        for &(ch, rank, file) in after_pieces {
            // Only check machine's own pieces
            if ch.is_uppercase() != is_white {
                continue;
            }
            let r = rank as usize;
            let f = file as usize;

            let was_attacked = before_map.opponent_attacks[r][f];
            let is_attacked = after_map.opponent_attacks[r][f];

            // New threat: attacked now but not before
            if is_attacked && !was_attacked {
                let value = crate::chess_eval::piece_value(ch);
                // Normalize piece value to severity (queen=9 → 0.9, pawn=1 → 0.2)
                let severity = (value as f64).max(1.0) / 10.0;
                let label = crate::chess_eval::piece_label(ch);
                let sq_name = format!("{}{}", (b'a' + file) as char, rank + 1);

                events.push(ThreatEvent {
                    domain: "chess".to_string(),
                    severity: severity.clamp(0.0, 1.0),
                    entity: format!("{}_{}", label, sq_name),
                    description: format!("{} on {} is newly under attack", label, sq_name),
                    class: ThreatClass::Threat,
                });
            }
        }

        events
    }
}

// ─── System Threat Detector ─────────────────────────────────────────────────
//
/// System state threat detector: classifies SVO triples from the SystemEncoder
/// using causal rules stored in the QA engine.
///
/// Detects:
///   - Unknown processes with outbound connections
///   - Processes accessing sensitive files (/etc/passwd, /etc/shadow)
///   - Processes running as root with unexpected network activity
///   - New listening ports
///
/// The threat rules are stored as QA causal rules so the system can learn
/// and update them through experience, just like chess rules.
pub struct SystemThreatDetector {
    /// Known entities from previous snapshots (for diff-based detection)
    pub known_processes: std::collections::HashSet<String>,
    pub known_connections: std::collections::HashSet<String>,
    pub known_listeners: std::collections::HashSet<String>,
    pub baseline_established: bool,
}

/// Diff two sets of SVO triples and return only the new ones (present in
/// `after` but not in `before`).
pub fn diff_triples(
    before: &[crate::perception::SvoTriple],
    after: &[crate::perception::SvoTriple],
) -> Vec<crate::perception::SvoTriple> {
    let before_set: std::collections::HashSet<&crate::perception::SvoTriple> =
        before.iter().collect();
    after
        .iter()
        .filter(|t| !before_set.contains(t))
        .cloned()
        .collect()
}

/// Seed threat detection rules into a QA engine.
///
/// These are the baseline patterns.  The QA engine can learn additional
/// patterns through experience (via evaluate_plan_outcome).
pub fn seed_threat_rules(qa: &mut crate::qa::QaEngine) {
    // Network threats
    qa.store_rule(
        "process",
        "connected_to",
        "external_ip",
        "connection",
        "is",
        "suspicious",
        "threat_model",
    );
    qa.store_rule(
        "process",
        "listening_on",
        "high_port",
        "process",
        "may_be",
        "backdoor",
        "threat_model",
    );

    // File access threats
    qa.store_rule(
        "process",
        "reading",
        "/etc/passwd",
        "process",
        "may_be",
        "credential_harvesting",
        "threat_model",
    );
    qa.store_rule(
        "process",
        "reading",
        "/etc/shadow",
        "process",
        "is",
        "privilege_escalation_attempt",
        "threat_model",
    );

    // Process threats
    qa.store_rule(
        "process",
        "has_user",
        "root",
        "unknown_process",
        "is",
        "privilege_escalation_risk",
        "threat_model",
    );
    qa.store_rule(
        "process",
        "writing_to",
        "/tmp",
        "process",
        "may_be",
        "dropping_payload",
        "threat_model",
    );

    // Composite: process with both network activity AND sensitive file access
    qa.store_rule(
        "connection",
        "is",
        "suspicious",
        "system",
        "has",
        "network_threat",
        "threat_model",
    );
    qa.store_rule(
        "process",
        "may_be",
        "credential_harvesting",
        "system",
        "has",
        "data_exfiltration_risk",
        "threat_model",
    );
}

impl SystemThreatDetector {
    pub fn new() -> Self {
        SystemThreatDetector {
            known_processes: std::collections::HashSet::new(),
            known_connections: std::collections::HashSet::new(),
            known_listeners: std::collections::HashSet::new(),
            baseline_established: false,
        }
    }

    /// Classify a single SVO triple through the QA engine's causal rules.
    /// Returns Some(ThreatEvent) if the triple matches a threat pattern.
    #[allow(dead_code)]
    fn classify(
        &self,
        triple: &crate::perception::SvoTriple,
        qa: &crate::qa::QaEngine,
    ) -> Option<ThreatEvent> {
        let (subject, verb, object) = triple;

        // Query the QA engine: ask what this triple means
        let query_str = format!("{} {} {}", subject, verb, object);
        let result = qa.answer(&query_str);

        // If the QA engine finds a threat conclusion
        if result.contains("threat") || result.contains("risk") || result.contains("suspicious") {
            let severity = if result.contains("privilege_escalation")
                || result.contains("data_exfiltration")
            {
                0.9
            } else if result.contains("credential_harvesting") || result.contains("backdoor") {
                0.8
            } else if result.contains("suspicious") || result.contains("dropping_payload") {
                0.7
            } else {
                0.5
            };

            return Some(ThreatEvent {
                domain: "system".to_string(),
                severity,
                entity: format!("{}_{}", subject, verb),
                description: format!("{} {} {} → {}", subject, verb, object, result),
                class: ThreatClass::Threat,
            });
        }

        // Rule-based checks for common threat patterns (fast path without QA)
        // These match patterns that the QA rules above encode
        if self.is_suspicious_network(subject, verb, object) {
            let desc = format!(
                "{} {} {} — unknown process with network activity",
                subject, verb, object
            );
            return Some(ThreatEvent {
                domain: "system".to_string(),
                severity: 0.6,
                entity: subject.clone(),
                description: desc,
                class: ThreatClass::Threat,
            });
        }

        if self.is_sensitive_file_access(subject, verb, object) {
            let desc = format!("{} {} {} — sensitive file access", subject, verb, object);
            return Some(ThreatEvent {
                domain: "system".to_string(),
                severity: 0.8,
                entity: subject.clone(),
                description: desc,
                class: ThreatClass::Threat,
            });
        }

        None
    }

    /// Fast check: is this an outbound connection to an external address?
    fn is_suspicious_network(&self, _subject: &str, verb: &str, object: &str) -> bool {
        verb == "connected_to"
            && !object.contains("127.0.0.1")
            && !object.contains("0.0.0.0")
            && !object.contains("::1")
    }

    /// Fast check: is a process accessing a sensitive system file?
    fn is_sensitive_file_access(&self, _subject: &str, verb: &str, object: &str) -> bool {
        verb == "has_open"
            || verb == "reading"
            || verb == "writing_to"
                && (object.contains("/etc/passwd")
                    || object.contains("/etc/shadow")
                    || object.contains(".ssh")
                    || object.contains(".gnupg"))
    }

    /// Update known entities from a set of triples.
    /// Call this with each new snapshot to maintain the baseline.
    pub fn update_baseline(&mut self, triples: &[crate::perception::SvoTriple]) {
        for (s, v, o) in triples {
            if v == "is_running" {
                self.known_processes.insert(s.clone());
            }
            if v == "connected_to" {
                self.known_connections.insert(format!("{}_{}", s, o));
            }
            if v == "listening_on" || (v == "connected_to" && o.ends_with(":0")) {
                self.known_listeners.insert(format!("{}_{}", s, o));
            }
        }
        self.baseline_established = true;
    }
}

/// Type alias for the system state used by SystemThreatDetector.
pub type SystemState = Vec<crate::perception::SvoTriple>;

impl ThreatDetector<SystemState> for SystemThreatDetector {
    fn detect(&self, before: &SystemState, after: &SystemState) -> Vec<ThreatEvent> {
        if !self.baseline_established {
            return Vec::new();
        }

        // Find new triples (present in after but not in before)
        let new_triples = diff_triples(before, after);
        if new_triples.is_empty() {
            return Vec::new();
        }

        // We can't access the QA engine from the trait method since it's
        // not in self.  We use the fast-path heuristic checks instead.
        let mut events = Vec::new();

        for triple in &new_triples {
            let (subject, verb, object) = triple;

            // Fast heuristic checks (these match the QA rules above)
            if self.is_suspicious_network(subject, verb, object) {
                events.push(ThreatEvent {
                    domain: "system".to_string(),
                    severity: 0.6,
                    entity: subject.clone(),
                    description: format!("Unknown process {} connected to {}", subject, object),
                    class: ThreatClass::Threat,
                });
            }

            if self.is_sensitive_file_access(subject, verb, object) {
                events.push(ThreatEvent {
                    domain: "system".to_string(),
                    severity: 0.8,
                    entity: subject.clone(),
                    description: format!("{} accessed sensitive file: {}", subject, object),
                    class: ThreatClass::Threat,
                });
            }

            // New listening port
            if verb == "listening_on"
                && !self
                    .known_listeners
                    .contains(&format!("{}_{}", subject, object))
            {
                events.push(ThreatEvent {
                    domain: "system".to_string(),
                    severity: 0.5,
                    entity: format!("{}_{}", subject, object),
                    description: format!("New listening port: {} on {}", subject, object),
                    class: ThreatClass::Threat,
                });
            }
        }

        events
    }
}

// ─── Existing DefenseSystem ─────────────────────────────────────────────────

#[derive(Clone)]
pub struct DefenseSystem {
    pub threat_level: Arc<RwLock<f64>>,
    pub active_port: Arc<RwLock<u16>>,
    pub stealth_mode: Arc<RwLock<bool>>,
    pub anxiety: Arc<RwLock<f64>>,
    /// ██ UPGRADE v2.0: Anxiety Inhibition (refractory period) ██
    /// After a pivot/DissonanceAlert, this counter counts down from
    /// INHIBITION_TICKS to 0.  During inhibition:
    ///   - Anxiety increases are dampened by 50%
    ///   - The vigilance threshold is fixed at 0.60 (midpoint)
    /// This prevents runaway feedback loops where anxiety → hypervigilance
    /// → more threat detection → more anxiety.
    pub inhibition_counter: Arc<RwLock<usize>>,
}

/// Number of ticks the inhibition period lasts after a pivot event.
pub const INHIBITION_TICKS: usize = 10;

impl DefenseSystem {
    pub fn new(initial_port: u16) -> Self {
        DefenseSystem {
            threat_level: Arc::new(RwLock::new(0.0)),
            active_port: Arc::new(RwLock::new(initial_port)),
            stealth_mode: Arc::new(RwLock::new(false)),
            anxiety: Arc::new(RwLock::new(0.0)),
            inhibition_counter: Arc::new(RwLock::new(0)),
        }
    }

    /// ██ UPGRADE v2.0: Inhibition-aware threat increment ██
    /// During inhibition, threat increases are dampened by 50%
    /// and the anxiety amplification factor is halved.
    pub async fn increment_threat(&self, amount: f64) {
        let anxiety_val = *self.anxiety.read().await;
        let inhibition = {
            let ic = self.inhibition_counter.read().await;
            *ic > 0
        };

        let dampener = if inhibition { 0.5 } else { 1.0 };
        let anxiety_amp = 1.0 + anxiety_val * dampener; // halved during inhibition
        let scaled_amount = amount * anxiety_amp * dampener;

        let mut level = self.threat_level.write().await;
        *level = (*level + scaled_amount).clamp(0.0, 1.0);
    }

    pub async fn decrement_threat(&self, amount: f64) {
        let mut level = self.threat_level.write().await;
        *level = (*level - amount).clamp(0.0, 1.0);
    }

    /// ██ UPGRADE v2.0: Inhibition-aware threat evaluation ██
    ///
    /// During inhibition:
    ///   - Vigilance threshold is fixed at 0.60 (preventing both
    ///     hypervigilance and complacency)
    ///   - Stealth mode deactivation threshold is also clamped
    ///
    /// After evaluating, decrements the inhibition counter.
    pub async fn evaluate_threat_response(&self) -> bool {
        let level = *self.threat_level.read().await;
        let anxiety_val = *self.anxiety.read().await;
        let mut inhibition = self.inhibition_counter.write().await;

        let (threshold, deact_threshold) = if *inhibition > 0 {
            // During inhibition: fixed mid-range thresholds
            *inhibition -= 1;
            (0.60, 0.25)
        } else {
            // Normal dynamic thresholds
            (0.8 - 0.4 * anxiety_val, 0.3 - 0.15 * anxiety_val)
        };

        if level >= threshold {
            let mut stealth = self.stealth_mode.write().await;
            if !*stealth {
                *stealth = true;
                let mut port = self.active_port.write().await;
                use rand::Rng;
                let new_port = rand::thread_rng().gen_range(9001..=9999);
                *port = new_port;

                // ██ Activate inhibition on pivot ██
                *inhibition = INHIBITION_TICKS;

                return true;
            }
        } else if level < deact_threshold {
            let mut stealth = self.stealth_mode.write().await;
            *stealth = false;
        }
        false
    }

    /// Trigger inhibition manually (e.g., from a DissonanceAlert).
    pub async fn trigger_inhibition(&self) {
        let mut inhibition = self.inhibition_counter.write().await;
        *inhibition = INHIBITION_TICKS;
    }

    /// Check whether the system is currently in the inhibition (refractory) period.
    pub async fn is_inhibited(&self) -> bool {
        let ic = self.inhibition_counter.read().await;
        *ic > 0
    }

    /// Energy gate: verify that executing an action is safe given the
    /// current cognitive state.
    ///
    /// Returns `Ok(())` if the action passes all gates:
    /// 1. Threat level is below critical threshold (or action is sys_read)
    /// 2. The parameter is not empty
    /// 3. Basic sandbox safety (delegates to `check_sandbox_safety`)
    ///
    /// Returns `Err(reason)` if any gate rejects the action.
    pub async fn check_action_safety(
        &self,
        action_name: &str,
        param_str: &str,
    ) -> Result<(), String> {
        let threat = *self.threat_level.read().await;
        let anxiety = *self.anxiety.read().await;
        let inhibited = self.is_inhibited().await;

        // Gate 1: sys_read is always safe (read-only)
        if action_name == "sys_read" {
            return Ok(());
        }

        // Gate 2: Empty parameters are rejected
        if param_str.is_empty() {
            return Err("Energy gate: empty parameter".to_string());
        }

        // Gate 3: Dynamic danger threshold (clamped to 0.60 during inhibition)
        let danger_threshold = if inhibited { 0.60 } else { 0.8 - 0.4 * anxiety };
        if threat >= danger_threshold && action_name == "execute_bash" {
            return Err(format!(
                "Energy gate: threat={:.2} exceeds threshold={:.2}. Blocking shell execution.",
                threat, danger_threshold
            ));
        }

        // Gate 4: Sandbox safety
        if action_name == "execute_bash" && !crate::action::check_sandbox_safety(param_str) {
            return Err("Energy gate: blocked by sandbox guard".to_string());
        }

        Ok(())
    }

    /// Overwrites system telemetry files or temporary traces with random noise
    pub async fn scrub_traces(&self) {
        let temp_file_path = "data/ledger_temp.tmp";
        if std::path::Path::new(temp_file_path).exists() {
            use rand::Rng;
            let mut rng = rand::thread_rng();
            let mut noise = vec![0u8; 1024];
            rng.fill(&mut noise[..]);
            let _ = std::fs::write(temp_file_path, noise);
            let _ = std::fs::remove_file(temp_file_path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_system_threat_detector_detects_new_connection() {
        let detector = SystemThreatDetector::new();
        let before: SystemState = vec![(
            "process_1".to_string(),
            "is_running".to_string(),
            "bash".to_string(),
        )];

        let after: SystemState = vec![
            (
                "process_1".to_string(),
                "is_running".to_string(),
                "bash".to_string(),
            ),
            (
                "process_1".to_string(),
                "connected_to".to_string(),
                "10.0.0.1:443".to_string(),
            ),
        ];

        // Without baseline, empty results
        let events = detector.detect(&before, &after);
        assert!(events.is_empty(), "No baseline → no detection");

        // With baseline
        let mut detector2 = SystemThreatDetector::new();
        detector2.update_baseline(&before);
        let events = detector2.detect(&before, &after);
        assert_eq!(events.len(), 1, "Should detect new connection");
        assert_eq!(events[0].domain, "system");
        assert!(events[0].description.contains("process_1"));
        assert!(events[0].description.contains("10.0.0.1"));
    }

    #[test]
    fn test_system_threat_detector_sensitive_file() {
        let mut detector = SystemThreatDetector::new();
        // Establish baseline with just the process
        let baseline: SystemState = vec![(
            "process_5".to_string(),
            "is_running".to_string(),
            "curl".to_string(),
        )];
        detector.update_baseline(&baseline);

        let after: SystemState = vec![
            (
                "process_5".to_string(),
                "is_running".to_string(),
                "curl".to_string(),
            ),
            (
                "process_5".to_string(),
                "has_open".to_string(),
                "/etc/passwd".to_string(),
            ),
        ];

        let events = detector.detect(&baseline, &after);
        assert!(!events.is_empty(), "Should detect sensitive file access");
        let has_passwd = events.iter().any(|e| e.description.contains("/etc/passwd"));
        assert!(has_passwd, "Should mention /etc/passwd");
    }

    #[test]
    fn test_diff_triples() {
        use crate::perception::SvoTriple;
        let before: Vec<SvoTriple> = vec![("a".to_string(), "runs".to_string(), "b".to_string())];
        let after: Vec<SvoTriple> = vec![
            ("a".to_string(), "runs".to_string(), "b".to_string()),
            ("c".to_string(), "connects".to_string(), "d".to_string()),
        ];
        let diff = diff_triples(&before, &after);
        assert_eq!(diff.len(), 1);
        assert_eq!(diff[0].0, "c");
    }

    #[test]
    fn test_seed_threat_rules() {
        let mut qa = crate::qa::QaEngine::new();
        seed_threat_rules(&mut qa);
        // QA should have at least the rules we seeded
        assert!(
            qa.rule_count() >= 7,
            "Should have at least 7 threat rules, got {}",
            qa.rule_count()
        );
    }

    #[tokio::test]
    async fn test_threat_evaluation_and_rotation() {
        let defense = DefenseSystem::new(9000);

        assert_eq!(*defense.threat_level.read().await, 0.0);
        assert_eq!(*defense.active_port.read().await, 9000);
        assert!(!*defense.stealth_mode.read().await);

        defense.increment_threat(0.5).await;
        assert_eq!(*defense.threat_level.read().await, 0.5);
        let rotation = defense.evaluate_threat_response().await;
        assert!(!rotation);
        assert!(!*defense.stealth_mode.read().await);

        defense.increment_threat(0.35).await;
        assert_eq!(*defense.threat_level.read().await, 0.85);
        let rotation = defense.evaluate_threat_response().await;
        assert!(rotation);
        assert!(*defense.stealth_mode.read().await);
        let new_port = *defense.active_port.read().await;
        assert!(new_port >= 9001 && new_port <= 9999);

        let rotation2 = defense.evaluate_threat_response().await;
        assert!(!rotation2);
    }

    #[tokio::test]
    async fn test_anxiety_threat_scaling() {
        let defense = DefenseSystem::new(9000);

        {
            let mut anxiety_guard = defense.anxiety.write().await;
            *anxiety_guard = 1.0;
        }

        defense.increment_threat(0.25).await;
        assert_eq!(*defense.threat_level.read().await, 0.50);

        let rotated = defense.evaluate_threat_response().await;
        assert!(rotated);
        assert!(*defense.stealth_mode.read().await);
    }

    #[tokio::test]
    async fn test_inhibition_dampens_threat() {
        let defense = DefenseSystem::new(9000);

        // Activate inhibition
        {
            let mut ic = defense.inhibition_counter.write().await;
            *ic = 5;
        }

        // During inhibition, threat increment should be dampened
        let initial = *defense.threat_level.read().await;
        defense.increment_threat(0.5).await;
        let after = *defense.threat_level.read().await;

        // Without inhibition: 0.5 * (1 + 0) = 0.5
        // With inhibition: 0.5 * (1 + 0*0.5) * 0.5 = 0.25
        assert!(
            (after - initial - 0.25).abs() < 0.001,
            "Inhibition should dampen threat increment: got {}",
            after - initial
        );
    }

    #[tokio::test]
    async fn test_inhibition_fixed_threshold() {
        let defense = DefenseSystem::new(9000);

        // Set anxiety very high and activate inhibition
        {
            let mut anxiety_guard = defense.anxiety.write().await;
            *anxiety_guard = 1.0;
        }
        {
            let mut ic = defense.inhibition_counter.write().await;
            *ic = 3;
        }

        // During inhibition, threshold should be 0.60 regardless of anxiety
        defense.increment_threat(0.55).await;
        // Threat should now be 0.55 (dampened: 0.55 * 0.5 * 0.5 = 0.1375)
        // That's well below 0.60, so no rotation yet
        let rotated = defense.evaluate_threat_response().await;
        assert!(
            !rotated,
            "Should not rotate at 0.1375 threat during inhibition"
        );

        // Manually set threat to 0.65
        {
            let mut t = defense.threat_level.write().await;
            *t = 0.65;
        }
        let rotated2 = defense.evaluate_threat_response().await;
        assert!(
            rotated2,
            "Should rotate at 0.65 threat during inhibition (threshold=0.60)"
        );
    }

    #[test]
    fn test_chess_threat_detection() {
        // Setup: machine is white.
        // Before: white queen on d1, black rook on d8 (attacks d1 through files).
        // After: black rook moves from d8 to d1 (captures? No, we model it as
        //   the opponent's rook now attacking d1).
        // The queen on d1 should be detected as newly attacked.
        let detector = ChessThreatDetector::new(true);

        // Before state: white king e1, white queen d1, black rook h8 (h8 does
        // NOT attack d1 — different rank AND file, no direct line).
        let before_pieces = vec![
            ('K', 0, 4), // white king e1
            ('Q', 0, 3), // white queen d1
            ('r', 7, 7), // black rook h8
        ];
        let before_board = {
            let mut b = [[None; 8]; 8];
            b[0][4] = Some('K');
            b[0][3] = Some('Q');
            b[7][7] = Some('r');
            b
        };

        // After state: same pieces (no capture), but the rook now on d3
        // directly attacks the queen along the d-file.
        let after_pieces = vec![
            ('K', 0, 4), // white king e1
            ('Q', 0, 3), // white queen d1 — still on d1
            ('r', 2, 3), // black rook d3 — now attacks d1 down the file
        ];
        let after_board = {
            let mut b = [[None; 8]; 8];
            b[0][4] = Some('K');
            b[0][3] = Some('Q');
            b[2][3] = Some('r');
            b
        };

        let events = detector.detect(&(before_pieces, before_board), &(after_pieces, after_board));

        assert!(
            !events.is_empty(),
            "Should detect at least one threat: {:?}",
            events
        );
        let queen_threat = events.iter().find(|e| e.entity == "wQ_d1");
        assert!(
            queen_threat.is_some(),
            "Should detect queen on d1 as threatened: events={:?}",
            events
        );
        if let Some(qt) = queen_threat {
            assert_eq!(qt.class, ThreatClass::Threat);
            assert!(qt.severity > 0.5, "Queen threat should be high severity");
            assert_eq!(qt.domain, "chess");
            assert!(
                qt.description.contains("newly under attack"),
                "description: {}",
                qt.description
            );
        }
    }

    #[test]
    fn test_chess_threat_detection_no_false_positive() {
        // Machine is white. If a piece was ALREADY under attack and stays
        // under attack, it should NOT trigger a new threat event.
        let detector = ChessThreatDetector::new(true);

        // Before: white queen d1, black rook d8 attacks down d-file to d1
        let before_pieces = vec![
            ('K', 0, 4),
            ('Q', 0, 3),
            ('r', 7, 3), // rook d8 — attacks d1 through d-file
        ];
        let mut board_b = [[None; 8]; 8];
        board_b[0][4] = Some('K');
        board_b[0][3] = Some('Q');
        board_b[7][3] = Some('r');

        // After: same position (rook didn't move), queen still attacked
        // but was already attacked before — no new threat.
        let after_pieces = before_pieces.clone();
        let mut board_a = [[None; 8]; 8];
        board_a[0][4] = Some('K');
        board_a[0][3] = Some('Q');
        board_a[7][3] = Some('r');

        let events = detector.detect(&(before_pieces, board_b), &(after_pieces, board_a));
        assert!(
            events.is_empty(),
            "Should NOT detect new threat when piece was already attacked: {:?}",
            events
        );
    }
}
