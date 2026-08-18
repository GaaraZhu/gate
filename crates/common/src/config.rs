use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::patterns::COLUMN_DENYLIST;

#[derive(Debug, Deserialize, Serialize)]
pub struct Config {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub tools: HashMap<String, ToolConfig>,
    #[serde(default)]
    pub pii: PiiConfig,
    #[serde(default)]
    pub mcp: McpConfig,
    #[serde(default)]
    pub stats: StatsConfig,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            enabled: true,
            tools: HashMap::new(),
            pii: PiiConfig::default(),
            mcp: McpConfig::default(),
            stats: StatsConfig::default(),
        }
    }
}

/// Controls whether `gate retro` collects per-event counts on disk.
#[derive(Debug, Deserialize, Serialize)]
pub struct StatsConfig {
    /// When false, `gate run` and `gate mcp` skip writing to the stats log.
    /// `gate retro` still reads any pre-existing log.
    #[serde(default = "default_true")]
    pub enabled: bool,
}

impl Default for StatsConfig {
    fn default() -> Self {
        Self { enabled: true }
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct McpConfig {
    /// When false, `gate mcp` forwards all tool results without redaction (debug mode).
    #[serde(default = "default_true")]
    pub redact_tool_results: bool,
    /// Payloads larger than this (bytes) are forwarded unredacted with a stderr warning.
    /// Default: 5 MiB. Prevents OOM on very large file-content reads.
    #[serde(default = "default_max_payload_bytes")]
    pub max_payload_bytes: usize,
}

impl Default for McpConfig {
    fn default() -> Self {
        Self {
            redact_tool_results: true,
            max_payload_bytes: default_max_payload_bytes(),
        }
    }
}

fn default_max_payload_bytes() -> usize {
    5 * 1024 * 1024
}

#[derive(Debug, Default, Deserialize, Serialize, Clone)]
pub struct ToolConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sql_arg: Option<String>,
    /// When set, the hook rewrites invocations of this tool to use the named
    /// JSON-output wrapper instead (e.g. `psql` → `psql-json`). The wrapper
    /// must accept `--sql <query>` and emit JSON consumable by Gate 2.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub json_tool: Option<String>,
    /// When set, the flag value (from `sql_arg`) is parsed as JSON and the SQL
    /// is extracted from this path (e.g. "statement" for Databricks).
    /// Only applies when `sql_arg` is set but `json_tool` is not.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub json_sql_path: Option<String>,
    /// When set, the hook wraps this tool's command as `sh -c '<command> | <pipe>'`
    /// so Gate 2 always receives the piped output. Useful for tools like curl whose
    /// output is not JSON by default (e.g. `pipe: "jq -c ."`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pipe: Option<String>,
    /// Extra arguments appended to the tool invocation before spawning. Useful for
    /// injecting output-format flags automatically (e.g. `["--csv"]` for psql).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub extra_args: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct PiiConfig {
    /// Extra column names to force-redact beyond the built-in denylist.
    /// Use `effective_column_denylist()` to get the merged set.
    /// The alias `column_names` is accepted for backward compatibility.
    #[serde(default, alias = "column_names")]
    pub column_denylist: Vec<String>,
    /// Column names that must never be auto-redacted by name. Overrides both the built-in
    /// denylist and `column_denylist`. Value-based checks (Luhn, regex patterns) still apply.
    #[serde(default)]
    pub column_allowlist: Vec<String>,
    #[serde(default)]
    pub action: Action,
    #[serde(default)]
    pub wildcard_policy: WildcardPolicy,
    #[serde(default)]
    pub patterns: HashMap<String, Pattern>,
    #[serde(default = "default_column_name_boost")]
    pub column_name_boost: f32,
    #[serde(default = "default_confidence_threshold")]
    pub confidence_threshold: f32,
    #[serde(default = "default_redaction")]
    pub redaction: String,
    #[serde(default = "default_true")]
    pub include_summary: bool,
    /// When true, redacted values are replaced with a deterministic 8-char hex hash
    /// (e.g. `[PII:email:7f83b165]`) instead of the bare type label. The hash is
    /// salted with `hash_salt`, enabling cross-record joins without raw data exposure.
    #[serde(default)]
    pub hash_values: bool,
    /// Salt prepended to each value before hashing. Set a fixed secret to get
    /// consistent hashes across runs; leave empty for zero-config determinism.
    #[serde(default)]
    pub hash_salt: String,
}

#[derive(Debug, Deserialize, Serialize, Default, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Action {
    #[default]
    Redact,
    Warn,
    Reject,
}

