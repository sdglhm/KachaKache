use crate::services::paths::AppPaths;
use crate::types::{DownloadProgressEvent, InstalledModel, RecommendedModel};
use anyhow::{anyhow, Context};
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use tokio::io::AsyncWriteExt;
use tokio::sync::Mutex;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct ModelsStateFile {
    active_model_id: Option<String>,
}

#[derive(Clone)]
pub struct ModelManager {
    paths: AppPaths,
    state: Arc<Mutex<ModelsStateFile>>,
    download_flags: Arc<Mutex<HashMap<String, Arc<AtomicBool>>>>,
    client: reqwest::Client,
}

impl ModelManager {
    pub async fn new(paths: AppPaths) -> anyhow::Result<Self> {
        let state = if paths.models_state_path.exists() {
            let raw = tokio::fs::read_to_string(&paths.models_state_path)
                .await
                .unwrap_or_default();
            serde_json::from_str(&raw).unwrap_or_default()
        } else {
            ModelsStateFile::default()
        };

        Ok(Self {
            paths,
            state: Arc::new(Mutex::new(state)),
            download_flags: Arc::new(Mutex::new(HashMap::new())),
            client: reqwest::Client::new(),
        })
    }

    pub fn list_recommended(&self) -> Vec<RecommendedModel> {
        recommended_models()
    }

    pub async fn list_installed(&self) -> anyhow::Result<Vec<InstalledModel>> {
        let state = self.state.lock().await.clone();
        let mut installed = vec![];

        for model in recommended_models() {
            let model_path = self.paths.models_dir.join(&model.file_name);
            if let Ok(metadata) = tokio::fs::metadata(&model_path).await {
                installed.push(InstalledModel {
                    id: model.id.clone(),
                    display_name: model.display_name.clone(),
                    file_name: model.file_name.clone(),
                    local_path: model_path.to_string_lossy().to_string(),
                    size_bytes: metadata.len(),
                    is_active: state.active_model_id.as_deref() == Some(model.id.as_str()),
                });
            }
        }

        Ok(installed)
    }

    pub async fn set_active_model(&self, model_id: String) -> anyhow::Result<()> {
        let model = self
            .list_recommended()
            .into_iter()
            .find(|m| m.id == model_id)
            .ok_or_else(|| anyhow!("unknown model id"))?;

        let model_path = self.paths.models_dir.join(model.file_name);
        if !model_path.exists() {
            return Err(anyhow!("model is not downloaded"));
        }

        {
            let mut state = self.state.lock().await;
            state.active_model_id = Some(model_id);
            self.save_state(&state).await?;
        }

        Ok(())
    }

    pub async fn get_active_model_id(&self) -> Option<String> {
        self.state.lock().await.active_model_id.clone()
    }

    pub async fn ensure_active_model_default(&self) -> anyhow::Result<()> {
        let has_active = self.state.lock().await.active_model_id.is_some();
        if has_active {
            return Ok(());
        }

        let base_model = self
            .list_recommended()
            .into_iter()
            .find(|m| m.id == "base.en")
            .ok_or_else(|| anyhow!("missing base.en model definition"))?;

        let base_path = self.paths.models_dir.join(base_model.file_name);
        if base_path.exists() {
            self.set_active_model("base.en".to_string()).await?;
        }

        Ok(())
    }

    pub async fn active_model_path(
        &self,
        requested: Option<String>,
    ) -> anyhow::Result<(String, PathBuf)> {
        let fallback = self.state.lock().await.active_model_id.clone();
        let requested_model = requested
            .as_ref()
            .and_then(|id| self.list_recommended().into_iter().find(|m| m.id == *id))
            .map(|model| {
                let path = self.paths.models_dir.join(&model.file_name);
                (model.id, path)
            });

        if let Some((model_id, model_path)) = requested_model {
            if model_path.exists() {
                return Ok((model_id, model_path));
            }
        }

        let fallback_id = fallback
            .or(requested)
            .unwrap_or_else(|| "base.en".to_string());
        let fallback_model = self
            .list_recommended()
            .into_iter()
            .find(|m| m.id == fallback_id)
            .ok_or_else(|| anyhow!("unknown active model"))?;
        let fallback_path = self.paths.models_dir.join(&fallback_model.file_name);

        if fallback_path.exists() {
            return Ok((fallback_model.id, fallback_path));
        }

        Err(anyhow!(
            "active model is not downloaded. Open Models and download one first"
        ))
    }

