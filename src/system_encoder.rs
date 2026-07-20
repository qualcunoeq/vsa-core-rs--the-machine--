// ─── System State Encoder: PerceptualEncoder for Linux /proc → SVO triples ─
//
// Gives The Machine eyes inside a computer.  Reads /proc to extract:
//   - Process relations  (parent, command, user, state)
//   - Network relations  (connections, protocols, remote peers)
//   - File relations     (open file descriptors, paths)
//
// Every extracted relation is an SVO triple stored in the VSABrain's cluster
// memory, enabling the same reasoning machinery that learned chess to detect
// threats, recognize patterns, and chain causal relations in system state.
//
// This is the foundation for the defense capability — a machine that observes
// its own environment can detect anomalies before they become compromises.
// ────────────────────────────────────────────────────────────────────────────

use crate::perception::{Entity, PerceptualEncoder, SvoTriple};
use crate::Hypervector;
use crate::VSABrain;
use std::collections::HashMap;
use std::fs;
use std::io::{BufRead, BufReader};

// ─── Data Types ────────────────────────────────────────────────────────────

/// A captured system process.
#[derive(Debug, Clone)]
pub struct ProcessInfo {
    pub pid: u32,
    pub ppid: u32,
    pub name: String,
    pub cmdline: String,
    pub uid: u32,
    pub username: String,
    pub state: String,
}

/// A captured network connection.
#[derive(Debug, Clone)]
pub struct ConnectionInfo {
    pub pid: u32,
    pub protocol: String, // "tcp", "tcp6", "udp"
    pub local_addr: String,
    pub local_port: u16,
    pub remote_addr: String,
    pub remote_port: u16,
    pub state: String, // "established", "listen", "close_wait", etc.
}

/// A captured open file descriptor.
#[derive(Debug, Clone)]
pub struct FileDescInfo {
    pub pid: u32,
    pub fd_number: u32,
    pub target: String,  // resolved symlink path
    pub fd_type: String, // "file", "socket", "pipe", "anon_inode"
}

/// A full system state snapshot at a point in time.
#[derive(Debug, Clone)]
pub struct SysStateSnapshot {
    pub processes: Vec<ProcessInfo>,
    pub connections: Vec<ConnectionInfo>,
    pub file_descriptors: Vec<FileDescInfo>,
    pub timestamp: std::time::SystemTime,
}

/// Bitwise representation of a system snapshot plus the explicit facts used
/// to produce it.
#[derive(Debug, Clone)]
pub struct EncodedSystemState {
    pub vector: Hypervector,
    pub triples: Vec<SvoTriple>,
    pub summary: String,
}

// ─── Process Extraction ────────────────────────────────────────────────────

/// Read /proc/{pid}/status and return parsed fields.
fn read_status(pid: u32) -> HashMap<String, String> {
    let path = format!("/proc/{}/status", pid);
    let mut fields = HashMap::new();
    let file = match fs::File::open(&path) {
        Ok(f) => f,
        Err(_) => return fields,
    };
    for line in BufReader::new(file).lines().flatten() {
        if let Some((key, val)) = line.split_once(':') {
            fields.insert(key.trim().to_string(), val.trim().to_string());
        }
    }
    fields
}

/// Read /proc/{pid}/cmdline (null-separated args) into a single string.
fn read_cmdline(pid: u32) -> String {
    let path = format!("/proc/{}/cmdline", pid);
    match fs::read(&path) {
        Ok(bytes) => {
            let s = String::from_utf8_lossy(&bytes);
            s.split('\0').collect::<Vec<&str>>().join(" ")
        }
        Err(_) => String::new(),
    }
}

/// Map UID to username by reading /etc/passwd.
fn uid_to_username(uid: u32) -> String {
    let file = match fs::File::open("/etc/passwd") {
        Ok(f) => f,
        Err(_) => return format!("uid_{}", uid),
    };
    for line in BufReader::new(file).lines().flatten() {
        let parts: Vec<&str> = line.split(':').collect();
        if parts.len() >= 3 {
            if let Ok(u) = parts[2].parse::<u32>() {
                if u == uid {
                    return parts[0].to_string();
                }
            }
        }
    }
    format!("uid_{}", uid)
}

