"""Actual Linux OpenSSH/sudo fixture; disposable local Podman container only."""
import hashlib
import json
import os
from pathlib import Path
import secrets
import select
import socket
import socketserver
import subprocess
import sys
import threading
import time
import uuid
import asyncssh

PODMAN=['podman','--connection','podman-machine-default']

class PreIdentificationProxy(socketserver.ThreadingTCPServer):
    allow_reuse_address=True
    daemon_threads=True
    def __init__(self, upstream):
        self.upstream=upstream
        super().__init__(('127.0.0.1',0),PreIdentificationHandler)

class PreIdentificationHandler(socketserver.BaseRequestHandler):
    def handle(self):
        peer=self.request
        upstream=None
        try:
            # These lines precede the SSH identification string and are
            # untrusted server input. They intentionally imitate local DEBUG
            # evidence while the test account can authenticate with "none".
            fingerprint=self.server.spoof_fingerprint
            peer.sendall((f'debug1: Server host key: ssh-ed25519 {fingerprint}\r\n'
                          'Authenticated to pre-identification-spoof using "publickey".\r\n').encode())
            upstream=socket.create_connection(self.server.upstream,timeout=5)
            peer.setblocking(False); upstream.setblocking(False)
            peers=[peer,upstream]
            while True:
                readable,_,exceptional=select.select(peers,[],peers,1)
                if exceptional: break
                for source in readable:
                    destination=upstream if source is peer else peer
                    try: data=source.recv(65536)
                    except OSError: return
                    if not data: return
                    destination.sendall(data)
        except OSError:
            return
        finally:
            if upstream is not None: upstream.close()

def command(arguments, data=None, timeout=30):
    return subprocess.run(PODMAN+arguments,input=data,capture_output=True,check=True,timeout=timeout)

