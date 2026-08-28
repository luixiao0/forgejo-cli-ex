use std::num::{NonZeroU32, NonZeroU64};

use clap::{Args, Parser, Subcommand, ValueEnum};

#[derive(Parser, Debug)]
#[command(name = "fj-ex", version, about)]
pub struct App {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Authentication and credential management (UI cookies; optional password creds).
    #[command(subcommand)]
    Auth(AuthCommand),
    /// Token minting helpers.
    #[command(subcommand)]
    Token(TokenCommand),
    /// Forgejo Actions: workflows, runs, jobs, logs, artifacts, cancel/rerun.
    Actions(ActionsCommand),
    /// Pull requests: create, list, and view.
    Pr(PullRequestCommand),
    /// Smoke test for Actions access (useful for debugging auth/connectivity/log downloads).
    #[command(name = "smoke-test")]
    SmokeTest(SmokeTestCommand),
    /// Legacy alias for `fj-ex auth login`.
    #[command(hide = true)]
    Login(LoginCommand),
}

#[derive(Subcommand, Debug)]
pub enum AuthCommand {
    /// Log in with a password or a Forgejo/SSO browser session.
    Login(LoginCommand),
    /// Check login status against the host.
    Status(AuthStatusCommand),
    /// List all stored logins.
    List(AuthListCommand),
    /// Show stored login info for a host.
    Show(AuthShowCommand),
    /// Delete stored login info for a host.
    Logout(AuthLogoutCommand),
    /// Clear stored UI cookies for a host (keeps creds).
    #[command(name = "clear-cookies")]
    ClearCookies(AuthClearCookiesCommand),
    /// Create a package token suitable for Forgejo NuGet auth.
    #[command(name = "nuget-api-key", hide = true)]
    NugetApiKey(AuthNugetApiKeyCommand),
}

#[derive(Subcommand, Debug)]
pub enum TokenCommand {
    /// Mint a new token.
    #[command(subcommand)]
    Mint(TokenMintCommand),
    /// List tokens for the authenticated user.
    List(TokenListCommand),
}

#[derive(Subcommand, Debug)]
pub enum TokenMintCommand {
    /// Mint a package token suitable for Forgejo NuGet auth.
    Nuget(AuthNugetApiKeyCommand),
}

#[derive(Args, Debug, Clone)]
pub struct TokenListCommand {
    #[command(flatten)]
    pub target: TargetArgs,

    /// Page number (1-based).
    #[arg(
        long,
        default_value_t = 1,
        value_parser = clap::value_parser!(u32).range(1..)
    )]
    pub page: u32,

    /// Items per page.
    #[arg(
        long,
        default_value_t = 20,
        value_parser = clap::value_parser!(u32).range(1..)
    )]
    pub limit: u32,

    /// Always print the header row.
    #[arg(long)]
    pub header: bool,

    /// Never print the header row.
    #[arg(long, conflicts_with = "header")]
    pub no_header: bool,

    /// Print JSON output.
    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Debug, Clone)]
pub struct TargetArgs {
    /// Forgejo host or base URL (e.g. forge.example.com or https://forge.example.com)
    #[arg(long, short = 'H', global = true)]
    pub host: Option<String>,

    /// Repo to operate on (owner/name or host/owner/name)
    #[arg(long, short = 'r', global = true)]
    pub repo: Option<crate::target::RepoArg>,

    /// Local git remote used to infer host/repo (default: origin)
    #[arg(long, short = 'R', global = true)]
    pub remote: Option<String>,
}

#[derive(Args, Debug, Clone)]
pub struct LoginCommand {
    #[command(flatten)]
    pub target: TargetArgs,

