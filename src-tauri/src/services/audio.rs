use crate::types::MicrophoneDevice;
use anyhow::{anyhow, Context};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, StreamConfig};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const SPEECH_THRESHOLD: f32 = 0.015;
const WAVEFORM_BINS: usize = 20;

#[derive(Debug, Clone)]
pub struct RecordingConfig {
    pub microphone_id: Option<String>,
    pub silence_timeout_ms: u64,
}

#[derive(Debug, Clone)]
pub struct RecordedAudio {
    pub samples: Vec<f32>,
    pub sample_rate: u32,
    pub channels: u16,
    pub duration_ms: u64,
    pub speech_ratio: f32,
    pub peak_level: f32,
}

#[derive(Debug)]
struct SharedCapture {
    samples: Vec<f32>,
    sample_rate: u32,
    channels: u16,
    silence_detector: SilenceDetector,
    latest_level: f32,
    peak_level: f32,
    voiced_samples: u64,
    total_samples: u64,
    consumed_samples: usize,
    waveform_floor: f32,
    waveform_peak: f32,
}

pub struct RecordingSession {
    stream: cpal::Stream,
    shared: Arc<Mutex<SharedCapture>>,
}

impl RecordingSession {
    pub fn stop(self) -> RecordedAudio {
        let _ = self.stream.pause();
        let guard = self.shared.lock().expect("capture mutex poisoned");
        let start = guard.consumed_samples.min(guard.samples.len());
        build_segment_recorded(&guard, start, guard.samples.len())
            .unwrap_or_else(|| empty_recorded(guard.sample_rate, guard.channels))
    }

    pub fn has_heard_speech(&self) -> bool {
        self.shared
            .lock()
            .expect("capture mutex poisoned")
            .silence_detector
            .heard_speech()
    }

    pub fn speaking_now(&self) -> bool {
        self.shared
            .lock()
            .expect("capture mutex poisoned")
            .latest_level
            >= SPEECH_THRESHOLD
    }

    pub fn silence_elapsed_ms(&self) -> u64 {
        let guard = self.shared.lock().expect("capture mutex poisoned");
        guard.silence_detector.silence_elapsed_ms(Instant::now())
    }

    pub fn take_silent_segment(&self, min_duration_ms: u64) -> Option<RecordedAudio> {
        let mut guard = self.shared.lock().expect("capture mutex poisoned");
        if !guard.silence_detector.is_silent() {
            return None;
        }
        take_elapsed_segment_locked(&mut guard, min_duration_ms)
    }

    pub fn take_elapsed_segment(&self, min_duration_ms: u64) -> Option<RecordedAudio> {
        let mut guard = self.shared.lock().expect("capture mutex poisoned");
        take_elapsed_segment_locked(&mut guard, min_duration_ms)
    }
}

fn take_elapsed_segment_locked(
    guard: &mut std::sync::MutexGuard<'_, SharedCapture>,
    min_duration_ms: u64,
) -> Option<RecordedAudio> {
    let start = guard.consumed_samples.min(guard.samples.len());
    let end = guard.samples.len();
    if end <= start {
        return None;
    }

    let sample_count = end - start;
    let duration_ms = duration_ms_for_samples(sample_count, guard.sample_rate, guard.channels);
    if duration_ms < min_duration_ms {
        return None;
    }

    let segment = build_segment_recorded(guard, start, end)?;
    guard.consumed_samples = end;
    Some(segment)
}

fn build_segment_recorded(
    guard: &SharedCapture,
    start: usize,
    end: usize,
) -> Option<RecordedAudio> {
    if end <= start || end > guard.samples.len() {
        return None;
    }

    let segment_samples = guard.samples[start..end].to_vec();
    let peak_level = segment_samples
        .iter()
        .map(|v| v.abs())
        .fold(0.0_f32, f32::max);
    let voiced_samples = segment_samples
        .iter()
        .filter(|sample| sample.abs() >= SPEECH_THRESHOLD)
        .count() as u64;
    let total_samples = segment_samples.len() as u64;
    let speech_ratio = if total_samples > 0 {
        voiced_samples as f32 / total_samples as f32
    } else {
        0.0
    };
    let duration_ms =
        duration_ms_for_samples(segment_samples.len(), guard.sample_rate, guard.channels);

    Some(RecordedAudio {
        samples: segment_samples,
        sample_rate: guard.sample_rate,
        channels: guard.channels,
        duration_ms,
        speech_ratio,
        peak_level,
    })
}

