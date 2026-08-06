#!/bin/bash
# Generate the build-time Tailwind stylesheet, then build the Dioxus frontend
# for release (output goes to ../dist for Tauri consumption).
set -e
npm run css:build
cargo dioxus build --platform web --release --out-dir ../dist
