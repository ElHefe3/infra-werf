"""Local Atlas coordinator: persistent topology and saved inventory. Python 3.11+."""
from __future__ import annotations
import argparse
from contextlib import contextmanager
from datetime import datetime, timezone
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import ipaddress
import json
import math
from pathlib import Path
import re
import sqlite3
import uuid

BASE = Path(__file__).resolve().parent
MAX_BODY = 1024 * 1024


class Invalid(ValueError):
    pass


class Missing(LookupError):
    pass


def now():
    return datetime.now(timezone.utc).isoformat()


def text(value, limit=120, empty=False):
    if not isinstance(value, str) or len(value) > limit or any(ord(c) < 32 for c in value):
        raise Invalid('Invalid text field')
    value = value.strip()
    if not empty and not value:
        raise Invalid('A required field is empty')
    return value


def hostname(value):
    value = text(value, 253)
    try:
        ipaddress.ip_address(value)
        return value
    except ValueError:
        pass
    if not all(re.fullmatch(r'[A-Za-z0-9](?:[A-Za-z0-9-]{0,61}[A-Za-z0-9])?', label)
               for label in value.rstrip('.').split('.')):
        raise Invalid('Enter a hostname or IP address, without a URL or SSH options')
    return value


def coordinate(value):
    if type(value) not in (int, float) or not math.isfinite(value) or abs(value) > 10000:
        raise Invalid('Invalid map coordinates')
    return value


def timestamp(value):
    value = text(value, 64)
    try:
        date = datetime.fromisoformat(value.replace('Z', '+00:00'))
    except ValueError as exc:
        raise Invalid('Invalid observation timestamp') from exc
    if date.tzinfo is None or date > datetime.now(timezone.utc):
        raise Invalid('Observation time must include a timezone and cannot be in the future')
    return date.isoformat()


def inventory(raw):
    """Rebuild approved metadata only; discard raw and unknown fields."""
    if not isinstance(raw, dict):
        raise Invalid('Inventory must be an object')
    result = {'hostname': hostname(raw.get('hostname')), 'collected_at': timestamp(raw.get('collected_at'))}
    for field in ('cpus', 'memory_total_kib'):
        value = raw.get(field)
        if type(value) is not int or not 0 < value < 10**12:
            raise Invalid('Invalid hardware count')
        result[field] = value
    for category, fields in [('services', ('scope', 'name', 'state', 'substate', 'boot', 'restart')),
                              ('containers', ('scope', 'engine', 'name', 'image', 'state', 'restart'))]:
        rows = raw.get(category, [])
        if not isinstance(rows, list) or len(rows) > 1024:
            raise Invalid('Invalid or excessive inventory rows')
        result[category] = []
        for row in rows:
            if not isinstance(row, dict):
                raise Invalid('Invalid inventory row')
            result[category].append({key: text(row.get(key, ''), 512, empty=key != 'name') for key in fields})
    warnings = raw.get('warnings', [])
    if not isinstance(warnings, list) or len(warnings) > 20:
        raise Invalid('Invalid inventory warnings')
    result['warnings'] = [text(w, 512) for w in warnings]
    return result


