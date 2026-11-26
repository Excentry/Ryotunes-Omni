#!/bin/bash
set -u
export XDG_RUNTIME_DIR=/run/user/1000
export DBUS_SESSION_BUS_ADDRESS=unix:path=$XDG_RUNTIME_DIR/bus WAYLAND_DISPLAY=ryotest GDK_BACKEND=wayland
export QML_IMPORT_PATH=/home/ryo/.local/lib/qt6/qml QML2_IMPORT_PATH=/home/ryo/.local/lib/qt6/qml
SOCK=ryotest
CTL=$XDG_RUNTIME_DIR/ryotunes/ctl-lab.sock
DAEMON=/home/ryo/target/debug/ryotune-omni-daemon

case $1 in
  build)
    cd /home/ryo/src && cargo build --locked -p ryotunesd;;
  up)
    [ -S "$XDG_RUNTIME_DIR/bus" ] || dbus-daemon --session --address="$DBUS_SESSION_BUS_ADDRESS" --fork --nopidfile
    pulseaudio --check 2>/dev/null || pulseaudio --start --exit-idle-time=-1 --log-target=file:/tmp/pulse.log
    [ -S "$XDG_RUNTIME_DIR/$SOCK" ] && exit 0
    setsid weston --backend=headless --renderer=pixman --shell=kiosk --width=1376 --height=820 --socket=$SOCK --idle-time=0 --no-config --debug > /tmp/weston.log 2>&1 &
    for _ in $(seq 1 40); do [ -S "$XDG_RUNTIME_DIR/$SOCK" ] && break; sleep 0.25; done;;
  daemon)
    setsid "$DAEMON" > /tmp/daemon.log 2>&1 &;;
  client)
    mkdir -p "$XDG_RUNTIME_DIR/ryotunes"; rm -f "$CTL"
    RYOTUNES_WINDOW_TITLE=RyoTest-lab RYOTUNES_CTL=$CTL setsid qs -p /home/ryo/src/client > /tmp/client.log 2>&1 &;;
  stop)
    pkill -x qs
    pkill -f "^$DAEMON";;
  ctl)
    printf "%s\n" "$2" | socat -t 1 - "UNIX-CONNECT:$CTL";;
  shot)
    t=$(mktemp -d)
    (cd "$t" && weston-screenshooter >/dev/null 2>&1)
    f=$(ls "$t"/*.png 2>/dev/null | head -1)
    [ -n "$f" ] || { echo "no screenshot" >&2; exit 1; }
    python3 -c "from PIL import Image; Image.open('$f').convert('RGB').save('$2')"
    rm -rf "$t";;
  *)
    echo "usage: lab.sh build|up|daemon|client|stop|ctl CMD|shot OUT.png" >&2; exit 2;;
esac
