use std::path::PathBuf;

use thiserror::Error;

pub mod editor;
mod versions;

pub use editor::EditorOpenError;

pub const DEFAULT_PR_DESCRIPTION_PROMPT: &str = r#"Update the PR that was just created with a better title and description.
The PR number is #{pr_number} and the URL is {pr_url}.

Analyze the changes in this branch and write:
1. A concise, descriptive title that summarizes the changes, postfixed with "(Vibe Kanban)"
2. A detailed description that explains:
   - What changes were made
   - Why they were made (based on the task context)
   - Any important implementation details
   - At the end, include a note: "This PR was written using [Vibe Kanban](https://vibekanban.com)"

Use the appropriate CLI tool to update the PR (gh pr edit for GitHub, az repos pr update for Azure DevOps)."#;

pub const DEFAULT_COMMIT_REMINDER_PROMPT: &str = "There are uncommitted changes. Please stage and commit them now with a descriptive commit message.";

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error("Validation error: {0}")]
    ValidationError(String),
}

pub type Config = versions::v9::Config;
pub type NotificationConfig = versions::v9::NotificationConfig;
pub type EditorConfig = versions::v9::EditorConfig;
pub type ThemeMode = versions::v9::ThemeMode;
pub type SoundFile = versions::v9::SoundFile;
pub type EditorType = versions::v9::EditorType;
pub type GitHubConfig = versions::v9::GitHubConfig;
pub type UiLanguage = versions::v9::UiLanguage;
pub type ShowcaseState = versions::v9::ShowcaseState;
pub type SendMessageShortcut = versions::v9::SendMessageShortcut;

/// Will always return config, trying old schemas or eventually returning default
pub async fn load_config_from_file(config_path: &PathBuf) -> Config {
    match std::fs::read_to_string(config_path) {
        Ok(raw_config) => {
            backup_v8_audit_artifact_if_needed(config_path, &raw_config);
            Config::from(raw_config)
        }
        Err(_) => {
            tracing::info!("No config file found, creating one");
            Config::default()
        }
    }
}

/// Preserve the v8 file as an audit artifact when about to migrate v8 → v9.
/// The v8 file is the per-engineer Config rollback path, independent of any
/// database rollback.
fn backup_v8_audit_artifact_if_needed(config_path: &PathBuf, raw_config: &str) {
    if !is_v8_raw_config(raw_config) {
        return;
    }

    let backup_path = config_path.with_extension("v8.bak");
    if backup_path.exists() {
        return;
    }

    match std::fs::write(&backup_path, raw_config) {
        Ok(()) => tracing::info!(
            "Preserved v8 config as audit artifact at {}",
            backup_path.display()
        ),
        Err(e) => tracing::warn!(
            "Failed to write v8 audit artifact to {}: {}",
            backup_path.display(),
            e
        ),
    }
}

fn is_v8_raw_config(raw_config: &str) -> bool {
    #[derive(serde::Deserialize)]
    struct VersionTag {
        config_version: Option<String>,
    }

    serde_json::from_str::<VersionTag>(raw_config)
        .ok()
        .and_then(|t| t.config_version)
        .as_deref()
        == Some("v8")
}

/// Saves the config to the given path
pub async fn save_config_to_file(
    config: &Config,
    config_path: &PathBuf,
) -> Result<(), ConfigError> {
    let raw_config = serde_json::to_string_pretty(config)?;
    std::fs::write(config_path, raw_config)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use tempfile::NamedTempFile;

    use super::*;

    fn write_temp_file(contents: &str) -> NamedTempFile {
        let mut file = NamedTempFile::new().expect("create temp file");
        file.write_all(contents.as_bytes()).expect("write contents");
        file.flush().expect("flush");
        file
    }

    #[tokio::test]
    async fn fresh_install_yields_v9_with_flags_off() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("config.json");
        let config = load_config_from_file(&path).await;
        assert_eq!(config.config_version, "v9");
        assert!(!config.analytics_enabled);
        assert!(!config.relay_enabled);
    }

    #[tokio::test]
    async fn upgrade_from_v8_writes_audit_artifact_and_forces_flags_off() {
        let v8_raw = r#"{
            "config_version": "v8",
            "theme": "SYSTEM",
            "executor_profile": { "executor": "CLAUDE_CODE", "variant": null },
            "disclaimer_acknowledged": true,
            "onboarding_acknowledged": true,
            "remote_onboarding_acknowledged": false,
            "notifications": {
                "sound_enabled": true,
                "push_enabled": true,
                "sound_file": "ABSTRACT_SOUND4"
            },
            "editor": { "editor_type": "VS_CODE", "custom_command": null },
            "github": {
                "pat": null,
                "oauth_token": null,
                "username": null,
                "primary_email": null,
                "default_pr_base": null
            },
            "analytics_enabled": true,
            "workspace_dir": null,
            "last_app_version": null,
            "show_release_notes": false,
            "language": "BROWSER",
            "git_branch_prefix": "vk",
            "showcases": { "seen_features": [] },
            "pr_auto_description_enabled": true,
            "pr_auto_description_prompt": null,
            "commit_reminder_enabled": true,
            "commit_reminder_prompt": null,
            "send_message_shortcut": "ModifierEnter",
            "relay_enabled": true,
            "host_nickname": null
        }"#;
        let file = write_temp_file(v8_raw);
        let path = file.path().to_path_buf();

        let config = load_config_from_file(&path).await;
        assert_eq!(config.config_version, "v9");
        assert!(!config.analytics_enabled);
        assert!(!config.relay_enabled);

        let backup_path = path.with_extension("v8.bak");
        let backup = std::fs::read_to_string(&backup_path).expect("audit backup written");
        assert!(backup.contains("\"config_version\": \"v8\""));
    }

    #[tokio::test]
    async fn loading_existing_v9_skips_backup() {
        let original = Config::default();
        let raw = serde_json::to_string_pretty(&original).expect("serialize v9");
        let file = write_temp_file(&raw);
        let path = file.path().to_path_buf();

        let config = load_config_from_file(&path).await;
        assert_eq!(config.config_version, "v9");

        let backup_path = path.with_extension("v8.bak");
        assert!(
            !backup_path.exists(),
            "no audit backup should be written for already-v9 files",
        );
    }
}
