"""Private synthetic desktop fixture; never opens the owner's database."""
import json
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT))
from atlas import Store

directory = Path(sys.argv[1]).resolve()
directory.mkdir(parents=True, exist_ok=False)
reports = directory / 'reports'
reports.mkdir()
source = directory / 'legacy.sqlite3'
store = Store(source)
for index, (key, name) in enumerate([('git', 'Core & data'), ('runner', 'Infrastructure'), ('game', 'Compute & apps'), ('archpc', 'Communications')]):
    host = key + '.fixture.invalid'
    identity = store.add_server({'name': name, 'host': host})
    report = {'hostname': host, 'collected_at': '2026-10-24T08:00:00+00:00', 'cpus': 8, 'memory_total_kib': 16777216,
        'services': [{'scope': 'system', 'name': f'unit-{i:03d}.service', 'state': 'active' if i < 3 else 'inactive', 'substate': 'running' if i < 3 else 'dead'} for i in range(540 if index == 0 else 4)],
        'containers': [{'scope': 'user', 'engine': 'podman', 'name': 'api', 'image': 'example/api:v1', 'state': 'running'}],
        'raw_config': 'synthetic-excluded-marker'}
    (reports / (key + '.json')).write_text(json.dumps({'report': report}), encoding='utf-8')
    store.update_server(identity, {'x': 45 + (index % 2) * 320, 'y': 65 + (index // 2) * 300})
store.import_reports(reports)
print(json.dumps({'directory': str(directory), 'source': str(source), 'reports': str(reports)}))
