# Self-hosting the Listen Together relay

The relay is an optional WebSocket service. It forwards playback and queue metadata; audio is streamed directly by each client. The relay keeps rooms in memory, so restarting it ends active rooms. It does not currently provide a health endpoint, graceful room draining, per-IP rate limits, or a global connection limit. Treat the examples below as a small self-hosted deployment, not as a managed or high-availability service.

## Build and install

Build the relay from the repository root:

```sh
cargo build --release --locked -p sync-server
```

Install `target/release/ryotune-omni-sync-server` at a stable path, such as `/usr/local/bin/ryotune-omni-sync-server`. Keep the binary owned by root and executable by the dedicated service identity. The Arch package also includes the relay binary, but does not install a service unit.

## systemd service

Create `/etc/systemd/system/ryotunes-sync.service`:

```ini
[Unit]
Description=Ryotune-Omni Listen Together relay
After=network.target

[Service]
Type=simple
DynamicUser=yes
ExecStart=/usr/local/bin/ryotune-omni-sync-server
Environment=RYOTUNES_SYNC_BIND=127.0.0.1:8080
Environment=RUST_LOG=info
Restart=on-failure
RestartSec=5s
TimeoutStopSec=15s
UMask=0077
NoNewPrivileges=yes
PrivateTmp=yes
ProtectHome=yes
ProtectSystem=strict
ProtectKernelTunables=yes
ProtectKernelModules=yes
ProtectControlGroups=yes
RestrictSUIDSGID=yes
LockPersonality=yes
MemoryDenyWriteExecute=yes
CapabilityBoundingSet=
RestrictAddressFamilies=AF_INET AF_INET6

[Install]
WantedBy=multi-user.target
```

Then load and start it:

```sh
sudo systemctl daemon-reload
sudo systemctl enable --now ryotunes-sync.service
systemctl status ryotunes-sync.service
journalctl -u ryotunes-sync.service
```

The service binds to loopback so clients cannot connect directly over the network. `RYOTUNES_SYNC_BIND` accepts a socket address. If running in a container or behind a proxy on a different host, bind to an address reachable from that proxy, for example `0.0.0.0:8080`, and restrict inbound access with the host firewall. `PORT` can choose the port when `RYOTUNES_SYNC_BIND` is unset, but the default bind address remains loopback.

There is no HTTP health or readiness route. Check process state and the listening socket with systemd and `ss -lntp`; an HTTP `curl` probe is not a valid relay check. Complete a connection from a Listen Together client to verify the WebSocket path end to end.

## TLS reverse proxy with Caddy

For a relay and Caddy on the same host, expose only Caddy's HTTPS listener and proxy WebSocket traffic to the loopback relay. A minimal Caddyfile is:

```caddyfile
listen.example.com {
    reverse_proxy 127.0.0.1:8080
}
```

Caddy obtains and renews the certificate when DNS points to the host and ports 80 and 443 are reachable. Configure the client endpoint as `wss://listen.example.com`. Do not expose the relay's plain `ws://` port to the public internet. If the proxy runs on another machine, use a private network or firewall rule to limit the relay port to that proxy.

Before opening a public endpoint, review [Security and privacy](../SECURITY.md). Room codes are not a substitute for network access controls. The relay has bounded room and message state, but does not cap incoming connections or enforce per-IP rate limits. Prefer a VPN or a tightly restricted network for private groups, and monitor service resource use.

## Operations and limitations

- Room and playback state is held in process memory; a service restart disconnects participants and removes rooms.
- systemd sends its normal stop signal, but the relay does not implement graceful shutdown or drain active rooms.
- The relay does not expose a health endpoint. Use service/process and socket checks, then verify with a real client.
- Keep the service and TLS proxy updated. Review logs with `journalctl`; do not publish raw logs without checking them for sensitive data.
- This repository does not provide a hosted deployment, connection-cap policy, or availability guarantee.
