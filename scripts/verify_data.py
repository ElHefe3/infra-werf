"""Controlled Python/Rust parity and rollback; real source is read-only/optional."""
from __future__ import annotations
import argparse
import hashlib
import json
from pathlib import Path
import queue
import sqlite3
import subprocess
import sys
import threading
import uuid

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT))
from atlas import Store as ReferenceStore

BINARY = ROOT / 'target' / 'debug' / ('atlas-data.exe' if sys.platform == 'win32' else 'atlas-data')


def command(*args, should_fail=False):
    result = subprocess.run([str(BINARY), *map(str, args)], capture_output=True, text=True, timeout=15)
    if should_fail:
        assert result.returncode != 0, 'Invalid native data command unexpectedly succeeded'
        assert 'excluded-marker' not in result.stderr, 'Raw input leaked into an error'
        return json.loads(result.stderr)
    if result.returncode:
        raise AssertionError('Native data command failed: ' + result.stderr.strip())
    return json.loads(result.stdout)


def digest(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def baseline(report):
    if report is None:
        return None
    result = {key: report[key] for key in ('hostname', 'collected_at', 'cpus', 'memory_total_kib')}
    for category, fields in [('services', ('scope', 'name', 'state', 'substate')),
                             ('containers', ('scope', 'engine', 'name', 'image', 'state'))]:
        result[category] = [{key: row.get(key, '') for key in fields} for row in report.get(category, [])]
    return result


def compare(expected, actual):
    assert actual['contract_version'] == 1
    expected_servers = sorted(expected['servers'], key=lambda s: s['id'])
    actual_servers = sorted(actual['servers'], key=lambda s: s['id'])
    assert len(expected_servers) == len(actual_servers), 'Server count differs'
    for old, new in zip(expected_servers, actual_servers):
        for key in ('id', 'name', 'host', 'x', 'y', 'created_at'):
            assert old[key] == new[key], 'Map field differs: ' + key
        assert baseline(old['inventory']) == baseline(new['inventory']), 'Approved inventory differs'
        if new['inventory'] is not None:
            assert new['inventory']['source_binding'] is None and new['inventory']['source_host'] is None, 'Legacy import invented authenticated provenance'
        assert new['reachability']['state'] == 'unknown', 'Invented reachability'
    assert expected['edges'] == actual['edges'], 'Relationship identity/direction differs'
    expected_events = [{**event, 'id': str(event['id'])} for event in expected['events']]
    assert expected_events == actual['events'], 'Activity history differs'


def python_writer_blocked(path):
    child = subprocess.Popen([str(BINARY), 'hold-lock', str(path)],
        stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    lines = queue.Queue(maxsize=1)
    reader = threading.Thread(target=lambda: lines.put(child.stdout.readline()), daemon=True)
    reader.start()
    try:
        try:
            ready = lines.get(timeout=5)
        except queue.Empty as error:
            raise AssertionError('Native data writer did not acquire its SQLite lock') from error
        assert json.loads(ready)['locked'] is True
        reader.join(timeout=5)
        try:
            with sqlite3.connect(path, timeout=0.05) as writer:
                writer.execute('UPDATE servers SET name=name')
        except sqlite3.OperationalError as error:
            assert error.sqlite_errorcode in (sqlite3.SQLITE_BUSY, sqlite3.SQLITE_LOCKED)
        else:
            raise AssertionError('Concurrent Python writer was not rejected')
        stdout, stderr = child.communicate(b'\n', timeout=5)
        assert child.returncode == 0 and json.loads(stdout.splitlines()[-1])['released'] is True
    except subprocess.TimeoutExpired as error:
        child.kill()
        child.communicate(timeout=5)
        raise AssertionError('Native data writer did not release its SQLite lock') from error
    finally:
        if child.poll() is None:
            child.kill()
            child.communicate(timeout=5)
        reader.join(timeout=5)


def verify(source, directory, tag):
    before = digest(source)
    expected = ReferenceStore(source).state()  # Controlled copy/fixture only.
    before = digest(source)
    native = directory / (tag + '-native.sqlite3')
    recovery = directory / (tag + '-recovery.sqlite3')
    counts = command('migrate', source, native, recovery)
    compare(expected, command('state', native))
    assert digest(source) == before, 'Migration changed its source copy'
    python_writer_blocked(native)
    restored = directory / (tag + '-restored.sqlite3')
    command('restore', recovery, restored)
    rolled_back = ReferenceStore(restored).state()
    assert {k: v for k, v in expected.items() if k != 'served_at'} == {k: v for k, v in rolled_back.items() if k != 'served_at'}, 'Legacy rollback differs'
    return {**counts, 'parity': True, 'rollback': True, 'python_writer_rejected': True}


def fixture(directory):
    source = directory / 'synthetic-legacy.sqlite3'
    store = ReferenceStore(source)
    a = store.add_server({'name': 'Original', 'host': 'observed.home'})
    b = store.add_server({'name': 'Second', 'host': '192.0.2.12'})
    reports = directory / 'reports'
    reports.mkdir()
    report = {'report': {'hostname': 'observed.home', 'collected_at': '2026-01-01T00:00:00+00:00',
        'cpus': 4, 'memory_total_kib': 4000000, 'services': [{'scope': 'system', 'name': 'ssh.service', 'state': 'active', 'substate': 'running', 'boot': 'enabled', 'restart': 'always'}],
        'containers': [], 'warnings': ['excluded free-form warning'], 'secret': 'excluded-marker'}}
    (reports / 'git.json').write_text(json.dumps(report), encoding='utf-8')
    store.import_reports(reports)
    store.update_server(a, {'name': 'Owner edited', 'host': 'edited.home', 'x': 123, 'y': -456})
    store.add_edge({'source': a, 'target': b, 'label': 'Backs up to'})
    return source


def import_journey(native, reports):
    before = command('state', native)
    assert command('import', native, reports)['imported'] == 0
    path = reports / 'git.json'
    record = json.loads(path.read_text(encoding='utf-8'))
    record['report']['collected_at'] = '2026-01-02T00:00:00+00:00'
    record['report']['raw_config'] = 'excluded-marker'
    path.write_text(json.dumps(record), encoding='utf-8')
    assert command('import', native, reports)['imported'] == 1
    after = command('state', native)
    assert before['edges'] == after['edges']
    for old, new in zip(before['servers'], after['servers']):
        for key in ('id', 'name', 'host', 'x', 'y', 'created_at'):
            assert old[key] == new[key], 'Import replaced an owner-authored map field'
    observed = [s for s in after['servers'] if s['inventory'] is not None]
    assert observed[0]['inventory']['collected_at'] == record['report']['collected_at']
    assert 'excluded-marker' not in json.dumps(after)
    assert command('import', native, reports)['imported'] == 0
    record['report']['collected_at'] = '2025-01-01T00:00:00+00:00'
    path.write_text(json.dumps(record), encoding='utf-8')
    assert command('import', native, reports)['imported'] == 0
    record['report']['collected_at'] = '2026-01-03T00:00:00+00:00'
    path.write_text(json.dumps(record), encoding='utf-8')
    (reports / 'game.json').write_text(json.dumps({'report': {'hostname': 'invalid.home', 'services': 'invalid'}}), encoding='utf-8')
    assert command('import', native, reports, should_fail=True)['code'] == 'invalid_input'
    actual = command('state', native)
    assert {k: v for k, v in after.items() if k != 'served_at'} == {k: v for k, v in actual.items() if k != 'served_at'}, 'Failed batch changed persisted state'
    return True


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--legacy-source', type=Path, help='Read-only source to back up before testing; never passed to ReferenceStore')
    args = parser.parse_args()
    tools = (ROOT / '.tools').resolve()
    directory = (tools / ('data-proof-' + uuid.uuid4().hex)).resolve()
    assert directory.is_relative_to(tools) and tools.is_relative_to(ROOT)
    directory.mkdir(parents=True)
    # Retain owned copies, including failing ones; no recursive cleanup/cutover.
    summaries = {'synthetic': verify(fixture(directory), directory, 'synthetic')}
    summaries['synthetic']['native_import_journey'] = import_journey(directory / 'synthetic-native.sqlite3', directory / 'reports')
    if args.legacy_source:
        source = args.legacy_source.resolve()
        assert source.is_file()
        original_digest = digest(source)
        copy = directory / 'reference-legacy.sqlite3'
        with sqlite3.connect(source.as_uri() + '?mode=ro', uri=True) as original, sqlite3.connect(copy) as backup:
            original.backup(backup)
        summaries['reference_copy'] = verify(copy, directory, 'reference')
        assert digest(source) == original_digest, 'Reference changed during the proof; retain the copy for investigation'
        summaries['original_unchanged'] = True
    print(json.dumps(summaries))
    print('Private proof copies retained at ' + str(directory.relative_to(ROOT)))


if __name__ == '__main__':
    main()
