use anyhow::Error;
use executors::{executors::BaseCodingAgent, profile::ExecutorProfileId};
use serde::{Deserialize, Serialize};
use ts_rs::TS;
pub use v8::{
    EditorConfig, EditorType, GitHubConfig, NotificationConfig, SendMessageShortcut, ShowcaseState,
    SoundFile, ThemeMode, UiLanguage,
};

use crate::services::config::versions::v8;

fn default_git_branch_prefix() -> String {
    "vk".to_string()
}

fn default_pr_auto_description_enabled() -> bool {
    true
}

fn default_commit_reminder_enabled() -> bool {
    true
}

fn default_analytics_enabled() -> bool {
    false
}

fn default_relay_enabled() -> bool {
    false
}

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
pub struct Config {
    pub config_version: String,
    pub theme: ThemeMode,
    pub executor_profile: ExecutorProfileId,
    pub disclaimer_acknowledged: bool,
    pub onboarding_acknowledged: bool,
    #[serde(default)]
    pub remote_onboarding_acknowledged: bool,
    pub notifications: NotificationConfig,
    pub editor: EditorConfig,
    pub github: GitHubConfig,
    #[serde(default = "default_analytics_enabled")]
    pub analytics_enabled: bool,
    pub workspace_dir: Option<String>,
    pub last_app_version: Option<String>,
    pub show_release_notes: bool,
    #[serde(default)]
    pub language: UiLanguage,
    #[serde(default = "default_git_branch_prefix")]
    pub git_branch_prefix: String,
    #[serde(default)]
    pub showcases: ShowcaseState,
    #[serde(default = "default_pr_auto_description_enabled")]
    pub pr_auto_description_enabled: bool,
    #[serde(default)]
    pub pr_auto_description_prompt: Option<String>,
    #[serde(default = "default_commit_reminder_enabled")]
    pub commit_reminder_enabled: bool,
    #[serde(default)]
    pub commit_reminder_prompt: Option<String>,
    #[serde(default)]
    pub send_message_shortcut: SendMessageShortcut,
    #[serde(default = "default_relay_enabled")]
    pub relay_enabled: bool,
    #[serde(default)]
    pub host_nickname: Option<String>,
}

impl Config {
    fn from_v8_config(old_config: v8::Config) -> Self {
        Self {
            config_version: "v9".to_string(),
            theme: old_config.theme,
            executor_profile: old_config.executor_profile,
            disclaimer_acknowledged: old_config.disclaimer_acknowledged,
            onboarding_acknowledged: old_config.onboarding_acknowledged,
            remote_onboarding_acknowledged: old_config.remote_onboarding_acknowledged,
            notifications: old_config.notifications,
            editor: old_config.editor,
            github: old_config.github,
            // Privacy default-off contract: force off regardless of v8 value.
            analytics_enabled: false,
            workspace_dir: old_config.workspace_dir,
            last_app_version: old_config.last_app_version,
            show_release_notes: old_config.show_release_notes,
            language: old_config.language,
            git_branch_prefix: old_config.git_branch_prefix,
            showcases: old_config.showcases,
            pr_auto_description_enabled: old_config.pr_auto_description_enabled,
            pr_auto_description_prompt: old_config.pr_auto_description_prompt,
            commit_reminder_enabled: old_config.commit_reminder_enabled,
            commit_reminder_prompt: old_config.commit_reminder_prompt,
            send_message_shortcut: old_config.send_message_shortcut,
            // Privacy default-off contract: force off regardless of v8 value.
            relay_enabled: false,
            host_nickname: old_config.host_nickname,
        }
    }

    pub fn from_previous_version(raw_config: &str) -> Result<Self, Error> {
        let old_config = v8::Config::from(raw_config.to_string());
        Ok(Self::from_v8_config(old_config))
    }
}

