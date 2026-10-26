"""Real OpenSSH + isolated loopback server proof; synthetic identities only."""
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import time
import uuid

ROOT = Path(__file__).resolve().parents[1]
directory = ROOT / '.tools' / ('ssh-proof-' + uuid.uuid4().hex)
server = subprocess.Popen([sys.executable, str(ROOT / 'scripts/ssh_fixture.py'), str(directory)], stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, cwd=ROOT)
try:
    line = server.stdout.readline()
    if not line: raise AssertionError('Fixture failed: ' + server.stderr.read()[-1000:])
    info = json.loads(line)
    config, known = Path(info['config']), Path(info['known_hosts'])
    inputs = [config, known, Path(info['fallback_known_hosts'])]
    before = {path.name: hashlib.sha256(path.read_bytes()).hexdigest() for path in inputs}
    binary = ROOT / 'target/debug/atlas-ssh.exe'
    results = {}
    for name, expected in [('trusted', 'verified'), ('agent', 'verified'), ('unknown', 'unknown_host'), ('changed', 'changed_host'), ('rejected', 'authentication_failed'), ('agent-missing', 'authentication_failed'), ('no-auth', 'authentication_failed'), ('certified', 'verified'), ('fallback', 'verified'), ('explicit-alias', 'verified'), ('unreachable', 'unreachable'), ('proxy', 'unsupported_configuration')]:
        response = subprocess.run([str(binary), str(config), 'atlas-' + name, '10000'], capture_output=True, text=True, timeout=15)
        assert response.returncode == 0, (name, response.stderr)
        value = json.loads(response.stdout)
        assert value['state'] == expected, (name, value)
        assert 'fixture-only-identity' not in response.stdout and 'synthetic-protected' not in response.stdout
        if name == 'trusted': assert value['fingerprint'] == info['fingerprint']
        if name == 'changed': assert value['fingerprint'] == info['changed_fingerprint']
        if name == 'certified': assert '-cert-' in value['key_type']
        results[name] = value['state']
    for name, expected in [('slow', 'timed_out'), ('overflow', 'output_limit')]:
        response = subprocess.run([str(binary), str(config), 'atlas-' + name, '600' if name == 'slow' else '10000'], capture_output=True, text=True, timeout=15)
        assert response.returncode != 0 and json.loads(response.stderr)['code'] == expected, (name, response.stdout, response.stderr)
        assert 'synthetic-protected' not in response.stderr
        results[name] = expected
    time.sleep(.1)
    stats = json.loads(Path(info['stats']).read_text())
    for name in ['unknown', 'changed']:
        assert stats[name]['auth_attempts'] == 0 and stats[name]['commands'] == 0, (name, stats[name])
    assert all(value['active'] == 0 for value in stats.values()), stats
    agent_stats = json.loads((directory / 'agent-stats.json').read_text())
    assert agent_stats['identities']>0 and agent_stats['signatures']==1 and agent_stats['active']==0, agent_stats
    assert not (directory / 'forbidden-local-command').exists()
    assert {path.name: hashlib.sha256(path.read_bytes()).hexdigest() for path in inputs} == before
    proof = {'result': 'pass', 'cases': results, 'agent':agent_stats, 'trust_config_unchanged': True, 'unknown_changed_stopped_before_auth': True, 'active_connections_after_cleanup': 0, 'directory': str(directory)}
    (directory / 'result.json').write_text(json.dumps(proof, indent=2), encoding='utf-8')
    print(json.dumps(proof, indent=2))
finally:
    if server.poll() is None:
        try: server.communicate('\n', timeout=5)
        except subprocess.TimeoutExpired: server.kill(); server.communicate()
