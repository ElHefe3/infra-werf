"""Disposable loopback fixture for remote application tunnels (F17/T035).

An SSH server that permits direct-tcpip only to localhost, plus a private
"dashboard" (HTTP + WebSocket echo) bound to 127.0.0.1. All keys synthetic.
"""
import asyncio, base64, csv, hashlib, json, os, re, subprocess, sys
from pathlib import Path
import asyncssh
from fixture_stats import publish as publish_stats

PAGE = """<!doctype html><html><head><meta charset="utf-8"><title>Fixture Dashboard</title>
<style>body{font-family:Segoe UI,system-ui,sans-serif;margin:0;background:#0f1a14;color:#e7f0ea}
header{padding:28px 36px;background:linear-gradient(120deg,#1d3a2b,#2f6b4d)}h1{margin:0;font-size:26px}
main{padding:28px 36px;display:grid;grid-template-columns:repeat(auto-fit,minmax(220px,1fr));gap:16px}
.card{background:#16261d;border:1px solid #2a4a38;border-radius:12px;padding:18px}.card b{display:block;font-size:30px;margin-top:8px}
.ok{color:#7ee2a8}.bad{color:#ff8f8f}small{color:#8fb39f}</style></head>
<body><header><h1>Private dashboard</h1><small>Listening only on the server's localhost</small></header>
<main><div class="card">HTTP API<b id="api" class="bad">waiting</b></div>
<div class="card">WebSocket<b id="ws" class="bad">waiting</b></div>
<div class="card">Workers<b>4 healthy</b></div><div class="card">Queue depth<b>12</b></div></main>
<script>
fetch('/api/ping').then(r=>r.json()).then(j=>{const e=document.getElementById('api');e.textContent=j.status;e.className='ok';});
const s=new WebSocket('ws://'+location.host+'/ws');s.onopen=()=>s.send('ping');
s.onmessage=m=>{const e=document.getElementById('ws');e.textContent=m.data;e.className='ok';};
</script></body></html>"""

async def http(reader, writer, stats, page=None):
    try:
        head = await asyncio.wait_for(reader.readuntil(b"\r\n\r\n"), 10)
        lines = head.decode("latin-1").split("\r\n")
        path = lines[0].split(" ")[1] if len(lines[0].split(" ")) > 1 else "/"
        headers = {k.lower(): v.strip() for k, _, v in (l.partition(":") for l in lines[1:] if l)}
        stats["http"] += 1
        if path == "/ws" and headers.get("upgrade", "").lower() == "websocket":
            accept = base64.b64encode(hashlib.sha1((headers["sec-websocket-key"] + "258EAFA5-E914-47DA-95CA-C5AB0DC85B11").encode()).digest()).decode()
            writer.write(f"HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: {accept}\r\n\r\n".encode()); await writer.drain()
            b1, b2 = await reader.readexactly(2); size = b2 & 0x7F
            if size == 126: size = int.from_bytes(await reader.readexactly(2), "big")
            mask = await reader.readexactly(4) if b2 & 0x80 else b"\0\0\0\0"
            data = bytes(c ^ mask[i % 4] for i, c in enumerate(await reader.readexactly(size)))
            reply = b"pong" if data == b"ping" else data
            writer.write(bytes([0x81, len(reply)]) + reply); await writer.drain(); stats["websocket"] += 1
            await asyncio.sleep(0.5)
        else:
            body, kind = (json.dumps({"status": "online"}).encode(), "application/json") if path == "/api/ping" else ((page or PAGE).encode(), "text/html; charset=utf-8")
            writer.write(f"HTTP/1.1 200 OK\r\nContent-Type: {kind}\r\nContent-Length: {len(body)}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n".encode() + body)
            await writer.drain()
    except Exception:
        pass
    finally:
        writer.close()

