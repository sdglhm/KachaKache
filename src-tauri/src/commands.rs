use crate::app_state::AppState;
use crate::types::{
    BootstrapState, DebugLogEvent, InstalledModel, MicrophoneDevice, PermissionKind,
    PermissionResult, PermissionsStatus, RecommendedModel, Settings, SettingsPatch,
    TranscriptEntry, TriggerSource,
};
use chrono::Utc;
use tauri::{AppHandle, Emitter, State};

#[tauri::command]
pub async fn bootstrap_state(state: State<'_, AppState>) -> Result<BootstrapState, String> {
    state.bootstrap_state().await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_microphones(state: State<'_, AppState>) -> Result<Vec<MicrophoneDevice>, String> {
    state
        .audio_service
        .list_microphones()
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn start_dictation(
    app: AppHandle,
    state: State<'_, AppState>,
    _trigger: Option<TriggerSource>,
) -> Result<(), String> {
    state.start_dictation(&app).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn stop_dictation(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    state.stop_dictation(&app).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_settings(state: State<'_, AppState>) -> Result<Settings, String> {
    Ok(state.current_settings().await)
}

#[tauri::command]
pub async fn update_settings(
    app: AppHandle,
    state: State<'_, AppState>,
    patch: SettingsPatch,
) -> Result<Settings, String> {
    let retention_changed = patch.transcript_retention.is_some();
    let previous = state.current_settings().await;
    let updated = state
        .update_settings(patch)
        .await
        .map_err(|e| e.to_string())?;

    if let Err(err) = state
        .register_shortcut(&app, updated.shortcut.clone())
        .await
    {
        state
            .settings_store
            .save(&previous)
            .map_err(|e| format!("failed to rollback settings after shortcut error: {e}"))?;
        *state.settings.write().await = previous.clone();
        let _ = state
            .register_shortcut(&app, previous.shortcut.clone())
            .await;
        return Err(err.to_string());
    }

    if previous.shortcut != updated.shortcut {
        let _ = app.emit("settings://shortcut-changed", updated.shortcut.clone());
    }

    if retention_changed {
        let history = state.history_store.load_pruned(&updated.transcript_retention)
            .map_err(|e| e.to_string())?;
        let _ = app.emit("history://updated", history);
    }

    Ok(updated)
}

#[tauri::command]
pub fn list_recommended_models(
    state: State<'_, AppState>,
) -> Result<Vec<RecommendedModel>, String> {
    Ok(state.model_manager.list_recommended())
}

#[tauri::command]
pub async fn list_installed_models(
    state: State<'_, AppState>,
) -> Result<Vec<InstalledModel>, String> {
    state
        .model_manager
        .list_installed()
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn download_model(
    app: AppHandle,
    state: State<'_, AppState>,
    model_id: String,
) -> Result<(), String> {
    let app_handle = app.clone();
    let manager = state.model_manager.clone();
    let app_state = state.inner().clone();
    tauri::async_runtime::spawn(async move {
        if let Err(err) = manager.download_model(&app_handle, model_id.clone()).await {
            emit_debug_log(
                &app_handle,
                "error",
                "models",
                format!("download failed for {}: {}", model_id, err),
            );
            let _ = app_handle.emit(
                "models://download-progress",
                crate::types::DownloadProgressEvent {
                    model_id,
                    received_bytes: 0,
                    total_bytes: None,
                    progress: 0.0,
                    done: true,
                    error: Some(err.to_string()),
                },
            );
            return;
        }

        if let Some(active_model_id) = manager.get_active_model_id().await {
            let mut settings = app_state.current_settings().await;
            if settings.active_model_id.as_deref() != Some(active_model_id.as_str()) {
                settings.active_model_id = Some(active_model_id.clone());
                if let Err(err) = app_state.settings_store.save(&settings) {
                    emit_debug_log(
                        &app_handle,
                        "warn",
                        "models",
                        format!(
                            "download completed but failed to sync active model into settings: {}",
                            err
                        ),
                    );
                } else {
                    *app_state.settings.write().await = settings;
                    emit_debug_log(
                        &app_handle,
                        "info",
                        "models",
                        format!("synced active model {} into settings", active_model_id),
                    );
                }
            }
        }
    });
    Ok(())
}

#[tauri::command]
pub async fn cancel_model_download(
    state: State<'_, AppState>,
    model_id: String,
) -> Result<(), String> {
    state.model_manager.cancel_download(&model_id).await;
    Ok(())
}

#[tauri::command]
pub async fn set_active_model(state: State<'_, AppState>, model_id: String) -> Result<(), String> {
    state
        .model_manager
        .set_active_model(model_id.clone())
        .await
        .map_err(|e| e.to_string())?;
    let mut settings = state.current_settings().await;
    settings.active_model_id = Some(model_id);
    state
        .settings_store
        .save(&settings)
        .map_err(|e| e.to_string())?;
    *state.settings.write().await = settings;
    Ok(())
}

#[tauri::command]
pub async fn delete_model(state: State<'_, AppState>, model_id: String) -> Result<(), String> {
    state
        .model_manager
        .delete_model(&model_id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_permissions_status(state: State<'_, AppState>) -> Result<PermissionsStatus, String> {
    Ok(state.permissions_service.status())
}

#[tauri::command]
pub fn request_permission(
    state: State<'_, AppState>,
    kind: PermissionKind,
) -> Result<PermissionResult, String> {
    Ok(state.permissions_service.request(kind))
}

#[tauri::command]
pub fn open_permission_settings(
    state: State<'_, AppState>,
    kind: PermissionKind,
) -> Result<(), String> {
    state
        .permissions_service
        .open_settings(kind)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_history(
    state: State<'_, AppState>,
    limit: Option<usize>,
) -> Result<Vec<TranscriptEntry>, String> {
    let settings = state.current_settings().await;
    let mut history = state
        .history_store
        .load_pruned(&settings.transcript_retention)
        .map_err(|e| e.to_string())?;
    if let Some(limit) = limit {
        history.truncate(limit);
    }
    Ok(history)
}

#[tauri::command]
pub fn clear_history(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    state.history_store.clear().map_err(|e| e.to_string())?;
    emit_debug_log(&app, "info", "history", "cleared transcript history");
    let _ = app.emit("history://updated", Vec::<TranscriptEntry>::new());
    Ok(())
}

#[tauri::command]
pub fn delete_history_entry(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
) -> Result<(), String> {
    let updated = state
        .history_store
        .delete_entry(&id)
        .map_err(|e| e.to_string())?;
    emit_debug_log(&app, "warn", "history", format!("deleted history entry {}", id));
    let _ = app.emit("history://updated", updated);
    Ok(())
}

#[tauri::command]
pub fn copy_history_entry(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
) -> Result<(), String> {
    let history = state.history_store.load().map_err(|e| e.to_string())?;
    let entry = history
        .into_iter()
        .find(|entry| entry.id == id)
        .ok_or_else(|| "history item not found".to_string())?;
    state
        .insertion_service
        .copy_to_clipboard(&entry.text)
        .map_err(|e| e.to_string())?;
    emit_debug_log(&app, "info", "history", format!("copied history entry {}", id));
    Ok(())
}

#[tauri::command]
pub fn open_setup_window(app: AppHandle) -> Result<(), String> {
    crate::show_setup_window(&app).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn complete_setup_flow(app: AppHandle) -> Result<(), String> {
    crate::close_setup_window(&app, true);
    Ok(())
}

#[tauri::command]
pub fn dismiss_setup_flow(app: AppHandle) -> Result<(), String> {
    crate::close_setup_window(&app, true);
    Ok(())
}

#[tauri::command]
pub fn is_debug_build() -> bool {
    cfg!(debug_assertions)
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