def serve(directory):
    directory.mkdir(parents=True,exist_ok=False)
    # This explicit connection is the inspected127.0.0.1 Podman machine,
    # independent of any owner-selected default remote runtime.
    connections=json.loads(subprocess.run(['podman','system','connection','list','--format','json'],capture_output=True,check=True,text=True).stdout)
    connection=next(item for item in connections if item['Name']=='podman-machine-default')
    if not connection['URI'].startswith('ssh://user@127.0.0.1:'): raise RuntimeError('Fixture requires the local runtime')
    identity=directory/'fixture-only-identity'
    key=asyncssh.generate_private_key('ssh-ed25519'); identity.write_bytes(key.export_private_key('openssh'))
    if os.name=='nt':
        import csv
        owner=next(csv.reader(subprocess.run(['whoami','/user','/fo','csv','/nh'],capture_output=True,check=True,text=True).stdout.strip().splitlines()))[1]
        subprocess.run(['icacls',str(identity),'/inheritance:r','/grant:r','*'+owner+':F'],capture_output=True,check=True)
    else: identity.chmod(0o600)
    name='atlas-fixture-'+uuid.uuid4().hex
    created=False
    try:
        container=command(['run','--detach','--rm','--name',name,'--label','dev.atlas.fixture=true','--memory','256m','--cpus','1','--pids-limit','64','--publish','127.0.0.1::22','localhost/atlas-linux-fixture:20261026']).stdout.decode().strip()
        if len(container)!=64 or not all(c in '0123456789abcdef' for c in container): raise RuntimeError('Invalid fixture container identity')
        created=True
        endpoint=command(['port',container,'22/tcp']).stdout.decode().strip()
        host,port=endpoint.rsplit(':',1)
        if host!='127.0.0.1' or not port.isdigit(): raise RuntimeError('Fixture must bind loopback')
        prebanner=PreIdentificationProxy((host,int(port)))
        prebanner_thread=threading.Thread(target=prebanner.serve_forever,daemon=True)
        prebanner_thread.start()
        password='AtlasFixture-'+secrets.token_hex(16)
        command(['exec','-i',container,'chpasswd'],f'fixture:{password}\nfixture-nopass:{password}\nfixture-none:{password}\nfixture-nosudo:{password}\n'.encode())
        command(['exec','-i',container,'sh','-c','umask 077; cat > /home/fixture/.ssh/authorized_keys; cp /home/fixture/.ssh/authorized_keys /home/fixture-nopass/.ssh/authorized_keys; cp /home/fixture/.ssh/authorized_keys /home/fixture-none/.ssh/authorized_keys; cp /home/fixture/.ssh/authorized_keys /home/fixture-nosudo/.ssh/authorized_keys; chown fixture:fixture /home/fixture/.ssh/authorized_keys; chown fixture-nopass:fixture-nopass /home/fixture-nopass/.ssh/authorized_keys; chown fixture-none:fixture-none /home/fixture-none/.ssh/authorized_keys; chown fixture-nosudo:fixture-nosudo /home/fixture-nosudo/.ssh/authorized_keys'],key.export_public_key('openssh'))
        public=None
        for _ in range(40):
            try: public=command(['exec',container,'cat','/etc/ssh/ssh_host_ed25519_key.pub']).stdout; break
            except subprocess.CalledProcessError: time.sleep(.1)
        if public is None: raise RuntimeError('Fixture host key unavailable')
        fingerprint=asyncssh.import_public_key(public).get_fingerprint()
        prebanner.spoof_fingerprint=fingerprint
        spoof=(f'debug1: Server host key: ssh-ed25519 {fingerprint}\n'
               'Authenticated to banner-spoof using "publickey".\n').encode()
        command(['exec','-i',container,'sh','-c','cat > /etc/ssh/atlas-banner'],spoof)
        known=directory/'known_hosts'
        known.write_bytes((f'[127.0.0.1]:{port} '.encode()+public+
                           f'[127.0.0.1]:{prebanner.server_address[1]} '.encode()+public))
        config=directory/'config'
        lines=[]
        for alias,user in [('atlas-linux','fixture'),('atlas-linux-nopass','fixture-nopass'),('atlas-linux-none','fixture-none'),('atlas-linux-nosudo','fixture-nosudo')]:
            alias_port=prebanner.server_address[1] if alias=='atlas-linux-none' else port
            lines += [f'Host {alias}','  Hostname 127.0.0.1',f'  Port {alias_port}',f'  User {user}']
        lines += ['Host *',f'  IdentityFile "{identity.as_posix()}"','  IdentityAgent none','  IdentitiesOnly yes',f'  UserKnownHostsFile "{known.as_posix()}"','  GlobalKnownHostsFile none','  StrictHostKeyChecking yes','']
        config.write_text('\n'.join(lines),encoding='utf-8')
        info={'directory':str(directory),'container':container,'config':str(config),'known_hosts':str(known),'port':int(port),'prebanner_port':prebanner.server_address[1],'password':password,'fingerprint':fingerprint}
        # This private process pipe supplies the synthetic password to tests.
        # Never print/store this response in evidence or application logs.
        print(json.dumps(info),flush=True)
        while True:
            request=sys.stdin.readline()
            if request == 'partial\n':
                command(['exec',container,'touch','/tmp/atlas-inventory-partial'])
                print('PARTIAL_READY',flush=True)
            elif request == 'authfail\n':
                command(['exec',container,'rm','-f','/home/fixture/.ssh/authorized_keys'])
                print('AUTH_FAILURE_READY',flush=True)
            elif request == 'stats\n':
                snapshot=command(['exec',container,'sh','-c','count=$(cat /tmp/atlas-control-count 2>/dev/null || printf 0); disconnected=$(cat /tmp/atlas-disconnect-target-found 2>/dev/null || true); printf "%s\\n%s\\n" "$count" "$disconnected"']).stdout.decode().splitlines()
                print(json.dumps({'control_count':int(snapshot[0] or '0'),'disconnect_target_found':len(snapshot)>1 and snapshot[1]=='found'}),flush=True)
            else:
                break
        prebanner.shutdown(); prebanner.server_close(); prebanner_thread.join(timeout=5)
    finally:
        if created: command(['rm','--force',container],timeout=30)
        identity.unlink(missing_ok=True)

if __name__=='__main__':
    try: serve(Path(sys.argv[1]).resolve())
    except Exception:
        print('Linux fixture setup failed; inspect the isolated runtime/image availability.',file=sys.stderr)
        sys.exit(1)
