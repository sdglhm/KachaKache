use crate::services::audio::AudioService;
use crate::services::dictation_controller::DictationController;
use crate::services::history::HistoryStore;
use crate::services::insertion::InsertionService;
use crate::services::models::ModelManager;
use crate::services::paths::AppPaths;
use crate::services::permissions::PermissionsService;
use crate::services::settings::SettingsStore;
use crate::services::transcription::TranscriptionService;
use crate::types::{BootstrapState, DebugLogEvent, DictationPhase, Settings, SettingsPatch, TriggerMode};
use anyhow::Context;
use chrono::Utc;
use serde_json::json;
use std::str::FromStr;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};
use tokio::sync::{Mutex, RwLock};
use tokio::time::{sleep, Duration};

const LISTEN_LOOP_TICK_MS: u64 = 180;
const BACKGROUND_SEGMENT_MIN_MS: u64 = 1200;
const BACKGROUND_SEGMENT_TARGET_MS: u64 = 6_000;
const BACKGROUND_SEGMENT_MIN_PEAK: f32 = 0.012;
const BACKGROUND_SEGMENT_MIN_SPEECH_RATIO: f32 = 0.01;

#[derive(Clone)]
pub struct AppState {
    pub settings_store: SettingsStore,
    pub settings: Arc<RwLock<Settings>>,
    pub history_store: HistoryStore,
    pub model_manager: ModelManager,
    pub permissions_service: PermissionsService,
    pub audio_service: AudioService,
    pub transcription_service: TranscriptionService,
    pub insertion_service: InsertionService,
    pub dictation: DictationController,
    pub registered_shortcut: Arc<Mutex<Option<String>>>,
}

impl AppState {
    pub async fn new(app: &AppHandle) -> anyhow::Result<Self> {
        let paths = AppPaths::new(app)?;
        let settings_store = SettingsStore::new(paths.settings_path.clone());
        let mut settings = settings_store.load()?;
        let history_store = HistoryStore::new(paths.history_path.clone(), 100);
        if !paths.history_path.exists() {
            history_store.clear()?;
        }

        let model_manager = ModelManager::new(paths.clone()).await?;
        model_manager
            .ensure_active_model_default()
            .await
            .context("failed to initialize model state")?;

        let mut should_save_settings = false;
        if let Some(settings_active_model_id) = settings.active_model_id.clone() {
            if model_manager
                .set_active_model(settings_active_model_id.clone())
                .await
                .is_err()
            {
                if let Some(manager_active_model_id) = model_manager.get_active_model_id().await {
                    if settings.active_model_id.as_deref()
                        != Some(manager_active_model_id.as_str())
                    {
                        settings.active_model_id = Some(manager_active_model_id);
                        should_save_settings = true;
                    }
                }
            }
        } else if let Some(manager_active_model_id) = model_manager.get_active_model_id().await {
            settings.active_model_id = Some(manager_active_model_id);
            should_save_settings = true;
        }

        if should_save_settings {
            settings_store.save(&settings)?;
        }

        Ok(Self {
            settings_store,
            settings: Arc::new(RwLock::new(settings)),
            history_store,
            model_manager,
            permissions_service: PermissionsService::new(),
            audio_service: AudioService::new(),
            transcription_service: TranscriptionService::new(),
            insertion_service: InsertionService::new(),
            dictation: DictationController::new(),
            registered_shortcut: Arc::new(Mutex::new(None)),
        })
    }

    pub async fn bootstrap_state(&self) -> anyhow::Result<BootstrapState> {
        let settings = self.settings.read().await.clone();
        let status = self.dictation.status();
        let permissions = self.permissions_service.status();
        let recommended_models = self.model_manager.list_recommended();
        let installed_models = self.model_manager.list_installed().await?;
        let microphones = self.audio_service.list_microphones()?;
        let history = self
            .history_store
            .load_pruned(&settings.transcript_retention)?;

        Ok(BootstrapState {
            settings,
            status,
            permissions,
            recommended_models,
            installed_models,
            microphones,
            history,
        })
    }

