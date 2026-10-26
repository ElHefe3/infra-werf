"""Loopback-only SSH fixture with synthetic keys; no OS accounts/services."""
import asyncio
import csv
import hashlib
import json
import os
import posixpath
from pathlib import Path
import re
import socket
import subprocess
import sys
import asyncssh
from ssh_agent_fixture import start_agent

async def serve(directory):
    directory.mkdir(parents=True, exist_ok=False)
    client = asyncssh.generate_private_key('ssh-ed25519')
    primary = asyncssh.generate_private_key('ssh-ed25519')
    changed = asyncssh.generate_private_key('ssh-ed25519')
    ca = asyncssh.generate_private_key('ssh-ed25519')
    certificate = ca.generate_host_certificate(primary, 'fixture-host-certificate', principals=['127.0.0.1'])
    identity = directory / 'fixture-only-identity'
    identity.write_bytes(client.export_private_key('openssh'))
    if os.name == 'nt':
        # Use the current token's numeric SID, avoiding slow/domain-dependent
        # account-name lookup while granting only this fixture file's owner.
        token = subprocess.run(['whoami', '/user', '/fo', 'csv', '/nh'], check=True, capture_output=True, text=True, timeout=10)
        sid = next(csv.reader(token.stdout.strip().splitlines()))[1]
        if not re.fullmatch(r'S-1-\d+(?:-\d+)+', sid): raise RuntimeError('Cannot identify fixture key owner')
        subprocess.run(['icacls', str(identity), '/inheritance:r', '/grant:r', '*' + sid + ':F'], check=True, capture_output=True, timeout=10)
    else:
        identity.chmod(0o600)
    stats, connections, listeners, ports = {}, set(), [], {}
    agent, close_agent = await start_agent(directory, client)
    filesystem = directory / 'filesystem'
    home = filesystem / 'home' / 'fixture'
    home.mkdir(parents=True)
    (home / 'hello.txt').write_text('fixture-protected-content-never-read', encoding='utf-8')
    (home / 'space folder').mkdir()
    (home / 'space folder' / '日本語.txt').write_text('fixture unicode', encoding='utf-8')
    (home / 'large').mkdir()
    for number in range(350): (home / 'large' / f'entry-{number:04}.txt').write_text('fixture', encoding='utf-8')
    deep = home / 'deep'
    for _ in range(18): deep = deep / 'next'
    deep.mkdir(parents=True)
    for name in ['denied', 'stall', 'disconnect']: (home / name).mkdir()
    writable = home / 'write-area'
    writable.mkdir()
    for name in ['target', 'denied', 'nonempty']: (writable / name).mkdir()
    for name in ['source file.txt', 'existing.txt', '日本語.txt']: (writable / name).write_text('fixture preserve ' + name, encoding='utf-8')
    (writable / 'nonempty' / 'keep.txt').write_text('never recurse', encoding='utf-8')
    (writable / 'target' / 'keep.txt').write_text('symlink target survives', encoding='utf-8')
    virtual = {b'/home/fixture/write-area/odd-\xff-source': asyncssh.SFTPAttrs(permissions=0o100640, size=37, uid=1000, gid=1000), b'/home/fixture/write-area/folder-link': asyncssh.SFTPAttrs(permissions=0o120777, size=6, uid=1000, gid=1000)}
    link_targets = {b'/home/fixture/write-area/folder-link': b'/home/fixture/write-area/target'}
    virtual[b'/home/fixture/write-area/odd-\\xff-source'] = asyncssh.SFTPAttrs(permissions=0o100640, size=51, uid=1000, gid=1000)
    def save_stats():
        temporary = directory / 'stats-next.json'
        temporary.write_text(json.dumps(stats), encoding='utf-8')
        temporary.replace(directory / 'stats.json')
    for name in ['trusted', 'unknown', 'changed', 'rejected', 'agent-missing', 'slow', 'overflow', 'no-auth', 'certified', 'fallback', 'explicit-alias'] + (['agent'] if agent else []):
        stats[name] = {'auth_attempts': 0, 'authenticated': 0, 'commands': 0, 'active': 0, 'subsystems': 0, 'listings': 0, 'mutations': 0, 'file_reads': 0, 'file_writes': 0, 'max_chunk': 0}
        class Server(asyncssh.SSHServer):
            def __init__(self, name=name): self.name = name
            def connection_made(self, connection):
                self.connection = connection; connections.add(connection)
                stats[self.name]['active'] += 1; save_stats()
            def connection_lost(self, error):
                connections.discard(self.connection)
                stats[self.name]['active'] -= 1; save_stats()
            def begin_auth(self, username):
                stats[self.name]['auth_attempts'] += 1; save_stats(); return self.name != 'no-auth'
            def public_key_auth_supported(self): return True
            def validate_public_key(self, username, key):
                return username == 'fixture' and key.get_fingerprint() == client.get_fingerprint()
            def auth_completed(self):
                stats[self.name]['authenticated'] += 1; save_stats()
        async def process(process, name=name):
            stats[name]['commands'] += 1; save_stats()
            match = re.fullmatch(r'echo (ATLAS_VERIFY_[a-f0-9]{32})', process.command or '')
            if not match: process.exit(126); return
            if name == 'slow': await asyncio.sleep(60)
            if name == 'no-auth': process.stderr.write('Authenticated to spoofed-output using "publickey".\n')
            if name == 'overflow': process.stdout.write('synthetic-protected-output-' * 10000)
            else: process.stdout.write(match[1] + '\n')
            process.exit(0)
        class Files(asyncssh.SFTPServer):
            def __init__(self, channel, name=name):
                super().__init__(channel, chroot=str(filesystem))
                self.fixture_name = name
                self.fixture_channel = channel
                stats[name]['subsystems'] += 1; save_stats()
            async def realpath(self, path):
                if self.fixture_name == 'slow': await asyncio.sleep(60)
                if path in [b'', b'.']: return b'/home/fixture'
                if path == b'/home/fixture/folder-link': return b'/home/fixture/space folder'
                if path == b'/home/fixture/broken-link': raise asyncssh.SFTPNoSuchFile('fixture missing')
                if path in link_targets: return link_targets[path]
                return super().realpath(path)
            async def lstat(self, path):
                if self.transfer_mode() == 'hold-final-stat' and path.endswith(b'/cancel-final.bin') and any(writable.glob('.atlas-upload-*')):
                    (directory / 'hold-stat-ready').write_text('ready', encoding='utf-8')
                    while not (directory / 'release-stat').exists(): await asyncio.sleep(0.01)
                if path in virtual: return virtual[path]
                if path in [b'/home/fixture/folder-link', b'/home/fixture/broken-link']:
                    return asyncssh.SFTPAttrs(permissions=0o120777, size=12, uid=1000, gid=1000)
                if path in [b'/home/fixture/odd-\xff-name', b'/home/fixture/;$(echo injected).txt']:
                    return asyncssh.SFTPAttrs(permissions=0o100640, size=9007199254740993, uid=1000, gid=1000)
                try: path.decode('utf-8')
                except UnicodeDecodeError: raise asyncssh.SFTPNoSuchFile('fixture virtual path missing')
                return super().lstat(path)
            async def scandir(self, path):
                stats[self.fixture_name]['listings'] += 1; save_stats()
                if path == b'/home/fixture/denied': raise asyncssh.SFTPPermissionDenied('fixture denied')
                if path == b'/home/fixture/stall': await asyncio.sleep(60)
                if path == b'/home/fixture/disconnect':
                    self.fixture_channel.get_connection().close()
                    raise asyncssh.SFTPFailure('fixture disconnected')
                if self.fixture_name == 'overflow':
                    yield asyncssh.SFTPName(b'x' * 5000, attrs=asyncssh.SFTPAttrs(permissions=0o100644))
                    return
                async for entry in super().scandir(path): yield entry
                for filename, attrs in list(virtual.items()):
                    if posixpath.dirname(filename) == path: yield asyncssh.SFTPName(posixpath.basename(filename), attrs=attrs)
                if path == b'/home/fixture':
                    for name in [b'folder-link', b'broken-link', b'odd-\xff-name', b';$(echo injected).txt']:
                        yield asyncssh.SFTPName(name, attrs=await self.lstat(path + b'/' + name))
            def transfer_mode(self):
                try: return (directory / 'transfer-mode').read_text(encoding='utf-8').strip()
                except FileNotFoundError: return ''
            def open(self, path, pflags, attrs):
                if pflags & (asyncssh.FXF_WRITE | asyncssh.FXF_CREAT | asyncssh.FXF_TRUNC): self.check_change(path)
                if path in virtual: raise asyncssh.SFTPPermissionDenied('fixture virtual content unavailable')
                return super().open(path, pflags, attrs)
            async def read(self, file_obj, offset, size):
                stats[self.fixture_name]['file_reads'] += 1
                stats[self.fixture_name]['max_chunk'] = max(stats[self.fixture_name]['max_chunk'], size); save_stats()
                if self.transfer_mode() == 'slow': await asyncio.sleep(0.02)
                data = super().read(file_obj, offset, size)
                if data and self.transfer_mode() == 'corrupt-read': return bytes([data[0] ^ 1]) + data[1:]
                return data
            async def write(self, file_obj, offset, data):
                stats[self.fixture_name]['file_writes'] += 1
                stats[self.fixture_name]['max_chunk'] = max(stats[self.fixture_name]['max_chunk'], len(data)); save_stats()
                if self.transfer_mode() == 'slow': await asyncio.sleep(0.02)
                if self.transfer_mode() == 'disconnect-write':
                    self.fixture_channel.get_connection().close(); raise asyncssh.SFTPFailure('fixture transfer interruption')
                return super().write(file_obj, offset, data)
            def check_change(self, *paths):
                for path in paths:
                    normalized = posixpath.normpath(path)
                    if not normalized.startswith(b'/home/fixture/write-area/') or b'/denied/' in normalized or normalized.endswith(b'/denied'):
                        raise asyncssh.SFTPPermissionDenied('fixture write boundary')
                stats[self.fixture_name]['mutations'] += 1; save_stats()
            async def mkdir(self, path, attrs):
                self.check_change(path)
                super().mkdir(path, attrs)
                if path.endswith(b'/apply-then-stall'): await asyncio.sleep(60)
                if path.endswith(b'/slow-confirmed'): await asyncio.sleep(0.8)
                if path.endswith(b'/apply-then-disconnect'): self.fixture_channel.get_connection().close()
            async def rename(self, oldpath, newpath):
                self.check_change(oldpath, newpath)
                if newpath.endswith(b'/race-target.txt'):
                    Path(os.fsdecode(self.map_path(newpath))).write_text('concurrent target preserved', encoding='utf-8')
                try: real_destination_exists = os.path.lexists(self.map_path(newpath))
                except UnicodeDecodeError: real_destination_exists = False
                if newpath in virtual or real_destination_exists: raise asyncssh.SFTPFileAlreadyExists('fixture collision')
                if oldpath in virtual:
                    virtual[newpath] = virtual.pop(oldpath)
                    if oldpath in link_targets: link_targets[newpath] = link_targets.pop(oldpath)
                    return
                super().rename(oldpath, newpath)
                if self.transfer_mode() == 'publish-lost-ack': self.fixture_channel.get_connection().close()
            def remove(self, path):
                self.check_change(path)
                if self.transfer_mode() == 'disconnect-cleanup':
                    self.fixture_channel.get_connection().close(); raise asyncssh.SFTPFailure('fixture cleanup response lost')
                if path in virtual:
                    del virtual[path]; link_targets.pop(path, None); return
                super().remove(path)
            def rmdir(self, path):
                self.check_change(path); super().rmdir(path)
        listener = await asyncssh.create_server(Server, '127.0.0.1', 0, server_host_keys=[changed if name == 'changed' else primary], server_host_certs=[certificate] if name == 'certified' else [], process_factory=process, sftp_factory=Files, encoding='utf-8')
        listeners.append(listener); ports[name] = listener.get_port()
    with socket.socket() as unused:
        unused.bind(('127.0.0.1', 0)); ports['unreachable'] = unused.getsockname()[1]
    known = directory / 'known_hosts'
    public = primary.export_public_key('openssh').decode().strip()
    known.write_text(''.join(f'[127.0.0.1]:{port} {public}\n' for name, port in ports.items() if name not in ['unknown', 'certified']) + f'@cert-authority [127.0.0.1]:{ports["certified"]} {ca.export_public_key("openssh").decode().strip()}\nfixture-owner-alias {public}\n', encoding='utf-8')
    fallback_known = directory / 'fallback_known_hosts'
    fallback_known.write_text(f'127.0.0.1 {public}\n', encoding='utf-8')
    config = directory / 'config'
    lines = []
    for name, port in ports.items():
        lines.extend([f'Host atlas-{name}', '  HostName 127.0.0.1', f'  Port {port}', f'  User {"denied" if name == "rejected" else "fixture"}'])
        if name == 'agent': lines.extend(['  IdentityFile none', '  IdentitiesOnly no', f'  IdentityAgent {agent}'])
        elif name == 'agent-missing': lines.extend(['  IdentityFile none', '  IdentityAgent none'])
        else: lines.append(f'  IdentityFile "{identity.as_posix()}"')
        if name == 'explicit-alias': lines.append('  HostKeyAlias fixture-owner-alias')
        if name == 'fallback': lines.append(f'  UserKnownHostsFile "{fallback_known.as_posix()}"')
    lines.extend(['Host atlas-proxy', '  HostName 127.0.0.1', '  ProxyCommand echo must-not-run'])
    lines.extend(['Host *', '  NoHostAuthenticationForLocalhost yes', '  FingerprintHash md5', '  IdentityAgent none', '  IdentitiesOnly yes', f'  UserKnownHostsFile "{known.as_posix()}"', '  GlobalKnownHostsFile none', '  StrictHostKeyChecking no', '  UpdateHostKeys yes', '  CheckHostIP yes', '  ControlMaster auto', '  ControlPersist 60', '  ForkAfterAuthentication yes', '  PermitLocalCommand yes', f'  LocalCommand echo should-not-run > "{(directory / "forbidden-local-command").as_posix()}"', '  RemoteCommand echo should-not-run', '  ForwardAgent yes', ''])
    config.write_text('\n'.join(lines), encoding='utf-8')
    save_stats()
    info = {'directory': str(directory), 'config': str(config), 'known_hosts': str(known), 'fallback_known_hosts': str(fallback_known), 'stats': str(directory / 'stats.json'), 'fingerprint': primary.get_fingerprint(), 'changed_fingerprint': changed.get_fingerprint(), 'aliases': ['atlas-' + name for name in ports] + ['atlas-proxy']}
    (directory / 'ready.json').write_text(json.dumps(info), encoding='utf-8')
    print(json.dumps(info), flush=True)
    try:
        await asyncio.to_thread(sys.stdin.readline)
    finally:
        close_agent()
        for listener in listeners: listener.close()
        for connection in list(connections): connection.close()
        await asyncio.gather(*(listener.wait_closed() for listener in listeners))
        await asyncio.gather(*(connection.wait_closed() for connection in list(connections)), return_exceptions=True)

if __name__ == '__main__':
    asyncssh.set_log_level('CRITICAL')
    asyncio.run(serve(Path(sys.argv[1]).resolve()))
