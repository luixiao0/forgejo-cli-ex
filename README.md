# forgejo-cli-ex

[![crates.io](https://img.shields.io/crates/v/forgejo-cli-ex)](https://crates.io/crates/forgejo-cli-ex)
[![license](https://img.shields.io/crates/l/forgejo-cli-ex)](LICENSE)
[![GitHub](https://img.shields.io/badge/GitHub-JKamsker%2Fforgejo--cli--ex-181717?logo=github)](https://github.com/JKamsker/forgejo-cli-ex)
[![Codeberg](https://img.shields.io/badge/Codeberg-JKamsker%2Fforgejo--cli--ex-2185D0?logo=codeberg)](https://codeberg.org/JKamsker/forgejo-cli-ex)
[![Blog](https://img.shields.io/badge/blog-how%20fj--ex%20was%20built-orange)](https://blog.kamsker.at/blog/how-fj-ex-was-built)

`fj-ex` extends the official Forgejo CLI (`fj`) with functionality that requires hitting **web UI endpoints** — full action logs, artifacts, cancel/rerun, workflow dispatch, and more.

## Install

```sh
cargo install forgejo-cli-ex
fj-ex --help
```

## Quickstart

```sh
# Login (interactive)
fj-ex auth login --host forge.example.com

# Login through Forgejo/SSO in a browser (no password is saved)
fj-ex auth login --host forge.example.com --web

# Login with 2FA in scripts (stdin lines are password, then passcode)
printf "my-password\n123456\n" | fj-ex auth login --host forge.example.com --username my-user --password-stdin --otp-stdin

# Mint a NuGet API key (requires `fj auth login` + `fj-ex auth login`)
fj-ex token mint nuget --host forge.example.com --owner my-org
fj-ex token list --host forge.example.com

# List recent runs
fj-ex actions runs --repo owner/name --latest

# Create and inspect pull requests
fj-ex pr create --title "fix: correct behavior" --body-file pr.md
fj-ex pr list --state open
fj-ex pr view 42
fj-ex pr merge 42

# Stream job logs to stdout
fj-ex actions logs job --repo owner/name --latest --job-index 0

# Cancel / rerun (preview first with --dry-run)
fj-ex actions cancel --repo owner/name --run-index 50 --dry-run
fj-ex actions rerun  --repo owner/name --latest --failed-only

# Runner registration token + queued jobs (requires `fj auth login`)
fj-ex actions runners token --repo owner/name
fj-ex actions runners jobs  --repo owner/name --waiting
```

> `--host` can be omitted — `fj-ex` infers it from the current repo's git remotes, or falls back to `$FJ_FALLBACK_HOST`.

## Commands

| Group | What it does |
|---|---|
| `auth` | Login, logout, status, list saved sessions |
| `token` | Mint and list personal access tokens |
| `actions runs` | List workflow runs (filter by status, workflow, latest) |
| `actions jobs` | List jobs for a run, optionally `--watch` |
| `actions logs` | Download logs for a job or full run |
| `actions artifacts` | List / download artifacts |
| `actions cancel` | Cancel a running workflow |
| `actions rerun` | Rerun a workflow (optionally `--failed-only`) |
| `actions trigger` | Dispatch a `workflow_dispatch` event |
| `actions runners` | Runner tokens + queued jobs (REST API; uses `fj` token store) |
| `pr` | Create, list, view, and merge pull requests through the Forgejo REST API |
| `smoke-test` | Non-destructive end-to-end validation |

Full command reference with all flags: [docs/commands.md](docs/commands.md)

## Target resolution

Most commands accept `--host`/`-H`, `--repo`/`-r`, or `--remote`/`-R`. If omitted, `fj-ex` infers host and repo from the current directory's git remotes.

## Credentials

Credentials and cookies are stored in plaintext at:

```text
%APPDATA%\Cyborus\forgejo-cli\data\ui-creds.json   # Windows
~/.local/share/Cyborus/forgejo-cli/data/ui-creds.json  # Linux
```

`fj-ex actions runners` uses the API token stored by the official `fj` CLI at:

```text
%APPDATA%\Cyborus\forgejo-cli\data\keys.json   # Windows
~/.local/share/Cyborus/forgejo-cli/data/keys.json  # Linux
```

`fj-ex token mint nuget` needs both:

```text
fj-ex auth login   # stored username/password for basic auth
fj auth login      # stored API token for Authorization: token ...
```

This is required for automatic re-login. Downloaded logs and artifacts may contain secrets — handle accordingly.

If your account uses two-factor authentication, `fj-ex auth login` prompts for the current passcode after the password step. For noninteractive use, pass `--otp`, `--otp-stdin`, or set `FJ_OTP`. The passcode is not stored; only the resulting UI cookies are stored so later commands can reuse the session until Forgejo expires it.

For Forgejo instances that use SSO or otherwise reject password login, use `fj-ex auth login --host forge.example.com --web`. `fj-ex` opens the Forgejo login page, then asks for the browser's request `Cookie` header after the web login completes (the browser Network panel can copy it as cURL; paste only the `Cookie: ...` header). It verifies that cookie against `/user/settings` and stores only the web session cookie, so no password is written for that login method. A later `auth status` or action command will report an expired web session; repeat the same `--web` login to refresh it.

## License

LGPL-3.0-or-later