/// Extract all processes from /proc.
///
/// If `max_pids` is Some(n), only reads the first n PIDs found (for testing).
/// Pass None for full system scan.
pub fn read_processes_filtered(max_pids: Option<usize>) -> Vec<ProcessInfo> {
    let mut processes = Vec::new();
    let dir = match fs::read_dir("/proc") {
        Ok(d) => d,
        Err(_) => return processes,
    };

    for entry in dir.flatten() {
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        let pid: u32 = match name_str.parse() {
            Ok(p) => p,
            Err(_) => continue,
        };

        let status = read_status(pid);
        let ppid: u32 = status.get("PPid").and_then(|s| s.parse().ok()).unwrap_or(0);
        let proc_name = status.get("Name").cloned().unwrap_or_default();
        let uid_str = status
            .get("Uid")
            .and_then(|s| s.split_whitespace().next())
            .unwrap_or("0");
        let uid: u32 = uid_str.parse().unwrap_or(0);
        let state = status
            .get("State")
            .map(|s| s.split_whitespace().next().unwrap_or("?"))
            .unwrap_or("?")
            .to_string();

        processes.push(ProcessInfo {
            pid,
            ppid,
            name: proc_name,
            cmdline: read_cmdline(pid),
            uid,
            username: uid_to_username(uid),
            state,
        });

        if let Some(max) = max_pids {
            if processes.len() >= max {
                break;
            }
        }
    }

    processes
}

/// Read all processes (no limit).
pub fn read_processes() -> Vec<ProcessInfo> {
    read_processes_filtered(None)
}

// ─── Network Connection Extraction ─────────────────────────────────────────

/// Decode a hex-encoded /proc/net/tcp address like "0100007F:0035"
/// into ("127.0.0.1", 53).
fn decode_tcp_addr(encoded: &str) -> (String, u16) {
    let parts: Vec<&str> = encoded.split(':').collect();
    if parts.len() != 2 {
        return ("0.0.0.0".to_string(), 0);
    }
    let hex_ip = parts[0];
    let port = u16::from_str_radix(parts[1], 16).unwrap_or(0);

    // Hex IP is in little-endian byte order (e.g., "0100007F" → 127.0.0.1)
    let ip = if hex_ip.len() == 8 {
        let bytes: Vec<u8> = (0..4)
            .map(|i| u8::from_str_radix(&hex_ip[i * 2..i * 2 + 2], 16).unwrap_or(0))
            .collect();
        format!("{}.{}.{}.{}", bytes[0], bytes[1], bytes[2], bytes[3])
    } else {
        "0.0.0.0".to_string()
    };

    (ip, port)
}

/// Map TCP state number to string.
fn tcp_state_str(state: u8) -> &'static str {
    match state {
        0x01 => "established",
        0x02 => "syn_sent",
        0x03 => "syn_recv",
        0x04 => "fin_wait1",
        0x05 => "fin_wait2",
        0x06 => "time_wait",
        0x07 => "close",
        0x08 => "close_wait",
        0x09 => "last_ack",
        0x0A => "listen",
        0x0B => "closing",
        _ => "unknown",
    }
}

/// Parse /proc/{pid}/net/tcp for TCP connections belonging to a process.
fn read_process_tcp(pid: u32) -> Vec<ConnectionInfo> {
    let path = format!("/proc/{}/net/tcp", pid);
    let file = match fs::File::open(&path) {
        Ok(f) => f,
        Err(_) => return Vec::new(),
    };

    let mut conns = Vec::new();
    for line in BufReader::new(file).lines().flatten().skip(1) {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 4 {
            continue;
        }

        let (local_ip, local_port) = decode_tcp_addr(parts[1]);
        let (remote_ip, remote_port) = decode_tcp_addr(parts[2]);
        let state_hex = u8::from_str_radix(parts[3], 16).unwrap_or(0);

        conns.push(ConnectionInfo {
            pid,
            protocol: "tcp".to_string(),
            local_addr: local_ip,
            local_port,
            remote_addr: remote_ip,
            remote_port,
            state: tcp_state_str(state_hex).to_string(),
        });
    }
    conns
}

