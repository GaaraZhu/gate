//! Ephemeral JSONL event feed for `gate log`.
//!
//! Mirrors [`crate::stats`] (counts and labels only — never values, never SQL,
//! never command lines) but covers every interception decision, not just
//! redaction outcomes, and lives under the OS temp dir rather than persistent
//! state so it self-clears across reboots and never grows unbounded.
//!
//! Failure is never propagated to callers: a write failure here must not
//! affect the hook/redaction pipeline.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::append_log;
use crate::redactor::RedactStats;

/// One recorded interception event. Serialised as a single JSONL line.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEvent {
    /// Unix epoch milliseconds.
    pub ts: u64,
    /// `"bash"`, `"mcp"`, or `"stdin"`.
    pub path: String,
    /// Tool basename (bash path), MCP server name (mcp path), or `"stdin"`.
    pub tool: String,
    /// One of `"intercepted"`, `"redacted"`, `"passthrough"`, `"rejected"`, `"blocked"`.
    pub outcome: String,
    /// Total PII fields redacted (only meaningful for `"redacted"`).
    #[serde(default)]
    pub fields_redacted: usize,
    /// Per-PII-type counts, e.g. `{"email": 23, "ssn": 8}`.
    #[serde(default)]
    pub types: HashMap<String, usize>,
    /// Column names Gate 1 force-redacted (names only, never values).
    #[serde(default)]
    pub forced_columns: Vec<String>,
    /// Low-confidence match warnings, same strings as `_gate_summary`.
    #[serde(default)]
    pub warnings: Vec<String>,
    /// Microseconds gate spent processing this event (0 if not applicable).
    #[serde(default)]
    pub overhead_us: u64,
    /// Short, PII-free human-readable detail (e.g. a rejection or block reason).
    #[serde(default)]
    pub detail: String,
}

impl LogEvent {
    fn base(path: &str, tool: &str, outcome: &str) -> Self {
        Self {
            ts: now_millis(),
            path: path.to_string(),
            tool: tool.to_string(),
            outcome: outcome.to_string(),
            fields_redacted: 0,
            types: HashMap::new(),
            forced_columns: Vec::new(),
            warnings: Vec::new(),
            overhead_us: 0,
            detail: String::new(),
        }
    }

    /// A hook matched a configured tool and is rewriting the command to route through `gate run`.
    pub fn intercepted(path: &str, tool: &str) -> Self {
        Self::base(path, tool, "intercepted")
    }

    /// A command was blocked outright (e.g. self-protection).
    pub fn blocked(path: &str, tool: &str, detail: &str) -> Self {
        Self {
            detail: detail.to_string(),
            ..Self::base(path, tool, "blocked")
        }
    }

    /// Gate 1 rejected the query before it ran (denylisted column or SELECT *).
    pub fn rejected(path: &str, tool: &str, detail: &str) -> Self {
        Self {
            detail: detail.to_string(),
            ..Self::base(path, tool, "rejected")
        }
    }

    /// Gate 2 finished: `"redacted"` if any field was redacted, else `"passthrough"`.
    pub fn outcome(
        path: &str,
        tool: &str,
        stats: &RedactStats,
        overhead_us: u64,
        forced_columns: Vec<String>,
    ) -> Self {
        let outcome = if stats.total > 0 {
            "redacted"
        } else {
            "passthrough"
        };
        Self {
            fields_redacted: stats.total,
            types: stats.type_counts.clone(),
            forced_columns,
            warnings: stats.warnings.clone(),
            overhead_us,
            ..Self::base(path, tool, outcome)
        }
    }
}

fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Resolve the event log file path.
///
/// Precedence: `GATE_LOG_PATH` env var; otherwise the OS temp dir under
/// `gate/events.jsonl`. Deliberately not the persistent stats dir — this feed
/// is ephemeral debug output, not an audit trail.
pub fn log_path() -> PathBuf {
    if let Ok(p) = std::env::var("GATE_LOG_PATH") {
        return PathBuf::from(p);
    }
    std::env::temp_dir().join("gate").join("events.jsonl")
}

const MAX_BYTES: u64 = 1_000_000;
const KEEP_LINES: usize = 1000;

/// Append one event to the log file. Best-effort: errors are swallowed by
/// callers (use `let _ = record(...)`), this never blocks the pipeline.
pub fn record(event: &LogEvent) -> Result<()> {
    let path = log_path();
    trim_if_oversized(&path);
    let mut line = serde_json::to_string(event)?;
    line.push('\n');
    append_log::append(&path, line.as_bytes())
}