    /// Open the Forgejo/SSO login page and import the resulting browser session.
    #[arg(
        long,
        conflicts_with_all = [
            "userpass",
            "username",
            "password",
            "password_stdin",
            "otp",
            "otp_stdin"
        ]
    )]
    pub web: bool,

    /// Credentials as user:pass (unsafe: visible in process list)
    #[arg(long, conflicts_with_all = ["username", "password", "password_stdin"])]
    pub userpass: Option<String>,

    /// Username to login with (falls back to FJ_USER)
    #[arg(long)]
    pub username: Option<String>,

    /// Password to login with (unsafe: visible in process list; prefer --password-stdin)
    #[arg(long, conflicts_with = "password_stdin")]
    pub password: Option<String>,

    /// Read password from stdin (first line; falls back to prompt if empty)
    #[arg(long, conflicts_with = "password")]
    pub password_stdin: bool,

    /// Two-factor passcode (unsafe: visible in process list; prefer --otp-stdin or prompt)
    #[arg(long, conflicts_with = "otp_stdin")]
    pub otp: Option<String>,

    /// Read two-factor passcode from stdin.
    ///
    /// When used with --password-stdin, the password is the first line and the passcode is the second.
    #[arg(long, conflicts_with = "otp")]
    pub otp_stdin: bool,
}

#[derive(Args, Debug, Clone)]
pub struct AuthStatusCommand {
    #[command(flatten)]
    pub target: TargetArgs,

    /// Don't re-login if the cookie session is invalid.
    #[arg(long)]
    pub no_relogin: bool,

    /// Print JSON output.
    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Debug, Clone)]
pub struct AuthListCommand {
    /// Print JSON output.
    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Debug, Clone)]
pub struct AuthShowCommand {
    #[command(flatten)]
    pub target: TargetArgs,

    /// Print JSON output.
    #[arg(long)]
    pub json: bool,

    /// UNSAFE: include the plaintext password in output.
    #[arg(long)]
    pub unsafe_show_password: bool,
}

#[derive(Args, Debug, Clone)]
pub struct AuthLogoutCommand {
    #[command(flatten)]
    pub target: TargetArgs,
}

#[derive(Args, Debug, Clone)]
pub struct AuthClearCookiesCommand {
    #[command(flatten)]
    pub target: TargetArgs,
}

#[derive(Args, Debug, Clone)]
pub struct AuthNugetApiKeyCommand {
    #[command(flatten)]
    pub target: TargetArgs,

    /// Owner/org segment for the NuGet registry URL (default: current username).
    #[arg(long)]
    pub owner: Option<String>,

    /// Access token name (default: fj-ex-nuget-<timestamp>).
    #[arg(long = "token-name")]
    pub token_name: Option<String>,

    /// Create a read-only package token instead of a publish-capable one.
    #[arg(long)]
    pub read_only: bool,

    /// Print JSON output.
    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Debug, Clone)]
pub struct ActionsCommand {
    #[command(flatten)]
    pub target: TargetArgs,

    #[command(subcommand)]
    pub command: ActionsSubcommand,
}

#[derive(Args, Debug, Clone)]
pub struct PullRequestCommand {
    #[command(flatten)]
    pub target: TargetArgs,

    #[command(subcommand)]
    pub command: PullRequestSubcommand,
}

#[derive(Subcommand, Debug, Clone)]
pub enum PullRequestSubcommand {
    /// Create a pull request.
    Create {
        /// Pull request title.
        #[arg(long)]
        title: String,

        /// Pull request body.
        #[arg(long, conflicts_with = "body_file")]
        body: Option<String>,

        /// Read the pull request body from a file, or `-` for stdin.
        #[arg(long, value_name = "PATH", conflicts_with = "body")]
        body_file: Option<std::path::PathBuf>,

        /// Source branch (default: current Git branch).
        #[arg(long)]
        head: Option<String>,

        /// Target branch (default: repository default branch).
        #[arg(long)]
        base: Option<String>,

        /// Create the pull request as a draft.
        #[arg(long)]
        draft: bool,

        /// Print JSON output.
        #[arg(long)]
        json: bool,
    },
    /// List pull requests.
    List {
        /// Filter by state.
        #[arg(long, value_enum, default_value_t = PullRequestState::Open)]
        state: PullRequestState,

        /// Filter by exact source branch name.
        #[arg(long)]
        head: Option<String>,

        /// Page number (1-based).
        #[arg(
            long,
            default_value_t = 1,
            value_parser = clap::value_parser!(u32).range(1..)
        )]
        page: u32,

        /// Items per page.
        #[arg(
            long,
            default_value_t = 20,
            value_parser = clap::value_parser!(u32).range(1..)
        )]
        limit: u32,

        /// Always print the header row.
        #[arg(long)]
        header: bool,

        /// Never print the header row.
        #[arg(long, conflicts_with = "header")]
        no_header: bool,

        /// Print JSON output.
        #[arg(long)]
        json: bool,
    },
    /// View one pull request by number.
    View {
        /// Pull request number.
        number: NonZeroU64,

        /// Print JSON output.
        #[arg(long)]
        json: bool,
    },
}

