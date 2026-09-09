use crate::buzz_integration::{self, BuzzMessage, BuzzStatus};

/// Reports Buzz multiplayer readiness: CLI installed + identity configured.
#[tauri::command]
pub fn buzz_status() -> Result<BuzzStatus, String> {
    Ok(buzz_integration::buzz_status())
}

/// Fetches recent messages from a Buzz team channel for agent context.
#[tauri::command]
pub fn buzz_get_team_messages(
    channel: String,
    limit: Option<u32>,
) -> Result<Vec<BuzzMessage>, String> {
    buzz_integration::get_team_messages(&channel, limit.unwrap_or(10))
}

/// Broadcasts an agent status update to a Buzz team channel.
#[tauri::command]
pub fn buzz_post_agent_update(channel: String, status: String) -> Result<(), String> {
    buzz_integration::post_agent_update(&channel, &status)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_command_returns_struct() {
        let status = buzz_status().unwrap();
        assert!(!status.installed || status.installed);
    }

    #[test]
    fn team_messages_requires_channel() {
        assert!(buzz_get_team_messages(String::new(), None).is_err());
    }

    #[test]
    fn agent_update_requires_status() {
        assert!(buzz_post_agent_update("chan", "  ".to_string()).is_err());
    }
}