/// Keep the log file from growing unbounded: once it exceeds `MAX_BYTES`,
/// truncate to the last `KEEP_LINES` lines. Best-effort, errors ignored.
fn trim_if_oversized(path: &Path) {
    let Ok(meta) = std::fs::metadata(path) else {
        return;
    };
    if meta.len() <= MAX_BYTES {
        return;
    }
    let Ok(contents) = std::fs::read_to_string(path) else {
        return;
    };
    let lines: Vec<&str> = contents.lines().collect();
    if lines.len() <= KEEP_LINES {
        return;
    }
    let kept = lines[lines.len() - KEEP_LINES..].join("\n") + "\n";
    let _ = std::fs::write(path, kept);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;
    use tempfile::NamedTempFile;

    static LOCK: Mutex<()> = Mutex::new(());

    fn with_log_path<F: FnOnce(&Path)>(f: F) {
        let _guard = LOCK.lock().unwrap();
        let tmp = NamedTempFile::new().unwrap();
        let path = tmp.path().to_path_buf();
        drop(tmp);
        unsafe { std::env::set_var("GATE_LOG_PATH", &path) };
        f(&path);
        unsafe { std::env::remove_var("GATE_LOG_PATH") };
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn log_path_respects_env_var() {
        let _guard = LOCK.lock().unwrap();
        unsafe { std::env::set_var("GATE_LOG_PATH", "/tmp/gate-log-test-xyz.jsonl") };
        let p = log_path();
        unsafe { std::env::remove_var("GATE_LOG_PATH") };
        assert_eq!(p, PathBuf::from("/tmp/gate-log-test-xyz.jsonl"));
    }

    #[test]
    fn intercepted_event_round_trips() {
        with_log_path(|path| {
            record(&LogEvent::intercepted("bash", "psql")).unwrap();
            let contents = std::fs::read_to_string(path).unwrap();
            let parsed: LogEvent = serde_json::from_str(contents.trim()).unwrap();
            assert_eq!(parsed.outcome, "intercepted");
            assert_eq!(parsed.tool, "psql");
            assert_eq!(parsed.path, "bash");
        });
    }

    #[test]
    fn blocked_event_carries_detail() {
        with_log_path(|path| {
            record(&LogEvent::blocked("bash", "gate", "self-protection")).unwrap();
            let contents = std::fs::read_to_string(path).unwrap();
            let parsed: LogEvent = serde_json::from_str(contents.trim()).unwrap();
            assert_eq!(parsed.outcome, "blocked");
            assert_eq!(parsed.detail, "self-protection");
        });
    }

    #[test]
    fn rejected_event_carries_detail() {
        with_log_path(|path| {
            record(&LogEvent::rejected("bash", "psql", "denylisted column")).unwrap();
            let contents = std::fs::read_to_string(path).unwrap();
            let parsed: LogEvent = serde_json::from_str(contents.trim()).unwrap();
            assert_eq!(parsed.outcome, "rejected");
            assert_eq!(parsed.detail, "denylisted column");
        });
    }

    #[test]
    fn outcome_event_redacted_when_fields_present() {
        let stats = RedactStats {
            total: 2,
            type_counts: [("email".to_string(), 2)].into_iter().collect(),
            warnings: vec![],
        };
        let ev = LogEvent::outcome("bash", "psql", &stats, 150, vec!["ssn".to_string()]);
        assert_eq!(ev.outcome, "redacted");
        assert_eq!(ev.fields_redacted, 2);
        assert_eq!(ev.forced_columns, vec!["ssn".to_string()]);
        assert_eq!(ev.overhead_us, 150);
    }

    #[test]
    fn outcome_event_passthrough_when_no_fields() {
        let stats = RedactStats::default();
        let ev = LogEvent::outcome("bash", "curl", &stats, 10, vec![]);
        assert_eq!(ev.outcome, "passthrough");
        assert_eq!(ev.fields_redacted, 0);
    }

    #[test]
    fn trim_keeps_only_recent_lines_once_oversized() {
        with_log_path(|path| {
            // Write more than KEEP_LINES short lines directly, then force the
            // size threshold low via a tiny temp scenario: write enough bytes
            // to exceed MAX_BYTES is impractical in a unit test, so exercise
            // trim_if_oversized directly with a synthetic large file.
            let many_lines: String = (0..(KEEP_LINES + 50))
                .map(|i| format!("{{\"n\":{i}}}\n"))
                .collect();
            std::fs::write(path, &many_lines).unwrap();
            // Pad to exceed MAX_BYTES so trim actually triggers.
            let mut padded = many_lines.clone();
            while (padded.len() as u64) <= MAX_BYTES {
                padded.push_str(&many_lines);
            }
            std::fs::write(path, &padded).unwrap();
            trim_if_oversized(path);
            let contents = std::fs::read_to_string(path).unwrap();
            assert!(contents.lines().count() <= KEEP_LINES);
        });
    }
}