fn duration_ms_for_samples(sample_count: usize, sample_rate: u32, channels: u16) -> u64 {
    if sample_rate == 0 || channels == 0 {
        return 0;
    }
    (sample_count as u64 * 1000) / (sample_rate as u64 * channels as u64)
}

fn empty_recorded(sample_rate: u32, channels: u16) -> RecordedAudio {
    RecordedAudio {
        samples: vec![],
        sample_rate,
        channels,
        duration_ms: 0,
        speech_ratio: 0.0,
        peak_level: 0.0,
    }
}

#[derive(Debug, Clone)]
pub struct AudioService;

impl AudioService {
    pub fn new() -> Self {
        Self
    }

    pub fn list_microphones(&self) -> anyhow::Result<Vec<MicrophoneDevice>> {
        let host = cpal::default_host();
        let default_name = host
            .default_input_device()
            .map(|d| device_name(&d, 0))
            .unwrap_or_default();

        let devices = host.input_devices().context("failed to list microphones")?;
        let mut out = vec![];

        for (idx, device) in devices.enumerate() {
            let name = device_name(&device, idx);
            out.push(MicrophoneDevice {
                id: format!("mic:{}:{}", idx, name),
                is_default: name == default_name,
                name,
            });
        }

        Ok(out)
    }

    pub fn start_recording<F>(
        &self,
        config: RecordingConfig,
        level_cb: F,
    ) -> anyhow::Result<RecordingSession>
    where
        F: Fn(f32, Vec<f32>) + Send + Sync + 'static,
    {
        let host = cpal::default_host();
        let device = self.resolve_device(&host, config.microphone_id)?;
        let default_cfg = device
            .default_input_config()
            .context("failed to get mic config")?;
        let stream_config = StreamConfig {
            channels: default_cfg.channels(),
            sample_rate: default_cfg.sample_rate(),
            buffer_size: cpal::BufferSize::Default,
        };

        let silence_timeout = Duration::from_millis(config.silence_timeout_ms.max(300));
        let shared = Arc::new(Mutex::new(SharedCapture {
            samples: vec![],
            sample_rate: stream_config.sample_rate,
            channels: stream_config.channels,
            silence_detector: SilenceDetector::new(SPEECH_THRESHOLD, silence_timeout),
            latest_level: 0.0,
            peak_level: 0.0,
            voiced_samples: 0,
            total_samples: 0,
            consumed_samples: 0,
            waveform_floor: 0.01,
            waveform_peak: 0.08,
        }));

        let level_cb: Arc<dyn Fn(f32, Vec<f32>) + Send + Sync> = Arc::new(level_cb);
        let err_fn = |err| eprintln!("audio stream error: {err}");
        let sample_format = default_cfg.sample_format();
        let stream = match sample_format {
            SampleFormat::F32 => {
                let capture = shared.clone();
                let level_cb = level_cb.clone();
                device.build_input_stream(
                    &stream_config,
                    move |data: &[f32], _| process_input_f32(data, &capture, &level_cb),
                    err_fn,
                    None,
                )?
            }
            SampleFormat::I16 => {
                let capture = shared.clone();
                let level_cb = level_cb.clone();
                device.build_input_stream(
                    &stream_config,
                    move |data: &[i16], _| process_input_i16(data, &capture, &level_cb),
                    err_fn,
                    None,
                )?
            }
            SampleFormat::U16 => {
                let capture = shared.clone();
                let level_cb = level_cb.clone();
                device.build_input_stream(
                    &stream_config,
                    move |data: &[u16], _| process_input_u16(data, &capture, &level_cb),
                    err_fn,
                    None,
                )?
            }
            _ => return Err(anyhow!("unsupported microphone sample format")),
        };

        stream.play().context("failed to start microphone stream")?;

        Ok(RecordingSession { stream, shared })
    }

    fn resolve_device(
        &self,
        host: &cpal::Host,
        preferred_id: Option<String>,
    ) -> anyhow::Result<cpal::Device> {
        if let Some(id) = preferred_id {
            for (idx, device) in host
                .input_devices()
                .context("failed to list microphones")?
                .enumerate()
            {
                let name = device_name(&device, idx);
                let device_id = format!("mic:{}:{}", idx, name);
                if device_id == id {
                    return Ok(device);
                }
            }
        }

        host.default_input_device()
            .ok_or_else(|| anyhow!("no microphone found on this Mac"))
    }
}