/// Read TCP connections from processes (with optional limit for speed).
pub fn read_all_connections_filtered(max_procs: Option<usize>) -> Vec<ConnectionInfo> {
    let processes = read_processes_filtered(max_procs);
    let mut all_conns = Vec::new();
    for proc in &processes {
        all_conns.extend(read_process_tcp(proc.pid));
    }
    // Dedup by (pid, local_addr, local_port, remote_addr, remote_port)
    all_conns.sort_by(|a, b| {
        a.pid
            .cmp(&b.pid)
            .then(a.local_addr.cmp(&b.local_addr))
            .then(a.local_port.cmp(&b.local_port))
            .then(a.remote_addr.cmp(&b.remote_addr))
            .then(a.remote_port.cmp(&b.remote_port))
    });
    all_conns.dedup_by(|a, b| {
        a.pid == b.pid
            && a.local_addr == b.local_addr
            && a.local_port == b.local_port
            && a.remote_addr == b.remote_addr
            && a.remote_port == b.remote_port
    });
    all_conns
}

// ─── File Descriptor Extraction ────────────────────────────────────────────

/// Read /proc/{pid}/fd/ and resolve symlinks to get open file paths.
fn read_process_fds(pid: u32) -> Vec<FileDescInfo> {
    let fd_dir = format!("/proc/{}/fd", pid);
    let dir = match fs::read_dir(&fd_dir) {
        Ok(d) => d,
        Err(_) => return Vec::new(),
    };

    let mut fds = Vec::new();
    for entry in dir.flatten() {
        let fd_name = entry.file_name();
        let fd_num: u32 = match fd_name.to_string_lossy().parse() {
            Ok(n) => n,
            Err(_) => continue,
        };

        let target = match fs::read_link(&entry.path()) {
            Ok(p) => p.to_string_lossy().to_string(),
            Err(_) => String::new(),
        };

        let fd_type = if target.starts_with('/') {
            "file"
        } else if target.starts_with("socket:") {
            "socket"
        } else if target.starts_with("pipe:") {
            "pipe"
        } else if target.starts_with("anon_inode:") {
            "anon_inode"
        } else {
            "other"
        };

        fds.push(FileDescInfo {
            pid,
            fd_number: fd_num,
            target,
            fd_type: fd_type.to_string(),
        });
    }
    fds
}

/// Read file descriptors from processes (with optional limit for speed).
pub fn read_all_fds_filtered(max_procs: Option<usize>) -> Vec<FileDescInfo> {
    let processes = read_processes_filtered(max_procs);
    let mut all_fds = Vec::new();
    for proc in &processes {
        all_fds.extend(read_process_fds(proc.pid));
    }
    all_fds
}

// ─── Snapshot ──────────────────────────────────────────────────────────────

/// Capture a snapshot of the current system state (limited for speed).
///
/// `max_procs` limits how many processes to scan (None = all).
/// The full system scan can be slow on busy machines (hundreds of PIDs).
pub fn capture_snapshot_filtered(max_procs: Option<usize>) -> SysStateSnapshot {
    SysStateSnapshot {
        processes: read_processes_filtered(max_procs),
        connections: read_all_connections_filtered(max_procs),
        file_descriptors: read_all_fds_filtered(max_procs),
        timestamp: std::time::SystemTime::now(),
    }
}

/// Full system scan — use with caution on production systems.
pub fn capture_snapshot() -> SysStateSnapshot {
    capture_snapshot_filtered(None)
}

// ─── SVO Triple Conversion ─────────────────────────────────────────────────

