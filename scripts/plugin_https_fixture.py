"""A loopback HTTPS server for the plugin install suite (T045).

Creates a throwaway certificate authority and a certificate for 127.0.0.1,
serves the plugin archive fixtures from 127.0.0.1 only, prints one JSON line
({"port", "ca"}) and runs until stdin closes. The CA exists only in the
given directory and is trusted only by the InfraWerf process the test starts
(ATLAS_PLUGIN_TEST_CA); nothing is added to Windows.
"""
import datetime, http.server, ipaddress, json, pathlib, re, ssl, sys, threading
from cryptography import x509
from cryptography.hazmat.primitives import hashes, serialization
from cryptography.hazmat.primitives.asymmetric import ec
from cryptography.x509.oid import NameOID

directory = pathlib.Path(sys.argv[1])
directory.mkdir(parents=True, exist_ok=False)
archives = pathlib.Path(__file__).resolve().parent.parent / "crates/atlas-core/tests/fixtures/plugin-archives"
now = datetime.datetime.now(datetime.timezone.utc)


def name(text):
    return x509.Name([x509.NameAttribute(NameOID.COMMON_NAME, text)])


ca_key = ec.generate_private_key(ec.SECP256R1())
ca = (x509.CertificateBuilder().subject_name(name("InfraWerf test CA")).issuer_name(name("InfraWerf test CA"))
      .public_key(ca_key.public_key()).serial_number(x509.random_serial_number())
      .not_valid_before(now - datetime.timedelta(minutes=5)).not_valid_after(now + datetime.timedelta(hours=2))
      .add_extension(x509.BasicConstraints(ca=True, path_length=0), critical=True)
      .add_extension(x509.KeyUsage(digital_signature=True, key_cert_sign=True, crl_sign=True, content_commitment=False, key_encipherment=False, data_encipherment=False, key_agreement=False, encipher_only=False, decipher_only=False), critical=True)
      .sign(ca_key, hashes.SHA256()))
key = ec.generate_private_key(ec.SECP256R1())
leaf = (x509.CertificateBuilder().subject_name(name("127.0.0.1")).issuer_name(ca.subject)
        .public_key(key.public_key()).serial_number(x509.random_serial_number())
        .not_valid_before(now - datetime.timedelta(minutes=5)).not_valid_after(now + datetime.timedelta(hours=2))
        .add_extension(x509.SubjectAlternativeName([x509.IPAddress(ipaddress.ip_address("127.0.0.1"))]), critical=False)
        .add_extension(x509.ExtendedKeyUsage([x509.oid.ExtendedKeyUsageOID.SERVER_AUTH]), critical=False)
        .sign(ca_key, hashes.SHA256()))
(directory / "ca.pem").write_bytes(ca.public_bytes(serialization.Encoding.PEM))
(directory / "leaf.pem").write_bytes(leaf.public_bytes(serialization.Encoding.PEM))
(directory / "leaf.key").write_bytes(key.private_bytes(serialization.Encoding.PEM, serialization.PrivateFormat.PKCS8, serialization.NoEncryption()))
requests = []
# Each update fixture version as a zip, served at /updates/<case>-<version>.zip.
import io as _io, zipfile as _zipfile
updates_dir = archives.parent / "plugin-updates"
updates = {}
for case in sorted(updates_dir.iterdir()):
    for version in sorted(case.iterdir()):
        buffer = _io.BytesIO()
        with _zipfile.ZipFile(buffer, "w", _zipfile.ZIP_DEFLATED) as archive:
            for file in sorted(version.iterdir()):
                archive.write(file, file.name)
        updates[f"/updates/{case.name}-{version.name}.zip"] = buffer.getvalue()


class Handler(http.server.BaseHTTPRequestHandler):
    def log_message(self, *args):
        pass

    def do_GET(self):
        requests.append(self.path)
        port = self.server.server_address[1]
        if self.path == "/redirect-to-http.zip":
            self.send_response(302)
            self.send_header("Location", f"http://127.0.0.1:{port}/good.zip")
            self.end_headers()
            return
        if self.path in updates:
            body = updates[self.path]
        elif self.path == "/big.zip":
            body = b"PK\x03\x04" + b"\0" * (6 * 1024 * 1024)
        else:
            name = self.path.lstrip("/")
            if not re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9._-]*", name):
                self.send_response(404)
                self.end_headers()
                return
            file = (archives / name).resolve()
            if file.parent != archives.resolve() or not file.is_file():
                self.send_response(404)
                self.end_headers()
                return
            body = file.read_bytes()
        self.send_response(200)
        self.send_header("Content-Type", "application/octet-stream")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)


server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), Handler)
context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
context.load_cert_chain(directory / "leaf.pem", directory / "leaf.key")
server.socket = context.wrap_socket(server.socket, server_side=True)
threading.Thread(target=server.serve_forever, daemon=True).start()
print(json.dumps({"port": server.server_address[1], "ca": str(directory / "ca.pem")}), flush=True)
sys.stdin.read()
(directory / "requests.json").write_text(json.dumps(requests))
server.shutdown()
