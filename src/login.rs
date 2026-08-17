use std::process::Command;

use crate::{cli::LoginCommand, store};

struct LoginInput {
    username: String,
    password: String,
    otp: Option<String>,
}

pub async fn run(args: LoginCommand) -> eyre::Result<()> {
    let target = crate::target::resolve_target(
        args.target.host.as_deref(),
        args.target.repo.as_ref(),
        args.target.remote.as_deref(),
    )?;
    let base_url = target.base_url;

    if args.web {
        return run_web_login(&base_url, target.unix_socket.as_deref()).await;
    }

    let input = resolve_login_input(args).await?;

    // Validate login by actually logging in via UI.
    let session =
        crate::session::UiSession::new_with_socket(&base_url, None, target.unix_socket.as_deref())?;
    match session
        .login_with_creds_and_otp(&input.username, &input.password, input.otp.as_deref())
        .await?
    {
        crate::session::LoginResult::LoggedIn => {}
        crate::session::LoginResult::TwoFactorRequired => {
            let passcode = prompt_password("Forgejo 2FA passcode").await?;
            session.submit_two_factor_code(&passcode).await?;
        }
    }

    // Persist plaintext creds (required by design).
    crate::store::set_ui_creds(&base_url, &input.username, &input.password).await?;

    // Persist cookies.
    session.persist_cookie_jar_required().await?;

    let store_path = crate::store::ui_creds_store_paths()?.path;
    let host_label = url::Url::parse(&base_url)
        .ok()
        .and_then(|u| u.host_str().map(|s| s.to_string()))
        .unwrap_or_else(|| base_url.clone());

    println!("{}@{host_label}", input.username);
    println!("Saved UI creds to: {}", store_path.display());

    Ok(())
}

async fn run_web_login(base_url: &str, unix_socket: Option<&std::path::Path>) -> eyre::Result<()> {
    if unix_socket.is_some() || base_url.starts_with("http+unix://") {
        return Err(eyre::eyre!(
            "--web requires an http:// or https:// Forgejo URL; Unix-socket targets cannot be opened in a browser."
        ));
    }

    let parsed = url::Url::parse(base_url)
        .map_err(|err| eyre::eyre!("Invalid Forgejo base URL '{base_url}': {err}"))?;
    if !matches!(parsed.scheme(), "http" | "https") || parsed.host_str().is_none() {
        return Err(eyre::eyre!(
            "--web requires an http:// or https:// Forgejo URL with a host."
        ));
    }

    let login_url = format!("{base_url}/user/login");
    println!("Opening Forgejo login: {login_url}");
    match open_browser(&login_url) {
        Ok(status) if status.success() => {}
        Ok(status) => {
            eprintln!("Browser opener exited with {status}.");
            eprintln!("Open the URL above manually and continue here.");
        }
        Err(err) => {
            eprintln!("Could not open a browser automatically: {err}");
            eprintln!("Open the URL above manually and continue here.");
        }
    }

    eprintln!(
        "After the browser login completes, copy the request Cookie header for this Forgejo host and paste it below."
    );
    eprintln!(
        "The cookie is validated against /user/settings and stored without saving a password."
    );
    let cookie_header = prompt_password("Browser Cookie header").await?;
    let cookie_jar = parse_cookie_header(base_url, &cookie_header)?;

    let session =
        crate::session::UiSession::new_with_socket(base_url, Some(&cookie_jar), unix_socket)?;
    if !session.test_session().await? {
        return Err(eyre::eyre!(
            "The browser session was not accepted by '{}'. Copy the Cookie header after completing Forgejo/SSO login and run `fj-ex auth login --host {} --web` again.",
            base_url,
            base_url
        ));
    }

    let cookie_jar = session.cookie_jar()?;
    store::set_web_cookie_jar(base_url, cookie_jar).await?;

    let store_path = store::ui_creds_store_paths()?.path;
    let host_label = parsed.host_str().unwrap_or(base_url);
    println!("web@{host_label}");
    println!("Saved browser session to: {}", store_path.display());
    Ok(())
}