async def main(directory):
    directory.mkdir(parents=True)
    identity = asyncssh.generate_private_key("ssh-ed25519"); host_key = asyncssh.generate_private_key("ssh-ed25519")
    key = directory / "identity"; key.write_bytes(identity.export_private_key("openssh"))
    if os.name == "nt":
        token = subprocess.run(["whoami", "/user", "/fo", "csv", "/nh"], capture_output=True, text=True, check=True, timeout=10)
        sid = next(csv.reader(token.stdout.strip().splitlines()))[1]
        if not re.fullmatch(r"S-1-\d+(?:-\d+)+", sid): raise RuntimeError("Invalid fixture owner")
        subprocess.run(["icacls", str(key), "/inheritance:r", "/grant:r", "*" + sid + ":F"], capture_output=True, check=True, timeout=10)
    else:
        key.chmod(0o600)
    stats = {"connections": 0, "active": 0, "forwards": [], "refused_forwards": 0, "http": 0, "websocket": 0, "discoveries": 0}
    def save():
        publish_stats(directory, stats)
    app = await asyncio.start_server(lambda r, w: http(r, w, stats), "127.0.0.1", 0)
    app_port = app.sockets[0].getsockname()[1]
    second_page = "<!doctype html><html><head><title>Second app</title></head><body><h1>Second app on 127.0.0.2</h1></body></html>"
    second = await asyncio.start_server(lambda r, w: http(r, w, stats, second_page), "127.0.0.2", 0)
    second_port = second.sockets[0].getsockname()[1]
    class Server(asyncssh.SSHServer):
        def connection_made(self, conn): stats["connections"] += 1; stats["active"] += 1; save()
        def connection_lost(self, exc): stats["active"] -= 1; save()
        def begin_auth(self, user): return True
        def public_key_auth_supported(self): return True
        def validate_public_key(self, user, key): return user == "fixture" and key.get_fingerprint() == identity.get_fingerprint()
        def connection_requested(self, dest_host, dest_port, orig_host, orig_port):
            # Permit only the server's own loopback, like a typical private app.
            if dest_host in ("localhost", "127.0.0.1", "::1", "127.0.0.2"):
                stats["forwards"].append([dest_host, dest_port]); save(); return True
            stats["refused_forwards"] += 1; save(); return False
        def server_requested(self, listen_host, listen_port):
            stats["refused_forwards"] += 1; save(); return False
    async def process(proc):
        command = proc.command or ""
        marker = re.fullmatch(r"echo (ATLAS_VERIFY_[a-f0-9]{32})", command)
        if marker: proc.stdout.write(marker[1] + "\n"); proc.exit(0)
        elif "ATLAS_APPS_V1" in command:
            # Canned read-only discovery reply describing this fixture.
            stats["discoveries"] += 1; save()
            proc.stdout.write("ATLAS_APPS_V1\n"
                f"APP\t127.0.0.1\t{app_port}\t200\t0\tpython\tFixture Dashboard\n"
                f"APP\t127.0.0.2\t{second_port}\t200\t0\tpython\tSecond app\n"
                "APP\t0.0.0.0\t22\t000\t0\tsshd\t\n"
                "APP\t*\t9443\t000\t1\t\t\n"
                "END\n")
            proc.exit(0)
        else: proc.exit(1)
    home = directory / "home"; home.mkdir()
    listener = await asyncssh.create_server(Server, "127.0.0.1", 0, server_host_keys=[host_key], process_factory=process, sftp_factory=lambda chan: asyncssh.SFTPServer(chan, chroot=str(home)))
    port = listener.get_port()
    known = directory / "known_hosts"; known.write_text(f"[127.0.0.1]:{port} {host_key.export_public_key().decode().strip()}\n")
    common = f"  HostName 127.0.0.1\n  Port {port}\n  User fixture\n  IdentityFile \"{key.as_posix()}\"\n  UserKnownHostsFile \"{known.as_posix()}\"\n  GlobalKnownHostsFile none\n"
    config = directory / "config"
    config.write_text(f"Host apps-node\n{common}Host apps-forwarding\n{common}  LocalForward 127.0.0.1:47999 localhost:22\n")
    save()
    print(json.dumps({"config": str(config), "stats": str(directory / "stats.json"), "app_port": app_port, "second_port": second_port}), flush=True)
    try:
        while True:
            line = (await asyncio.to_thread(sys.stdin.readline)).strip()
            if not line or line == "quit": break
    finally:
        listener.close(); await listener.wait_closed(); app.close(); second.close()

if __name__ == "__main__":
    asyncio.run(main(Path(sys.argv[1]).resolve()))
