use crate::types::{RulesPatch, Settings, SettingsPatch};
use anyhow::Context;
use serde_json::Value;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct SettingsStore {
    path: PathBuf,
}

impl SettingsStore {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn load(&self) -> anyhow::Result<Settings> {
        if !self.path.exists() {
            let defaults = Settings::default();
            self.save(&defaults)?;
            return Ok(defaults);
        }

        let raw = fs::read_to_string(&self.path).context("failed to read settings")?;
        match serde_json::from_str::<Settings>(&raw) {
            Ok(settings) => {
                let json = serde_json::from_str::<Value>(&raw).ok();
                let migrated = migrate_settings(settings, json.as_ref());
                self.save(&migrated)?;
                Ok(migrated)
            }
            Err(_) => {
                let defaults = Settings::default();
                self.save(&defaults)?;
                Ok(defaults)
            }
        }
    }

    pub fn save(&self, settings: &Settings) -> anyhow::Result<()> {
        let json =
            serde_json::to_string_pretty(settings).context("failed to serialize settings")?;
        fs::write(&self.path, json).context("failed to write settings")?;
        Ok(())
    }

    pub fn apply_patch(
        &self,
        current: &Settings,
        patch: SettingsPatch,
    ) -> anyhow::Result<Settings> {
        let mut next = current.clone();

        if let Some(shortcut) = patch.shortcut {
            next.shortcut = shortcut;
        }
        if let Some(mode) = patch.trigger_mode {
            next.trigger_mode = mode;
        }
        if let Some(mic_id) = patch.mic_device_id {
            next.mic_device_id = mic_id;
        }
        if let Some(model_id) = patch.active_model_id {
            next.active_model_id = model_id;
        }
        if let Some(mode) = patch.insertion_mode {
            next.insertion_mode = mode;
        }
        if let Some(retention) = patch.transcript_retention {
            next.transcript_retention = retention;
        }
        if let Some(timeout) = patch.silence_timeout_ms {
            next.silence_timeout_ms = timeout.clamp(300, 5000);
        }
        if let Some(value) = patch.overlay_enabled {
            next.overlay_enabled = value;
        }
        if let Some(value) = patch.hide_dock_icon {
            next.hide_dock_icon = value;
        }
        if let Some(value) = patch.launch_at_login_placeholder {
            next.launch_at_login_placeholder = value;
        }
        if let Some(value) = patch.onboarding_completed {
            next.onboarding_completed = value;
        }
        if let Some(rules_patch) = patch.rules {
            apply_rules_patch(&mut next, rules_patch);
        }

        self.save(&next)?;
        Ok(next)
    }
}

fn migrate_settings(mut settings: Settings, raw_json: Option<&Value>) -> Settings {
    if settings.shortcut == "Cmd+Shift+Space" {
        settings.shortcut = Settings::default().shortcut;
    }

    let onboarding_present = raw_json
        .and_then(|value| value.as_object())
        .map(|map| map.contains_key("onboardingCompleted"))
        .unwrap_or(true);
    if !onboarding_present {
        settings.onboarding_completed = true;
    }

    let hide_dock_icon_present = raw_json
        .and_then(|value| value.as_object())
        .map(|map| map.contains_key("hideDockIcon"))
        .unwrap_or(true);
    if !hide_dock_icon_present {
        settings.hide_dock_icon = Settings::default().hide_dock_icon;
    }

    settings
}

