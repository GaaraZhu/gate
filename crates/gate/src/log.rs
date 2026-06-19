//! `gate log` — view interception events.
//!
//! Reads the ephemeral event feed written by `gate hook`, `gate run`, and
//! `gate mcp` (see `common::event_log`). By default dumps everything
//! currently recorded and exits, like `git log`; pass `--follow` to keep
//! watching for new events, like `docker logs -f`. Counts and labels only —
//! never command lines, SQL text, or PII values, matching the same
//! non-negotiable that keeps `_gate_summary` and `gate retro` PII-free.

use common::event_log::{log_path, LogEvent};
use std::io::{Read, Seek, SeekFrom};
use std::time::Duration;

const POLL_INTERVAL: Duration = Duration::from_millis(150);

pub fn run(json: bool, tool: Option<String>, path: Option<String>, follow: bool) {
    let file_path = log_path();
    if let Some(parent) = file_path.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    if !file_path.exists() {
        std::fs::File::create(&file_path).ok();
    }

    if !json {
        println!("[gate log] times are UTC");
    }

    let mut offset = 0u64;
    let mut printed_any = false;
    process_new_lines(
        &file_path,
        &mut offset,
        &tool,
        &path,
        json,
        &mut printed_any,
    );

    if !follow {
        if !json && !printed_any {
            println!("No events recorded yet. Run a command through a configured tool to see events here.");
        }
        return;
    }

    if !json {
        println!();
        println!(
            "[gate log] watching {} for new interception events (Ctrl+C to stop)",
            file_path.display()
        );
    }

    loop {
        std::thread::sleep(POLL_INTERVAL);
        process_new_lines(
            &file_path,
            &mut offset,
            &tool,
            &path,
            json,
            &mut printed_any,
        );
    }
}

/// Read and print any lines appended to `file_path` since `offset`, advancing `offset`
/// past the bytes consumed. Handles truncation (the writer's size-cap rotation) by
/// restarting from the top when the file has shrunk.
fn process_new_lines(
    file_path: &std::path::Path,
    offset: &mut u64,
    tool: &Option<String>,
    path: &Option<String>,
    json: bool,
    printed_any: &mut bool,
) {
    let len = match std::fs::metadata(file_path) {
        Ok(m) => m.len(),
        Err(_) => return,
    };
    if len < *offset {
        *offset = 0;
    }
    if len == *offset {
        return;
    }

    let mut file = match std::fs::File::open(file_path) {
        Ok(f) => f,
        Err(_) => return,
    };
    if file.seek(SeekFrom::Start(*offset)).is_err() {
        return;
    }
    let mut buf = Vec::new();
    if file.read_to_end(&mut buf).is_err() {
        return;
    }

    let color = crate::color::supports_color();
    let mut start = 0usize;
    while let Some(pos) = buf[start..].iter().position(|&b| b == b'\n') {
        let line_bytes = &buf[start..start + pos];
        start += pos + 1;
        let line = String::from_utf8_lossy(line_bytes);
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let Ok(event) = serde_json::from_str::<LogEvent>(trimmed) else {
            continue;
        };
        if let Some(t) = tool {
            if &event.tool != t {
                continue;
            }
        }
        if let Some(p) = path {
            if &event.path != p {
                continue;
            }
        }
        if json {
            println!("{trimmed}");
        } else {
            println!("{}", format_event(&event, color));
        }
        *printed_any = true;
    }
    *offset += start as u64;
}

fn format_event(ev: &LogEvent, color: bool) -> String {
    let ts = format_ts(ev.ts);
    let (outcome_color, reset) = if color {
        (outcome_style(&ev.outcome), "\x1b[0m")
    } else {
        ("", "")
    };

    let mut detail_parts: Vec<String> = Vec::new();
    if !ev.detail.is_empty() {
        detail_parts.push(ev.detail.clone());
    }
    if ev.fields_redacted > 0 {
        let mut types: Vec<(&String, &usize)> = ev.types.iter().collect();
        types.sort_by(|a, b| b.1.cmp(a.1).then(a.0.cmp(b.0)));
        let types_str = types
            .iter()
            .map(|(k, v)| format!("{k}:{v}"))
            .collect::<Vec<_>>()
            .join(",");
        detail_parts.push(format!("fields={} types=[{types_str}]", ev.fields_redacted));
    }
    if !ev.forced_columns.is_empty() {
        detail_parts.push(format!("forced=[{}]", ev.forced_columns.join(",")));
    }
    if ev.overhead_us > 0 {
        detail_parts.push(format!("overhead={}", format_overhead(ev.overhead_us)));
    }
    if !ev.warnings.is_empty() {
        detail_parts.push(format!("warnings={}", ev.warnings.len()));
    }
    let detail = detail_parts.join(" ");

    format!(
        "{ts}  {:<5} {:<10} {outcome_color}{:<11}{reset} {detail}",
        ev.path,
        ev.tool,
        ev.outcome.to_uppercase(),
    )
}

