#!/bin/sh
# Re-records demo/at-tui-demo.{gif,mp4} with VHS against the mock server.
# Needs: cargo, python3, vhs (brew install vhs). Touches nothing outside
# a scratch HOME, so your real at-tui accounts are never used.
set -eu
cd "$(dirname "$0")/.."
PORT=${PORT:-47821}
cargo build --release -q
AT_TUI_BIN="$(cargo metadata --format-version 1 --no-deps | python3 -c 'import json,sys; print(json.load(sys.stdin)["target_directory"])')/release/at-tui"
DEMO_HOME="$(mktemp -d)"
export AT_TUI_BIN DEMO_HOME
python3 demo/mock_server.py "$PORT" & SERVER=$!
trap 'kill $SERVER 2>/dev/null; wait $SERVER 2>/dev/null; rm -rf "$DEMO_HOME"' EXIT
# The server draws its images before listening; wait until it answers.
until curl -sf "http://127.0.0.1:$PORT/xrpc/app.bsky.feed.getTimeline" >/dev/null; do sleep 0.2; done
for account in alt demo; do
  HOME="$DEMO_HOME" "$AT_TUI_BIN" --service "http://127.0.0.1:$PORT" login \
    --account "$account" --handle "$account.example" --app-password demo >/dev/null
done
vhs demo/demo.tape
