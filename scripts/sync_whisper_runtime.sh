#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DEST_DIR="$ROOT_DIR/src-tauri/resources/whisper"

if ! command -v brew >/dev/null 2>&1; then
  echo "Homebrew is required to sync bundled whisper runtime."
  exit 1
fi

WHISPER_PREFIX="$(brew --prefix whisper-cpp 2>/dev/null || true)"
GGML_PREFIX="$(brew --prefix ggml 2>/dev/null || true)"
LIBOMP_PREFIX="$(brew --prefix libomp 2>/dev/null || true)"

if [[ -z "$WHISPER_PREFIX" || -z "$GGML_PREFIX" || -z "$LIBOMP_PREFIX" ]]; then
  echo "Missing required formulas. Install with:"
  echo "  brew install whisper-cpp"
  exit 1
fi

mkdir -p "$DEST_DIR"
rm -f "$DEST_DIR"/*

cp "$WHISPER_PREFIX/bin/whisper-cli" "$DEST_DIR/"
cp "$WHISPER_PREFIX/lib/libwhisper.1.dylib" "$DEST_DIR/"
cp "$GGML_PREFIX/lib/libggml.0.dylib" "$DEST_DIR/"
cp "$GGML_PREFIX/lib/libggml-base.0.dylib" "$DEST_DIR/"
cp "$LIBOMP_PREFIX/lib/libomp.dylib" "$DEST_DIR/"

OPTIONAL_BACKENDS=(
  "$GGML_PREFIX/libexec/libggml-blas.so"
  "$GGML_PREFIX/libexec/libggml-metal.so"
  "$GGML_PREFIX/libexec/libggml-cpu-apple_m1.so"
  "$GGML_PREFIX/libexec/libggml-cpu-apple_m2_m3.so"
  "$GGML_PREFIX/libexec/libggml-cpu-apple_m4.so"
)

for backend in "${OPTIONAL_BACKENDS[@]}"; do
  if [[ -f "$backend" ]]; then
    cp "$backend" "$DEST_DIR/"
  fi
done

if ! ls "$DEST_DIR"/libggml-cpu-apple_*.so >/dev/null 2>&1; then
  echo "No CPU ggml backend library found in $GGML_PREFIX/libexec"
  exit 1
fi

chmod u+w "$DEST_DIR"/*
chmod +x "$DEST_DIR/whisper-cli" "$DEST_DIR"/libggml-*.so 2>/dev/null || true

install_name_tool -change "@rpath/libwhisper.1.dylib" "@loader_path/libwhisper.1.dylib" "$DEST_DIR/whisper-cli"
install_name_tool -change "/opt/homebrew/opt/ggml/lib/libggml.0.dylib" "@loader_path/libggml.0.dylib" "$DEST_DIR/whisper-cli"
install_name_tool -change "/opt/homebrew/opt/ggml/lib/libggml-base.0.dylib" "@loader_path/libggml-base.0.dylib" "$DEST_DIR/whisper-cli"

install_name_tool -id "@loader_path/libwhisper.1.dylib" "$DEST_DIR/libwhisper.1.dylib"
install_name_tool -change "/opt/homebrew/opt/whisper-cpp/lib/libwhisper.1.dylib" "@loader_path/libwhisper.1.dylib" "$DEST_DIR/libwhisper.1.dylib"
install_name_tool -change "/opt/homebrew/opt/ggml/lib/libggml.0.dylib" "@loader_path/libggml.0.dylib" "$DEST_DIR/libwhisper.1.dylib"
install_name_tool -change "/opt/homebrew/opt/ggml/lib/libggml-base.0.dylib" "@loader_path/libggml-base.0.dylib" "$DEST_DIR/libwhisper.1.dylib"

install_name_tool -id "@loader_path/libggml.0.dylib" "$DEST_DIR/libggml.0.dylib"
install_name_tool -change "/opt/homebrew/opt/ggml/lib/libggml.0.dylib" "@loader_path/libggml.0.dylib" "$DEST_DIR/libggml.0.dylib" || true
install_name_tool -change "@rpath/libggml-base.0.dylib" "@loader_path/libggml-base.0.dylib" "$DEST_DIR/libggml.0.dylib"

install_name_tool -id "@loader_path/libggml-base.0.dylib" "$DEST_DIR/libggml-base.0.dylib"

for backend in "$DEST_DIR"/libggml-*.so; do
  install_name_tool -change "@rpath/libggml-base.0.dylib" "@loader_path/libggml-base.0.dylib" "$backend" || true
done

for cpu_backend in "$DEST_DIR"/libggml-cpu-*.so; do
  install_name_tool -change "/opt/homebrew/opt/libomp/lib/libomp.dylib" "@loader_path/libomp.dylib" "$cpu_backend" || true
done

for file in "$DEST_DIR"/*; do
  codesign --force --sign - "$file" >/dev/null 2>&1 || true
done

echo "Bundled whisper runtime synced to: $DEST_DIR"
