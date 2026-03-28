use crate::types::{InsertionMode, InsertionStrategy};
use anyhow::{anyhow, Context};
use std::io::Write;
use std::process::{Command, Output, Stdio};
use std::thread;
use std::time::Duration;

#[cfg(target_os = "macos")]
use core_graphics::event::{CGEvent, CGEventFlags, CGEventTapLocation, KeyCode};
#[cfg(target_os = "macos")]
use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};

const INSERTION_KEY_RELEASE_DELAY_MS: u64 = 360;
const INSERTION_RETRY_DELAY_MS: u64 = 60;
const CLIPBOARD_SETTLE_DELAY_MS: u64 = 70;
const PASTE_RETRY_ATTEMPTS: usize = 2;
const PASTE_RETRY_DELAY_MS: u64 = 90;
const TYPE_CHUNK_CHARS: usize = 80;
const V_KEYCODE: u16 = 9;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum InsertionAttempt {
    Typed,
    TypedOsascript,
    Paste,
    ClipboardOnly,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InsertionResult {
    pub inserted: bool,
    pub strategy_used: String,
    pub transcript_strategy: InsertionStrategy,
    pub frontmost_app_name: String,
    pub frontmost_app_bundle_id: String,
    pub failure_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FrontmostAppInfo {
    name: String,
    bundle_id: String,
}

#[derive(Debug, Clone)]
pub struct InsertionService;

impl InsertionService {
    pub fn new() -> Self {
        Self
    }

    pub fn insert_text(&self, text: &str, _mode: InsertionMode) -> anyhow::Result<InsertionResult> {
        let normalized = text.replace("\r\n", "\n");
        if normalized.trim().is_empty() {
            let frontmost = frontmost_app_info().unwrap_or_default();
            return Ok(InsertionResult {
                inserted: false,
                strategy_used: "clipboard_only".to_string(),
                transcript_strategy: InsertionStrategy::ClipboardOnly,
                frontmost_app_name: frontmost.name,
                frontmost_app_bundle_id: frontmost.bundle_id,
                failure_reason: Some("empty text cannot be inserted".to_string()),
            });
        }

        if !accessibility_trusted() {
            return Err(anyhow!(
                "Accessibility permission is required to insert text into other apps"
            ));
        }

        let frontmost = frontmost_app_info().unwrap_or_default();

        // Let global shortcut modifiers release before injecting text.
        thread::sleep(Duration::from_millis(INSERTION_KEY_RELEASE_DELAY_MS));

        let mut last_err: Option<anyhow::Error> = None;
        let mut attempt_notes: Vec<String> = Vec::new();
        for attempt in insertion_attempt_order() {
            let result = match attempt {
                InsertionAttempt::Typed => self.try_type_native_with_retry(&normalized),
                InsertionAttempt::TypedOsascript => self.try_type_osascript_with_retry(&normalized),
                InsertionAttempt::Paste => self.try_auto_paste(&normalized),
                InsertionAttempt::ClipboardOnly => self.copy_to_clipboard(&normalized),
            };

            match result {
                Ok(()) => {
                    return Ok(InsertionResult {
                        inserted: attempt != InsertionAttempt::ClipboardOnly,
                        strategy_used: insertion_attempt_name(attempt).to_string(),
                        transcript_strategy: InsertionStrategy::from(attempt),
                        frontmost_app_name: frontmost.name.clone(),
                        frontmost_app_bundle_id: frontmost.bundle_id.clone(),
                        failure_reason: (!attempt_notes.is_empty())
                            .then_some(attempt_notes.join(" | ")),
                    });
                }
                Err(err) => {
                    attempt_notes.push(format!(
                        "{} failed: {}",
                        insertion_attempt_name(attempt),
                        err
                    ));
                    last_err = Some(err);
                }
            }
        }

        let failure_reason = last_err
            .as_ref()
            .map(|err| err.to_string())
            .unwrap_or_else(|| "failed to insert text".to_string());

        Ok(InsertionResult {
            inserted: false,
            strategy_used: "clipboard_only".to_string(),
            transcript_strategy: InsertionStrategy::ClipboardOnly,
            frontmost_app_name: frontmost.name,
            frontmost_app_bundle_id: frontmost.bundle_id,
            failure_reason: Some(if attempt_notes.is_empty() {
                failure_reason
            } else {
                attempt_notes.join(" | ")
            }),
        })
    }

    pub fn copy_to_clipboard(&self, text: &str) -> anyhow::Result<()> {
        set_clipboard(text)
    }

    fn try_auto_paste(&self, text: &str) -> anyhow::Result<()> {
        let previous = read_clipboard().ok();
        set_clipboard(text)?;
        thread::sleep(Duration::from_millis(CLIPBOARD_SETTLE_DELAY_MS));
        trigger_cmd_v_with_retry()?;

        if let Some(old) = previous {
            thread::spawn(move || {
                thread::sleep(Duration::from_millis(250));
                let _ = set_clipboard(&old);
            });
        }

        Ok(())
    }

    fn type_text<F, G>(&self, text: &str, type_chunk: F, press_return: G) -> anyhow::Result<()>
    where
        F: Fn(&str) -> anyhow::Result<()>,
        G: Fn() -> anyhow::Result<()>,
    {
        let normalized = text.replace("\r\n", "\n");
        if normalized.trim().is_empty() {
            return Err(anyhow!("empty text cannot be typed"));
        }

        let lines: Vec<&str> = normalized.split('\n').collect();
        for (line_idx, line) in lines.iter().enumerate() {
            let chunks = chunk_by_chars(line, TYPE_CHUNK_CHARS);
            for chunk in chunks {
                if !chunk.is_empty() {
                    type_chunk(&chunk)?;
                    thread::sleep(Duration::from_millis(6));
                }
            }

            if line_idx < lines.len() - 1 {
                press_return()?;
                thread::sleep(Duration::from_millis(8));
            }
        }

        Ok(())
    }

    fn try_type_native_with_retry(&self, text: &str) -> anyhow::Result<()> {
        let mut last_err: Option<anyhow::Error> = None;
        for _ in 0..2 {
            match self.type_text(text, type_chunk_native, press_return_native) {
                Ok(()) => return Ok(()),
                Err(err) => {
                    last_err = Some(err);
                    thread::sleep(Duration::from_millis(INSERTION_RETRY_DELAY_MS));
                }
            }
        }
        Err(last_err.unwrap_or_else(|| anyhow!("native typing insertion failed")))
    }

    fn try_type_osascript_with_retry(&self, text: &str) -> anyhow::Result<()> {
        let mut last_err: Option<anyhow::Error> = None;
        for _ in 0..2 {
            match self.type_text(text, type_chunk_osascript, press_return_osascript) {
                Ok(()) => return Ok(()),
                Err(err) => {
                    last_err = Some(err);
                    thread::sleep(Duration::from_millis(INSERTION_RETRY_DELAY_MS));
                }
            }
        }
        Err(last_err.unwrap_or_else(|| anyhow!("AppleScript typing insertion failed")))
    }
}

fn set_clipboard(text: &str) -> anyhow::Result<()> {
    let mut child = Command::new("pbcopy")
        .stdin(Stdio::piped())
        .spawn()
        .context("failed to start pbcopy")?;
    let mut stdin = child.stdin.take().context("failed to open pbcopy stdin")?;
    stdin
        .write_all(text.as_bytes())
        .context("failed to write clipboard text")?;
    drop(stdin);
    let status = child.wait().context("failed waiting for pbcopy")?;
    if !status.success() {
        return Err(anyhow!("pbcopy exited with non-success status"));
    }
    Ok(())
}

fn read_clipboard() -> anyhow::Result<String> {
    let output = Command::new("pbpaste")
        .output()
        .context("failed to start pbpaste")?;
    if !output.status.success() {
        return Err(anyhow!("pbpaste exited with non-success status"));
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

fn trigger_cmd_v() -> anyhow::Result<()> {
    // Prefer System Events as primary path; it is slower but generally more reliable across apps.
    trigger_cmd_v_osascript().or_else(|osa_err| {
        trigger_cmd_v_native().map_err(|native_err| anyhow!("{osa_err}; {native_err}"))
    })
}

fn trigger_cmd_v_with_retry() -> anyhow::Result<()> {
    let mut last_err: Option<anyhow::Error> = None;
    for attempt in 0..PASTE_RETRY_ATTEMPTS {
        match trigger_cmd_v() {
            Ok(()) => return Ok(()),
            Err(err) => {
                last_err = Some(err);
                if attempt + 1 < PASTE_RETRY_ATTEMPTS {
                    thread::sleep(Duration::from_millis(PASTE_RETRY_DELAY_MS));
                }
            }
        }
    }

    Err(last_err.unwrap_or_else(|| anyhow!("failed to send paste shortcut")))
}

#[cfg(target_os = "macos")]
fn event_source() -> anyhow::Result<CGEventSource> {
    CGEventSource::new(CGEventSourceStateID::CombinedSessionState)
        .map_err(|_| anyhow!("failed to create CGEvent source"))
}

#[cfg(target_os = "macos")]
fn post_key_event(keycode: u16, flags: CGEventFlags) -> anyhow::Result<()> {
    let source = event_source()?;
    let key_down = CGEvent::new_keyboard_event(source.clone(), keycode, true)
        .map_err(|_| anyhow!("failed to create key-down event"))?;
    key_down.set_flags(flags);
    key_down.post(CGEventTapLocation::HID);

    let key_up = CGEvent::new_keyboard_event(source, keycode, false)
        .map_err(|_| anyhow!("failed to create key-up event"))?;
    key_up.set_flags(flags);
    key_up.post(CGEventTapLocation::HID);

    Ok(())
}

#[cfg(target_os = "macos")]
fn post_unicode_text(text: &str) -> anyhow::Result<()> {
    if text.is_empty() {
        return Ok(());
    }

    let source = event_source()?;
    let key_down = CGEvent::new_keyboard_event(source.clone(), 0, true)
        .map_err(|_| anyhow!("failed to create unicode key-down event"))?;
    key_down.set_string(text);
    key_down.post(CGEventTapLocation::HID);

    let key_up = CGEvent::new_keyboard_event(source, 0, false)
        .map_err(|_| anyhow!("failed to create unicode key-up event"))?;
    key_up.set_string(text);
    key_up.post(CGEventTapLocation::HID);

    Ok(())
}

#[cfg(target_os = "macos")]
fn trigger_cmd_v_native() -> anyhow::Result<()> {
    post_key_event(V_KEYCODE, CGEventFlags::CGEventFlagCommand)
        .context("native Cmd+V injection failed")
}

#[cfg(not(target_os = "macos"))]
fn trigger_cmd_v_native() -> anyhow::Result<()> {
    Err(anyhow!("native Cmd+V injection is only supported on macOS"))
}

fn trigger_cmd_v_osascript() -> anyhow::Result<()> {
    run_osascript(
        &["tell application \"System Events\" to keystroke \"v\" using command down"],
        &[],
        "AppleScript Cmd+V injection failed",
    )
}

#[cfg(target_os = "macos")]
fn type_chunk_native(text: &str) -> anyhow::Result<()> {
    post_unicode_text(text).context("native unicode typing failed")
}

#[cfg(not(target_os = "macos"))]
fn type_chunk_native(_text: &str) -> anyhow::Result<()> {
    Err(anyhow!("native typing is only supported on macOS"))
}

fn type_chunk_osascript(text: &str) -> anyhow::Result<()> {
    run_osascript(
        &[
            "on run argv",
            "tell application \"System Events\" to keystroke (item 1 of argv)",
            "end run",
        ],
        &[text],
        "AppleScript typing injection failed",
    )
}

#[cfg(target_os = "macos")]
fn press_return_native() -> anyhow::Result<()> {
    post_key_event(KeyCode::RETURN, CGEventFlags::CGEventFlagNull)
        .context("native return-key injection failed")
}

#[cfg(not(target_os = "macos"))]
fn press_return_native() -> anyhow::Result<()> {
    Err(anyhow!(
        "native return-key injection is only supported on macOS"
    ))
}

fn press_return_osascript() -> anyhow::Result<()> {
    run_osascript(
        &["tell application \"System Events\" to key code 36"],
        &[],
        "AppleScript return-key injection failed",
    )
}

fn run_osascript(lines: &[&str], args: &[&str], context: &str) -> anyhow::Result<()> {
    let mut command = Command::new("osascript");
    for line in lines {
        command.arg("-e").arg(line);
    }
    for arg in args {
        command.arg(arg);
    }

    let output = command
        .output()
        .with_context(|| format!("{context}: failed to launch osascript"))?;
    if output.status.success() {
        return Ok(());
    }

    Err(anyhow!(format_command_error(context, &output)))
}

fn format_command_error(context: &str, output: &Output) -> String {
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();

    if !stderr.is_empty() {
        format!("{context}: {stderr}")
    } else if !stdout.is_empty() {
        format!("{context}: {stdout}")
    } else {
        format!("{context}: command returned non-success status")
    }
}

#[derive(serde::Deserialize)]
struct FrontmostAppAppleScriptResult {
    name: String,
    bundle_id: String,
}

impl Default for FrontmostAppInfo {
    fn default() -> Self {
        Self {
            name: "Unknown".to_string(),
            bundle_id: "unknown".to_string(),
        }
    }
}

fn frontmost_app_info() -> anyhow::Result<FrontmostAppInfo> {
    let output = Command::new("osascript")
        .arg("-e")
        .arg(
            "tell application \"System Events\" to tell first application process whose frontmost is true to return \"{\\\"name\\\":\\\"\" & name & \"\\\",\\\"bundle_id\\\":\\\"\" & bundle identifier & \"\\\"}\"",
        )
        .output()
        .context("failed to inspect frontmost app")?;

    if !output.status.success() {
        return Err(anyhow!(format_command_error(
            "frontmost app lookup failed",
            &output
        )));
    }

    let raw = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let parsed: FrontmostAppAppleScriptResult =
        serde_json::from_str(&raw).context("failed to parse frontmost app info")?;

    Ok(FrontmostAppInfo {
        name: parsed.name,
        bundle_id: parsed.bundle_id,
    })
}

#[cfg(target_os = "macos")]
fn accessibility_trusted() -> bool {
    macos_accessibility_client::accessibility::application_is_trusted()
}

#[cfg(not(target_os = "macos"))]
fn accessibility_trusted() -> bool {
    true
}

fn chunk_by_chars(text: &str, chunk_size: usize) -> Vec<String> {
    if chunk_size == 0 {
        return vec![text.to_string()];
    }

    let mut out = Vec::new();
    let mut current = String::new();
    let mut count = 0;

    for ch in text.chars() {
        current.push(ch);
        count += 1;
        if count >= chunk_size {
            out.push(current);
            current = String::new();
            count = 0;
        }
    }

    if !current.is_empty() {
        out.push(current);
    }

    if out.is_empty() {
        out.push(String::new());
    }

    out
}

fn insertion_attempt_order() -> [InsertionAttempt; 4] {
    [
        InsertionAttempt::Typed,
        InsertionAttempt::TypedOsascript,
        InsertionAttempt::Paste,
        InsertionAttempt::ClipboardOnly,
    ]
}

fn insertion_attempt_name(value: InsertionAttempt) -> &'static str {
    match value {
        InsertionAttempt::Typed => "typed",
        InsertionAttempt::TypedOsascript => "typed_osascript",
        InsertionAttempt::Paste => "paste",
        InsertionAttempt::ClipboardOnly => "clipboard_only",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mode_selection_prefers_autopaste() {
        let mode = InsertionMode::Automatic;
        assert!(matches!(mode, InsertionMode::Automatic));
        assert_eq!(
            insertion_attempt_order(),
            [
                InsertionAttempt::Typed,
                InsertionAttempt::TypedOsascript,
                InsertionAttempt::Paste,
                InsertionAttempt::ClipboardOnly
            ]
        );
    }

    #[test]
    fn chunking_preserves_text() {
        let input = "hello world from kachakache";
        let chunks = chunk_by_chars(input, 5);
        assert_eq!(chunks.concat(), input);
    }

    #[test]
    fn chunking_handles_unicode_boundaries() {
        let input = "සිංහල dictation test";
        let chunks = chunk_by_chars(input, 4);
        assert_eq!(chunks.concat(), input);
    }

    #[test]
    fn command_error_prefers_stderr() {
        let output = Output {
            status: std::process::ExitStatus::from_raw(1 << 8),
            stdout: b"stdout msg".to_vec(),
            stderr: b"stderr msg".to_vec(),
        };
        assert_eq!(
            format_command_error("ctx", &output),
            "ctx: stderr msg".to_string()
        );
    }

    #[test]
    fn insertion_strategy_maps_typed_like_attempts() {
        assert_eq!(InsertionStrategy::from(InsertionAttempt::Typed), InsertionStrategy::Typed);
        assert_eq!(
            InsertionStrategy::from(InsertionAttempt::TypedOsascript),
            InsertionStrategy::Typed
        );
        assert_eq!(InsertionStrategy::from(InsertionAttempt::Paste), InsertionStrategy::Paste);
    }
}

#[cfg(test)]
#[cfg(unix)]
use std::os::unix::process::ExitStatusExt;

impl From<InsertionAttempt> for InsertionStrategy {
    fn from(value: InsertionAttempt) -> Self {
        match value {
            InsertionAttempt::Typed | InsertionAttempt::TypedOsascript => Self::Typed,
            InsertionAttempt::Paste => Self::Paste,
            InsertionAttempt::ClipboardOnly => Self::ClipboardOnly,
        }
    }
}
