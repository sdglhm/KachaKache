# KachaKache (macOS MVP)

KachaKache is a local-first dictation desktop app built with Tauri v2 + Rust backend + React/TypeScript frontend.

It is designed as a menu-bar friendly, privacy-first dictation flow:

- global shortcut starts/stops dictation
- microphone audio is captured locally
- transcription runs locally with open models (`ggml` Whisper models)
- final text is inserted into the active app via local automation
- settings, models, and transcript history stay on-device

## Stack

- Tauri v2
- Rust backend (commands + services)
- React + TypeScript frontend
- `cpal` for microphone capture
- `whisper.cpp` CLI for local inference orchestration

## Prerequisites (macOS)

1. Xcode Command Line Tools
```bash
xcode-select --install
```
2. Rust toolchain
```bash
curl https://sh.rustup.rs -sSf | sh
```
3. Node.js 22.x
4. `whisper.cpp` runtime source (for local dev/build packaging):
```bash
brew install whisper-cpp
```
KachaKache bundles `whisper-cli` + ggml runtime into the app during build (`npm run sync:whisper-runtime`).

## Run Locally

```bash
source ~/.nvm/nvm.sh && nvm use
npm install
npm run tauri dev
```

Create a bundled macOS app:
```bash
source ~/.nvm/nvm.sh && nvm use
npm run tauri build
```

Build output:
- `.app`: `src-tauri/target/release/bundle/macos/KachaKache.app`
- disk image/installer (when generated): `src-tauri/target/release/bundle/dmg/`

Verification helper (local):
```bash
npm run sync:whisper-runtime
DYLD_LIBRARY_PATH="$(pwd)/src-tauri/resources/whisper" \
GGML_BACKEND_PATH="$(pwd)/src-tauri/resources/whisper/libggml-cpu-apple_m1.so" \
./src-tauri/resources/whisper/whisper-cli -h
```

## MVP Features Implemented

- menu-bar tray with quick actions
- compact main settings window
- tiny overlay window (`Listening`, `Processing`, `Done`)
- global shortcut trigger with `Toggle` and `PushToTalk`
- default shortcut: `Cmd+L`
- microphone selection
- basic silence timeout auto-stop
- local model manager:
  - recommended `tiny.en`, `base.en`, `small.en`, `distil-large-v3` (Distil-Whisper GGML)
  - download with progress/cancel
  - detect installed models
  - set active model
  - delete local model
- local permissions section:
  - microphone
  - accessibility
  - request + open system settings links
- insertion strategies:
  - auto-paste (clipboard + Cmd+V)
  - typed fallback
- configurable transcript rules:
  - cleanup (repeated filler words, spacing, sentence capitalization, pause punctuation)
  - spoken punctuation (`comma`, `full stop`, `question mark`)
  - spoken formatting (`new line`, `new paragraph`, `bullet point`, `numbered list`, brackets)
  - self-corrections (`actually`, `delete that`, `scratch that`, `replace X with Y`)
- transcript history with local persistence and copy action

## Data Storage (Local Only)

App state is saved under the Tauri app data directory (macOS):

- `settings.json`
- `models_state.json`
- `history.json`
- `models/` downloaded model binaries

No cloud APIs, no remote transcription service, no accounts.

## Architecture

Backend service modules:

- `audio`: capture + silence detection + sample normalization
- `transcription`: whisper.cpp CLI orchestration
- `models`: manifest + download + active model + local model files
- `insertion`: paste/typing insertion strategies
- `permissions`: microphone/accessibility checks + settings deep links
- `settings`: local settings persistence
- `history`: local transcript persistence
- `dictation_controller`: recording/transcription/insertion state machine

## macOS Native Foundation

The desktop shell is now organized around macOS-first patterns:

- native app-wide menu bar (`KachaKache`, `File`, `Edit`, `View`, `Window`, `Help`)
- menu-bar / tray control for quick dictation access
- native template icons in the tray menu for key actions
- source-list style primary navigation instead of a dashboard layout
- toolbar-driven primary actions (`Start`, transcript actions, debug actions)
- transcript list/detail view instead of stacked cards
- native context menu support for transcript actions
- system-font typography and a restrained light/dark semantic color system

## Native Limitations and Workarounds

Some parts of the app are close approximations because Tauri/WebView is not AppKit or SwiftUI:

- `hiddenInset` title bar:
  Tauri v2 exposes `Visible`, `Transparent`, and `Overlay` title bar styles, but not AppKit's true `hiddenInset`.
  KachaKache uses `titleBarStyle: "Overlay"` with hidden title text and adjusted traffic-light placement as the closest available approximation.
- AppKit controls:
  Buttons, toggles, lists, and grouped settings are styled to match macOS conventions, but they are still HTML controls inside a WebView.
- Native settings window behavior:
  The app now uses a source-list/preferences-style structure, but it is still a single Tauri window rather than a dedicated SwiftUI/AppKit settings scene.
- Text insertion:
  Insertion relies on macOS accessibility automation and synthetic input. This is the most platform-sensitive part of the product and can still vary by target app.
- Overlay HUD:
  The floating dictation overlay is rendered in a transparent WebView window. It is visually aligned with macOS utility HUDs, but it is not an NSPanel/AppKit HUD.

If KachaKache ever needs a fully indistinguishable macOS shell, the next step would be:

- keep Rust dictation/transcription services
- replace the WebView shell with SwiftUI/AppKit for window chrome, settings scenes, tables/lists, and HUD presentation
- keep Tauri only for packaging/IPC or move to a native macOS app host entirely

## Known MVP Limitations

- dev/build currently expects `whisper-cpp` from Homebrew so runtime files can be copied into bundle resources.
- transcription is final-result oriented (no partial live transcript stream).
- insertion reliability depends on macOS accessibility/automation permissions and target app behavior.
- launch-at-login is currently a persisted placeholder toggle.
- model checksum verification is currently optional metadata only.
- no notarization/signing pipeline included yet.

## TODO (Post-MVP)

- stronger VAD/speech-end detection
- richer insertion fallbacks and recovery
- true launch-at-login integration
- packaged distribution, code-signing, notarization
- expanded model catalog and checksum enforcement
- optional realtime partial transcript UX
