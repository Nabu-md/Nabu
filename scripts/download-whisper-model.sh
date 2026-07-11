#!/bin/bash
#
# download-whisper-model.sh
#
# Downloads whisper.cpp model files for Nabu audio dictation.
# Usage: ./scripts/download-whisper-model.sh [base|large-v3-turbo-q5]
#
# Requirements: 41.1, 41.2, 41.6, 42.4, 42.5, 42.6

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# Model definitions
BASE_URL="https://huggingface.co/ggerganov/whisper.cpp/resolve/main"

declare -A MODELS=(
  ["base"]="ggml-base.en.bin"
  ["large-v3-turbo-q5"]="ggml-large-v3-turbo-q5_0.bin"
)

# Default to base model
MODEL="${1:-base}"

if [[ -z "${MODELS[$MODEL]}" ]]; then
  echo "Error: Unknown model '$MODEL'. Valid options: ${!MODELS[*]}"
  exit 1
fi

FILENAME="${MODELS[$MODEL]}"
URL="$BASE_URL/$FILENAME"

# Determine target directory
if [[ "$NODE_ENV" == "development" ]]; then
  TARGET_DIR="$PROJECT_ROOT/resources/whisper-models"
else
  TARGET_DIR="$HOME/.nabu/whisper-models"
fi

mkdir -p "$TARGET_DIR"
TARGET_PATH="$TARGET_DIR/$FILENAME"

echo "Downloading $MODEL model..."
echo "  URL: $URL"
echo "  Target: $TARGET_PATH"

if command -v curl >/dev/null 2>&1; then
  curl -L -o "$TARGET_PATH" "$URL"
elif command -v wget >/dev/null 2>&1; then
  wget -O "$TARGET_PATH" "$URL"
else
  echo "Error: Neither curl nor wget is available."
  exit 1
fi

echo "Download complete: $TARGET_PATH"
echo "File size: $(du -h "$TARGET_PATH" | cut -f1)"
