"""Disposable loopback monitoring server. All keys, files and observations synthetic."""
import asyncio, csv, json, os, re, subprocess, sys
from datetime import datetime, timezone, timedelta
from pathlib import Path
import asyncssh
from fixture_stats import publish as publish_stats

async def main(directory):
    directory.mkdir(parents=True)
    identity=asyncssh.generate_private_key('ssh-ed25519')
    host_key=asyncssh.generate_private_key('ssh-ed25519')
    replacement=asyncssh.generate_private_key('ssh-ed25519')
    key=directory/'identity';key.write_bytes(identity.export_private_key('openssh'))
    if os.name=='nt':
        token=subprocess.run(['whoami','/user','/fo','csv','/nh'],capture_output=True,text=True,check=True,timeout=10)
        sid=next(csv.reader(token.stdout.strip().splitlines()))[1]
        if not re.fullmatch(r'S-1-\d+(?:-\d+)+',sid):raise RuntimeError('Invalid fixture owner')
        subprocess.run(['icacls',str(key),'/inheritance:r','/grant:r','*'+sid+':F'],capture_output=True,check=True,timeout=10)
    else:key.chmod(0o600)
    home=directory/'home';home.mkdir()
    stats={'connections':0,'offline_auth':0,'collections':0};connections=set();state='active';port=0
    def save():
        publish_stats(directory,stats)
    class Server(asyncssh.SSHServer):
        def connection_made(self,conn):self.conn=conn;connections.add(conn);stats['connections']+=1;save()
        def connection_lost(self,exc):connections.discard(self.conn)
        def begin_auth(self,user):
            if user=='offline':stats['offline_auth']+=1;save()
            return True
        def public_key_auth_supported(self):return True
        def validate_public_key(self,user,key):return user=='fixture' and key.get_fingerprint()==identity.get_fingerprint()
    async def process(proc):
        command=proc.command or ''
        marker=re.fullmatch(r'echo (ATLAS_VERIFY_[a-f0-9]{32})',command)
        if marker:proc.stdout.write(marker[1]+'\n');proc.exit(0)
        elif 'ATLAS_BASELINE_V1' in command:
            stats['collections']+=1;save()
            at=(datetime.now(timezone.utc)-timedelta(seconds=2)).isoformat()
            proc.stdout.write(f'ATLAS_BASELINE_V1\nTIME\t{at}\nHOST\tmonitor-fixture\nCPUS\t4\nMEMORY_KIB\t4000000\nSERVICE\tsystem\tworker.service\t{state}\t{state}\nSYSTEM_SERVICES_OK\nUSER_SERVICES_OK\nCONTAINERS_NONE\nEND\n');proc.exit(0)
        else:proc.exit(1)
    async def start(which):return await asyncssh.create_server(Server,'127.0.0.1',port,server_host_keys=[which],process_factory=process,sftp_factory=lambda chan:asyncssh.SFTPServer(chan,chroot=str(home)))
    listener=await start(host_key);port=listener.get_port()
    known=directory/'known_hosts'
    def trust(which):known.write_text(f'[127.0.0.1]:{port} {which.export_public_key().decode().strip()}\n')
    trust(host_key)
    config=directory/'config';config.write_text(''.join(f'Host {alias}\n  HostName 127.0.0.1\n  Port {port}\n  User {user}\n  IdentityFile "{key.as_posix()}"\n  UserKnownHostsFile "{known.as_posix()}"\n  GlobalKnownHostsFile none\n' for alias,user in [('monitor-node','fixture'),('monitor-offline','offline')]))
    save();print(json.dumps({'config':str(config),'known_hosts':str(known),'stats':str(directory/'stats.json'),'fingerprint':host_key.get_fingerprint(),'replacement':replacement.get_fingerprint()}),flush=True)
    try:
        while True:
            line=(await asyncio.to_thread(sys.stdin.readline)).strip()
            if not line or line=='quit':break
            if line in ('down','replace','original'):
                listener.close();await listener.wait_closed()
                for conn in list(connections):conn.close()
                await asyncio.gather(*(conn.wait_closed() for conn in list(connections)),return_exceptions=True)
            if line in ('up','replace','original'):listener=await start(replacement if line=='replace' else host_key)
            if line=='trust-replacement':trust(replacement)
            if line=='trust-original':trust(host_key)
            if line=='failed':state='failed'
            print(json.dumps({'ack':line}),flush=True)
    finally:
        listener.close();await listener.wait_closed()
        for conn in list(connections):conn.close()
        await asyncio.gather(*(conn.wait_closed() for conn in list(connections)),return_exceptions=True)

if __name__=='__main__':asyncio.run(main(Path(sys.argv[1]).resolve()))