    pub async fn current_settings(&self) -> Settings {
        self.settings.read().await.clone()
    }

    pub async fn update_settings(&self, patch: SettingsPatch) -> anyhow::Result<Settings> {
        let current = self.settings.read().await.clone();
        let updated = self.settings_store.apply_patch(&current, patch)?;

        let _ = self
            .history_store
            .load_pruned(&updated.transcript_retention)?;

        if let Some(active_model_id) = updated.active_model_id.clone() {
            let _ = self.model_manager.set_active_model(active_model_id).await;
        }

        *self.settings.write().await = updated.clone();
        Ok(updated)
    }

    pub async fn register_shortcut(&self, app: &AppHandle, shortcut: String) -> anyhow::Result<()> {
        let requested = canonical_shortcut_for_registration(&shortcut)?;
        let previous = self.registered_shortcut.lock().await.clone();
        if previous.as_deref() == Some(requested.as_str()) {
            return Ok(());
        }

        let handler_app = app.clone();
        app.global_shortcut()
            .on_shortcut(requested.as_str(), move |_, _, event| {
                let app_handle = handler_app.clone();
                tauri::async_runtime::spawn(async move {
                    let state = app_handle.state::<AppState>().inner().clone();
                    let _ = state.handle_hotkey_state(&app_handle, event.state).await;
                });
            })?;

        if let Some(old) = previous {
            let _ = app.global_shortcut().unregister(old.as_str());
        }

        *self.registered_shortcut.lock().await = Some(requested);
        emit_debug_log(
            app,
            "info",
            "shortcut",
            format!("registered global shortcut {}", shortcut),
        );
        Ok(())
    }

    pub async fn handle_hotkey_state(
        &self,
        app: &AppHandle,
        shortcut_state: ShortcutState,
    ) -> anyhow::Result<()> {
        let settings = self.current_settings().await;
        emit_debug_log(
            app,
            "debug",
            "shortcut",
            format!("hotkey event {:?} in mode {:?}", shortcut_state, settings.trigger_mode),
        );
        match settings.trigger_mode {
            TriggerMode::Toggle => {
                if shortcut_state == ShortcutState::Pressed {
                    if self.dictation.is_listening() {
                        self.stop_dictation(app).await?;
                    } else {
                        self.start_dictation(app).await?;
                    }
                }
            }
            TriggerMode::PushToTalk => {
                if shortcut_state == ShortcutState::Pressed {
                    self.start_dictation(app).await?;
                } else if shortcut_state == ShortcutState::Released && self.dictation.is_listening()
                {
                    self.stop_dictation(app).await?;
                }
            }
        }

        Ok(())
    }

