use anyhow::{anyhow, Context};
use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

#[derive(Clone, Debug, Default)]
pub struct TranscriptionService {
    cached_binary: Arc<Mutex<Option<PathBuf>>>,
    backend_preference: Arc<Mutex<BackendPreference>>,
}

#[derive(Clone, Debug, Default)]
struct BackendPreference {
    initialized: bool,
    path: Option<PathBuf>,
}

impl TranscriptionService {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn ensure_runtime_available(&self) -> anyhow::Result<PathBuf> {
        if let Some(cached) = self
            .cached_binary
            .lock()
            .expect("cached binary mutex poisoned")
            .clone()
        {
            if cached.exists() {
                return Ok(cached);
            }
        }

        let found = find_whisper_binary().ok_or_else(|| {
            anyhow!(
                "whisper runtime not found. Run `npm run sync:whisper-runtime` or set KACHAKACHE_WHISPER_CLI"
            )
        })?;
        *self
            .cached_binary
            .lock()
            .expect("cached binary mutex poisoned") = Some(found.clone());
        Ok(found)
    }

    pub async fn transcribe(&self, model_path: PathBuf, audio: Vec<f32>) -> anyhow::Result<String> {
        let binary = self.ensure_runtime_available()?;
        let service = self.clone();

        tauri::async_runtime::spawn_blocking(move || {
            let work_dir = std::env::temp_dir().join(format!("kachakache-{}", Uuid::new_v4()));
            std::fs::create_dir_all(&work_dir)
                .context("failed to create temp transcription dir")?;
            let wav_path = work_dir.join("input.wav");
            let out_prefix = work_dir.join("transcript");
            let out_text = work_dir.join("transcript.txt");
            let runtime_dir = binary
                .parent()
                .map(PathBuf::from)
                .ok_or_else(|| anyhow!("invalid whisper-cli path"))?;
            let mut dyld_library_path = runtime_dir.to_string_lossy().to_string();
            if let Ok(existing) = env::var("DYLD_LIBRARY_PATH") {
                if !existing.trim().is_empty() {
                    dyld_library_path = format!("{dyld_library_path}:{existing}");
                }
            }
            let backend_candidates = service.resolve_backend_candidates(&runtime_dir);

            write_wav_16k_mono(&wav_path, &audio)?;

            let mut last_error = String::new();
            for backend_path in backend_candidates {
                let mut command = Command::new(&binary);
                command
                    .current_dir(&runtime_dir)
                    .env("DYLD_LIBRARY_PATH", &dyld_library_path);
                if let Some(path) = backend_path.as_ref() {
                    command.env("GGML_BACKEND_PATH", path);
                } else {
                    command.env_remove("GGML_BACKEND_PATH");
                }

                let args = whisper_cli_args(&model_path, &wav_path, &out_prefix);
                let output = command.args(&args).output().with_context(|| {
                    format!("failed to start whisper-cli at {}", binary.display())
                })?;

                if !output.status.success() {
                    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
                    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
                    last_error = if !stderr.is_empty() {
                        stderr
                    } else if !stdout.is_empty() {
                        stdout
                    } else {
                        "unknown whisper-cli error".to_string()
                    };
                    continue;
                }

                if !out_text.exists() {
                    last_error = "whisper-cli finished without transcript output".to_string();
                    continue;
                }

                let transcript = std::fs::read_to_string(&out_text)
                    .with_context(|| format!("failed to read {}", out_text.display()))?;
                let normalized = post_process_transcript(transcript.trim());
                let _ = std::fs::remove_file(&out_text);
                if normalized.is_empty() {
                    last_error = "no transcript detected".to_string();
                    continue;
                }

                service.remember_backend(backend_path.clone());
                let _ = std::fs::remove_dir_all(&work_dir);
                return Ok(normalized);
            }

            let _ = std::fs::remove_dir_all(&work_dir);
            if last_error.is_empty() {
                return Err(anyhow!("failed to transcribe audio with bundled runtime"));
            }
            Err(anyhow!("failed to transcribe audio: {last_error}"))
        })
        .await
        .context("transcription task join failed")?
    }

    fn resolve_backend_candidates(&self, runtime_dir: &Path) -> Vec<Option<PathBuf>> {
        let preference = self
            .backend_preference
            .lock()
            .expect("backend preference mutex poisoned")
            .clone();

        if preference.initialized {
            match preference.path {
                Some(path) if path.exists() => return vec![Some(path)],
                Some(_) => {
                    *self
                        .backend_preference
                        .lock()
                        .expect("backend preference mutex poisoned") = BackendPreference::default();
                }
                None => return vec![None],
            }
        }

        backend_candidates(runtime_dir)
    }

    fn remember_backend(&self, backend_path: Option<PathBuf>) {
        *self
            .backend_preference
            .lock()
            .expect("backend preference mutex poisoned") = BackendPreference {
            initialized: true,
            path: backend_path,
        };
    }
}