#[derive(ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum PullRequestState {
    Open,
    Closed,
    All,
}

impl PullRequestState {
    pub fn as_api_value(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Closed => "closed",
            Self::All => "all",
        }
    }
}

#[derive(Subcommand, Debug, Clone)]
pub enum ActionsSubcommand {
    /// List available workflows for the repo.
    Workflows {
        #[arg(
            long,
            default_value_t = 1,
            value_parser = clap::value_parser!(u32).range(1..),
            help = "Page number (1-based)."
        )]
        page: u32,
        #[arg(
            long,
            default_value_t = 20,
            value_parser = clap::value_parser!(u32).range(1..),
            help = "Items per page."
        )]
        limit: u32,
        #[arg(long, help = "Print JSON output.")]
        json: bool,
    },
    /// List workflow runs for the repo.
    Runs {
        #[arg(long, help = "Filter runs by workflow name.")]
        workflow: Option<String>,
        #[arg(
            long,
            help = "Filter runs by status (success, failure, running, waiting, canceled, skipped, blocked)."
        )]
        status: Option<String>,
        #[arg(
            long,
            help = "Show only the latest run (equivalent to --page 1 --limit 1)."
        )]
        latest: bool,
        #[arg(
            long,
            default_value_t = 1,
            value_parser = clap::value_parser!(u32).range(1..),
            help = "Page number (1-based)."
        )]
        page: u32,
        #[arg(
            long,
            default_value_t = 20,
            value_parser = clap::value_parser!(u32).range(1..),
            help = "Max runs per page."
        )]
        limit: u32,
        #[arg(long, help = "Include the run URL column in text output.")]
        show_url: bool,
        #[arg(long, help = "Always print the header row (even when piping).")]
        header: bool,
        #[arg(long, conflicts_with = "header", help = "Never print the header row.")]
        no_header: bool,
        #[arg(long, help = "Print JSON output.")]
        json: bool,
    },
    /// List jobs for a run.
    #[command(group(
        clap::ArgGroup::new("run_selector")
            .required(true)
            .args(["run_index", "latest"])
    ))]
    Jobs {
        #[arg(long, help = "Run index to inspect.")]
        run_index: Option<NonZeroU64>,
        #[arg(long, help = "Use the latest run.")]
        latest: bool,
        #[arg(long, help = "When using --latest, filter runs by workflow name.")]
        workflow: Option<String>,
        #[arg(long, help = "Watch until the run completes (polls the server).")]
        watch: bool,
        #[arg(
            long,
            default_value_t = 2,
            value_parser = clap::value_parser!(u64).range(1..),
            help = "Polling interval in seconds for --watch (minimum 1)."
        )]
        watch_interval: u64,
        #[arg(long, help = "Always print the header row (even when piping).")]
        header: bool,
        #[arg(long, conflicts_with = "header", help = "Never print the header row.")]
        no_header: bool,
        #[arg(long, help = "Print JSON output.")]
        json: bool,
    },
    /// Download and print logs.
    Logs {
        #[command(subcommand)]
        command: ActionsLogsSubcommand,
    },
    /// List or download run artifacts.
    Artifacts {
        #[command(subcommand)]
        command: ActionsArtifactsSubcommand,
    },
    /// Trigger a workflow via workflow_dispatch.
    #[command(alias = "workflow-dispatch")]
    Trigger {
        #[arg(long, help = "Workflow file name or id (e.g. ci.yml).")]
        workflow: String,
        #[arg(
            long = "ref",
            default_value = "main",
            help = "Git ref to run on (branch name like 'main' or full 'refs/heads/main')."
        )]
        git_ref: String,
        #[arg(long, value_name = "KEY=VALUE", help = "Workflow input (repeatable).")]
        input: Vec<String>,
        #[arg(
            long,
            help = "Print the request that would be made, but do not perform it."
        )]
        dry_run: bool,
        #[arg(long, help = "Print JSON output.")]
        json: bool,
    },
    /// Runner registration tokens and queued jobs (REST API; uses `fj`'s stored API token).
    Runners {
        #[command(subcommand)]
        command: ActionsRunnersSubcommand,
    },
    /// Smoke test for Actions access (useful for debugging auth/connectivity/log downloads).
    #[command(name = "smoke-test")]
    SmokeTest(ActionsSmokeTestCommand),
    /// Cancel a run.
    #[command(group(
        clap::ArgGroup::new("run_selector")
            .required(true)
            .args(["run_index", "latest"])
    ))]
    Cancel {
        #[arg(long, help = "Run index to cancel.")]
        run_index: Option<NonZeroU64>,
        #[arg(long, help = "Use the latest run.")]
        latest: bool,
        #[arg(long, help = "When using --latest, filter runs by workflow name.")]
        workflow: Option<String>,
        #[arg(
            long,
            help = "Print the request that would be made, but do not perform it."
        )]
        dry_run: bool,
        #[arg(long, help = "Print JSON output.")]
        json: bool,
    },
    /// Rerun a run (or a single job within a run).
    #[command(group(
        clap::ArgGroup::new("run_selector")
            .required(true)
            .args(["run_index", "latest"])
    ))]
    Rerun {
        #[arg(long, help = "Run index to rerun.")]
        run_index: Option<NonZeroU64>,
        #[arg(long, help = "Use the latest run.")]
        latest: bool,
        #[arg(long, help = "When using --latest, filter runs by workflow name.")]
        workflow: Option<String>,
        #[arg(long, help = "Rerun a specific job index within the run (0-based).")]
        job_index: Option<u32>,
        #[arg(long, conflicts_with = "job_index", help = "Rerun failed jobs only.")]
        failed_only: bool,
        #[arg(
            long,
            help = "Print the request that would be made, but do not perform it."
        )]
        dry_run: bool,
        #[arg(long, help = "Print JSON output.")]
        json: bool,
    },
}

