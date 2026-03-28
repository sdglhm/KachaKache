use crate::types::{PermissionKind, PermissionResult, PermissionsStatus};
use anyhow::Context;
use std::process::Command;

#[derive(Debug, Clone)]
pub struct PermissionsService;

impl PermissionsService {
    pub fn new() -> Self {
        Self
    }

    pub fn status(&self) -> PermissionsStatus {
        PermissionsStatus {
            microphone_granted: check_microphone_permission(),
            accessibility_granted: check_accessibility_permission(),
        }
    }

    pub fn request(&self, kind: PermissionKind) -> PermissionResult {
        match kind {
            PermissionKind::Microphone => {
                let _ = request_microphone_permission();
                PermissionResult {
                    kind,
                    granted: check_microphone_permission(),
                }
            }
            PermissionKind::Accessibility => {
                let _ = request_accessibility_permission();
                PermissionResult {
                    kind,
                    granted: check_accessibility_permission(),
                }
            }
        }
    }

    pub fn open_settings(&self, kind: PermissionKind) -> anyhow::Result<()> {
        let url = match kind {
            PermissionKind::Microphone => {
                "x-apple.systempreferences:com.apple.preference.security?Privacy_Microphone"
            }
            PermissionKind::Accessibility => {
                "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility"
            }
        };

        Command::new("open")
            .arg(url)
            .status()
            .context("failed to open system settings")?;
        Ok(())
    }
}

#[cfg(target_os = "macos")]
fn check_accessibility_permission() -> bool {
    macos_accessibility_client::accessibility::application_is_trusted()
}

#[cfg(not(target_os = "macos"))]
fn check_accessibility_permission() -> bool {
    true
}

#[cfg(target_os = "macos")]
fn request_accessibility_permission() -> bool {
    macos_accessibility_client::accessibility::application_is_trusted_with_prompt()
}

#[cfg(not(target_os = "macos"))]
fn request_accessibility_permission() -> bool {
    true
}

#[cfg(target_os = "macos")]
fn check_microphone_permission() -> bool {
    use objc2::{class, msg_send};
    use objc2_foundation::NSString;

    // Status 3 maps to AVAuthorizationStatusAuthorized.
    unsafe {
        let av_media_type = NSString::from_str("soun");
        let status: i32 = msg_send![
            class!(AVCaptureDevice),
            authorizationStatusForMediaType: &*av_media_type
        ];
        status == 3
    }
}

#[cfg(not(target_os = "macos"))]
fn check_microphone_permission() -> bool {
    true
}

#[cfg(target_os = "macos")]
fn request_microphone_permission() -> anyhow::Result<()> {
    use objc2::{class, msg_send, runtime::Bool};
    use objc2_foundation::NSString;

    unsafe {
        let av_media_type = NSString::from_str("soun");
        type CompletionBlock = Option<extern "C" fn(Bool)>;
        let completion_block: CompletionBlock = None;
        let _: () = msg_send![
            class!(AVCaptureDevice),
            requestAccessForMediaType: &*av_media_type,
            completionHandler: completion_block
        ];
    }

    Ok(())
}

#[cfg(not(target_os = "macos"))]
fn request_microphone_permission() -> anyhow::Result<()> {
    Ok(())
}
