#!/bin/bash
set -u
L=/home/ryo/lab.sh
OUT=/home/ryo/shots/$(date -u +%Y%m%dT%H%M%SZ)
ALBUM=MPREb_K8qWMWVqXGi
ARTIST=UCRr1xG_2WIDs18a6cIiCxeA
QUERY="Daft Punk Get Lucky"
mkdir -p "$OUT"

shot() { sleep "${2:-6}"; $L shot "$OUT/$1.png" && echo "$1"; }
ctl() { $L ctl "$1"; }
rpc() { python3 /home/ryo/rpc.py "$@"; }

$L stop; sleep 2
rm -rf /home/ryo/.local/share/ryotune-omni-daemon /home/ryo/.local/share/dev.excentry.ryotune-omni \
  /home/ryo/.cache/ryotune-omni-daemon /home/ryo/.cache/dev.excentry.ryotune-omni /home/ryo/.config/ryotune-omni
$L up
$L daemon; sleep 6

for theme in dark light; do
  pkill -x qs; sleep 1
  $L client
  shot "$theme-00-home-loading" 1.5
  sleep 8
  ctl "theme $theme"
  ctl "nav home"
  shot "$theme-01-home-no-track" 8
  ctl "nav search {\"q\":\"$QUERY\"}"
  shot "$theme-02-search"
  ctl "nav library {\"tab\":\"songs\"}"
  shot "$theme-03-library"
  ctl "nav album {\"id\":\"$ALBUM\",\"title\":\"Random Access Memories\"}"
  shot "$theme-04-album" 8
  ctl "nav artist {\"id\":\"$ARTIST\",\"title\":\"Daft Punk\"}"
  shot "$theme-05-artist" 8
  ctl "sidebar off"
  shot "$theme-06-rail-collapsed" 3
  ctl "sidebar on"
done

ctl "theme dark"
ctl "nav search {\"q\":\"$QUERY\"}"
sleep 4
python3 - "$QUERY" <<'EOF'
import json, subprocess, sys
items = json.loads(subprocess.check_output(["python3", "/home/ryo/rpc.py", "search", json.dumps({"query": sys.argv[1]})]))
subprocess.run(["python3", "/home/ryo/rpc.py", "play", json.dumps({"item": items[0]})], check=True, stdout=subprocess.DEVNULL)
EOF
shot "dark-07-playing" 15
rpc toggle_pause >/dev/null
shot "dark-08-paused" 3
rpc toggle_pause >/dev/null
ctl "panel on"
shot "dark-09-panel-queue" 4
ctl "panel off"
ctl "np queue"
shot "dark-10-nowplaying-queue" 5
ctl "np lyrics"
shot "dark-11-nowplaying-lyrics" 6
ctl "np off"
ctl "theme light"
ctl "np queue"
shot "light-10-nowplaying-queue" 5
ctl "np off"
ctl "theme dark"
rpc get_playback > "$OUT/playback.json"
echo "$OUT"