fn apply_rules_patch(settings: &mut Settings, patch: RulesPatch) {
    if let Some(value) = patch.remove_filler_words {
        settings.rules.remove_filler_words = value;
    }
    if let Some(value) = patch.capitalize_sentence_starts {
        settings.rules.capitalize_sentence_starts = value;
    }
    if let Some(value) = patch.convert_pauses_to_punctuation {
        settings.rules.convert_pauses_to_punctuation = value;
    }
    if let Some(value) = patch.normalize_spaces {
        settings.rules.normalize_spaces = value;
    }
    if let Some(value) = patch.smart_newline_handling {
        settings.rules.smart_newline_handling = value;
    }
    if let Some(value) = patch.detect_spoken_punctuation {
        settings.rules.detect_spoken_punctuation = value;
    }
    if let Some(value) = patch.spoken_formatting_rules {
        settings.rules.spoken_formatting_rules = value;
    }
    if let Some(value) = patch.self_correction_rules {
        settings.rules.self_correction_rules = value;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_patch_updates_selected_fields() {
        let temp =
            std::env::temp_dir().join(format!("kachakache-settings-{}.json", uuid::Uuid::new_v4()));
        let store = SettingsStore::new(temp.clone());
        let current = Settings::default();
        store.save(&current).unwrap();

        let patch = SettingsPatch {
            silence_timeout_ms: Some(42),
            overlay_enabled: Some(false),
            hide_dock_icon: Some(true),
            ..Default::default()
        };

        let updated = store.apply_patch(&current, patch).unwrap();
        assert_eq!(updated.silence_timeout_ms, 300);
        assert!(!updated.overlay_enabled);
        assert!(updated.hide_dock_icon);

        let _ = std::fs::remove_file(temp);
    }

    #[test]
    fn rules_patch_updates_nested_flags() {
        let temp =
            std::env::temp_dir().join(format!("kachakache-settings-{}.json", uuid::Uuid::new_v4()));
        let store = SettingsStore::new(temp.clone());
        let current = Settings::default();
        store.save(&current).unwrap();

        let patch = SettingsPatch {
            rules: Some(RulesPatch {
                remove_filler_words: Some(false),
                self_correction_rules: Some(false),
                ..Default::default()
            }),
            ..Default::default()
        };

        let updated = store.apply_patch(&current, patch).unwrap();
        assert!(!updated.rules.remove_filler_words);
        assert!(!updated.rules.self_correction_rules);
        assert!(updated.rules.normalize_spaces);

        let _ = std::fs::remove_file(temp);
    }

    #[test]
    fn transcript_retention_patch_updates_setting() {
        let temp =
            std::env::temp_dir().join(format!("kachakache-settings-{}.json", uuid::Uuid::new_v4()));
        let store = SettingsStore::new(temp.clone());
        let current = Settings::default();
        store.save(&current).unwrap();

        let patch = SettingsPatch {
            transcript_retention: Some(crate::types::TranscriptRetention::ThirtyDays),
            ..Default::default()
        };

        let updated = store.apply_patch(&current, patch).unwrap();
        assert_eq!(
            updated.transcript_retention,
            crate::types::TranscriptRetention::ThirtyDays
        );

        let _ = std::fs::remove_file(temp);
    }

    #[test]
    fn load_migrates_legacy_default_shortcut() {
        let temp =
            std::env::temp_dir().join(format!("kachakache-settings-{}.json", uuid::Uuid::new_v4()));
        let store = SettingsStore::new(temp.clone());
        let legacy = Settings {
            shortcut: "Cmd+Shift+Space".to_string(),
            ..Settings::default()
        };
        store.save(&legacy).unwrap();

        let loaded = store.load().unwrap();
        assert_eq!(loaded.shortcut, "Cmd+L");

        let _ = std::fs::remove_file(temp);
    }

    #[test]
    fn load_marks_existing_users_as_onboarded_when_field_is_missing() {
        let temp =
            std::env::temp_dir().join(format!("kachakache-settings-{}.json", uuid::Uuid::new_v4()));
        let store = SettingsStore::new(temp.clone());
        std::fs::write(
            &temp,
            r#"{
  "shortcut": "Cmd+L",
  "triggerMode": "toggle",
  "micDeviceId": null,
  "activeModelId": "base.en",
  "insertionMode": "autoPaste",
  "transcriptRetention": "indefinite",
  "silenceTimeoutMs": 1200,
  "overlayEnabled": true,
  "launchAtLoginPlaceholder": false,
  "rules": {
    "removeFillerWords": true,
    "capitalizeSentenceStarts": true,
    "convertPausesToPunctuation": true,
    "normalizeSpaces": true,
    "smartNewlineHandling": true,
    "detectSpokenPunctuation": true,
    "spokenFormattingRules": true,
    "selfCorrectionRules": true
  }
}"#,
        )
        .unwrap();

        let loaded = store.load().unwrap();
        assert!(loaded.onboarding_completed);

        let _ = std::fs::remove_file(temp);
    }

    #[test]
    fn load_defaults_hide_dock_icon_when_field_is_missing() {
        let temp =
            std::env::temp_dir().join(format!("kachakache-settings-{}.json", uuid::Uuid::new_v4()));
        let store = SettingsStore::new(temp.clone());
        std::fs::write(
            &temp,
            r#"{
  "shortcut": "Cmd+L",
  "triggerMode": "toggle",
  "micDeviceId": null,
  "activeModelId": "base.en",
  "insertionMode": "autoPaste",
  "transcriptRetention": "indefinite",
  "silenceTimeoutMs": 1200,
  "overlayEnabled": true,
  "launchAtLoginPlaceholder": false,
  "onboardingCompleted": true,
  "rules": {
    "removeFillerWords": true,
    "capitalizeSentenceStarts": true,
    "convertPausesToPunctuation": true,
    "normalizeSpaces": true,
    "smartNewlineHandling": true,
    "detectSpokenPunctuation": true,
    "spokenFormattingRules": true,
    "selfCorrectionRules": true
  }
}"#,
        )
        .unwrap();

        let loaded = store.load().unwrap();
        assert!(loaded.hide_dock_icon);

        let _ = std::fs::remove_file(temp);
    }
}
