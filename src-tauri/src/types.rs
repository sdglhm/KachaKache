use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum TriggerMode {
    PushToTalk,
    Toggle,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum InsertionMode {
    #[serde(alias = "autoPaste", alias = "type")]
    Automatic,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum InsertionStrategy {
    Typed,
    Paste,
    ClipboardOnly,
    Unknown,
}

impl Default for InsertionStrategy {
    fn default() -> Self {
        Self::Unknown
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum TranscriptRetention {
    Indefinite,
    NinetyDays,
    ThirtyDays,
    FourteenDays,
    SevenDays,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum PermissionKind {
    Microphone,
    Accessibility,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum TriggerSource {
    Manual,
    ShortcutPressed,
    ShortcutReleased,
    SilenceAuto,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum DictationPhase {
    Ready,
    Listening,
    Processing,
    Done,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DictationStatus {
    pub phase: DictationPhase,
    pub message: String,
}

impl Default for DictationStatus {
    fn default() -> Self {
        Self {
            phase: DictationPhase::Ready,
            message: "Ready".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(default)]
pub struct Settings {
    pub shortcut: String,
    pub trigger_mode: TriggerMode,
    pub mic_device_id: Option<String>,
    pub active_model_id: Option<String>,
    pub insertion_mode: InsertionMode,
    pub transcript_retention: TranscriptRetention,
    pub silence_timeout_ms: u64,
    pub overlay_enabled: bool,
    pub hide_dock_icon: bool,
    pub launch_at_login_placeholder: bool,
    pub onboarding_completed: bool,
    #[serde(default)]
    pub rules: RulesConfig,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            shortcut: "Cmd+L".to_string(),
            trigger_mode: TriggerMode::Toggle,
            mic_device_id: None,
            active_model_id: Some("base.en".to_string()),
            insertion_mode: InsertionMode::Automatic,
            transcript_retention: TranscriptRetention::Indefinite,
            silence_timeout_ms: 1200,
            overlay_enabled: true,
            hide_dock_icon: true,
            launch_at_login_placeholder: false,
            onboarding_completed: false,
            rules: RulesConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RulesConfig {
    pub remove_filler_words: bool,
    pub capitalize_sentence_starts: bool,
    pub convert_pauses_to_punctuation: bool,
    pub normalize_spaces: bool,
    pub smart_newline_handling: bool,
    pub detect_spoken_punctuation: bool,
    pub spoken_formatting_rules: bool,
    pub self_correction_rules: bool,
}

impl Default for RulesConfig {
    fn default() -> Self {
        Self {
            remove_filler_words: true,
            capitalize_sentence_starts: true,
            convert_pauses_to_punctuation: true,
            normalize_spaces: true,
            smart_newline_handling: true,
            detect_spoken_punctuation: true,
            spoken_formatting_rules: true,
            self_correction_rules: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SettingsPatch {
    pub shortcut: Option<String>,
    pub trigger_mode: Option<TriggerMode>,
    pub mic_device_id: Option<Option<String>>,
    pub active_model_id: Option<Option<String>>,
    pub insertion_mode: Option<InsertionMode>,
    pub transcript_retention: Option<TranscriptRetention>,
    pub silence_timeout_ms: Option<u64>,
    pub overlay_enabled: Option<bool>,
    pub hide_dock_icon: Option<bool>,
    pub launch_at_login_placeholder: Option<bool>,
    pub onboarding_completed: Option<bool>,
    pub rules: Option<RulesPatch>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RulesPatch {
    pub remove_filler_words: Option<bool>,
    pub capitalize_sentence_starts: Option<bool>,
    pub convert_pauses_to_punctuation: Option<bool>,
    pub normalize_spaces: Option<bool>,
    pub smart_newline_handling: Option<bool>,
    pub detect_spoken_punctuation: Option<bool>,
    pub spoken_formatting_rules: Option<bool>,
    pub self_correction_rules: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecommendedModel {
    pub id: String,
    pub display_name: String,
    pub file_name: String,
    pub size_mb: u64,
    pub speed_note: String,
    pub quality_note: String,
    pub url: String,
    pub sha256: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledModel {
    pub id: String,
    pub display_name: String,
    pub file_name: String,
    pub local_path: String,
    pub size_bytes: u64,
    pub is_active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadProgressEvent {
    pub model_id: String,
    pub received_bytes: u64,
    pub total_bytes: Option<u64>,
    pub progress: f32,
    pub done: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PermissionsStatus {
    pub microphone_granted: bool,
    pub accessibility_granted: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PermissionResult {
    pub kind: PermissionKind,
    pub granted: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MicrophoneDevice {
    pub id: String,
    pub name: String,
    pub is_default: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptEntry {
    pub id: String,
    pub text: String,
    pub created_at: String,
    pub model_id: String,
    pub duration_ms: u64,
    #[serde(default)]
    pub inserted: bool,
    #[serde(default)]
    pub insertion_strategy: InsertionStrategy,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BootstrapState {
    pub settings: Settings,
    pub status: DictationStatus,
    pub permissions: PermissionsStatus,
    pub recommended_models: Vec<RecommendedModel>,
    pub installed_models: Vec<InstalledModel>,
    pub microphones: Vec<MicrophoneDevice>,
    pub history: Vec<TranscriptEntry>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FinalTextEvent {
    pub text: String,
    pub inserted: bool,
    pub insertion_strategy: InsertionStrategy,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DebugLogEvent {
    pub timestamp: String,
    pub level: String,
    pub scope: String,
    pub message: String,
}