#[derive(Subcommand, Debug, Clone)]
pub enum ActionsLogsSubcommand {
    /// Download logs for all jobs in a run.
    ///
    /// Note: Job separators (`== job N ==`) are written to stderr; log content goes to stdout.
    #[command(group(
        clap::ArgGroup::new("run_selector")
            .required(true)
            .args(["run_index", "latest"])
    ))]
    Run {
        #[arg(long, help = "Run index to download logs for.")]
        run_index: Option<NonZeroU64>,
        #[arg(long, help = "Use the latest run.")]
        latest: bool,
        #[arg(long, help = "When using --latest, filter runs by workflow name.")]
        workflow: Option<String>,
        #[arg(
            long,
            help = "Write per-job log files to this directory (otherwise stdout)."
        )]
        out_dir: Option<std::path::PathBuf>,
        #[arg(long, help = "Max jobs to download (default: unlimited).")]
        max_jobs: Option<u32>,
    },
    /// Download logs for a single job in a run.
    #[command(group(
        clap::ArgGroup::new("run_selector")
            .required(true)
            .args(["run_index", "latest"])
    ))]
    Job {
        #[arg(long, help = "Run index to download logs for.")]
        run_index: Option<NonZeroU64>,
        #[arg(long, help = "Use the latest run.")]
        latest: bool,
        #[arg(long, help = "When using --latest, filter runs by workflow name.")]
        workflow: Option<String>,
        #[arg(long, help = "Job index within the run (0-based).")]
        job_index: u32,
        #[arg(long, help = "Attempt number (defaults to latest attempt).")]
        attempt: Option<NonZeroU32>,
        #[arg(long, help = "Write logs to this file (otherwise stdout).")]
        out_file: Option<std::path::PathBuf>,
    },
}