    pub async fn start_dictation(&self, app: &AppHandle) -> anyhow::Result<()> {
        if self.dictation.is_listening() || self.dictation.is_processing() {
            return Ok(());
        }

        let permissions = self.permissions_service.status();
        if !permissions.microphone_granted {
            emit_debug_log(app, "warn", "permissions", "microphone permission missing");
            self.dictation
                .set_error(app, "Microphone permission required");
            return Ok(());
        }

        if let Err(err) = self.transcription_service.ensure_runtime_available() {
            emit_debug_log(
                app,
                "error",
                "runtime",
                format!("whisper runtime unavailable: {}", format_error_chain(&err)),
            );
            self.dictation.set_error(
                app,
                format!(
                    "Transcription runtime missing. Run `npm run sync:whisper-runtime` ({})",
                    format_error_chain(&err)
                ),
            );
            return Ok(());
        }

        let settings = self.current_settings().await;
        self.dictation.start(app, &self.audio_service, &settings)?;

        let controller = self.dictation.clone();
        let state = self.clone();
        let app_handle = app.clone();
        tauri::async_runtime::spawn(async move {
            loop {
                sleep(Duration::from_millis(LISTEN_LOOP_TICK_MS)).await;
                if !controller.is_listening() {
                    break;
                }

                let runtime_settings = state.current_settings().await;
                let heard_speech = controller.has_heard_speech();
                let speaking_now = controller.speaking_now();
                let silence_ms = controller.silence_elapsed_ms();
                let timeout_ms = runtime_settings.silence_timeout_ms;
                let auto_stop_in_ms = if heard_speech {
                    timeout_ms.saturating_sub(silence_ms)
                } else {
                    timeout_ms
                };

                let _ = app_handle.emit(
                    "dictation://vad",
                    json!({
                        "heardSpeech": heard_speech,
                        "speakingNow": speaking_now,
                        "silenceMs": silence_ms,
                        "autoStopInMs": auto_stop_in_ms
                    }),
                );

                if heard_speech && controller.begin_background_transcription() {
                    let segment = controller
                        .take_elapsed_background_segment(BACKGROUND_SEGMENT_TARGET_MS)
                        .or_else(|| {
                            if !speaking_now {
                                controller.take_silent_background_segment(BACKGROUND_SEGMENT_MIN_MS)
                            } else {
                                None
                            }
                        });

                    if let Some(segment) = segment {
                        emit_debug_log(
                            &app_handle,
                            "debug",
                            "transcription",
                            format!(
                                "captured background chunk duration={}ms peak={:.4} speech_ratio={:.4}",
                                segment.duration_ms, segment.peak_level, segment.speech_ratio
                            ),
                        );
                        let session_id = controller.active_session_id();
                        let state_clone = state.clone();
                        let app_clone = app_handle.clone();
                        let controller_clone = controller.clone();
                        tauri::async_runtime::spawn(async move {
                            if !background_segment_is_meaningful(&segment) {
                                controller_clone
                                    .finish_background_transcription_for_session(session_id);
                                return;
                            }

                            let prepared = crate::services::audio::to_mono_16k(&segment);
                            if prepared.is_empty() {
                                controller_clone
                                    .finish_background_transcription_for_session(session_id);
                                return;
                            }

                            let settings = state_clone.current_settings().await;
                            let model_path = state_clone
                                .model_manager
                                .active_model_path(settings.active_model_id)
                                .await;

                            if let Ok((_, path)) = model_path {
                                if let Ok(text) = state_clone
                                    .transcription_service
                                    .transcribe(path, prepared)
                                    .await
                                {
                                    if !text.trim().is_empty() {
                                        emit_debug_log(
                                            &app_clone,
                                            "debug",
                                            "transcription",
                                            format!(
                                                "background chunk produced {} chars",
                                                text.trim().chars().count()
                                            ),
                                        );
                                        controller_clone.append_background_transcript_for_session(
                                            session_id, &text,
                                        );
                                        let _ = app_clone.emit("dictation://partial-text", text);
                                    }
                                }
                            }

                            controller_clone
                                .finish_background_transcription_for_session(session_id);
                        });
                    } else {
                        controller.finish_background_transcription_for_session(
                            controller.active_session_id(),
                        );
                    }
                }

                if runtime_settings.trigger_mode == TriggerMode::Toggle
                    && heard_speech
                    && silence_ms >= timeout_ms
                {
                    let _ = state.stop_dictation(&app_handle).await;
                    break;
                }
            }
        });

        Ok(())
    }

    pub async fn stop_dictation(&self, app: &AppHandle) -> anyhow::Result<()> {
        if self.dictation.is_processing() || !self.dictation.is_listening() {
            return Ok(());
        }

        let settings = self.current_settings().await;
        let result = self
            .dictation
            .stop_and_process(
                app,
                &settings,
                &self.model_manager,
                &self.transcription_service,
                &self.insertion_service,
                &self.history_store,
            )
            .await;

        if let Err(err) = result {
            self.dictation.set_error(
                app,
                format!("Dictation failed: {}", format_error_chain(&err)),
            );
            sleep(Duration::from_millis(1200)).await;
            if self.dictation.status().phase == DictationPhase::Error {
                self.dictation.reset_ready(app);
            }
        }

        Ok(())
    }
}