    pub async fn delete_model(&self, model_id: &str) -> anyhow::Result<()> {
        let model = self
            .list_recommended()
            .into_iter()
            .find(|m| m.id == model_id)
            .ok_or_else(|| anyhow!("unknown model"))?;

        let model_path = self.paths.models_dir.join(model.file_name);
        if model_path.exists() {
            tokio::fs::remove_file(model_path)
                .await
                .context("failed to remove model file")?;
        }

        let mut state = self.state.lock().await;
        if state.active_model_id.as_deref() == Some(model_id) {
            state.active_model_id = None;
            self.save_state(&state).await?;
        }

        Ok(())
    }

    pub async fn cancel_download(&self, model_id: &str) {
        if let Some(flag) = self.download_flags.lock().await.get(model_id).cloned() {
            flag.store(true, Ordering::Relaxed);
        }
    }

    pub async fn download_model(&self, app: &AppHandle, model_id: String) -> anyhow::Result<()> {
        let model = self
            .list_recommended()
            .into_iter()
            .find(|m| m.id == model_id)
            .ok_or_else(|| anyhow!("unknown model"))?;

        let target_path = self.paths.models_dir.join(&model.file_name);
        if target_path.exists() {
            self.emit_download(
                app,
                DownloadProgressEvent {
                    model_id: model.id,
                    received_bytes: 0,
                    total_bytes: None,
                    progress: 1.0,
                    done: true,
                    error: None,
                },
            );
            return Ok(());
        }

        let cancel_flag = Arc::new(AtomicBool::new(false));
        self.download_flags
            .lock()
            .await
            .insert(model.id.clone(), cancel_flag.clone());

        let temp_path = self
            .paths
            .models_dir
            .join(format!("{}.tmp", model.file_name));
        let response = self
            .client
            .get(&model.url)
            .send()
            .await
            .with_context(|| format!("failed to download {}", model.display_name))?;

        if !response.status().is_success() {
            return Err(anyhow!(
                "download request failed with {}",
                response.status()
            ));
        }

        let total = response.content_length();
        let mut stream = response.bytes_stream();
        let mut file = tokio::fs::File::create(&temp_path)
            .await
            .context("failed to create temporary model file")?;

        let mut received: u64 = 0;
        while let Some(chunk) = stream.next().await {
            if cancel_flag.load(Ordering::Relaxed) {
                let _ = tokio::fs::remove_file(&temp_path).await;
                self.download_flags.lock().await.remove(&model.id);
                self.emit_download(
                    app,
                    DownloadProgressEvent {
                        model_id: model.id,
                        received_bytes: received,
                        total_bytes: total,
                        progress: 0.0,
                        done: true,
                        error: Some("Cancelled".to_string()),
                    },
                );
                return Ok(());
            }

            let bytes = chunk.context("failed while reading model download stream")?;
            file.write_all(&bytes)
                .await
                .context("failed to write model bytes")?;
            received += bytes.len() as u64;

            let progress = if let Some(total_bytes) = total {
                (received as f32 / total_bytes as f32).clamp(0.0, 1.0)
            } else {
                0.0
            };

            self.emit_download(
                app,
                DownloadProgressEvent {
                    model_id: model.id.clone(),
                    received_bytes: received,
                    total_bytes: total,
                    progress,
                    done: false,
                    error: None,
                },
            );
        }

        file.flush().await.ok();
        tokio::fs::rename(&temp_path, &target_path)
            .await
            .context("failed to move model into final location")?;

        self.download_flags.lock().await.remove(&model.id);

        if self.get_active_model_id().await.is_none() {
            self.set_active_model(model.id.clone()).await?;
        }

        self.emit_download(
            app,
            DownloadProgressEvent {
                model_id: model.id,
                received_bytes: received,
                total_bytes: total,
                progress: 1.0,
                done: true,
                error: None,
            },
        );

        Ok(())
    }

