import json
import socket
import sys

path = "/run/user/1000/ryotune-omni/daemon.sock"
method = sys.argv[1]
params = json.loads(sys.argv[2]) if len(sys.argv) > 2 else None

with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as s:
    s.settimeout(60)
    s.connect(path)
    s.sendall((json.dumps({"id": 1, "method": method, "params": params}) + "\n").encode())
    buf = b""
    while True:
        chunk = s.recv(65536)
        if not chunk:
            sys.exit("daemon closed the connection")
        buf += chunk
        while b"\n" in buf:
            line, buf = buf.split(b"\n", 1)
            msg = json.loads(line)
            if msg.get("id") == 1:
                print(json.dumps(msg.get("result", msg.get("error")), ensure_ascii=False))
                sys.exit(0)
