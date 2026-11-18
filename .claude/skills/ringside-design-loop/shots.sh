#!/usr/bin/env bash
# Screenshots of the Ringside browser demo (no Tauri, fallback data from src/lib/api.ts).
#
#   shots.sh <out-dir> <name>=<query> [<name>=<query> ...]
#
# Each view is shot at 980x700 (the app's minimum), 1440x900 and 2400x1300, at 2x scale:
#   shots.sh /tmp/ringside-shots/hud/r1 editor='?tab=hud' overlay='?view=overlay'
# Other sizes: SIZES="980,700 1920,1080" shots.sh …
# Whole page instead of the window: FULL=1 shots.sh …
# Pixel density (default 2): SHOT_SCALE=1 for 2560 and 3840 px wide windows.
# shot.mjs drives Chrome over DevTools and waits for fonts and painted frames; reduced motion
# keeps entry animations from being caught half-way.
# Writes <out-dir>/<name>-<width>.png. Starts its own Vite on a free port and stops it again.
set -euo pipefail

out=${1:?Ausgabeordner fehlt}
shift
[ $# -gt 0 ] || { echo "Mindestens eine Ansicht name='?query' angeben" >&2; exit 2; }
repo=$(git rev-parse --show-toplevel)
here=$(cd "$(dirname "$0")" && pwd)
chrome="/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"
mkdir -p "$out"

port=$(python3 -c 'import socket; s=socket.socket(); s.bind(("127.0.0.1",0)); print(s.getsockname()[1])')
(cd "$repo" && exec pnpm vite --port "$port" --strictPort >"$out/vite.log" 2>&1) &
vite=$!
trap 'kill $vite 2>/dev/null; pkill -P $vite 2>/dev/null || true' EXIT
for _ in $(seq 1 60); do
  curl -s -o /dev/null "http://localhost:$port/" && break
  sleep 0.5
done
# The first page a browser loads makes Vite optimize dependencies, regenerate SvelteKit's files and
# reload the page, which would end a shot half-way: load one page first, then wait until the log
# has shown no new reload for 5 s.
node "$here/shot.mjs" "$chrome" "http://localhost:$port/" 980 700 "$out/.warmup.png" >/dev/null 2>&1 || true
rm -f "$out/.warmup.png"
last=-1; quiet=0
for _ in $(seq 1 60); do
  n=$(grep -c "page reload" "$out/vite.log" || true)
  if [ "$n" = "$last" ]; then quiet=$((quiet + 1)); else quiet=0; last=$n; fi
  [ $quiet -ge 5 ] && break
  sleep 1
done

for view in "$@"; do
  name=${view%%=*}
  query=${view#*=}
  for size in ${SIZES:-980,700 1440,900 2400,1300}; do
    node "$here/shot.mjs" "$chrome" "http://localhost:$port/$query" "${size%%,*}" "${size##*,}" "$out/$name-${size%%,*}.png" ${FULL:+full}
    echo "$out/$name-${size%%,*}.png"
  done
done