fn open_browser(url: &str) -> std::io::Result<std::process::ExitStatus> {
    #[cfg(target_os = "macos")]
    {
        return Command::new("open").arg(url).status();
    }

    #[cfg(target_os = "linux")]
    {
        return Command::new("xdg-open").arg(url).status();
    }

    #[cfg(target_os = "windows")]
    {
        return Command::new("cmd").args(["/C", "start", "", url]).status();
    }

    #[allow(unreachable_code)]
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "no supported browser opener for this platform",
    ))
}

fn parse_cookie_header(base_url: &str, header: &str) -> eyre::Result<store::CookieJar> {
    let url = url::Url::parse(base_url)
        .map_err(|err| eyre::eyre!("Invalid Forgejo base URL '{base_url}': {err}"))?;
    let domain = url
        .host_str()
        .ok_or_else(|| eyre::eyre!("Forgejo base URL has no host: {base_url}"))?;
    let mut value = header.trim();
    if let Some(cookie_header) = value
        .get(..7)
        .filter(|prefix| prefix.eq_ignore_ascii_case("cookie:"))
    {
        value = value[cookie_header.len()..].trim();
    }

    let mut cookies = Vec::new();
    for (index, part) in value.split(';').enumerate() {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }

        if is_cookie_attribute(part) {
            continue;
        }

        let (name, value) = part.split_once('=').ok_or_else(|| {
            eyre::eyre!(
                "Invalid browser Cookie header segment {}: expected name=value.",
                index + 1
            )
        })?;
        let name = name.trim();
        let value = value.trim();
        if is_cookie_attribute(name) {
            continue;
        }
        if name.is_empty() || value.is_empty() {
            return Err(eyre::eyre!(
                "Browser Cookie header contains an empty cookie name or value."
            ));
        }

        cookies.push(store::CookieRecord {
            name: name.to_string(),
            value: value.to_string(),
            domain: domain.to_string(),
            host_only: true,
            path: "/".to_string(),
            expires_utc: None,
            secure: url.scheme() == "https",
            http_only: false,
            same_site: None,
        });
    }

    if cookies.is_empty() {
        return Err(eyre::eyre!(
            "Browser Cookie header was empty. Copy the Cookie request header after login."
        ));
    }

    Ok(store::CookieJar {
        saved_utc: Some(
            time::OffsetDateTime::now_utc()
                .format(&time::format_description::well_known::Rfc3339)
                .unwrap_or_default(),
        ),
        cookies,
    })
}

fn is_cookie_attribute(name: &str) -> bool {
    matches!(
        name.to_ascii_lowercase().as_str(),
        "domain" | "expires" | "max-age" | "path" | "samesite" | "secure" | "httponly"
    )
}

async fn resolve_login_input(args: LoginCommand) -> eyre::Result<LoginInput> {
    let stdin_lines = if args.password_stdin || args.otp_stdin {
        read_lines_from_stdin().await?
    } else {
        Vec::new()
    };

    if let Some(userpass) = args.userpass.as_deref() {
        let idx = userpass
            .find(':')
            .ok_or_else(|| eyre::eyre!("Invalid --userpass format. Expected 'user:pass'."))?;
        if idx == 0 || idx >= userpass.len() - 1 {
            return Err(eyre::eyre!(
                "Invalid --userpass format. Expected 'user:pass'."
            ));
        }
        let username = userpass[..idx].to_string();
        let password = userpass[idx + 1..].to_string();
        let otp = resolve_otp(&args, &stdin_lines);
        return Ok(LoginInput {
            username,
            password,
            otp,
        });
    }

    let username = args
        .username
        .clone()
        .or_else(|| std::env::var("FJ_USER").ok())
        .unwrap_or_else(|| "".to_string());

    let username = if username.trim().is_empty() {
        prompt_line("Forgejo username").await?
    } else {
        username
    };

    let mut password = args
        .password
        .clone()
        .or_else(|| std::env::var("FJ_PASS").ok())
        .unwrap_or_else(|| "".to_string());

    if args.password_stdin && password.trim().is_empty() {
        password = stdin_lines.first().cloned().unwrap_or_default();
    }

    if password.trim().is_empty() {
        password = prompt_password("Forgejo password").await?;
    }

    if username.trim().is_empty() || password.trim().is_empty() {
        return Err(eyre::eyre!("Username/password must not be empty."));
    }

    let otp = resolve_otp(&args, &stdin_lines);

    Ok(LoginInput {
        username,
        password,
        otp,
    })
}

