use clap::{Parser, Subcommand, ValueEnum};

#[derive(ValueEnum, Clone)]
enum Harness {
    #[value(name = "claude-code")]
    ClaudeCode,
    Opencode,
    Cursor,
    #[value(name = "copilot-cli")]
    CopilotCli,
    Codex,
    Gemini,
    #[value(name = "codebuddy")]
    CodeBuddy,
}

impl Harness {
    fn as_str(&self) -> &'static str {
        match self {
            Self::ClaudeCode => "claude-code",
            Self::Opencode => "opencode",
            Self::Cursor => "cursor",
            Self::CopilotCli => "copilot-cli",
            Self::Codex => "codex",
            Self::Gemini => "gemini",
            Self::CodeBuddy => "codebuddy",
        }
    }
}

#[derive(ValueEnum, Clone)]
enum HookFormat {
    #[value(name = "claude-code")]
    ClaudeCode,
    Copilot,
    Cursor,
    Codex,
    Gemini,
    #[value(name = "codebuddy")]
    CodeBuddy,
}

impl HookFormat {
    fn as_str(&self) -> &'static str {
        match self {
            Self::ClaudeCode => "claude-code",
            Self::Copilot => "copilot",
            Self::Cursor => "cursor",
            Self::Codex => "codex",
            Self::Gemini => "gemini",
            Self::CodeBuddy => "codebuddy",
        }
    }
}

mod allowlist;
mod color;
mod command;
mod config_cmd;
mod enable_disable;
mod hook;
mod init;
mod init_opencode;
mod list;
mod log;
mod protect;
mod retro;
mod run;
mod scan;
mod starter;
mod uninstall;
mod validate;

#[derive(Parser)]
#[command(
    name = "gate",
    version,
    about = "PII-filtering proxy for AI agent query tools"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    // ── Setup ────────────────────────────────────────────────────────────────
    #[command(
        about = "Register the PreToolUse hook in the agent harness settings.\nWith --wrap-mcp, converts existing MCP servers to gate mcp proxies (dry-run by default; use --yes to apply).\nWith --mcp, registers a single gate mcp proxy entry for a named MCP server."
    )]
    Init {
        /// Agent harness to install the hook into
        #[arg(long, default_value = "claude-code")]
        harness: Harness,
        /// Installation scope: global/user (default) or project
        #[arg(long, default_value = "global")]
        scope: String,
        /// Name of the MCP server to register (e.g. "postgres")
        #[arg(long)]
        mcp: Option<String>,
        /// Upstream MCP server command string (used with --mcp), e.g. "uvx mcp-server-postgres"
        #[arg(long = "mcp-cmd")]
        mcp_cmd: Option<String>,
        /// Convert all existing MCP servers in the harness config to gate mcp proxies (dry-run by default)
        #[arg(long = "wrap-mcp")]
        wrap_mcp: bool,
        /// Comma-separated list of server names to wrap (used with --wrap-mcp; default wraps all)
        #[arg(long)]
        servers: Option<String>,
        /// Apply changes (used with --wrap-mcp; default is dry-run)
        #[arg(long)]
        yes: bool,
    },
    #[command(
        about = "Export your personal config into config.yaml in the current directory so it can be committed and shared with the team. No git repo required.\nAlways overwrites an existing team config — it's meant to be git-tracked, so commit before re-running if you want a rollback point."
    )]
    Export,
    #[command(
        about = "Manage the gate config file.\nWith --sync, finds a team config — .gate/config.yaml or a bare config.yaml, walking up from the current directory; no git repo required — and merges it into your personal config. Does nothing if none is found (use `gate export` to create one). Non-interactive, safe inside an agent harness. Re-run after a git pull to pick up team config changes."
    )]
    Config {
        /// Print the resolved config file path and exit
        #[arg(long)]
        path: bool,
        /// Print the raw config file contents and exit
        #[arg(long)]
        print: bool,
        /// Write a starter config if missing, then exit (no editor)
        #[arg(long = "init-only")]
        init_only: bool,
        /// Ensure .gate/config.yaml exists and merge it into personal config, then exit (no editor)
        #[arg(long)]
        sync: bool,
    },
    // ── Daily use ────────────────────────────────────────────────────────────
    #[command(
        about = "Read columnar JSON from stdin and report PII-exposed column names.\nPipe the output of a schema query (SELECT TABLE_NAME, COLUMN_NAME ...) into this command.\nExample: tkdbr query --sql \"SELECT TABLE_NAME, COLUMN_NAME FROM ...\" | gate scan"
    )]
    Scan {
        /// Show all detected columns in the Top Findings section (not truncated)
        #[arg(long)]
        verbose: bool,
        /// Emit results as JSON instead of human-readable text
        #[arg(long)]
        json: bool,
        /// After showing results, interactively mark false-positive columns to add to the allowlist
        #[arg(long)]
        review: bool,
    },
    #[command(
        about = "Manage the column allowlist — columns that skip name-based PII redaction.\nValue-based checks (Luhn, regex patterns) still apply to allowlisted columns"
    )]
    Allowlist {
        #[command(subcommand)]
        action: AllowlistAction,
    },
    #[command(
        about = "Show a protection retrospective: how many queries gate protected and how many PII fields it redacted (also known as stats/audit/report)"
    )]
    Retro,
    #[command(
        about = "Show interception events: which commands gate matched, redacted, passed through, or blocked.\nCounts and labels only — never command lines, SQL text, or PII values.\nBy default prints recorded events and exits; use --follow to keep watching for new ones."
    )]
    Log {
        /// Keep watching for new events after printing existing ones (like `docker logs -f`)
        #[arg(short, long)]
        follow: bool,
        /// Emit raw JSON event lines instead of human-readable text
        #[arg(long)]
        json: bool,
        /// Only show events for this tool/server name (e.g. psql)
        #[arg(long)]
        tool: Option<String>,
        /// Only show events from this path: bash, mcp, or stdin
        #[arg(long)]
        path: Option<String>,
    },
    /// Enable PII redaction (sets enabled: true in config)
    Enable,
    /// Disable PII redaction (sets enabled: false in config)
    Disable,
    /// Load config, compile patterns, and report errors or warnings
    Validate,
    /// List configured tools and their sql_arg values
    List,
    // ── Plumbing (called by harness) ─────────────────────────────────────────
    #[command(
        about = "Execute a tool with Gate 1 + Gate 2 PII redaction on its JSON output.\nWith no args, reads JSON from stdin and applies Gate 2 directly"
    )]
    Run {
        /// Print per-field redaction decisions to stderr for debugging
        #[arg(long)]
        verbose: bool,
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// PreToolUse hook: rewrite matching Bash commands to route through gate run
    Hook {
        /// Output format matching the harness that invokes this hook
        #[arg(long, default_value = "claude-code")]
        format: HookFormat,
    },
    #[command(
        about = "Run a stdio MCP proxy: intercepts tools/call responses and redacts PII.\nUsage: gate mcp [--name <server>] [--] <upstream-cmd> [args...]\nExample: gate mcp --name postgres -- uvx mcp-server-postgres"
    )]
    Mcp {
        /// Logical server name used in `gate retro` stats. Defaults to the upstream
        /// binary basename when omitted.
        #[arg(long)]
        name: Option<String>,
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        upstream: Vec<String>,
    },
    // ── Advanced / rare ──────────────────────────────────────────────────────
    #[command(
        about = "Protect the config file by transferring ownership to root (Unix only).\nAfter this, all config changes require sudo. Run as: sudo gate protect"
    )]
    Protect,
    #[command(
        about = "Remove root ownership from the config file, restoring direct write access.\nRun as: sudo gate unprotect"
    )]
    Unprotect,
    /// Remove gate hooks from all harnesses, the config directory, and gate-generated plugin files
    Uninstall,
    /// Print version
    Version,
}

