#!/bin/bash
# Generate the build-time Tailwind stylesheet, then serve the frontend with
# the Dioxus CLI (`cargo dioxus serve --platform web`).
#
# On first run this will compile dioxus-cli from source, so it can take a
# few minutes.  Once installed, subsequent invocations are fast.
set -e
npm run css:build

# Install dioxus-cli if it's not already available.
if ! command -v cargo-dioxus &>/dev/null; then
    cargo install dioxus-cli
fi

# `cargo dioxus serve` builds to wasm32-unknown-unknown, runs wasm-bindgen,
# and serves a dev server at http://localhost:8080.
cargo dioxus serve --platform web --port 8080