fn resolve_otp(args: &LoginCommand, stdin_lines: &[String]) -> Option<String> {
    let otp = args
        .otp
        .clone()
        .or_else(|| {
            if !args.otp_stdin {
                return None;
            }

            let index = if args.password_stdin { 1 } else { 0 };
            stdin_lines.get(index).cloned()
        })
        .or_else(|| std::env::var("FJ_OTP").ok());

    otp.map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

async fn prompt_line(label: &str) -> eyre::Result<String> {
    let label = label.to_string();
    tokio::task::spawn_blocking(move || -> eyre::Result<String> {
        use std::io::Write;
        print!("{label}: ");
        std::io::stdout().flush()?;
        let mut buf = String::new();
        std::io::stdin().read_line(&mut buf)?;
        Ok(buf.trim().to_string())
    })
    .await?
}

async fn prompt_password(label: &str) -> eyre::Result<String> {
    let prompt = format!("{label}: ");
    tokio::task::spawn_blocking(move || rpassword::prompt_password(prompt).map_err(Into::into))
        .await?
}

async fn read_lines_from_stdin() -> eyre::Result<Vec<String>> {
    use tokio::io::AsyncReadExt;
    let mut stdin = tokio::io::stdin();
    let mut buf = String::new();
    stdin.read_to_string(&mut buf).await?;
    Ok(buf.lines().map(|line| line.trim().to_string()).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn otp_stdin_uses_second_line_when_password_stdin_is_enabled() {
        let mut args = test_login_command();
        args.password_stdin = true;
        args.otp_stdin = true;

        let lines = vec!["secret".to_string(), "123456".to_string()];

        assert_eq!(resolve_otp(&args, &lines).as_deref(), Some("123456"));
    }

    #[test]
    fn explicit_otp_argument_wins_over_stdin() {
        let mut args = test_login_command();
        args.otp = Some("654321".to_string());
        args.otp_stdin = true;

        let lines = vec!["123456".to_string()];

        assert_eq!(resolve_otp(&args, &lines).as_deref(), Some("654321"));
    }

    #[test]
    fn browser_cookie_header_is_imported_for_the_target_host() {
        let jar = parse_cookie_header(
            "https://forge.example.com",
            "Cookie: i_like_forgejo=session%3D1; _csrf=csrf-value=with-equals",
        )
        .unwrap();

        assert_eq!(jar.cookies.len(), 2);
        assert_eq!(jar.cookies[0].name, "i_like_forgejo");
        assert_eq!(jar.cookies[0].value, "session%3D1");
        assert_eq!(jar.cookies[0].domain, "forge.example.com");
        assert!(jar.cookies[0].host_only);
        assert!(jar.cookies[0].secure);
        assert_eq!(jar.cookies[1].value, "csrf-value=with-equals");
    }

    #[test]
    fn empty_browser_cookie_header_is_rejected() {
        let error = parse_cookie_header("https://forge.example.com", "Cookie:").unwrap_err();
        assert!(error.to_string().contains("Cookie header was empty"));
    }

    #[test]
    fn malformed_browser_cookie_header_does_not_echo_cookie_value() {
        let error = parse_cookie_header(
            "https://forge.example.com",
            "session-secret-without-an-equals-sign",
        )
        .unwrap_err();
        let message = error.to_string();

        assert!(message.contains("segment 1"));
        assert!(!message.contains("session-secret-without-an-equals-sign"));
    }

    #[test]
    fn cookie_attributes_are_not_stored_as_request_cookies() {
        let jar = parse_cookie_header(
            "https://forge.example.com",
            "session=value; Path=/; Secure; HttpOnly; SameSite=Lax",
        )
        .unwrap();

        assert_eq!(jar.cookies.len(), 1);
        assert_eq!(jar.cookies[0].name, "session");
    }

    fn test_login_command() -> LoginCommand {
        LoginCommand {
            target: crate::cli::TargetArgs {
                host: None,
                repo: None,
                remote: None,
            },
            web: false,
            userpass: None,
            username: None,
            password: None,
            password_stdin: false,
            otp: None,
            otp_stdin: false,
        }
    }
}