fn write_wav_16k_mono(path: &Path, samples: &[f32]) -> anyhow::Result<()> {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: 16_000,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer =
        hound::WavWriter::create(path, spec).context("failed to create temp wav file")?;
    for sample in samples {
        let sample_i16 = (sample.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
        writer
            .write_sample(sample_i16)
            .context("failed while writing wav sample")?;
    }
    writer.finalize().context("failed to finalize wav writer")?;
    Ok(())
}

fn find_whisper_binary() -> Option<PathBuf> {
    if let Ok(explicit) = std::env::var("KACHAKACHE_WHISPER_CLI") {
        let path = PathBuf::from(explicit);
        if path.exists() {
            return Some(path);
        }
    }

    for candidate in bundled_binary_candidates() {
        if candidate.exists() {
            return Some(candidate);
        }
    }

    let candidates = [
        "/opt/homebrew/bin/whisper-cli",
        "/usr/local/bin/whisper-cli",
        "/opt/homebrew/bin/main",
        "/usr/local/bin/main",
    ];
    for candidate in candidates {
        let path = PathBuf::from(candidate);
        if path.exists() {
            return Some(path);
        }
    }

    if let Ok(output) = Command::new("which").arg("whisper-cli").output() {
        if output.status.success() {
            let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !path.is_empty() {
                return Some(PathBuf::from(path));
            }
        }
    }

    None
}

fn bundled_binary_candidates() -> Vec<PathBuf> {
    let mut candidates = vec![];

    if let Ok(exe) = env::current_exe() {
        if let Some(exe_dir) = exe.parent() {
            candidates.push(exe_dir.join("whisper").join("whisper-cli"));
            candidates.push(
                exe_dir
                    .join("..")
                    .join("Resources")
                    .join("whisper")
                    .join("whisper-cli"),
            );
            candidates.push(
                exe_dir
                    .join("..")
                    .join("Resources")
                    .join("resources")
                    .join("whisper")
                    .join("whisper-cli"),
            );
        }
    }

    if let Ok(cwd) = env::current_dir() {
        candidates.push(
            cwd.join("src-tauri")
                .join("resources")
                .join("whisper")
                .join("whisper-cli"),
        );
        candidates.push(cwd.join("resources").join("whisper").join("whisper-cli"));
        candidates.push(cwd.join("whisper").join("whisper-cli"));
    }

    candidates
}

fn backend_candidates(runtime_dir: &Path) -> Vec<Option<PathBuf>> {
    let mut candidates = vec![];
    let preferred = [
        "libggml-cpu-apple_m4.so",
        "libggml-cpu-apple_m2_m3.so",
        "libggml-cpu-apple_m1.so",
        "libggml-blas.so",
    ];

    for name in preferred {
        let candidate = runtime_dir.join(name);
        if candidate.exists() {
            candidates.push(Some(candidate));
        }
    }

    candidates.push(None);
    candidates
}

fn whisper_cli_args(model_path: &Path, wav_path: &Path, out_prefix: &Path) -> Vec<String> {
    let mut args = vec![
        "-m".to_string(),
        model_path.display().to_string(),
        "-f".to_string(),
        wav_path.display().to_string(),
        "-l".to_string(),
        "en".to_string(),
        "-otxt".to_string(),
        "-of".to_string(),
        out_prefix.display().to_string(),
        "-nt".to_string(),
        "-np".to_string(),
        "-nf".to_string(),
    ];

    if let Ok(parallelism) = std::thread::available_parallelism() {
        let threads = parallelism.get().clamp(2, 8);
        args.push("-t".to_string());
        args.push(threads.to_string());
    }

    args
}

pub fn post_process_transcript(raw: &str) -> String {
    raw.replace("\r\n", "\n")
        .replace('\r', "\n")
        .trim()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn whisper_args_use_no_fallback_and_allow_gpu() {
        let args = whisper_cli_args(
            Path::new("/tmp/model.bin"),
            Path::new("/tmp/audio.wav"),
            Path::new("/tmp/out"),
        );
        assert!(args.iter().any(|arg| arg == "-nf"));
        assert!(!args.iter().any(|arg| arg == "-ng"));
    }

    #[test]
    fn post_process_transcript_normalizes_line_endings() {
        let input = "hello\r\nworld\r\n";
        assert_eq!(post_process_transcript(input), "hello\nworld");
    }

    #[test]
    fn remembers_backend_preference_after_success() {
        let service = TranscriptionService::new();
        let temp = std::env::temp_dir().join(format!("backend-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&temp).expect("create temp dir");
        let candidate = temp.join("libggml-cpu-apple_m1.so");
        std::fs::write(&candidate, b"stub").expect("create candidate");

        service.remember_backend(Some(candidate.clone()));
        let ordered = service.resolve_backend_candidates(&temp);
        assert_eq!(ordered, vec![Some(candidate)]);

        let _ = std::fs::remove_dir_all(temp);
    }
}
