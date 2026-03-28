use crate::services::audio::{
    to_mono_16k, AudioService, RecordedAudio, RecordingConfig, RecordingSession,
};
use crate::services::history::HistoryStore;
use crate::services::insertion::InsertionService;
use crate::services::models::ModelManager;
use crate::services::text_rules::apply_transcript_rules;
use crate::services::transcription::TranscriptionService;
use crate::types::{
    DebugLogEvent, DictationPhase, DictationStatus, FinalTextEvent, Settings, TranscriptEntry,
    TriggerMode,
};
use anyhow::anyhow;
use chrono::Utc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;
use tauri::{AppHandle, Emitter};
use tokio::time::{sleep, Duration};
use uuid::Uuid;

const BACKGROUND_FLUSH_MAX_WAIT_MS: u64 = 2_000;

#[derive(Clone)]
pub struct DictationController {
    status: Arc<Mutex<DictationStatus>>,
    recording: Arc<Mutex<Option<RecordingSession>>>,
    transcript_chunks: Arc<Mutex<Vec<String>>>,
    background_transcribing: Arc<Mutex<bool>>,
    session_id: Arc<AtomicU64>,
}

impl DictationController {
    pub fn new() -> Self {
        Self {
            status: Arc::new(Mutex::new(DictationStatus::default())),
            recording: Arc::new(Mutex::new(None)),
            transcript_chunks: Arc::new(Mutex::new(Vec::new())),
            background_transcribing: Arc::new(Mutex::new(false)),
            session_id: Arc::new(AtomicU64::new(0)),
        }
    }

    pub fn status(&self) -> DictationStatus {
        self.status.lock().expect("status mutex poisoned").clone()
    }

    pub fn is_listening(&self) -> bool {
        let status = self.status.lock().expect("status mutex poisoned");
        status.phase == DictationPhase::Listening
    }

    pub fn is_processing(&self) -> bool {
        let status = self.status.lock().expect("status mutex poisoned");
        status.phase == DictationPhase::Processing
    }

    pub fn start(
        &self,
        app: &AppHandle,
        audio: &AudioService,
        settings: &Settings,
    ) -> anyhow::Result<()> {
        let current = self.status();
        if current.phase == DictationPhase::Listening || current.phase == DictationPhase::Processing
        {
            return Ok(());
        }

        emit_debug_log(
            app,
            "info",
            "dictation",
            format!("starting dictation with trigger mode {:?}", settings.trigger_mode),
        );
        self.set_status(app, DictationPhase::Listening, "Listening");
        self.session_id.fetch_add(1, Ordering::SeqCst);
        *self
            .transcript_chunks
            .lock()
            .expect("transcript chunk mutex poisoned") = Vec::new();
        *self
            .background_transcribing
            .lock()
            .expect("background transcribing mutex poisoned") = false;
        let app_handle = app.clone();
        let recording = audio.start_recording(
            RecordingConfig {
                microphone_id: settings.mic_device_id.clone(),
                silence_timeout_ms: if settings.trigger_mode == TriggerMode::Toggle {
                    settings.silence_timeout_ms.max(5_000)
                } else {
                    settings.silence_timeout_ms
                },
            },
            move |level, waveform| {
                let _ = app_handle.emit("dictation://level", level);
                let _ = app_handle.emit("dictation://waveform", waveform);
            },
        )?;

        *self.recording.lock().expect("recording mutex poisoned") = Some(recording);
        Ok(())
    }

    pub fn has_heard_speech(&self) -> bool {
        self.recording
            .lock()
            .expect("recording mutex poisoned")
            .as_ref()
            .is_some_and(|s| s.has_heard_speech())
    }

    pub fn speaking_now(&self) -> bool {
        self.recording
            .lock()
            .expect("recording mutex poisoned")
            .as_ref()
            .is_some_and(|s| s.speaking_now())
    }

    pub fn silence_elapsed_ms(&self) -> u64 {
        self.recording
            .lock()
            .expect("recording mutex poisoned")
            .as_ref()
            .map(|s| s.silence_elapsed_ms())
            .unwrap_or_default()
    }

    pub fn take_silent_background_segment(&self, min_duration_ms: u64) -> Option<RecordedAudio> {
        self.recording
            .lock()
            .expect("recording mutex poisoned")
            .as_ref()
            .and_then(|s| s.take_silent_segment(min_duration_ms))
    }

    pub fn take_elapsed_background_segment(&self, min_duration_ms: u64) -> Option<RecordedAudio> {
        self.recording
            .lock()
            .expect("recording mutex poisoned")
            .as_ref()
            .and_then(|s| s.take_elapsed_segment(min_duration_ms))
    }

    pub fn begin_background_transcription(&self) -> bool {
        let mut guard = self
            .background_transcribing
            .lock()
            .expect("background transcribing mutex poisoned");
        if *guard {
            return false;
        }
        *guard = true;
        true
    }

