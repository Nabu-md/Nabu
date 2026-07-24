#!/usr/bin/env bash
set -e

echo "Adding Rust target architectures..."
rustup target add x86_64-apple-darwin
rustup target add aarch64-apple-darwin

echo "Building Universal macOS DMG installer..."
cargo tauri build --target universal-apple-darwin --bundles dmg

echo "Done! Installer located in target/universal-apple-darwin/release/bundle/dmg/"