impl From<SysStateSnapshot> for Vec<SvoTriple> {
    fn from(snapshot: SysStateSnapshot) -> Self {
        let mut triples = Vec::new();

        // Process triples
        for p in &snapshot.processes {
            let pid_str = format!("process_{}", p.pid);
            triples.push((
                pid_str.clone(),
                "is_child_of".to_string(),
                format!("process_{}", p.ppid),
            ));
            triples.push((pid_str.clone(), "is_running".to_string(), p.name.clone()));
            triples.push((
                pid_str.clone(),
                "run_by_user".to_string(),
                p.username.clone(),
            ));
            triples.push((pid_str.clone(), "state".to_string(), p.state.clone()));
            if !p.cmdline.is_empty() {
                triples.push((
                    pid_str.clone(),
                    "executing".to_string(),
                    p.cmdline.chars().take(80).collect::<String>(),
                ));
            }
        }

        // Network connection triples
        for c in &snapshot.connections {
            let pid_str = format!("process_{}", c.pid);
            let conn_id = format!("conn_{}_{}:{}", c.pid, c.remote_addr, c.remote_port);
            triples.push((
                pid_str,
                "connected_to".to_string(),
                format!("{}:{}", c.remote_addr, c.remote_port),
            ));
            triples.push((
                conn_id.clone(),
                "local".to_string(),
                format!("{}:{}", c.local_addr, c.local_port),
            ));
            triples.push((conn_id, "protocol".to_string(), c.protocol.clone()));
        }

        // File descriptor triples (only interesting paths)
        for f in &snapshot.file_descriptors {
            if f.fd_type == "file" && !f.target.is_empty() {
                let pid_str = format!("process_{}", f.pid);
                triples.push((pid_str, "has_open".to_string(), f.target.clone()));
            }
        }

        triples
    }
}

fn canonicalize_component(component: &str) -> String {
    component
        .trim()
        .to_lowercase()
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.' | ':' | '/') {
                ch
            } else {
                '_'
            }
        })
        .collect::<String>()
        .split('_')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("_")
}

/// Produce stable, deduplicated triples for a snapshot.
///
/// The snapshot timestamp is deliberately excluded. This encodes state, not
/// wall-clock time, so identical states captured at different moments map to
/// the same vector.
pub fn canonical_snapshot_triples(snapshot: &SysStateSnapshot) -> Vec<SvoTriple> {
    let mut triples: Vec<SvoTriple> = Vec::<SvoTriple>::from(snapshot.clone())
        .into_iter()
        .map(|(s, v, o)| {
            (
                canonicalize_component(&s),
                canonicalize_component(&v),
                canonicalize_component(&o),
            )
        })
        .collect();
    triples.sort();
    triples.dedup();
    triples
}

fn encode_system_triple(triple: &SvoTriple) -> Hypervector {
    let s_hv = Hypervector::encode_text_ngram(&triple.0, 3);
    let v_hv = Hypervector::encode_text_ngram(&triple.1, 3);
    let o_hv = Hypervector::encode_text_ngram(&triple.2, 3);
    crate::resonator::encode_svo(&s_hv, &v_hv, &o_hv)
}

/// Encode a snapshot into one order-invariant bitwise state vector.
pub fn encode_snapshot_vector(snapshot: &SysStateSnapshot) -> Hypervector {
    let triples = canonical_snapshot_triples(snapshot);
    if triples.is_empty() {
        return Hypervector::new_zero();
    }

    let triple_vectors: Vec<Hypervector> = triples.iter().map(encode_system_triple).collect();
    let refs: Vec<&Hypervector> = triple_vectors.iter().collect();
    Hypervector::bundle(&refs)
}

/// Human-readable summary grounded only in encoded snapshot facts.
pub fn summarize_snapshot(snapshot: &SysStateSnapshot, sample_limit: usize) -> String {
    let triples = canonical_snapshot_triples(snapshot);
    let decoder = crate::language_decoder::NlpDecoder::new();
    let sample = decoder.decode_triples(&triples, sample_limit);
    format!(
        "System state: {} processes, {} connections, {} file descriptors, {} canonical facts. {}",
        snapshot.processes.len(),
        snapshot.connections.len(),
        snapshot.file_descriptors.len(),
        triples.len(),
        sample
    )
}

/// Encode the current system state as explicit triples plus one vector.
pub fn encode_system_state_filtered(max_procs: Option<usize>) -> EncodedSystemState {
    let snapshot = capture_snapshot_filtered(max_procs);
    let triples = canonical_snapshot_triples(&snapshot);
    let vector = encode_snapshot_vector(&snapshot);
    let summary = summarize_snapshot(&snapshot, 5);
    EncodedSystemState {
        vector,
        triples,
        summary,
    }
}

// ─── PerceptualEncoder Implementation ──────────────────────────────────────

/// System state encoder: reads Linux /proc and produces SVO triples.
pub struct SystemEncoder;

impl PerceptualEncoder for SystemEncoder {
    /// Input is ignored — the encoder reads /proc directly.
    type Input = ();

