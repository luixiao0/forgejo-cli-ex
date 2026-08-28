use std::io::Read;
use std::path::Path;

use eyre::{eyre, Context};
use serde::{Deserialize, Serialize};

use crate::cli::{PullRequestCommand, PullRequestSubcommand};

#[derive(Debug, Deserialize, Serialize)]
struct RepositoryInfo {
    default_branch: String,
}

#[derive(Debug, Deserialize, Serialize)]
struct PullRef {
    #[serde(rename = "ref")]
    branch: String,
    #[serde(default)]
    sha: String,
}

#[derive(Debug, Deserialize, Serialize)]
struct PullRequest {
    number: u64,
    title: String,
    state: String,
    html_url: String,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    mergeable: Option<bool>,
    head: PullRef,
    base: PullRef,
}

#[derive(Debug, Serialize)]
struct CreatePullRequest<'a> {
    title: &'a str,
    body: &'a str,
    head: &'a str,
    base: &'a str,
    draft: bool,
}

pub async fn run(args: PullRequestCommand) -> eyre::Result<()> {
    let target = crate::target::resolve_target(
        args.target.host.as_deref(),
        args.target.repo.as_ref(),
        args.target.remote.as_deref(),
    )?;
    let repo = target.repo.clone().ok_or_else(|| {
        eyre!(
            "Repo could not be resolved. Pass --repo owner/name or run inside a git repo with a Forgejo remote."
        )
    })?;
    let client = api_client(&target).await?;
    let pulls_url = client.api_v1_url(&format!("/repos/{repo}/pulls"));

    match args.command {
        PullRequestSubcommand::Create {
            title,
            body,
            body_file,
            head,
            base,
            draft,
            json,
        } => {
            let body = read_body(body, body_file.as_deref())?;
            let head = head.map(Ok).unwrap_or_else(current_branch)?;
            let base = match base {
                Some(base) => base,
                None => {
                    let repo_url = client.api_v1_url(&format!("/repos/{repo}"));
                    let info: RepositoryInfo = client.get_json(&repo_url).await?;
                    if info.default_branch.trim().is_empty() {
                        return Err(eyre!(
                            "Forgejo returned an empty default branch; pass --base explicitly."
                        ));
                    }
                    info.default_branch
                }
            };
            let request = CreatePullRequest {
                title: &title,
                body: &body,
                head: &head,
                base: &base,
                draft,
            };
            let pull: PullRequest = client.post_json(&pulls_url, &request).await?;
            print_pull(&target.base_url, &repo, &pull, json)?;
        }
        PullRequestSubcommand::List {
            state,
            head,
            page,
            limit,
            header,
            no_header,
            json,
        } => {
            let url = format!(
                "{pulls_url}?state={}&page={page}&limit={limit}",
                state.as_api_value()
            );
            let mut pulls: Vec<PullRequest> = client.get_json(&url).await?;
            if let Some(head) = head.as_deref() {
                pulls.retain(|pull| pull.head.branch == head);
            }
            if json {
                let payload = serde_json::json!({
                    "baseUrl": target.base_url,
                    "repo": repo,
                    "state": state.as_api_value(),
                    "head": head,
                    "page": page,
                    "limit": limit,
                    "pulls": pulls,
                });
                println!("{}", serde_json::to_string_pretty(&payload)?);
                return Ok(());
            }

            let show_header = crate::output::should_print_header(header, no_header);
            let rows = pulls
                .into_iter()
                .map(|pull| {
                    vec![
                        pull.number.to_string(),
                        pull.state,
                        pull.title,
                        pull.head.branch,
                        pull.base.branch,
                        pull.html_url,
                    ]
                })
                .collect::<Vec<_>>();
            crate::output::print_table(
                &["Number", "State", "Title", "Head", "Base", "URL"],
                &rows,
                show_header,
            );
        }
        PullRequestSubcommand::View { number, json } => {
            let url = format!("{pulls_url}/{number}");
            let pull: PullRequest = client.get_json(&url).await?;
            print_pull(&target.base_url, &repo, &pull, json)?;
        }
    }

    Ok(())
}