fn outcome_style(outcome: &str) -> &'static str {
    match outcome {
        "intercepted" => "\x1b[36m", // cyan
        "redacted" => "\x1b[33m",    // yellow
        "passthrough" => "\x1b[2m",  // dim
        "rejected" => "\x1b[31m",    // red
        "blocked" => "\x1b[1;31m",   // bold red
        _ => "",
    }
}

fn format_ts(ts_ms: u64) -> String {
    let ms = ts_ms % 1000;
    let secs_of_day = (ts_ms / 1000) % 86400;
    let h = secs_of_day / 3600;
    let m = (secs_of_day % 3600) / 60;
    let s = secs_of_day % 60;
    format!("{h:02}:{m:02}:{s:02}.{ms:03}")
}

fn format_overhead(us: u64) -> String {
    if us >= 1000 {
        format!("{:.1}ms", us as f64 / 1000.0)
    } else {
        format!("{us}us")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::sync::Mutex;
    use tempfile::NamedTempFile;

    static LOCK: Mutex<()> = Mutex::new(());

    fn ev(outcome: &str) -> LogEvent {
        LogEvent {
            ts: 3_723_004, // 01:02:03.004 UTC
            path: "bash".to_string(),
            tool: "psql".to_string(),
            outcome: outcome.to_string(),
            fields_redacted: 0,
            types: HashMap::new(),
            forced_columns: vec![],
            warnings: vec![],
            overhead_us: 0,
            detail: String::new(),
        }
    }

    #[test]
    fn format_ts_renders_utc_clock() {
        assert_eq!(format_ts(3_723_004), "01:02:03.004");
    }

    #[test]
    fn format_overhead_switches_units() {
        assert_eq!(format_overhead(500), "500us");
        assert_eq!(format_overhead(1500), "1.5ms");
    }

    #[test]
    fn format_event_includes_outcome_and_path() {
        let line = format_event(&ev("intercepted"), false);
        assert!(line.contains("bash"));
        assert!(line.contains("psql"));
        assert!(line.contains("INTERCEPTED"));
    }

    #[test]
    fn format_event_includes_redaction_detail() {
        let mut e = ev("redacted");
        e.fields_redacted = 3;
        e.types.insert("email".to_string(), 2);
        e.types.insert("ssn".to_string(), 1);
        e.forced_columns = vec!["ssn".to_string()];
        e.overhead_us = 1500;
        let line = format_event(&e, false);
        assert!(line.contains("fields=3"));
        assert!(line.contains("email:2"));
        assert!(line.contains("forced=[ssn]"));
        assert!(line.contains("overhead=1.5ms"));
    }

    #[test]
    fn format_event_includes_block_detail() {
        let mut e = ev("blocked");
        e.detail = "self-protection".to_string();
        let line = format_event(&e, false);
        assert!(line.contains("BLOCKED"));
        assert!(line.contains("self-protection"));
    }

    fn with_log_path<F: FnOnce(&std::path::Path)>(f: F) {
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
    fn process_new_lines_reads_pre_existing_content_from_offset_zero() {
        with_log_path(|path| {
            let line = serde_json::to_string(&ev("redacted")).unwrap();
            std::fs::write(path, format!("{line}\n")).unwrap();

            let mut offset = 0u64;
            let mut printed_any = false;
            // Simulate dump mode: collect output via the same logic, but assert via offset
            // advancing past the whole pre-existing line.
            process_new_lines(path, &mut offset, &None, &None, true, &mut printed_any);
            assert!(printed_any);
            assert_eq!(offset, (line.len() + 1) as u64);
        });
    }

    #[test]
    fn process_new_lines_resets_offset_when_file_shrinks() {
        with_log_path(|path| {
            let line = serde_json::to_string(&ev("redacted")).unwrap();
            std::fs::write(path, format!("{line}\n{line}\n")).unwrap();
            let mut offset = ((line.len() + 1) * 2) as u64;
            let mut printed_any = false;

            // File rotated/truncated down to a single line.
            std::fs::write(path, format!("{line}\n")).unwrap();
            process_new_lines(path, &mut offset, &None, &None, true, &mut printed_any);
            assert!(printed_any);
            assert_eq!(offset, (line.len() + 1) as u64);
        });
    }
}