#[derive(Subcommand)]
enum AllowlistAction {
    /// Add column names to the allowlist
    Add {
        /// One or more column names to allowlist
        #[arg(required = true)]
        columns: Vec<String>,
    },
    /// Remove column names from the allowlist
    Remove {
        /// One or more column names to remove
        #[arg(required = true)]
        columns: Vec<String>,
    },
    /// Show the current allowlist
    List,
}

fn main() {
    let cli = Cli::parse();
    match cli.command {
        Commands::Init {
            harness,
            scope,
            mcp,
            mcp_cmd,
            wrap_mcp,
            servers,
            yes,
        } => init::run(
            harness.as_str(),
            &scope,
            mcp.as_deref(),
            mcp_cmd.as_deref(),
            wrap_mcp,
            servers.as_deref(),
            yes,
        ),
        Commands::Export => init::run_export(),
        Commands::Config {
            path,
            print,
            init_only,
            sync,
        } => config_cmd::run(path, print, init_only, sync),
        Commands::Scan {
            verbose,
            json,
            review,
        } => scan::run(verbose, json, review),
        Commands::Allowlist { action } => match action {
            AllowlistAction::Add { columns } => allowlist::run(allowlist::Action::Add(columns)),
            AllowlistAction::Remove { columns } => {
                allowlist::run(allowlist::Action::Remove(columns))
            }
            AllowlistAction::List => allowlist::run(allowlist::Action::List),
        },
        Commands::Retro => retro::run(),
        Commands::Log {
            follow,
            json,
            tool,
            path,
        } => log::run(json, tool, path, follow),
        Commands::Enable => enable_disable::run(true),
        Commands::Disable => enable_disable::run(false),
        Commands::Validate => validate::run(),
        Commands::List => list::run(),
        Commands::Run { verbose, args } => run::run(args, verbose),
        Commands::Hook { format } => hook::run(format.as_str()),
        Commands::Mcp { name, upstream } => {
            // Strip a leading "--" separator if clap passed it through
            let upstream = if upstream.first().map(String::as_str) == Some("--") {
                upstream[1..].to_vec()
            } else {
                upstream
            };
            mcp::run(name, upstream)
        }
        Commands::Protect => protect::protect(),
        Commands::Unprotect => protect::unprotect(),
        Commands::Uninstall => uninstall::run(),
        Commands::Version => println!("{}", env!("CARGO_PKG_VERSION")),
    }
}