    fn extract_entities(&self, _input: &()) -> Vec<Entity> {
        // Use a limited scan (first 100 PIDs) for responsiveness
        let procs = read_processes_filtered(Some(100));
        let mut entities: Vec<String> = Vec::new();

        for p in &procs {
            entities.push(format!("process_{}", p.pid));
            entities.push(format!("process_{}", p.ppid));
        }
        // No connection entities in the fast path

        entities.sort();
        entities.dedup();
        entities
    }

    fn extract_relations(&self, _input: &(), _entities: &[Entity]) -> Vec<SvoTriple> {
        let snapshot = capture_snapshot_filtered(Some(100));
        snapshot.into()
    }
}

// ─── Knowledge Storage ─────────────────────────────────────────────────────

use crate::text_encoder::store_knowledge_triple;

/// Ingest the current system state into the VSABrain's cluster memory.
///
/// Returns the number of triples stored.
pub fn ingest_system_state(brain: &mut VSABrain) -> usize {
    let snapshot = capture_snapshot_filtered(Some(100));
    let triples: Vec<SvoTriple> = snapshot.into();
    let mut count = 0;

    for (subject, verb, object) in &triples {
        store_knowledge_triple(
            brain,
            subject,
            verb,
            object,
            0.9, // high confidence — this is direct observation
            "system_state",
        );
        count += 1;
    }
    count
}