impl From<String> for Config {
    fn from(raw_config: String) -> Self {
        if let Ok(config) = serde_json::from_str::<Config>(&raw_config)
            && config.config_version == "v9"
        {
            return config;
        }

        match Self::from_previous_version(&raw_config) {
            Ok(config) => {
                tracing::info!("Config upgraded to v9");
                config
            }
            Err(e) => {
                tracing::warn!("Config migration failed: {}, using default", e);
                Self::default()
            }
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            config_version: "v9".to_string(),
            theme: ThemeMode::System,
            executor_profile: ExecutorProfileId::new(BaseCodingAgent::ClaudeCode),
            disclaimer_acknowledged: false,
            onboarding_acknowledged: false,
            remote_onboarding_acknowledged: false,
            notifications: NotificationConfig::default(),
            editor: EditorConfig::default(),
            github: GitHubConfig::default(),
            analytics_enabled: false,
            workspace_dir: None,
            last_app_version: None,
            show_release_notes: false,
            language: UiLanguage::default(),
            git_branch_prefix: default_git_branch_prefix(),
            showcases: ShowcaseState::default(),
            pr_auto_description_enabled: true,
            pr_auto_description_prompt: None,
            commit_reminder_enabled: true,
            commit_reminder_prompt: None,
            send_message_shortcut: SendMessageShortcut::default(),
            relay_enabled: false,
            host_nickname: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v8_raw(analytics: bool, relay: bool) -> String {
        format!(
            r#"{{
                "config_version": "v8",
                "theme": "SYSTEM",
                "executor_profile": {{ "executor": "CLAUDE_CODE", "variant": null }},
                "disclaimer_acknowledged": true,
                "onboarding_acknowledged": true,
                "remote_onboarding_acknowledged": false,
                "notifications": {{
                    "sound_enabled": true,
                    "push_enabled": true,
                    "sound_file": "ABSTRACT_SOUND4"
                }},
                "editor": {{ "editor_type": "VS_CODE", "custom_command": null }},
                "github": {{
                    "pat": null,
                    "oauth_token": null,
                    "username": null,
                    "primary_email": null,
                    "default_pr_base": null
                }},
                "analytics_enabled": {analytics},
                "workspace_dir": null,
                "last_app_version": null,
                "show_release_notes": false,
                "language": "BROWSER",
                "git_branch_prefix": "vk",
                "showcases": {{ "seen_features": [] }},
                "pr_auto_description_enabled": true,
                "pr_auto_description_prompt": null,
                "commit_reminder_enabled": true,
                "commit_reminder_prompt": null,
                "send_message_shortcut": "ModifierEnter",
                "relay_enabled": {relay},
                "host_nickname": "my-host"
            }}"#,
        )
    }

    #[test]
    fn default_v9_has_both_flags_off() {
        let config = Config::default();
        assert_eq!(config.config_version, "v9");
        assert!(!config.analytics_enabled);
        assert!(!config.relay_enabled);
    }

    #[test]
    fn migration_from_v8_forces_flags_off_when_v8_had_them_on() {
        let raw = v8_raw(true, true);
        let config = Config::from(raw);
        assert_eq!(config.config_version, "v9");
        assert!(!config.analytics_enabled);
        assert!(!config.relay_enabled);
    }

    #[test]
    fn migration_from_v8_keeps_flags_off_when_v8_had_them_off() {
        let raw = v8_raw(false, false);
        let config = Config::from(raw);
        assert_eq!(config.config_version, "v9");
        assert!(!config.analytics_enabled);
        assert!(!config.relay_enabled);
    }

    #[test]
    fn migration_from_v8_preserves_retained_fields() {
        let raw = v8_raw(true, true);
        // Sanity-check the test fixture deserializes cleanly as v8 before
        // exercising the migration. If this fails, the fixture is wrong.
        let _v8_parsed: v8::Config = serde_json::from_str(&raw)
            .map_err(|e| format!("v8 fixture parse failed: {e}"))
            .unwrap();
        let config = Config::from(raw);
        assert!(config.disclaimer_acknowledged);
        assert!(config.onboarding_acknowledged);
        assert_eq!(config.git_branch_prefix, "vk");
        assert_eq!(config.host_nickname.as_deref(), Some("my-host"));
    }

    #[test]
    fn loading_existing_v9_is_a_noop() {
        let original = Config::default();
        let raw = serde_json::to_string(&original).expect("serialize v9");
        let parsed = Config::from(raw);
        assert_eq!(parsed.config_version, "v9");
        assert!(!parsed.analytics_enabled);
        assert!(!parsed.relay_enabled);
    }

    #[test]
    fn loading_existing_v9_with_flags_on_round_trips_unchanged() {
        // Already-v9 files are skipped by the forward chain, so explicit
        // values on disk are honored even if they conflict with the schema
        // default. Operators can intentionally re-enable flags after upgrade.
        let mut config = Config::default();
        config.analytics_enabled = true;
        config.relay_enabled = true;
        let raw = serde_json::to_string(&config).expect("serialize v9");
        let parsed = Config::from(raw);
        assert_eq!(parsed.config_version, "v9");
        assert!(parsed.analytics_enabled);
        assert!(parsed.relay_enabled);
    }
}