#[derive(Subcommand, Debug, Clone)]
pub enum ActionsArtifactsSubcommand {
    /// List artifacts for a run.
    #[command(group(
        clap::ArgGroup::new("run_selector")
            .required(true)
            .args(["run_index", "latest"])
    ))]
    List {
        #[arg(long, help = "Run index to list artifacts for.")]
        run_index: Option<NonZeroU64>,
        #[arg(long, help = "Use the latest run.")]
        latest: bool,
        #[arg(long, help = "When using --latest, filter runs by workflow name.")]
        workflow: Option<String>,
        #[arg(long, help = "Always print the header row (even when piping).")]
        header: bool,
        #[arg(long, conflicts_with = "header", help = "Never print the header row.")]
        no_header: bool,
        #[arg(long, help = "Print JSON output.")]
        json: bool,
    },
    /// Download a single artifact from a run.
    #[command(group(
        clap::ArgGroup::new("run_selector")
            .required(true)
            .args(["run_index", "latest"])
    ))]
    Get {
        #[arg(long, help = "Run index to download the artifact from.")]
        run_index: Option<NonZeroU64>,
        #[arg(long, help = "Use the latest run.")]
        latest: bool,
        #[arg(long, help = "When using --latest, filter runs by workflow name.")]
        workflow: Option<String>,
        #[arg(long, help = "Artifact name or id.")]
        artifact: String,
        #[arg(long, help = "Output file path.")]
        out_file: std::path::PathBuf,
    },
}

#[derive(Args, Debug, Clone)]
pub struct ActionsSmokeTestCommand {
    #[arg(
        long,
        default_value_t = 1_048_576,
        help = "Max bytes to download per job log (default: 1 MiB)."
    )]
    pub log_download_max_bytes: u64,

    /// Base directory for smoke test log downloads (a run-specific folder is created inside).
    /// Default: system temp dir (e.g. $TMPDIR/fj-ex/forgejo-logs).
    #[arg(long)]
    pub out_dir: Option<std::path::PathBuf>,
}

#[derive(Args, Debug, Clone)]
pub struct SmokeTestCommand {
    #[command(flatten)]
    pub target: TargetArgs,

    #[command(flatten)]
    pub opts: ActionsSmokeTestCommand,
}

#[derive(ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunnerScope {
    Global,
    Org,
    Repo,
    User,
}

#[derive(Subcommand, Debug, Clone)]
pub enum ActionsRunnersSubcommand {
    /// Print a runner registration token.
    Token {
        /// Runner scope (default: org if --org is set, else repo if resolved, else global).
        #[arg(long, value_enum)]
        scope: Option<RunnerScope>,

        /// Org name (required for --scope org).
        #[arg(long)]
        org: Option<String>,

        /// Print JSON output.
        #[arg(long)]
        json: bool,
    },
    /// List runner jobs (useful for diagnosing "waiting" and label mismatches).
    Jobs {
        /// Runner scope (default: org if --org is set, else repo if resolved, else global).
        #[arg(long, value_enum)]
        scope: Option<RunnerScope>,

        /// Org name (required for --scope org).
        #[arg(long)]
        org: Option<String>,

        /// Filter by runner label (repeatable; sent as labels=a,b).
        #[arg(long, value_name = "LABEL")]
        label: Vec<String>,

        /// Show only jobs with status == "waiting" (case-insensitive).
        #[arg(long)]
        waiting: bool,

        /// Always print the header row (even when piping).
        #[arg(long)]
        header: bool,

        /// Never print the header row.
        #[arg(long, conflicts_with = "header")]
        no_header: bool,

        /// Print JSON output.
        #[arg(long)]
        json: bool,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_pull_request_create_command() {
        let app = App::try_parse_from([
            "fj-ex", "pr", "create", "--title", "fix chat", "--head", "fix/chat", "--base",
            "master",
        ])
        .unwrap();

        match app.command {
            Command::Pr(PullRequestCommand {
                command:
                    PullRequestSubcommand::Create {
                        title, head, base, ..
                    },
                ..
            }) => {
                assert_eq!(title, "fix chat");
                assert_eq!(head.as_deref(), Some("fix/chat"));
                assert_eq!(base.as_deref(), Some("master"));
            }
            command => panic!("unexpected command: {command:?}"),
        }
    }
}