class Store:
    def __init__(self, path):
        self.path = Path(path)
        self.path.parent.mkdir(parents=True, exist_ok=True)
        with self.connect() as db:
            db.executescript('''
                CREATE TABLE IF NOT EXISTS servers (
                    id TEXT PRIMARY KEY, name TEXT NOT NULL, host TEXT NOT NULL UNIQUE COLLATE NOCASE,
                    x REAL NOT NULL, y REAL NOT NULL, created_at TEXT NOT NULL,
                    source_key TEXT UNIQUE, report TEXT
                );
                CREATE TABLE IF NOT EXISTS edges (
                    id TEXT PRIMARY KEY, source TEXT NOT NULL REFERENCES servers(id),
                    target TEXT NOT NULL REFERENCES servers(id), label TEXT NOT NULL,
                    UNIQUE(source, target, label), CHECK(source != target)
                );
                CREATE TABLE IF NOT EXISTS events (
                    id INTEGER PRIMARY KEY, at TEXT NOT NULL, message TEXT NOT NULL, server_id TEXT
                );
                PRAGMA user_version=1;
            ''')

    @contextmanager
    def connect(self):
        db = sqlite3.connect(self.path, timeout=10)
        db.row_factory = sqlite3.Row
        db.execute('PRAGMA foreign_keys=ON')
        try:
            with db:
                yield db
        finally:
            db.close()

    @staticmethod
    def event(db, message, sid=None):
        db.execute('INSERT INTO events(at,message,server_id) VALUES(?,?,?)', (now(), message, sid))
        db.execute('DELETE FROM events WHERE id NOT IN (SELECT id FROM events ORDER BY id DESC LIMIT 200)')

    def state(self):
        with self.connect() as db:
            servers = []
            for row in db.execute('SELECT * FROM servers ORDER BY created_at,id'):
                s = dict(row)
                report = s.pop('report')
                s['inventory'] = json.loads(report) if report else None
                s.pop('source_key')
                servers.append(s)
            return {'servers': servers, 'edges': [dict(r) for r in db.execute('SELECT * FROM edges ORDER BY rowid')],
                    'events': [dict(r) for r in db.execute('SELECT * FROM events ORDER BY id DESC LIMIT 50')],
                    'served_at': now()}

    def add_server(self, payload):
        if set(payload) - {'name', 'host'}:
            raise Invalid('Unknown server fields')
        name, host = text(payload.get('name'), 80), hostname(payload.get('host'))
        sid = uuid.uuid4().hex
        try:
            with self.connect() as db:
                count = db.execute('SELECT count(*) FROM servers').fetchone()[0]
                db.execute('INSERT INTO servers VALUES(?,?,?,?,?,?,NULL,NULL)',
                           (sid, name, host, 45+(count % 3)*300, 65+(count // 3)*250, now()))
                self.event(db, 'Server added: '+name, sid)
        except sqlite3.IntegrityError as exc:
            raise Invalid('That host is already on the map') from exc
        return sid

    def update_server(self, sid, payload):
        if not payload or set(payload) - {'name', 'host', 'x', 'y'}:
            raise Invalid('Unknown or empty server fields')
        fields = {key: coordinate(value) if key in ('x', 'y') else hostname(value) if key == 'host' else text(value, 80)
                  for key, value in payload.items()}
        try:
            with self.connect() as db:
                if not db.execute('SELECT 1 FROM servers WHERE id=?', (sid,)).fetchone():
                    raise Missing('Server not found')
                # Names are drawn exclusively from the fixed field allowlist.
                db.execute('UPDATE servers SET '+','.join(k+'=?' for k in fields)+' WHERE id=?', (*fields.values(), sid))
                if any(k in fields for k in ('name', 'host')):
                    self.event(db, 'Server details updated', sid)
        except sqlite3.IntegrityError as exc:
            raise Invalid('That host is already on the map') from exc

    def add_edge(self, payload):
        if set(payload) != {'source', 'target', 'label'}:
            raise Invalid('Connection requires source, target and label')
        a, b, label = text(payload['source'], 64), text(payload['target'], 64), text(payload['label'], 80)
        if a == b:
            raise Invalid('Choose two different servers')
        try:
            with self.connect() as db:
                if db.execute('SELECT count(*) FROM servers WHERE id IN (?,?)', (a, b)).fetchone()[0] != 2:
                    raise Missing('Connection server not found')
                db.execute('INSERT INTO edges VALUES(?,?,?,?)', (uuid.uuid4().hex, a, b, label))
                self.event(db, 'Relationship documented: '+label, a)
        except sqlite3.IntegrityError as exc:
            raise Invalid('That relationship already exists') from exc

    def remove_edge(self, eid):
        with self.connect() as db:
            if not db.execute('DELETE FROM edges WHERE id=?', (eid,)).rowcount:
                raise Missing('Relationship not found')
            self.event(db, 'Relationship removed from map')

    def import_reports(self, directory):
        if not Path(directory).is_dir():
            raise Invalid('Saved inventory directory is unavailable')
        names = [('git', 'Core & data'), ('runner', 'Infrastructure'), ('game', 'Compute & apps'), ('archpc', 'Communications')]
        reports = []
        for key, name in names:
            path = Path(directory)/(key+'.json')
            if not path.is_file():
                continue
            if path.stat().st_size > MAX_BODY:
                raise Invalid('Saved inventory is too large: '+key)
            try:
                record = json.loads(path.read_text(encoding='utf-8'))
                report = inventory(record.get('report'))
            except (ValueError, AttributeError, TypeError) as exc:
                raise Invalid('Invalid saved inventory: '+key) from exc
            reports.append((key, name, report))
        count = 0
        with self.connect() as db:
            for index, (key, name, report) in enumerate(reports):
                row = db.execute('SELECT * FROM servers WHERE source_key=? OR host=? ORDER BY source_key DESC LIMIT 1',
                                 (key, report['hostname'])).fetchone()
                if row and row['report']:
                    before = json.loads(row['report'])
                    if datetime.fromisoformat(before['collected_at']) >= datetime.fromisoformat(report['collected_at']):
                        continue
                if row:
                    sid = row['id']
                    db.execute('UPDATE servers SET source_key=?,report=? WHERE id=?', (key, json.dumps(report), sid))
                else:
                    sid = uuid.uuid4().hex
                    db.execute('INSERT INTO servers VALUES(?,?,?,?,?,?,?,?)',
                               (sid, name, report['hostname'], 45+(index%2)*320, 65+(index//2)*260, now(), key, json.dumps(report)))
                self.event(db, 'Saved inventory imported ('+report['collected_at']+')', sid)
                count += 1
        return count


def make_handler(store, report_dir=None):
    class Handler(BaseHTTPRequestHandler):
        def respond(self, status, data, content_type='application/json; charset=utf-8'):
            body = data if isinstance(data, bytes) else json.dumps(data, allow_nan=False).encode()
            self.send_response(status)
            self.send_header('Content-Type', content_type)
            self.send_header('Content-Length', str(len(body)))
            self.send_header('Cache-Control', 'no-store')
            self.send_header('X-Content-Type-Options', 'nosniff')
            self.send_header('Referrer-Policy', 'no-referrer')
            self.send_header('Content-Security-Policy', "default-src 'self'; style-src 'self' 'unsafe-inline' https://fonts.googleapis.com; font-src 'self' https://fonts.gstatic.com; script-src 'self'; connect-src 'self'; img-src 'self' data:; frame-ancestors 'none'; base-uri 'none'; form-action 'self'")
            self.end_headers()
            self.wfile.write(body)

        def permitted(self, write=False):
            allowed = {'127.0.0.1:'+str(self.server.server_port), 'localhost:'+str(self.server.server_port)}
            if self.headers.get('Host') not in allowed:
                self.respond(403, {'error': 'This coordinator accepts local connections only'})
                return False
            if write:
                origin = self.headers.get('Origin')
                if self.headers.get('X-Atlas-Client') != 'browser' or (origin and origin != 'http://'+self.headers['Host']):
                    self.respond(403, {'error': 'Same-origin Atlas client required'})
                    return False
            return True

        def do_GET(self):
            if not self.permitted():
                return
            route = self.path.split('?', 1)[0]
            if route == '/api/map':
                return self.respond(200, store.state())
            if route == '/api/health':
                return self.respond(200, {'status': 'ok', 'schema_version': 1, 'inventory_import_configured': report_dir is not None})
            assets = {'/': 'desktop/index.html', '/desktop': 'desktop/index.html', '/desktop/': 'desktop/index.html',
                      '/mobile': 'mobile/index.html', '/mobile/': 'mobile/index.html', '/api.js': 'api.js',
                      '/common.css': 'common.css'}
            for mode in ('desktop', 'mobile'):
                for file in ('app.js', 'style.css'):
                    assets['/'+mode+'/'+file] = mode+'/'+file
            file = assets.get(route)
            if not file:
                return self.respond(404, {'error': 'Not found'})
            path = BASE/'static'/file
            mime = 'text/html' if path.suffix == '.html' else 'text/css' if path.suffix == '.css' else 'text/javascript'
            self.respond(200, path.read_bytes(), mime+'; charset=utf-8')

        def mutation(self):
            if not self.permitted(write=True):
                return
            try:
                if self.headers.get('Content-Type', '').split(';')[0] != 'application/json':
                    raise Invalid('Send application/json')
                length = int(self.headers.get('Content-Length', '0'))
                if length <= 0 or length > MAX_BODY:
                    raise Invalid('Body must be between 1 byte and 1 MiB')
                self.connection.settimeout(10)
                payload = json.loads(self.rfile.read(length), parse_constant=lambda _: (_ for _ in ()).throw(Invalid('Invalid number')))
                if not isinstance(payload, dict):
                    raise Invalid('Request must be an object')
                route = self.path
                if self.command == 'POST' and route == '/api/servers':
                    return self.respond(201, {'id': store.add_server(payload)})
                if self.command == 'PATCH' and re.fullmatch(r'/api/servers/[a-f0-9]{32}', route):
                    store.update_server(route.rsplit('/', 1)[1], payload)
                elif self.command == 'POST' and route == '/api/edges':
                    store.add_edge(payload)
                elif self.command == 'DELETE' and re.fullmatch(r'/api/edges/[a-f0-9]{32}', route):
                    store.remove_edge(route.rsplit('/', 1)[1])
                elif self.command == 'POST' and route == '/api/import':
                    if report_dir is None:
                        raise Invalid('No saved inventory directory configured')
                    if payload:
                        raise Invalid('Inventory source is configured on the coordinator')
                    return self.respond(200, {'imported': store.import_reports(report_dir)})
                else:
                    return self.respond(404, {'error': 'Not found'})
                self.respond(200, {'ok': True})
            except Missing as exc:
                self.respond(404, {'error': str(exc)})
            except (ValueError, TypeError) as exc:
                self.respond(400, {'error': str(exc) if isinstance(exc, Invalid) else 'Invalid request'})
            except (OSError, sqlite3.Error):
                self.respond(503, {'error': 'Storage or inventory source unavailable; retry later'})

        do_POST = mutation
        do_PATCH = mutation
        do_DELETE = mutation
    return Handler


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--port', type=int, default=8770)
    parser.add_argument('--database', type=Path, default=BASE/'data'/'atlas.sqlite3')
    parser.add_argument('--inventory-dir', type=Path, help='Sanitized dashboard records; read-only source')
    args = parser.parse_args()
    store = Store(args.database)
    if args.inventory_dir:
        print('Newer saved inventories imported:', store.import_reports(args.inventory_dir), flush=True)
    server = ThreadingHTTPServer(('127.0.0.1', args.port), make_handler(store, args.inventory_dir))
    print('Atlas desktop: http://127.0.0.1:'+str(server.server_port), flush=True)
    print('Atlas mobile:  http://127.0.0.1:'+str(server.server_port)+'/mobile', flush=True)
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        pass
    finally:
        server.server_close()


if __name__ == '__main__':
    main()