    pub fn active_session_id(&self) -> u64 {
        self.session_id.load(Ordering::SeqCst)
    }

    pub fn finish_background_transcription(&self) {
        *self
            .background_transcribing
            .lock()
            .expect("background transcribing mutex poisoned") = false;
    }

    pub fn append_background_transcript(&self, text: &str) {
        if text.trim().is_empty() {
            return;
        }
        let mut guard = self
            .transcript_chunks
            .lock()
            .expect("transcript chunk mutex poisoned");
        guard.push(text.trim().to_string());
    }

    pub fn append_background_transcript_for_session(&self, session_id: u64, text: &str) {
        if session_id != self.active_session_id() {
            return;
        }
        self.append_background_transcript(text);
    }

    pub fn transcript_chunks(&self) -> Vec<String> {
        self.transcript_chunks
            .lock()
            .expect("transcript chunk mutex poisoned")
            .clone()
    }

    pub async fn stop_and_process(
        &self,
        app: &AppHandle,
        settings: &Settings,
        model_manager: &ModelManager,
        transcription: &TranscriptionService,
        insertion: &InsertionService,
        history: &HistoryStore,
    ) -> anyhow::Result<()> {
        let session = self
            .recording
            .lock()
            .expect("recording mutex poisoned")
            .take()
            .ok_or_else(|| anyhow!("no active recording"))?;

        self.set_status(app, DictationPhase::Processing, "Transcribing");
        self.wait_for_background_flush(Duration::from_millis(BACKGROUND_FLUSH_MAX_WAIT_MS))
            .await;

        let mut chunks = self.transcript_chunks();
        let recorded = session.stop();
        emit_debug_log(
            app,
            "info",
            "dictation",
            format!(
                "recording stopped: duration={}ms peak={:.4} speech_ratio={:.4}",
                recorded.duration_ms, recorded.peak_level, recorded.speech_ratio
            ),
        );
        if !has_meaningful_audio(&recorded) && chunks.is_empty() {
            emit_debug_log(app, "warn", "dictation", "discarded blank audio");
            self.set_status(app, DictationPhase::Done, "No speech detected");
            sleep(Duration::from_millis(800)).await;
            self.set_status(app, DictationPhase::Ready, "Ready");
            return Ok(());
        }

        let prepared = prepare_audio(recorded);
        let (model_id, model_path) = model_manager
            .active_model_path(settings.active_model_id.clone())
            .await?;
        emit_debug_log(
            app,
            "info",
            "transcription",
            format!("using model {}", model_id),
        );
        if has_meaningful_audio(&prepared) && !prepared.samples.is_empty() {
            match transcription.transcribe(model_path, prepared.samples).await {
                Ok(transcript) => {
                    if has_meaningful_text(&transcript) {
                        emit_debug_log(
                            app,
                            "info",
                            "transcription",
                            format!("final segment produced {} chars", transcript.trim().chars().count()),
                        );
                        chunks.push(transcript.trim().to_string());
                    }
                }
                Err(err) => {
                    // Keep already transcribed background text instead of failing the whole dictation.
                    emit_debug_log(
                        app,
                        "warn",
                        "transcription",
                        format!("final segment failed: {err:#}"),
                    );
                    eprintln!("final segment transcription failed: {err:#}");
                }
            }
        }

        let combined = merge_transcript_chunks(&chunks);
        emit_debug_log(
            app,
            "info",
            "transcription",
            format!("merged {} transcript chunks into {} chars", chunks.len(), combined.chars().count()),
        );
        let finalized = apply_transcript_rules(&combined, &settings.rules);
        if !has_meaningful_text(&finalized) {
            emit_debug_log(app, "warn", "transcription", "finalized transcript was empty");
            self.set_status(app, DictationPhase::Done, "No speech detected");
            sleep(Duration::from_millis(800)).await;
            self.set_status(app, DictationPhase::Ready, "Ready");
            return Ok(());
        }

        let insertion_result = match insertion.insert_text(&finalized, settings.insertion_mode.clone()) {
            Ok(result) => {
                emit_debug_log(
                    app,
                    "info",
                    "insertion",
                    format!(
                        "frontmost={} ({}) strategy={} inserted={} reason={}",
                        result.frontmost_app_name,
                        result.frontmost_app_bundle_id,
                        result.strategy_used,
                        result.inserted,
                        result
                            .failure_reason
                            .clone()
                            .unwrap_or_else(|| "none".to_string())
                    ),
                );
                result
            }
            Err(err) => {
                emit_debug_log(
                    app,
                    "error",
                    "insertion",
                    format!("insert_text failed before result capture: {err:#}"),
                );
                return Err(err);
            }
        };

        let entry = TranscriptEntry {
            id: Uuid::new_v4().to_string(),
            text: finalized.clone(),
            created_at: Utc::now().to_rfc3339(),
            model_id,
            duration_ms: prepared.duration_ms,
            inserted: insertion_result.inserted,
            insertion_strategy: insertion_result.transcript_strategy.clone(),
        };

        let updated_history = history.append(entry, &settings.transcript_retention)?;
        emit_debug_log(
            app,
            "info",
            "history",
            format!("saved transcript with {} chars", combined.chars().count()),
        );
        let _ = app.emit("history://updated", &updated_history);
        let _ = app.emit(
            "dictation://final-text",
            FinalTextEvent {
                text: finalized,
                inserted: insertion_result.inserted,
                insertion_strategy: insertion_result.transcript_strategy,
            },
        );

        if insertion_result.inserted {
            self.set_status(app, DictationPhase::Done, "Done");
        } else {
            self.set_status(app, DictationPhase::Done, "Copied");
        }
        sleep(Duration::from_millis(900)).await;
        self.set_status(app, DictationPhase::Ready, "Ready");
        Ok(())
    }

