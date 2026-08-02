#!/bin/bash
# Generate the build-time Tailwind stylesheet, watch it for live CSS reloads,
# then serve the frontend with trunk.
set -e
npm run css:build
npm run css:watch &
WATCH_PID=$!
# Kill the tailwind watcher (and any child it spawned) when trunk exits.
trap 'kill $WATCH_PID 2>/dev/null; pkill -P $WATCH_PID 2>/dev/null' EXIT
env -u NO_COLOR trunk serve --config Trunk.toml --port 8080
