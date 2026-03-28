# KachaKache

KachaKache is a local-first macOS dictation app built with Tauri v2, Rust, React, and TypeScript.

It is designed to feel like a compact Mac utility:

- lives in the menu bar
- starts dictation from a global shortcut
- records microphone audio locally
- transcribes with downloadable open Whisper-compatible models
- inserts the final text into the currently focused app
- stores settings, models, and transcript history on-device only

No cloud transcription. No user accounts. No remote runtime dependency.

## What It Does

KachaKache is aimed at the same core workflow as tools like Wispr Flow, but it runs fully on your Mac:

1. Press the global shortcut or start from the tray menu.
2. Speak into your selected microphone.
3. KachaKache transcribes locally using a downloaded model.
4. The final text is inserted into the active app using macOS accessibility automation.

The app includes:

- a compact main window
- a small recording overlay
- a first-run setup assistant
- local model download and management
- transcript history with retention controls
- rules for cleanup, punctuation, formatting, and simple self-corrections

## Current Product Status

This repository contains a production-minded MVP for macOS.

Implemented today:

- macOS-only desktop app with Tauri v2
- Rust backend for audio, shortcuts, transcription orchestration, insertion, settings, and history
- local model manager with recommended starter models
- native-feeling tray/menu-bar flow
- first-run setup assistant for permissions + model setup + practice phrase
- transcript history with per-entry delete and retention options
- GitHub Actions workflow to build a macOS `.app` and `.dmg`

Still MVP-scoped:

- unsigned / unnotarized distribution by default
- final-result dictation UX rather than full live streaming transcript UI
- insertion reliability depends on macOS accessibility behavior and the target app

## Stack

- Tauri v2
- Rust
- React 19
- TypeScript
- Vite
- Tailwind CSS
- `cpal` for microphone capture
- bundled `whisper.cpp` runtime for local transcription

## Supported Platform

- macOS only
- Apple Silicon build path is the primary target today

## Core Features

### Dictation

- global shortcut to start and stop dictation
- tray/menu-bar quick action
- compact overlay while listening / processing
- microphone selection
- `Toggle` and `Push to Talk` modes
- silence timeout support

Default shortcut:

- `Cmd+L`

### Local Models

Recommended models currently include:

- `tiny.en`
- `base.en`
- `small.en`
- `distil-large-v3`

Model management supports:

- download with progress
- cancel download
- detect installed models
- activate a model
- delete a local model

### Text Insertion

KachaKache uses a smart automatic insertion pipeline:

1. typed insertion first
2. AppleScript typing retry
3. paste fallback
4. clipboard-only fallback if insertion cannot complete

Insertion attempts are logged in the Debug tab with:

- focused app
- chosen strategy
- success / failure reason

### Permissions

The app checks and guides the user through:

- Microphone access
- Accessibility access

### Transcript History

- recent transcripts stored locally
- copy individual transcripts
- delete individual transcripts
- clear all history
- configurable retention:
  - keep indefinitely
  - 3 months
  - 1 month
  - 2 weeks
  - 1 week

### Rules

Built-in local cleanup rules include:

- repeated filler word cleanup
- capitalization fixes
- pause to punctuation normalization
- spoken punctuation like `comma`, `full stop`, `question mark`
- spoken formatting like `new line`, `new paragraph`, `bullet point`
- simple correction patterns like `scratch that` and `replace X with Y`

## First-Run Experience

On first launch, KachaKache opens a dedicated setup assistant.

The setup flow walks through:

1. permissions
2. model selection and download
3. practice phrase and accuracy check

Setup can also be reopened later from the app.

## Repository Layout

```text
.
├── src/                     # React frontend
├── src-tauri/              # Rust backend + Tauri config
├── scripts/                # local build helpers
└── .github/workflows/      # GitHub Actions build workflow
```

Key backend service areas:

- `audio`
- `transcription`
- `models`
- `insertion`
- `permissions`
- `settings`
- `history`
- `dictation_controller`

## Local Data Storage