async fn api_client(target: &crate::target::ResolvedTarget) -> eyre::Result<crate::api::ApiClient> {
    if let Some(token) = crate::store::get_fj_api_token_for_base_url(&target.base_url)? {
        return crate::api::ApiClient::new_with_socket(
            &target.base_url,
            &token,
            target.unix_socket.as_deref(),
        );
    }

    let creds = crate::store::get_ui_creds(&target.base_url)
        .await?
        .ok_or_else(|| {
            eyre!(
                "No Forgejo API authentication found for '{}'. Run `fj auth login` or `fj-ex auth login` first.",
                target.base_url
            )
        })?;
    crate::api::ApiClient::new_basic_with_socket(
        &target.base_url,
        &creds.username,
        &creds.password,
        target.unix_socket.as_deref(),
    )
}

fn read_body(body: Option<String>, body_file: Option<&Path>) -> eyre::Result<String> {
    match (body, body_file) {
        (Some(body), None) => Ok(body),
        (None, Some(path)) if path == Path::new("-") => {
            let mut body = String::new();
            std::io::stdin()
                .read_to_string(&mut body)
                .wrap_err("failed to read pull request body from stdin")?;
            Ok(body)
        }
        (None, Some(path)) => std::fs::read_to_string(path).wrap_err_with(|| {
            format!("failed to read pull request body from '{}'", path.display())
        }),
        (None, None) => Ok(String::new()),
        (Some(_), Some(_)) => unreachable!("clap prevents conflicting body arguments"),
    }
}

fn current_branch() -> eyre::Result<String> {
    let repo = git2::Repository::discover(".")
        .wrap_err("unable to find the current Git repository; pass --head explicitly")?;
    let head = repo
        .head()
        .wrap_err("unable to read the current Git branch; pass --head explicitly")?;
    if !head.is_branch() {
        return Err(eyre!(
            "Current Git checkout is detached; pass --head explicitly."
        ));
    }
    head.shorthand()
        .filter(|branch| !branch.trim().is_empty())
        .map(str::to_string)
        .ok_or_else(|| eyre!("Current Git branch has no name; pass --head explicitly."))
}

fn print_pull(base_url: &str, repo: &str, pull: &PullRequest, json: bool) -> eyre::Result<()> {
    if json {
        let payload = serde_json::json!({
            "baseUrl": base_url,
            "repo": repo,
            "pull": pull,
        });
        println!("{}", serde_json::to_string_pretty(&payload)?);
    } else {
        println!("{}", pull.html_url);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::PullRequestState;

    #[test]
    fn reads_inline_and_empty_bodies() {
        assert_eq!(read_body(Some("body".into()), None).unwrap(), "body");
        assert_eq!(read_body(None, None).unwrap(), "");
    }

    #[test]
    fn pull_request_deserializes_forgejo_shape() {
        let pull: PullRequest = serde_json::from_str(
            r#"{
                "number": 596,
                "title": "fix chat",
                "state": "open",
                "html_url": "https://forge.example/owner/repo/pulls/596",
                "draft": false,
                "mergeable": true,
                "head": {"ref": "fix/chat", "sha": "abc"},
                "base": {"ref": "master", "sha": "def"}
            }"#,
        )
        .unwrap();
        assert_eq!(pull.number, 596);
        assert_eq!(pull.head.branch, "fix/chat");
        assert_eq!(pull.base.branch, "master");
    }

    #[test]
    fn pull_request_state_uses_forgejo_values() {
        assert_eq!(PullRequestState::Open.as_api_value(), "open");
        assert_eq!(PullRequestState::Closed.as_api_value(), "closed");
        assert_eq!(PullRequestState::All.as_api_value(), "all");
    }
}