#[derive(Debug, Deserialize, Serialize, Default, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum WildcardPolicy {
    #[default]
    Warn,
    Reject,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Pattern {
    pub regex: String,
    pub confidence: f32,
}

fn default_column_name_boost() -> f32 {
    0.15
}
fn default_confidence_threshold() -> f32 {
    0.8
}
fn default_redaction() -> String {
    "[PII:{type}]".to_string()
}
fn default_true() -> bool {
    true
}

impl Default for PiiConfig {
    fn default() -> Self {
        Self {
            column_denylist: Vec::new(),
            column_allowlist: Vec::new(),
            action: Action::default(),
            wildcard_policy: WildcardPolicy::default(),
            patterns: HashMap::new(),
            column_name_boost: default_column_name_boost(),
            confidence_threshold: default_confidence_threshold(),
            redaction: default_redaction(),
            include_summary: true,
            hash_values: false,
            hash_salt: String::new(),
        }
    }
}

impl PiiConfig {
    /// Returns the merged column denylist: built-in defaults union user-supplied additions.
    /// All names are lowercased. Order: builtins first, then user additions not already present.
    pub fn effective_column_denylist(&self) -> Vec<String> {
        let mut names: Vec<String> = COLUMN_DENYLIST.iter().map(|s| s.to_string()).collect();
        for name in &self.column_denylist {
            let lower = name.to_lowercase();
            if !names.iter().any(|n| n == &lower) {
                names.push(lower);
            }
        }
        names
    }

    /// Returns the lowercased allowlist. Columns in this list skip name-based redaction.
    pub fn effective_column_allowlist(&self) -> Vec<String> {
        self.column_allowlist
            .iter()
            .map(|s| s.to_lowercase())
            .collect()
    }
}

impl Config {
    pub fn load() -> Result<Self> {
        Ok(Self::load_with_provenance()?.0)
    }

    /// Loads the user config and, if present, merges the project config
    /// (`.gate/config.yaml`) over it using tighten-only semantics. Returns the
    /// merged config alongside provenance describing what came from where, for
    /// `gate validate` to report.
    pub fn load_with_provenance() -> Result<(Self, Provenance)> {
        let user_path = config_path()?;
        let mut config = Self::load_from_path(&user_path)?;

        let project_path = project_config_path();
        let mut provenance = Provenance {
            user_path,
            project_path: project_path.clone(),
            user_confidence_threshold: config.pii.confidence_threshold,
            project_confidence_threshold: None,
            effective_confidence_threshold: config.pii.confidence_threshold,
            project_tool_names: Vec::new(),
            user_tool_names: config.tools.keys().cloned().collect(),
            min_gate_version: None,
            project_column_allowlist: Vec::new(),
        };

        if let Some(path) = &project_path {
            let project = ProjectConfig::load_from_path(path)?;
            provenance.project_confidence_threshold = project.pii.confidence_threshold;
            provenance.project_tool_names = project.tools.keys().cloned().collect();
            provenance.min_gate_version = project.min_gate_version.clone();
            provenance.project_column_allowlist = project.pii.column_allowlist.clone();
            config.apply_project(&project);
            provenance.effective_confidence_threshold = config.pii.confidence_threshold;
        }

        Ok((config, provenance))
    }

    pub(crate) fn load_from_path(path: &std::path::Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let raw = std::fs::read_to_string(path)?;
        // Normalize Windows CRLF → LF so serde_yaml doesn't choke on \r.
        let contents = raw.replace('\r', "");
        serde_yaml::from_str(&contents)
            .map_err(|e| anyhow::anyhow!("Failed to parse config at {}: {}", path.display(), e))
    }