    async fn save_state(&self, state: &ModelsStateFile) -> anyhow::Result<()> {
        let json = serde_json::to_string_pretty(state)?;
        tokio::fs::write(&self.paths.models_state_path, json)
            .await
            .context("failed to write models state")?;
        Ok(())
    }

    fn emit_download(&self, app: &AppHandle, payload: DownloadProgressEvent) {
        let _ = app.emit("models://download-progress", payload);
    }
}

fn recommended_models() -> Vec<RecommendedModel> {
    vec![
        RecommendedModel {
            id: "tiny.en".to_string(),
            display_name: "Tiny (English)".to_string(),
            file_name: "ggml-tiny.en.bin".to_string(),
            size_mb: 75,
            speed_note: "Fastest".to_string(),
            quality_note: "Good for quick notes".to_string(),
            url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-tiny.en.bin"
                .to_string(),
            sha256: None,
        },
        RecommendedModel {
            id: "base.en".to_string(),
            display_name: "Base (English)".to_string(),
            file_name: "ggml-base.en.bin".to_string(),
            size_mb: 142,
            speed_note: "Balanced".to_string(),
            quality_note: "Best default for MVP".to_string(),
            url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.en.bin"
                .to_string(),
            sha256: None,
        },
        RecommendedModel {
            id: "small.en".to_string(),
            display_name: "Small (English)".to_string(),
            file_name: "ggml-small.en.bin".to_string(),
            size_mb: 466,
            speed_note: "Slower".to_string(),
            quality_note: "Higher quality".to_string(),
            url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-small.en.bin"
                .to_string(),
            sha256: None,
        },
        RecommendedModel {
            id: "distil-large-v3".to_string(),
            display_name: "Distil Large v3".to_string(),
            file_name: "ggml-distil-large-v3.bin".to_string(),
            size_mb: 1450,
            speed_note: "Faster than Large v3".to_string(),
            quality_note: "High quality distilled model".to_string(),
            url: "https://huggingface.co/distil-whisper/distil-large-v3-ggml/resolve/main/ggml-distil-large-v3.bin"
                .to_string(),
            sha256: None,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use uuid::Uuid;

    #[test]
    fn recommended_models_are_present() {
        let models = recommended_models();
        assert!(models.iter().any(|m| m.id == "tiny.en"));
        assert!(models.iter().any(|m| m.id == "base.en"));
        assert!(models.iter().any(|m| m.id == "small.en"));
        assert!(models.iter().any(|m| m.id == "distil-large-v3"));
    }

    #[tokio::test]
    async fn active_model_path_falls_back_to_manager_active_model_when_settings_are_stale() {
        let root = std::env::temp_dir().join(format!("kachakache-model-test-{}", Uuid::new_v4()));
        let models_dir = root.join("models");
        fs::create_dir_all(&models_dir).unwrap();

        let paths = AppPaths {
            models_dir: models_dir.clone(),
            settings_path: root.join("settings.json"),
            models_state_path: root.join("models_state.json"),
            history_path: root.join("history.json"),
        };

        let active_model = recommended_models()
            .into_iter()
            .find(|model| model.id == "tiny.en")
            .unwrap();
        fs::write(
            &paths.models_state_path,
            r#"{"activeModelId":"tiny.en"}"#,
        )
        .unwrap();
        fs::write(models_dir.join(active_model.file_name), b"model").unwrap();

        let manager = ModelManager::new(paths.clone()).await.unwrap();
        let (resolved_id, resolved_path) = manager
            .active_model_path(Some("base.en".to_string()))
            .await
            .unwrap();

        assert_eq!(resolved_id, "tiny.en");
        assert!(resolved_path.exists());

        let _ = fs::remove_dir_all(root);
    }
}