#[allow(deprecated)]
fn device_name(device: &cpal::Device, idx: usize) -> String {
    device
        .name()
        .unwrap_or_else(|_| format!("Microphone {}", idx + 1))
}

fn process_input_f32(
    data: &[f32],
    shared: &Arc<Mutex<SharedCapture>>,
    level_cb: &Arc<dyn Fn(f32, Vec<f32>) + Send + Sync>,
) {
    let mut guard = shared.lock().expect("capture mutex poisoned");
    let level = rms(data);
    let raw_bars = waveform_bins(data, WAVEFORM_BINS);
    let bars = normalize_waveform(&mut guard, raw_bars);
    guard.samples.extend_from_slice(data);
    guard.latest_level = level;
    guard.peak_level = guard.peak_level.max(level);
    guard.total_samples += data.len() as u64;
    if level >= SPEECH_THRESHOLD {
        guard.voiced_samples += data.len() as u64;
    }
    guard.silence_detector.update(level, Instant::now());
    level_cb(level, bars);
}

fn process_input_i16(
    data: &[i16],
    shared: &Arc<Mutex<SharedCapture>>,
    level_cb: &Arc<dyn Fn(f32, Vec<f32>) + Send + Sync>,
) {
    let converted: Vec<f32> = data.iter().map(|v| *v as f32 / i16::MAX as f32).collect();
    process_input_f32(&converted, shared, level_cb);
}

fn process_input_u16(
    data: &[u16],
    shared: &Arc<Mutex<SharedCapture>>,
    level_cb: &Arc<dyn Fn(f32, Vec<f32>) + Send + Sync>,
) {
    let converted: Vec<f32> = data
        .iter()
        .map(|v| (*v as f32 - 32768.0) / 32768.0)
        .collect();
    process_input_f32(&converted, shared, level_cb);
}

fn rms(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    let energy = samples.iter().map(|v| v * v).sum::<f32>() / samples.len() as f32;
    energy.sqrt()
}

fn waveform_bins(samples: &[f32], bins: usize) -> Vec<f32> {
    if bins == 0 {
        return vec![];
    }
    if samples.is_empty() {
        return vec![0.0; bins];
    }

    let chunk = ((samples.len() as f32) / bins as f32).ceil() as usize;
    let chunk = chunk.max(1);
    let mut values = Vec::with_capacity(bins);

    for i in 0..bins {
        let start = i * chunk;
        if start >= samples.len() {
            values.push(0.0);
            continue;
        }
        let end = ((i + 1) * chunk).min(samples.len());
        let slice = &samples[start..end];
        let mean_square = slice.iter().map(|v| v * v).sum::<f32>() / slice.len() as f32;
        let rms = mean_square.sqrt();
        let peak = slice.iter().map(|v| v.abs()).fold(0.0_f32, f32::max);
        // Blend RMS and peak so normal speech creates clear movement while plosives still pop.
        values.push((rms * 0.65) + (peak * 0.35));
    }

    values
}

fn normalize_waveform(capture: &mut SharedCapture, raw: Vec<f32>) -> Vec<f32> {
    if raw.is_empty() {
        return raw;
    }

    let frame_peak = raw.iter().copied().fold(0.0_f32, f32::max);
    let frame_floor = raw.iter().copied().sum::<f32>() / raw.len() as f32;

    // Adaptive envelope: quick rise on louder speech, slow decay to keep visibility.
    if frame_peak > capture.waveform_peak {
        capture.waveform_peak = frame_peak;
    } else {
        capture.waveform_peak *= 0.986;
    }
    capture.waveform_peak = capture.waveform_peak.max(0.045);

    capture.waveform_floor = (capture.waveform_floor * 0.92) + (frame_floor * 0.08);
    capture.waveform_floor = capture.waveform_floor.clamp(0.002, 0.06);

    let denom = (capture.waveform_peak - capture.waveform_floor).max(0.02);
    raw.into_iter()
        .map(|v| {
            let normalized = ((v - (capture.waveform_floor * 0.85)) / denom).clamp(0.0, 1.0);
            // Compression curve to lift quieter syllables.
            let boosted = normalized.powf(0.58);
            if boosted < 0.03 {
                0.0
            } else {
                boosted
            }
        })
        .collect()
}

