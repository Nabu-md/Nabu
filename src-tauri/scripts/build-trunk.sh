#!/bin/bash
# Generate the build-time Tailwind stylesheet, then build the frontend for release.
set -e
npm run css:build
env -u NO_COLOR trunk build --config Trunk.toml --release