    /// Applies a project config over `self` using tighten-only merge rules:
    /// thresholds take the higher value, tools/patterns/column_denylist are unioned
    /// (project entries win on key collision). `column_allowlist` is the one
    /// deliberate exception — it's also unioned, but unlike the other fields a
    /// project allowlist entry *reduces* what gets redacted for everyone who picks
    /// up this file. `gate validate` surfaces it explicitly for that reason.
    fn apply_project(&mut self, project: &ProjectConfig) {
        if let Some(t) = project.pii.confidence_threshold {
            if t > self.pii.confidence_threshold {
                self.pii.confidence_threshold = t;
            }
        }
        if let Some(b) = project.pii.column_name_boost {
            if b > self.pii.column_name_boost {
                self.pii.column_name_boost = b;
            }
        }
        for (name, pattern) in &project.pii.patterns {
            self.pii.patterns.insert(name.clone(), pattern.clone());
        }
        for (name, tool) in &project.tools {
            self.tools.insert(name.clone(), tool.clone());
        }
        for name in &project.pii.column_denylist {
            if !self.pii.column_denylist.contains(name) {
                self.pii.column_denylist.push(name.clone());
            }
        }
        for name in &project.pii.column_allowlist {
            if !self.pii.column_allowlist.contains(name) {
                self.pii.column_allowlist.push(name.clone());
            }
        }
    }
}

/// Where the effective config's values came from — used by `gate validate` to
/// show config provenance and by `gate hook`/`gate validate` for the
/// `min_gate_version` floor check.
#[derive(Default)]
pub struct Provenance {
    pub user_path: std::path::PathBuf,
    pub project_path: Option<std::path::PathBuf>,
    pub user_confidence_threshold: f32,
    pub project_confidence_threshold: Option<f32>,
    pub effective_confidence_threshold: f32,
    pub project_tool_names: Vec<String>,
    pub user_tool_names: Vec<String>,
    pub min_gate_version: Option<String>,
    /// Non-empty when the project config adds column_allowlist entries — the one
    /// field that can reduce redaction for the whole team. Surfaced by
    /// `gate validate` so it's never a silent effect of pulling `.gate/config.yaml`.
    pub project_column_allowlist: Vec<String>,
}

/// The subset of config a project (`.gate/config.yaml`) can express. Deliberately
/// narrower than `Config` — a project config cannot set `enabled: false`, redaction
/// format, hashing, MCP/stats settings, or `action`/`wildcard_policy`. Every field
/// here is additive/tightening (union or take-the-max) with one deliberate
/// exception: `pii.column_allowlist`, which is unioned in too but *reduces*
/// redaction — see `Config::apply_project`.
#[derive(Debug, Default, Deserialize, Serialize)]
pub struct ProjectConfig {
    /// Minimum installed `gate` version required by this project. `gate hook` and
    /// `gate validate` warn (not block) when the installed version is older.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_gate_version: Option<String>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub tools: HashMap<String, ToolConfig>,
    #[serde(default)]
    pub pii: ProjectPiiConfig,
}

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct ProjectPiiConfig {
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub patterns: HashMap<String, Pattern>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confidence_threshold: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub column_name_boost: Option<f32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub column_denylist: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub column_allowlist: Vec<String>,
}

impl ProjectConfig {
    fn load_from_path(path: &std::path::Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let raw = std::fs::read_to_string(path)?;
        let contents = raw.replace('\r', "");
        serde_yaml::from_str(&contents).map_err(|e| {
            anyhow::anyhow!(
                "Failed to parse project config at {}: {}",
                path.display(),
                e
            )
        })
    }
}

pub fn config_path() -> Result<std::path::PathBuf> {
    if let Ok(path) = std::env::var("GATE_CONFIG") {
        return Ok(std::path::PathBuf::from(path));
    }
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .map_err(|_| anyhow::anyhow!("cannot resolve home directory: set HOME or USERPROFILE"))?;
    Ok(std::path::PathBuf::from(home)
        .join(".config")
        .join("gate")
        .join("config.yaml"))
}

/// Finds the project config by walking up from the current directory looking for
/// `.gate/config.yaml`, the same way git walks up looking for `.git`. Returns
/// `None` if no such file is found before reaching the filesystem root.
/// `GATE_PROJECT_CONFIG` overrides this (used by tests and to point at a config
/// outside the CWD walk).
pub fn project_config_path() -> Option<std::path::PathBuf> {
    if let Ok(path) = std::env::var("GATE_PROJECT_CONFIG") {
        let p = std::path::PathBuf::from(path);
        return if p.exists() { Some(p) } else { None };
    }
    let mut dir = std::env::current_dir().ok()?;
    loop {
        let candidate = dir.join(".gate").join("config.yaml");
        if candidate.exists() {
            return Some(candidate);
        }
        if !dir.pop() {
            return None;
        }
    }
}