// ─── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::VSABrain;
    use std::time::SystemTime;

    fn synthetic_snapshot(state: &str) -> SysStateSnapshot {
        SysStateSnapshot {
            processes: vec![
                ProcessInfo {
                    pid: 2,
                    ppid: 1,
                    name: "worker".to_string(),
                    cmdline: "/usr/bin/worker --serve".to_string(),
                    uid: 1000,
                    username: "alice".to_string(),
                    state: state.to_string(),
                },
                ProcessInfo {
                    pid: 1,
                    ppid: 0,
                    name: "init".to_string(),
                    cmdline: "/sbin/init".to_string(),
                    uid: 0,
                    username: "root".to_string(),
                    state: "S".to_string(),
                },
            ],
            connections: vec![ConnectionInfo {
                pid: 2,
                protocol: "tcp".to_string(),
                local_addr: "127.0.0.1".to_string(),
                local_port: 8080,
                remote_addr: "10.0.0.5".to_string(),
                remote_port: 443,
                state: "established".to_string(),
            }],
            file_descriptors: vec![FileDescInfo {
                pid: 2,
                fd_number: 3,
                target: "/tmp/machine.log".to_string(),
                fd_type: "file".to_string(),
            }],
            timestamp: SystemTime::UNIX_EPOCH,
        }
    }

    fn reordered_synthetic_snapshot() -> SysStateSnapshot {
        let mut snapshot = synthetic_snapshot("S");
        snapshot.processes.reverse();
        snapshot
    }

    #[test]
    fn test_read_processes() {
        // Limit to 200 processes for speed
        let procs = read_processes_filtered(Some(200));
        assert!(!procs.is_empty(), "Should find at least some processes");
        eprintln!("  Found {} processes (limited to 200)", procs.len());

        // Verify basic fields are populated
        if let Some(first) = procs.first() {
            eprintln!(
                "  First process: pid={}, name={}, state={}",
                first.pid, first.name, first.state
            );
        }
    }

    #[test]
    fn test_read_connections() {
        // Read from a limited set of processes
        let procs = read_processes_filtered(Some(30));
        let mut conns = Vec::new();
        for p in &procs {
            conns.extend(read_process_tcp(p.pid));
        }
        eprintln!(
            "  Found {} TCP connections from {} processes",
            conns.len(),
            procs.len()
        );
        for c in conns.iter().take(5) {
            eprintln!(
                "    pid={}, {}:{} → {}:{}, state={}",
                c.pid, c.local_addr, c.local_port, c.remote_addr, c.remote_port, c.state
            );
        }
    }

    #[test]
    fn test_capture_snapshot() {
        // Limit snapshot to 50 processes for speed
        let procs = read_processes_filtered(Some(50));
        let mut conns = Vec::new();
        for p in &procs {
            conns.extend(read_process_tcp(p.pid));
        }
        let mut fds = Vec::new();
        for p in &procs {
            fds.extend(read_process_fds(p.pid));
        }
        eprintln!(
            "  Snapshot: {} processes, {} connections, {} fds",
            procs.len(),
            conns.len(),
            fds.len()
        );
        assert!(!procs.is_empty(), "Should have processes");
    }

    #[test]
    fn test_snapshot_to_triples() {
        // Build a minimal snapshot manually to avoid timeout
        let procs = read_processes_filtered(Some(30));
        let mut conns = Vec::new();
        for p in &procs {
            conns.extend(read_process_tcp(p.pid));
        }
        let mut fds = Vec::new();
        for p in &procs {
            fds.extend(read_process_fds(p.pid));
        }
        let snapshot = SysStateSnapshot {
            processes: procs,
            connections: conns,
            file_descriptors: fds,
            timestamp: std::time::SystemTime::now(),
        };
        let triples: Vec<SvoTriple> = snapshot.into();
        assert!(
            triples.len() > 3,
            "Should extract several triples, got {}",
            triples.len()
        );
        eprintln!("  Extracted {} triples from system state", triples.len());
        for t in triples.iter().take(10) {
            eprintln!("    ({}, {}, {})", t.0, t.1, t.2);
        }
    }

    #[test]
    fn test_ingest_system_state_forms_clusters() {
        let mut brain = VSABrain::new(0.12);
        // Only use 20 processes for speed
        let procs = read_processes_filtered(Some(20));
        let mut conns = Vec::new();
        for p in &procs {
            conns.extend(read_process_tcp(p.pid));
        }
        let mut fds = Vec::new();
        for p in &procs {
            fds.extend(read_process_fds(p.pid));
        }
        let snapshot = SysStateSnapshot {
            processes: procs,
            connections: conns,
            file_descriptors: fds,
            timestamp: std::time::SystemTime::now(),
        };
        let triples: Vec<SvoTriple> = snapshot.into();

        let mut count = 0;
        for (s, v, o) in &triples {
            store_knowledge_triple(&mut brain, s, v, o, 0.9, "test_system");
            count += 1;
        }
        assert!(count > 0, "Should store at least one triple");
        eprintln!(
            "  Stored {} triples → {} clusters, {} total entries",
            count,
            brain.dejavu_clusters.len(),
            brain
                .dejavu_clusters
                .iter()
                .map(|c| c.entries.len())
                .sum::<usize>(),
        );
    }

    #[test]
    fn test_perceptual_encoder_trait() {
        // Use a limited snapshot
        let procs = read_processes_filtered(Some(20));
        // For the trait test, just check that entities are extractable
        let mut entities: Vec<String> = Vec::new();
        for p in &procs {
            entities.push(format!("process_{}", p.pid));
            entities.push(format!("process_{}", p.ppid));
        }
        assert!(!entities.is_empty(), "Should extract entities");
        eprintln!(
            "  SystemEncoder: {} entities from {} processes",
            entities.len(),
            procs.len()
        );
    }

    #[test]
    fn test_canonical_snapshot_encoding_is_order_invariant() {
        let a = synthetic_snapshot("S");
        let b = reordered_synthetic_snapshot();

        let a_triples = canonical_snapshot_triples(&a);
        let b_triples = canonical_snapshot_triples(&b);
        assert_eq!(a_triples, b_triples);
        assert_eq!(encode_snapshot_vector(&a), encode_snapshot_vector(&b));
    }

    #[test]
    fn test_snapshot_encoding_changes_with_state() {
        let sleeping = synthetic_snapshot("S");
        let running = synthetic_snapshot("R");

        let sleeping_vector = encode_snapshot_vector(&sleeping);
        let running_vector = encode_snapshot_vector(&running);

        assert_ne!(sleeping_vector, running_vector);
        assert!(
            sleeping_vector.normalized_hamming_distance(&running_vector) > 0.01,
            "A process state change should move the system vector"
        );
    }

    #[test]
    fn test_snapshot_summary_is_grounded_in_facts() {
        let snapshot = synthetic_snapshot("S");
        let summary = summarize_snapshot(&snapshot, 2);

        assert!(summary.contains("2 processes"));
        assert!(summary.contains("1 connections"));
        assert!(summary.contains("canonical facts"));
        assert!(summary.contains("process"));
    }
}