pub fn to_mono_16k(input: &RecordedAudio) -> Vec<f32> {
    let mono = if input.channels <= 1 {
        input.samples.clone()
    } else {
        let channels = input.channels as usize;
        input
            .samples
            .chunks(channels)
            .map(|frame| frame.iter().copied().sum::<f32>() / channels as f32)
            .collect::<Vec<_>>()
    };

    if input.sample_rate == 16_000 {
        return mono;
    }

    if mono.len() < 2 {
        return mono;
    }

    let ratio = input.sample_rate as f64 / 16_000_f64;
    let out_len = ((mono.len() as f64) / ratio).round() as usize;
    let mut out = Vec::with_capacity(out_len);

    for i in 0..out_len {
        let src_pos = i as f64 * ratio;
        let left = src_pos.floor() as usize;
        let right = (left + 1).min(mono.len() - 1);
        let frac = (src_pos - left as f64) as f32;
        let value = mono[left] * (1.0 - frac) + mono[right] * frac;
        out.push(value);
    }

    out
}

#[derive(Debug)]
struct SilenceDetector {
    threshold: f32,
    timeout: Duration,
    last_voice_at: Instant,
    silent: bool,
    heard_speech: bool,
}

impl SilenceDetector {
    fn new(threshold: f32, timeout: Duration) -> Self {
        Self {
            threshold,
            timeout,
            last_voice_at: Instant::now(),
            silent: false,
            heard_speech: false,
        }
    }

    fn update(&mut self, level: f32, now: Instant) {
        if level >= self.threshold {
            self.last_voice_at = now;
            self.heard_speech = true;
            self.silent = false;
            return;
        }

        if self.heard_speech && now.duration_since(self.last_voice_at) >= self.timeout {
            self.silent = true;
        } else {
            self.silent = false;
        }
    }

    fn is_silent(&self) -> bool {
        self.silent
    }

    fn heard_speech(&self) -> bool {
        self.heard_speech
    }

    fn silence_elapsed_ms(&self, now: Instant) -> u64 {
        if !self.heard_speech || !self.silent {
            return 0;
        }
        now.duration_since(self.last_voice_at).as_millis() as u64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_capture(samples: Vec<f32>, consumed_samples: usize) -> SharedCapture {
        SharedCapture {
            samples,
            sample_rate: 16_000,
            channels: 1,
            silence_detector: SilenceDetector::new(0.1, Duration::from_millis(500)),
            latest_level: 0.0,
            peak_level: 0.0,
            voiced_samples: 0,
            total_samples: 0,
            consumed_samples,
            waveform_floor: 0.01,
            waveform_peak: 0.08,
        }
    }

    #[test]
    fn silence_detector_triggers_after_timeout() {
        let mut detector = SilenceDetector::new(0.1, Duration::from_millis(500));
        let start = Instant::now();
        detector.update(0.2, start);
        detector.update(0.01, start + Duration::from_millis(200));
        assert!(!detector.is_silent());
        detector.update(0.01, start + Duration::from_millis(800));
        assert!(detector.is_silent());
    }

    #[test]
    fn silence_detector_does_not_trigger_before_any_speech() {
        let mut detector = SilenceDetector::new(0.1, Duration::from_millis(500));
        let start = Instant::now();
        detector.update(0.01, start + Duration::from_millis(1200));
        assert!(!detector.is_silent());
    }

    #[test]
    fn build_segment_recorded_uses_remaining_audio_stats() {
        let mut capture = sample_capture(vec![0.0; 16_000], 12_000);
        for sample in &mut capture.samples[12_000..16_000] {
            *sample = 0.05;
        }

        let segment =
            build_segment_recorded(&capture, capture.consumed_samples, capture.samples.len())
                .expect("segment expected");
        assert_eq!(segment.samples.len(), 4_000);
        assert_eq!(segment.duration_ms, 250);
        assert!(segment.peak_level >= 0.05);
        assert!(segment.speech_ratio > 0.9);
    }

    #[test]
    fn take_elapsed_segment_requires_min_duration() {
        let capture = sample_capture(vec![0.02; 10_000], 0);
        let mutex = std::sync::Mutex::new(capture);
        let mut guard = mutex.lock().expect("mutex lock");
        assert!(take_elapsed_segment_locked(&mut guard, 800).is_none());
        assert!(take_elapsed_segment_locked(&mut guard, 600).is_some());
        assert_eq!(guard.consumed_samples, 10_000);
    }
}
