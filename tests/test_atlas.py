import http.client
import json
from pathlib import Path
import sys
import tempfile
import threading
import unittest
from http.server import ThreadingHTTPServer

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from atlas import Invalid, Missing, Store, make_handler


def report(at='2026-01-01T00:00:00+00:00'):
    return {'report': {'hostname': 'lab-test-01', 'collected_at': at, 'cpus': 4,
            'memory_total_kib': 4000000, 'services': [{'scope': 'system', 'name': 'ssh.service',
            'state': 'active'}], 'containers': [], 'warnings': [], 'secret': 'must be discarded'}}


class Fixture(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.path = Path(self.temp.name)
        self.db_path = self.path/'atlas.sqlite3'
        self.store = Store(self.db_path)

    def tearDown(self):
        self.temp.cleanup()


class StoreTests(Fixture):
    def test_servers_positions_and_edges_survive_restart(self):
        a = self.store.add_server({'name': 'A', 'host': 'alpha.home'})
        b = self.store.add_server({'name': 'B', 'host': '192.0.2.5'})
        self.store.update_server(a, {'x': 500, 'y': -10, 'name': 'Renamed'})
        self.store.add_edge({'source': a, 'target': b, 'label': 'Backs up to'})
        state = Store(self.db_path).state()
        self.assertEqual((state['servers'][0]['name'], state['servers'][0]['x'], state['servers'][0]['y']), ('Renamed', 500, -10))
        self.assertEqual(state['edges'][0]['target'], b)
        self.store.remove_edge(state['edges'][0]['id'])
        self.assertEqual(Store(self.db_path).state()['edges'], [])

    def test_rejects_duplicate_hosts_and_invalid_coordinates(self):
        sid = self.store.add_server({'name': 'A', 'host': 'alpha.home'})
        with self.assertRaises(Invalid):
            self.store.add_server({'name': 'B', 'host': 'ALPHA.home'})
        for value in [float('nan'), float('inf'), 10001, True, '25']:
            with self.subTest(value=value), self.assertRaises(Invalid):
                self.store.update_server(sid, {'x': value})

    def test_rejects_invalid_targets_and_hosts(self):
        sid = self.store.add_server({'name': 'A', 'host': 'alpha.home'})
        for host in ['-oProxyCommand=bad', 'ssh://foo', 'user@foo', 'x y', '', 'a..b']:
            with self.subTest(host=host), self.assertRaises(Invalid):
                self.store.add_server({'name': 'Bad', 'host': host})
        with self.assertRaises(Invalid):
            self.store.add_edge({'source': sid, 'target': sid, 'label': 'self'})
        with self.assertRaises(Missing):
            self.store.add_edge({'source': sid, 'target': 'missing', 'label': 'missing'})
        self.assertEqual(self.store.state()['edges'], [])

    def test_import_preserves_times_layout_and_manual_names(self):
        (self.path/'game.json').write_text(json.dumps(report()))
        self.assertEqual(self.store.import_reports(self.path), 1)
        s = self.store.state()['servers'][0]
        self.assertEqual(s['inventory']['collected_at'], '2026-01-01T00:00:00+00:00')
        self.assertNotIn('secret', s['inventory'])
        self.store.update_server(s['id'], {'name': 'My server', 'x': 123, 'y': 456})
        (self.path/'game.json').write_text(json.dumps(report('2026-01-02T00:00:00+00:00')))
        self.assertEqual(self.store.import_reports(self.path), 1)
        s2 = self.store.state()['servers'][0]
        self.assertEqual((s2['id'], s2['name'], s2['x'], s2['y']), (s['id'], 'My server', 123, 456))
        self.assertEqual(self.store.import_reports(self.path), 0)
        (self.path/'game.json').write_text(json.dumps(report()))
        self.assertEqual(self.store.import_reports(self.path), 0)
        self.assertEqual(len(self.store.state()['events']), 3)

    def test_invalid_import_is_atomic(self):
        (self.path/'git.json').write_text(json.dumps(report()))
        bad = report()
        bad['report']['hostname'] = 'second.home'
        bad['report']['services'] = 'invalid'
        (self.path/'game.json').write_text(json.dumps(bad))
        with self.assertRaises(Invalid):
            self.store.import_reports(self.path)
        self.assertEqual(self.store.state()['servers'], [])

    def test_import_adopts_existing_host(self):
        sid = self.store.add_server({'name': 'Existing', 'host': 'lab-test-01'})
        (self.path/'game.json').write_text(json.dumps(report()))
        self.store.import_reports(self.path)
        self.assertEqual(len(self.store.state()['servers']), 1)
        self.assertEqual(self.store.state()['servers'][0]['id'], sid)


class HTTPTests(Fixture):
    def setUp(self):
        super().setUp()
        self.server = ThreadingHTTPServer(('127.0.0.1', 0), make_handler(self.store, self.path))
        self.thread = threading.Thread(target=self.server.serve_forever, daemon=True)
        self.thread.start()
        self.port = self.server.server_port

    def tearDown(self):
        self.server.shutdown()
        self.server.server_close()
        self.thread.join()
        super().tearDown()

    def request(self, method, path, data=None, headers=None):
        h = {'Content-Type': 'application/json', 'X-Atlas-Client': 'browser'}
        if headers is not None:
            h.update(headers)
        connection = http.client.HTTPConnection('127.0.0.1', self.port, timeout=5)
        connection.request(method, path, json.dumps(data) if data is not None else None, h)
        response = connection.getresponse()
        result = (response.status, response.read(), dict(response.getheaders()))
        connection.close()
        return result

    def test_shared_api_and_both_interfaces(self):
        status, body, _ = self.request('POST', '/api/servers', {'name': 'Saved host', 'host': 'test.home'})
        self.assertEqual(status, 201)
        sid = json.loads(body)['id']
        self.assertEqual(self.request('PATCH', '/api/servers/'+sid, {'x': 321, 'y': 45})[0], 200)
        state = json.loads(self.request('GET', '/api/map')[1])
        self.assertEqual(state['servers'][0]['x'], 321)
        for route in ['/', '/mobile', '/desktop/app.js', '/mobile/app.js', '/api.js', '/common.css']:
            with self.subTest(route=route):
                status, body, headers = self.request('GET', route)
                self.assertEqual(status, 200)
                self.assertGreater(len(body), 100)
                self.assertEqual(headers['Cache-Control'], 'no-store')

    def test_cross_origin_and_rebinding_are_rejected(self):
        data = {'name': 'Bad', 'host': 'bad.home'}
        self.assertEqual(self.request('POST', '/api/servers', data, {'Origin': 'https://untrusted.example'})[0], 403)
        self.assertEqual(self.request('POST', '/api/servers', data, {'X-Atlas-Client': ''})[0], 403)
        self.assertEqual(self.request('GET', '/api/map', headers={'Host': 'untrusted.example'})[0], 403)
        self.assertEqual(self.store.state()['servers'], [])

    def test_private_files_and_bad_bodies_are_not_exposed(self):
        for route in ['/atlas.py', '/data/atlas.sqlite3', '/../atlas.py', '/mobile/../../atlas.py']:
            self.assertEqual(self.request('GET', route)[0], 404)
        for payload in [[], {'name': 'A', 'host': 'bad host'}, {'name': 'A', 'host': 'ok.home', 'report': {}}]:
            self.assertEqual(self.request('POST', '/api/servers', payload)[0], 400)
        self.assertEqual(self.store.state()['servers'], [])

    def test_import_endpoint_uses_configured_directory_only(self):
        (self.path/'game.json').write_text(json.dumps(report()))
        self.assertEqual(self.request('POST', '/api/import', {'directory': 'elsewhere'})[0], 400)
        status, body, _ = self.request('POST', '/api/import', {})
        self.assertEqual(status, 200)
        self.assertEqual(json.loads(body)['imported'], 1)


if __name__ == '__main__':
    unittest.main()