All app data stays local under the Tauri app data directory on macOS, typically under:

```text
~/Library/Application Support/com.sdglhm.kachakache/
```

Files stored there include:

- `settings.json`
- `models_state.json`
- `history.json`
- `models/`

No cloud sync or remote user data storage is used.

## Prerequisites

### 1. Xcode Command Line Tools

```bash
xcode-select --install
```

### 2. Rust

```bash
curl https://sh.rustup.rs -sSf | sh
```

### 3. Node.js 22

If you use `nvm`:

```bash
source ~/.nvm/nvm.sh
nvm use 22
```

### 4. Homebrew whisper runtime dependencies

KachaKache currently syncs the bundled local runtime from Homebrew-installed packages during development and build.

Install:

```bash
brew install whisper-cpp
```

The sync script also expects compatible `ggml` and `libomp` dependencies through Homebrew.

## Running Locally

Install dependencies:

```bash
source ~/.nvm/nvm.sh
nvm use 22
npm install
```

Start the app in development:

```bash
npm run tauri dev
```

## Building Locally

Build the desktop app:

```bash
source ~/.nvm/nvm.sh
nvm use 22
npm run tauri build
```

Build outputs:

- app bundle: `src-tauri/target/release/bundle/macos/KachaKache.app`
- DMG: `src-tauri/target/release/bundle/dmg/`

## Building on GitHub

The repository includes a GitHub Actions workflow:

- workflow: `Build macOS App`
- file: `.github/workflows/build-macos.yml`

It can be triggered in two ways:

1. manually from the GitHub Actions tab
2. automatically by pushing a version tag such as:

```bash
git tag v0.1.0
git push origin v0.1.0
```

The workflow:

- runs on `macos-14`
- installs Node 22 and Rust
- installs `whisper-cpp` with Homebrew
- builds the Tauri app bundle and DMG
- uploads two artifacts:
  - `KachaKache-app-macos`
  - `KachaKache-dmg-macos`

## Runtime Verification

To verify the bundled local whisper runtime on your machine:

```bash
npm run sync:whisper-runtime
DYLD_LIBRARY_PATH="$(pwd)/src-tauri/resources/whisper" \
GGML_BACKEND_PATH="$(pwd)/src-tauri/resources/whisper/libggml-cpu-apple_m1.so" \
./src-tauri/resources/whisper/whisper-cli -h
```

## Permissions Required

KachaKache needs:

- Microphone permission to capture speech
- Accessibility permission to insert text into other apps

Without Accessibility permission:

- dictation can still transcribe
- insertion may fail or fall back to clipboard-only behavior

## Debugging

The app includes a Debug area with:

- live internal logs
- overlay preview controls
- setup wizard reopen button
- log filters by level and scope
- log copy support
- insertion failure simulation

This is useful when diagnosing:

- shortcut registration issues
- permission problems
- model download failures
- insertion fallback behavior

## Native macOS Direction

KachaKache is intentionally designed around macOS conventions:

- tray/menu-bar control
- app-wide menu bar
- source-list style navigation
- compact utility window
- restrained toolbar styling
- dedicated setup and About windows

## Known Limitations

Current limitations worth knowing:

- the shell is still Tauri/WebView, so some controls are approximations of AppKit rather than true native AppKit views
- title bar behavior uses Tauri's `Overlay` style, which is close to macOS but not a full `hiddenInset` implementation
- insertion behavior can vary across target apps
- live streaming partial transcript UX is not the current focus
- launch at login is still a placeholder setting
- model checksum verification is not fully enforced yet
- builds are not signed or notarized by default

## Future Work

Likely next improvements:

- stronger VAD and speech-end detection
- faster progressive transcription feel
- even more robust insertion fallback logic
- signed and notarized release pipeline
- expanded model catalog and verification
- richer real-time transcription UI if needed

## License / Credits

KachaKache uses open local speech tooling and bundles a local `whisper.cpp` runtime for macOS builds.

For project-specific credits and bundled notices, use the in-app About window.