    pub fn set_error(&self, app: &AppHandle, message: impl Into<String>) {
        let message = message.into();
        emit_debug_log(app, "error", "dictation", message.clone());
        self.set_status(app, DictationPhase::Error, message);
    }

    pub fn reset_ready(&self, app: &AppHandle) {
        self.set_status(app, DictationPhase::Ready, "Ready");
    }

    pub fn finish_background_transcription_for_session(&self, session_id: u64) {
        if session_id != self.active_session_id() {
            return;
        }
        self.finish_background_transcription();
    }

    fn set_status(&self, app: &AppHandle, phase: DictationPhase, message: impl Into<String>) {
        let status = DictationStatus {
            phase,
            message: message.into(),
        };
        *self.status.lock().expect("status mutex poisoned") = status.clone();
        let _ = app.emit("dictation://state-changed", &status);
        emit_debug_log(
            app,
            "debug",
            "status",
            format!("phase={:?} message={}", status.phase, status.message),
        );
    }

    async fn wait_for_background_flush(&self, max_wait: Duration) {
        let started = Instant::now();
        loop {
            let active = *self
                .background_transcribing
                .lock()
                .expect("background transcribing mutex poisoned");
            if !active || started.elapsed() >= max_wait {
                break;
            }
            sleep(Duration::from_millis(45)).await;
        }
    }
}

fn prepare_audio(recorded: RecordedAudio) -> RecordedAudio {
    let samples = to_mono_16k(&recorded);
    let duration_ms = if samples.is_empty() {
        0
    } else {
        (samples.len() as u64 * 1000) / 16_000
    };
    RecordedAudio {
        samples,
        sample_rate: 16_000,
        channels: 1,
        duration_ms,
        speech_ratio: recorded.speech_ratio,
        peak_level: recorded.peak_level,
    }
}

fn has_meaningful_audio(recorded: &RecordedAudio) -> bool {
    recorded.duration_ms >= 350 && recorded.peak_level >= 0.018 && recorded.speech_ratio >= 0.025
}

fn has_meaningful_text(transcript: &str) -> bool {
    transcript.chars().any(|c| c.is_alphanumeric())
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

fn merge_transcript_chunks(chunks: &[String]) -> String {
    let mut merged = String::new();

    for chunk in chunks.iter().filter_map(|chunk| {
        let trimmed = chunk.trim();
        (!trimmed.is_empty()).then_some(trimmed)
    }) {
        if merged.is_empty() {
            merged.push_str(chunk);
            continue;
        }

        let deduped = dedupe_chunk_overlap(&merged, chunk);
        if !deduped.is_empty() {
            if !merged.ends_with(char::is_whitespace) {
                merged.push(' ');
            }
            merged.push_str(&deduped);
        }
    }

    merged.trim().to_string()
}

fn dedupe_chunk_overlap(existing: &str, incoming: &str) -> String {
    let existing_words = existing.split_whitespace().collect::<Vec<_>>();
    let incoming_words = incoming.split_whitespace().collect::<Vec<_>>();
    let max_overlap = existing_words.len().min(incoming_words.len()).min(8);

    for overlap in (1..=max_overlap).rev() {
        if existing_words[existing_words.len() - overlap..]
            .iter()
            .map(|word| normalize_merge_token(word))
            .eq(incoming_words[..overlap].iter().map(|word| normalize_merge_token(word)))
        {
            return incoming_words[overlap..].join(" ");
        }
    }

    incoming.trim().to_string()
}

fn normalize_merge_token(token: &str) -> String {
    token
        .trim_matches(|c: char| !c.is_alphanumeric())
        .to_ascii_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merge_transcript_chunks_dedupes_overlap_boundaries() {
        let merged = merge_transcript_chunks(&[
            "hello world this is".to_string(),
            "this is a test".to_string(),
            "a test of chunk merging".to_string(),
        ]);

        assert_eq!(merged, "hello world this is a test of chunk merging");
    }
}
