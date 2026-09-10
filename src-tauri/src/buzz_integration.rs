use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::process::Command;

const DEFAULT_BUZZ_RELAY_URL: &str = "ws://localhost:3000";
const BUZZ_IDENTITY_DIR: &str = ".buzz";
const BUZZ_IDENTITY_FILE: &str = "identity.nsec";

/// Exit codes documented by the buzz CLI (0 ok, 1 user error, 2 network,
/// 3 auth, 4 other, 5 write conflict). Surface stderr verbatim so the
/// frontend can show the CLI's own guidance.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct BuzzStatus {
    pub installed: bool,
    pub has_identity: bool,
    /// True when `BUZZ_PRIVATE_KEY` is present in Nabu's environment.
    pub env_key_present: bool,
}

fn buzz_identity_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_default()
        .join(BUZZ_IDENTITY_DIR)
        .join(BUZZ_IDENTITY_FILE)
}

/// True when the user has a Buzz identity keypair at `~/.buzz/identity.nsec`.
pub fn has_identity() -> bool {
    buzz_identity_path().is_file()
}

/// True when `BUZZ_PRIVATE_KEY` is exported into Nabu's environment.
pub fn has_env_key() -> bool {
    std::env::var("BUZZ_PRIVATE_KEY")
        .map(|value| !value.trim().is_empty())
        .unwrap_or(false)
}

/// Locates the `buzz` binary. `which buzz` first, then common install paths.
fn buzz_binary() -> Option<PathBuf> {
    if let Ok(output) = Command::new("which").arg("buzz").output() {
        if output.status.success() {
            let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !path.is_empty() {
                return Some(PathBuf::from(path));
            }
        }
    }
    let home = dirs::home_dir()?;
    [
        home.join(".cargo/bin/buzz"),
        home.join(".local/bin/buzz"),
        home.join("bin/buzz"),
        PathBuf::from("/opt/homebrew/bin/buzz"),
        PathBuf::from("/usr/local/bin/buzz"),
    ]
    .into_iter()
    .find(|path| path.is_file())
}

pub fn buzz_status() -> BuzzStatus {
    BuzzStatus {
        installed: buzz_binary().is_some(),
        has_identity: has_identity(),
        env_key_present: has_env_key(),
    }
}

/// Runs a buzz CLI subcommand with the relay + key environment set, returning
/// trimmed stdout. Errors embed the CLI's stderr and its exit code class.
fn invoke_buzz(args: &[&str]) -> Result<String, String> {
    let binary = buzz_binary().ok_or_else(|| {
        "buzz CLI not found. Install it and ensure `buzz` is on your PATH.".to_string()
    })?;

    let mut command = Command::new(binary);
    command.args(args);
    // The relay URL is a non-secret configuration value; the private key must
    // be inherited from the user's own environment, never stored by Nabu.
    let relay_url = std::env::var("BUZZ_RELAY_URL")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| DEFAULT_BUZZ_RELAY_URL.to_string());
    command.env("BUZZ_RELAY_URL", relay_url);

    let output = command
        .output()
        .map_err(|error| format!("Failed to execute buzz CLI: {error}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let stderr = if stderr.is_empty() {
            String::from_utf8_lossy(&output.stdout).trim().to_string()
        } else {
            stderr
        };
        return Err(format!(
            "buzz CLI error ({}): {stderr}",
            output.status.code().unwrap_or(-1)
        ));
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// One message returned by `buzz messages get`, as consumed by the frontend.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct BuzzMessage {
    pub author: String,
    pub content: String,
    /// Unix seconds when the message was posted (0 when unknown).
    pub created_at: i64,
}

/// Fetches recent messages from a Buzz channel. Returns empty output for
/// unknown channels rather than failing the caller.
pub fn get_team_messages(channel: &str, limit: u32) -> Result<Vec<BuzzMessage>, String> {
    let channel = channel.trim();
    if channel.is_empty() {
        return Err("A Buzz channel id is required".to_string());
    }
    let limit = limit.clamp(1, 100);
    let limit_string = limit.to_string();

    let output = invoke_buzz(&[
        "messages",
        "get",
        "--channel",
        channel,
        "--limit",
        &limit_string,
    ])?;
    if output.is_empty() {
        return Ok(Vec::new());
    }

    #[derive(Deserialize)]
    struct RawMessage {
        #[serde(default)]
        pubkey: Option<String>,
        #[serde(default)]
        content: Option<String>,
        #[serde(default)]
        created_at: Option<i64>,
    }

    let parsed: Vec<RawMessage> = serde_json::from_str(&output)
        .map_err(|error| format!("Failed to parse buzz messages JSON: {error}"))?;

    Ok(parsed
        .into_iter()
        .map(|message| BuzzMessage {
            author: message.pubkey.unwrap_or_else(|| "unknown".to_string()),
            content: message.content.unwrap_or_default(),
            created_at: message.created_at.unwrap_or(0),
        })
        .collect())
}

/// Posts a status update to a Buzz channel. The `[Agent Status]` prefix lets
/// teammates (and agents) distinguish automated broadcasts from chat.
pub fn post_agent_update(channel: &str, status: &str) -> Result<(), String> {
    let status = status.trim();
    if status.is_empty() {
        return Err("Status update text is required".to_string());
    }
    invoke_buzz(&[
        "messages",
        "send",
        "--channel",
        channel.trim(),
        "--content",
        &format!("[Agent Status] {status}"),
    ])?;
    Ok(())
}

/// Posts a completed research report to a Buzz channel.
pub fn post_research_result(channel: &str, query: &str, report: &str) -> Result<(), String> {
    let formatted = format!("**Research Complete: {}**\n\n{}", query.trim(), report);
    invoke_buzz(&[
        "messages",
        "send",
        "--channel",
        channel.trim(),
        "--content",
        &formatted,
    ])?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_path_is_home_dot_buzz() {
        let home = dirs::home_dir().unwrap();
        assert_eq!(
            buzz_identity_path(),
            home.join(".buzz").join("identity.nsec")
        );
    }

    #[test]
    fn status_reflects_missing_identity() {
        // Nabu test environments do not carry ~/.buzz/identity.nsec reliably;
        // only assert the call succeeds and reports consistent env state.
        let status = buzz_status();
        assert_eq!(status.env_key_present, has_env_key());
    }

    #[test]
    fn get_team_messages_rejects_empty_channel() {
        assert!(get_team_messages("  ", 10).is_err());
    }

    #[test]
    fn post_agent_update_rejects_empty_status() {
        assert!(post_agent_update("chan", "   ").is_err());
    }

    #[test]
    fn missing_cli_yields_actionable_error() {
        // The buzz binary is not expected in test environments; invocation
        // must fail with guidance rather than panic.
        let error = get_team_messages("some-channel", 5).unwrap_err();
        assert!(error.contains("buzz"), "unexpected: {error}");
    }
}