fn emit_debug_log(app: &AppHandle, level: &str, scope: &str, message: impl Into<String>) {
    let _ = app.emit(
        "debug://log",
        DebugLogEvent {
            timestamp: Utc::now().to_rfc3339(),
            level: level.to_string(),
            scope: scope.to_string(),
            message: message.into(),
        },
    );
}

fn background_segment_is_meaningful(segment: &crate::services::audio::RecordedAudio) -> bool {
    segment.duration_ms >= BACKGROUND_SEGMENT_MIN_MS
        && segment.peak_level >= BACKGROUND_SEGMENT_MIN_PEAK
        && segment.speech_ratio >= BACKGROUND_SEGMENT_MIN_SPEECH_RATIO
}

fn format_error_chain(err: &anyhow::Error) -> String {
    let mut chain = err.chain();
    if let Some(first) = chain.next() {
        let mut out = first.to_string();
        for cause in chain {
            out.push_str(": ");
            out.push_str(&cause.to_string());
        }
        out
    } else {
        "unknown error".to_string()
    }
}

fn canonical_shortcut_for_registration(raw: &str) -> anyhow::Result<String> {
    let tokens = raw
        .split('+')
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .map(normalize_shortcut_token)
        .collect::<Vec<_>>();

    if tokens.is_empty() {
        anyhow::bail!("shortcut cannot be empty");
    }

    let has_main_key = tokens.iter().any(|t| !is_modifier_token(t));
    if !has_main_key {
        anyhow::bail!("shortcut requires at least one non-modifier key");
    }

    let candidate = tokens.join("+");
    let parsed = Shortcut::from_str(&candidate)
        .map_err(|e| anyhow::anyhow!("invalid shortcut `{raw}`: {e}"))?;
    Ok(parsed.into_string())
}

fn normalize_shortcut_token(token: &str) -> String {
    match token.to_ascii_uppercase().as_str() {
        "CMD" | "COMMAND" | "SUPER" | "META" => "CommandOrControl".to_string(),
        "CTRL" | "CONTROL" => "Control".to_string(),
        "ALT" | "OPTION" => "Alt".to_string(),
        "SHIFT" => "Shift".to_string(),
        "ESC" => "Escape".to_string(),
        other => match other {
            "ARROWUP" | "UP" => "ArrowUp".to_string(),
            "ARROWDOWN" | "DOWN" => "ArrowDown".to_string(),
            "ARROWLEFT" | "LEFT" => "ArrowLeft".to_string(),
            "ARROWRIGHT" | "RIGHT" => "ArrowRight".to_string(),
            _ => token.to_string(),
        },
    }
}

fn is_modifier_token(token: &str) -> bool {
    matches!(
        token.to_ascii_uppercase().as_str(),
        "CMD"
            | "COMMAND"
            | "SUPER"
            | "META"
            | "COMMANDORCONTROL"
            | "COMMANDORCTRL"
            | "CMDORCTRL"
            | "CMDORCONTROL"
            | "CTRL"
            | "CONTROL"
            | "ALT"
            | "OPTION"
            | "SHIFT"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonicalizes_cmd_shortcuts() {
        let canonical = canonical_shortcut_for_registration("Cmd+L").expect("valid shortcut");
        assert!(
            canonical.to_ascii_uppercase().contains("SUPER")
                || canonical.to_ascii_uppercase().contains("COMMANDORCONTROL")
        );
    }

    #[test]
    fn rejects_modifier_only_shortcuts() {
        let err = canonical_shortcut_for_registration("Cmd+Shift").expect_err("should fail");
        assert!(err
            .to_string()
            .contains("shortcut requires at least one non-modifier key"));
    }
}