/// Compares two `"x.y.z"` version strings without pulling in a semver dependency.
/// Missing/non-numeric components are treated as 0. Returns true if `installed` is
/// strictly older than `floor`.
pub fn version_is_older(installed: &str, floor: &str) -> bool {
    fn parts(v: &str) -> [u32; 3] {
        let mut out = [0u32; 3];
        for (i, p) in v.trim().split('.').take(3).enumerate() {
            out[i] = p.parse().unwrap_or(0);
        }
        out
    }
    parts(installed) < parts(floor)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::sync::Mutex;
    use tempfile::NamedTempFile;

    static LOCK: Mutex<()> = Mutex::new(());

    // No-project-config sentinel: keeps these tests isolated from whatever real
    // `.gate/config.yaml` might exist above the test process's CWD.
    const NO_PROJECT: &str = "/tmp/redact_nonexistent_project_xyz_abc/config.yaml";

    fn load_from_yaml(yaml: &str) -> Result<Config> {
        let _guard = LOCK.lock().unwrap();
        let mut f = NamedTempFile::new().unwrap();
        f.write_all(yaml.as_bytes()).unwrap();
        unsafe {
            std::env::set_var("GATE_CONFIG", f.path());
            std::env::set_var("GATE_PROJECT_CONFIG", NO_PROJECT);
        }
        let result = Config::load();
        unsafe {
            std::env::remove_var("GATE_CONFIG");
            std::env::remove_var("GATE_PROJECT_CONFIG");
        }
        result
    }

    fn load_missing() -> Result<Config> {
        let _guard = LOCK.lock().unwrap();
        unsafe {
            std::env::set_var("GATE_CONFIG", "/tmp/redact_nonexistent_xyz_abc.yaml");
            std::env::set_var("GATE_PROJECT_CONFIG", NO_PROJECT);
        }
        let result = Config::load();
        unsafe {
            std::env::remove_var("GATE_CONFIG");
            std::env::remove_var("GATE_PROJECT_CONFIG");
        }
        result
    }

    #[test]
    fn defaults_when_file_missing() {
        let config = load_missing().unwrap();
        assert!(config.enabled);
        assert_eq!(config.pii.column_name_boost, 0.15);
        assert_eq!(config.pii.confidence_threshold, 0.8);
        assert_eq!(config.pii.redaction, "[PII:{type}]");
        assert!(config.pii.include_summary);
        assert_eq!(config.pii.action, Action::Redact);
        assert_eq!(config.pii.wildcard_policy, WildcardPolicy::Warn);
        assert!(config.tools.is_empty());
        assert!(config.pii.column_denylist.is_empty());
        assert!(config.pii.column_allowlist.is_empty());
        assert!(config.pii.patterns.is_empty());
        assert!(!config.pii.hash_values);
        assert_eq!(config.pii.hash_salt, "");
    }

    #[test]
    fn enabled_false_parses_correctly() {
        let config = load_from_yaml("enabled: false\n").unwrap();
        assert!(!config.enabled);
    }

    #[test]
    fn enabled_defaults_to_true_when_key_absent() {
        let config = load_from_yaml("pii:\n  action: warn\n").unwrap();
        assert!(config.enabled);
    }

    #[test]
    fn round_trip_parse() {
        let yaml = r#"
tools:
  tkpsql:
    sql_arg: "--sql"
  mysql:
    sql_arg: ~
pii:
  action: warn
  wildcard_policy: warn
  column_name_boost: 0.2
  confidence_threshold: 0.9
  redaction: "[REDACTED:{type}]"
  include_summary: false
  hash_values: true
  hash_salt: "my-secret"
  column_names:
    - secret_token
  patterns:
    custom_id:
      regex: "\\bID-\\d{6}\\b"
      confidence: 0.9
"#;
        let config = load_from_yaml(yaml).unwrap();
        assert_eq!(config.tools["tkpsql"].sql_arg, Some("--sql".to_string()));
        assert!(config.tools["mysql"].sql_arg.is_none());
        assert_eq!(config.pii.action, Action::Warn);
        assert_eq!(config.pii.wildcard_policy, WildcardPolicy::Warn);
        assert_eq!(config.pii.column_name_boost, 0.2);
        assert_eq!(config.pii.confidence_threshold, 0.9);
        assert_eq!(config.pii.redaction, "[REDACTED:{type}]");
        assert!(!config.pii.include_summary);
        assert!(config.pii.hash_values);
        assert_eq!(config.pii.hash_salt, "my-secret");
        assert_eq!(config.pii.column_denylist, vec!["secret_token"]); // parsed via alias "column_names"
        let pat = &config.pii.patterns["custom_id"];
        assert_eq!(pat.regex, r"\bID-\d{6}\b");
        assert_eq!(pat.confidence, 0.9);
    }

    #[test]
    fn partial_yaml_fills_defaults() {
        // Only override one field; all others must stay at their defaults.
        let config = load_from_yaml("pii:\n  action: warn\n").unwrap();
        assert_eq!(config.pii.action, Action::Warn);
        assert_eq!(config.pii.column_name_boost, 0.15);
        assert_eq!(config.pii.confidence_threshold, 0.8);
        assert_eq!(config.pii.redaction, "[PII:{type}]");
        assert!(config.pii.include_summary);
        assert_eq!(config.pii.wildcard_policy, WildcardPolicy::Warn);
        assert!(config.tools.is_empty());
        assert!(!config.pii.hash_values, "hash_values must default to false");
        assert_eq!(config.pii.hash_salt, "", "hash_salt must default to empty");
    }

    #[test]
    fn hash_values_parsed_from_yaml() {
        let config =
            load_from_yaml("pii:\n  hash_values: true\n  hash_salt: \"my-secret\"\n").unwrap();
        assert!(config.pii.hash_values);
        assert_eq!(config.pii.hash_salt, "my-secret");
    }

    #[test]
    fn empty_yaml_uses_all_defaults() {
        let config = load_from_yaml("").unwrap();
        assert_eq!(config.pii.column_name_boost, 0.15);
        assert_eq!(config.pii.confidence_threshold, 0.8);
        assert_eq!(config.pii.redaction, "[PII:{type}]");
        assert!(config.pii.include_summary);
    }

    #[test]
    fn crlf_line_endings_parse_correctly() {
        // Windows config files use CRLF; serde_yaml chokes on \r without normalization.
        let yaml = "enabled: true\r\npii:\r\n  action: warn\r\n";
        let config = load_from_yaml(yaml).unwrap();
        assert!(config.enabled);
        assert_eq!(config.pii.action, Action::Warn);
    }

    #[test]
    fn malformed_yaml_returns_error() {
        let result = load_from_yaml("pii: {bad: yaml: :: :");
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Failed to parse config"));
    }

    #[test]
    fn unknown_action_variant_is_error() {
        let result = load_from_yaml("pii:\n  action: explode\n");
        assert!(result.is_err());
    }

    #[test]
    fn pipe_field_parses_correctly() {
        let config = load_from_yaml("tools:\n  curl:\n    pipe: \"jq -c .\"\n").unwrap();
        assert_eq!(config.tools["curl"].pipe, Some("jq -c .".to_string()));
        assert!(config.tools["curl"].sql_arg.is_none());
        assert!(config.tools["curl"].json_tool.is_none());
    }

    #[test]
    fn pipe_defaults_to_none() {
        let config = load_from_yaml("tools:\n  psql:\n    sql_arg: \"-c\"\n").unwrap();
        assert!(config.tools["psql"].pipe.is_none());
    }

    #[test]
    fn mcp_defaults_when_absent() {
        let config = load_missing().unwrap();
        assert!(config.mcp.redact_tool_results);
        assert_eq!(config.mcp.max_payload_bytes, 5 * 1024 * 1024);
    }

    #[test]
    fn mcp_parses_from_yaml() {
        let config =
            load_from_yaml("mcp:\n  redact_tool_results: false\n  max_payload_bytes: 1048576\n")
                .unwrap();
        assert!(!config.mcp.redact_tool_results);
        assert_eq!(config.mcp.max_payload_bytes, 1_048_576);
    }

    #[test]
    fn mcp_partial_yaml_fills_defaults() {
        let config = load_from_yaml("mcp:\n  redact_tool_results: false\n").unwrap();
        assert!(!config.mcp.redact_tool_results);
        assert_eq!(config.mcp.max_payload_bytes, 5 * 1024 * 1024);
    }

    #[test]
    fn stats_defaults_enabled() {
        let config = load_missing().unwrap();
        assert!(config.stats.enabled);
    }

    #[test]
    fn stats_can_be_disabled() {
        let config = load_from_yaml("stats:\n  enabled: false\n").unwrap();
        assert!(!config.stats.enabled);
    }

    #[test]
    fn column_allowlist_parses_from_yaml() {
        let yaml = "pii:\n  column_allowlist:\n    - city\n    - state\n";
        let config = load_from_yaml(yaml).unwrap();
        assert_eq!(config.pii.column_allowlist, vec!["city", "state"]);
    }

    #[test]
    fn column_allowlist_defaults_to_empty() {
        let config = load_from_yaml("pii:\n  action: warn\n").unwrap();
        assert!(config.pii.column_allowlist.is_empty());
    }

    #[test]
    fn effective_column_allowlist_lowercases() {
        let config =
            load_from_yaml("pii:\n  column_allowlist:\n    - City\n    - STATE\n").unwrap();
        let al = config.pii.effective_column_allowlist();
        assert!(al.contains(&"city".to_string()));
        assert!(al.contains(&"state".to_string()));
    }

    // ── project config merge (team support) ────────────────────────────────

    fn load_merged(user_yaml: &str, project_yaml: &str) -> Config {
        let _guard = LOCK.lock().unwrap();
        let mut user_f = NamedTempFile::new().unwrap();
        user_f.write_all(user_yaml.as_bytes()).unwrap();
        let mut project_f = NamedTempFile::new().unwrap();
        project_f.write_all(project_yaml.as_bytes()).unwrap();
        unsafe {
            std::env::set_var("GATE_CONFIG", user_f.path());
            std::env::set_var("GATE_PROJECT_CONFIG", project_f.path());
        }
        let result = Config::load();
        unsafe {
            std::env::remove_var("GATE_CONFIG");
            std::env::remove_var("GATE_PROJECT_CONFIG");
        }
        result.unwrap()
    }

    #[test]
    fn project_config_absent_leaves_user_config_untouched() {
        let config = load_from_yaml("pii:\n  confidence_threshold: 0.5\n").unwrap();
        assert_eq!(config.pii.confidence_threshold, 0.5);
    }

    #[test]
    fn project_threshold_raises_user_threshold() {
        let config = load_merged(
            "pii:\n  confidence_threshold: 0.5\n",
            "pii:\n  confidence_threshold: 0.9\n",
        );
        assert_eq!(config.pii.confidence_threshold, 0.9);
    }

    #[test]
    fn project_threshold_cannot_lower_user_threshold() {
        // Security-critical: a project config with a *lower* threshold than the
        // user's personal config must never weaken protection.
        let config = load_merged(
            "pii:\n  confidence_threshold: 0.9\n",
            "pii:\n  confidence_threshold: 0.3\n",
        );
        assert_eq!(
            config.pii.confidence_threshold, 0.9,
            "project config must never lower the effective threshold"
        );
    }

    #[test]
    fn project_column_name_boost_raises_but_never_lowers() {
        let raised = load_merged(
            "pii:\n  column_name_boost: 0.1\n",
            "pii:\n  column_name_boost: 0.4\n",
        );
        assert_eq!(raised.pii.column_name_boost, 0.4);

        let not_lowered = load_merged(
            "pii:\n  column_name_boost: 0.4\n",
            "pii:\n  column_name_boost: 0.1\n",
        );
        assert_eq!(not_lowered.pii.column_name_boost, 0.4);
    }

    #[test]
    fn project_tools_union_with_user_tools() {
        let config = load_merged(
            "tools:\n  tkpsql:\n    sql_arg: \"--sql\"\n",
            "tools:\n  bq:\n    sql_arg: \"--sql\"\n",
        );
        assert!(config.tools.contains_key("tkpsql"));
        assert!(config.tools.contains_key("bq"));
    }

    #[test]
    fn project_tool_wins_on_key_collision() {
        let config = load_merged(
            "tools:\n  psql:\n    sql_arg: \"-c\"\n",
            "tools:\n  psql:\n    sql_arg: \"--sql\"\n",
        );
        assert_eq!(config.tools["psql"].sql_arg, Some("--sql".to_string()));
    }

    #[test]
    fn project_patterns_union_with_user_patterns() {
        let config = load_merged(
            "pii:\n  patterns:\n    user_pat:\n      regex: 'a'\n      confidence: 0.5\n",
            "pii:\n  patterns:\n    team_pat:\n      regex: 'b'\n      confidence: 0.9\n",
        );
        assert!(config.pii.patterns.contains_key("user_pat"));
        assert!(config.pii.patterns.contains_key("team_pat"));
    }

    #[test]
    fn project_column_denylist_unions_with_user_denylist() {
        let config = load_merged(
            "pii:\n  column_denylist:\n    - user_secret\n",
            "pii:\n  column_denylist:\n    - team_secret\n",
        );
        assert!(config
            .pii
            .column_denylist
            .contains(&"user_secret".to_string()));
        assert!(config
            .pii
            .column_denylist
            .contains(&"team_secret".to_string()));
    }

    #[test]
    fn project_column_allowlist_unions_with_user_allowlist() {
        // Deliberate exception: unlike every other project field, this can reduce
        // redaction team-wide. Covered explicitly so a future change to this
        // behavior is a visible, intentional diff — not an accidental regression.
        let config = load_merged(
            "pii:\n  column_allowlist:\n    - city\n",
            "pii:\n  column_allowlist:\n    - employee_id\n",
        );
        assert!(config.pii.column_allowlist.contains(&"city".to_string()));
        assert!(config
            .pii
            .column_allowlist
            .contains(&"employee_id".to_string()));
    }

    #[test]
    fn provenance_reports_project_column_allowlist() {
        let _guard = LOCK.lock().unwrap();
        let mut user_f = NamedTempFile::new().unwrap();
        user_f.write_all(b"").unwrap();
        let mut project_f = NamedTempFile::new().unwrap();
        project_f
            .write_all(b"pii:\n  column_allowlist:\n    - employee_id\n")
            .unwrap();
        unsafe {
            std::env::set_var("GATE_CONFIG", user_f.path());
            std::env::set_var("GATE_PROJECT_CONFIG", project_f.path());
        }
        let (_config, provenance) = Config::load_with_provenance().unwrap();
        unsafe {
            std::env::remove_var("GATE_CONFIG");
            std::env::remove_var("GATE_PROJECT_CONFIG");
        }
        assert_eq!(
            provenance.project_column_allowlist,
            vec!["employee_id".to_string()]
        );
    }

    #[test]
    fn project_config_cannot_disable_gate() {
        // ProjectConfig has no `enabled` field at all, so this key is silently
        // ignored — a project config can never turn protection off.
        let config = load_merged("", "enabled: false\n");
        assert!(config.enabled);
    }

    #[test]
    fn load_with_provenance_reports_sources() {
        let _guard = LOCK.lock().unwrap();
        let mut user_f = NamedTempFile::new().unwrap();
        user_f
            .write_all(b"pii:\n  confidence_threshold: 0.5\n")
            .unwrap();
        let mut project_f = NamedTempFile::new().unwrap();
        project_f
            .write_all(b"min_gate_version: \"1.2.0\"\npii:\n  confidence_threshold: 0.9\n")
            .unwrap();
        unsafe {
            std::env::set_var("GATE_CONFIG", user_f.path());
            std::env::set_var("GATE_PROJECT_CONFIG", project_f.path());
        }
        let (config, provenance) = Config::load_with_provenance().unwrap();
        unsafe {
            std::env::remove_var("GATE_CONFIG");
            std::env::remove_var("GATE_PROJECT_CONFIG");
        }
        assert_eq!(config.pii.confidence_threshold, 0.9);
        assert_eq!(provenance.user_confidence_threshold, 0.5);
        assert_eq!(provenance.project_confidence_threshold, Some(0.9));
        assert_eq!(provenance.effective_confidence_threshold, 0.9);
        assert_eq!(provenance.min_gate_version.as_deref(), Some("1.2.0"));
        assert!(provenance.project_path.is_some());
    }

    #[test]
    fn project_config_path_none_when_env_points_to_missing_file() {
        let _guard = LOCK.lock().unwrap();
        unsafe { std::env::set_var("GATE_PROJECT_CONFIG", NO_PROJECT) };
        let result = project_config_path();
        unsafe { std::env::remove_var("GATE_PROJECT_CONFIG") };
        assert!(result.is_none());
    }

    #[test]
    fn version_is_older_basic_comparisons() {
        assert!(version_is_older("1.1.0", "1.2.0"));
        assert!(!version_is_older("1.2.0", "1.2.0"));
        assert!(!version_is_older("1.3.0", "1.2.0"));
        assert!(version_is_older("0.9.5", "0.10.0"));
        assert!(version_is_older("1.2", "1.2.1"));
    }
}
