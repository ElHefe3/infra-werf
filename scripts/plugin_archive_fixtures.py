"""Writes the plugin archive fixtures for T045 (S12/F22).

Run with the SSH fixture venv, which has `cryptography`:

    .tools/ssh-fixture-venv/Scripts/python.exe scripts/plugin_archive_fixtures.py

Every archive is built from the example pack. The good ones install; each
`bad-*` one must be refused with the problem code in its name and leave
nothing behind. Signatures use minisign's format with a fixed TEST key that
signs nothing else.
"""
import base64, gzip, hashlib, io, json, pathlib, sys, tarfile, zipfile
from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PrivateKey
from cryptography.hazmat.primitives import serialization

root = pathlib.Path(__file__).resolve().parent.parent
out = root / "crates/atlas-core/tests/fixtures/plugin-archives"
out.mkdir(parents=True, exist_ok=True)
manifest = (root / "examples/host-basics/infrawerf-plugin.json").read_bytes()
readme = b"# Host basics\n\nA test copy of the example pack.\n"
FIXED = (2026, 11, 17, 0, 0, 0)


def zip_bytes(entries, symlink=None):
    buffer = io.BytesIO()
    with zipfile.ZipFile(buffer, "w", zipfile.ZIP_DEFLATED) as archive:
        for name, data in entries:
            info = zipfile.ZipInfo(name, FIXED)
            info.compress_type = zipfile.ZIP_DEFLATED
            if name == symlink:
                info.create_system = 3
                info.external_attr = (0o120777 << 16)
            archive.writestr(info, data)
    return buffer.getvalue()


def tar_gz_bytes(entries):
    """entries: (name, data) or (name, kind, target) for special entries."""
    raw = io.BytesIO()
    with tarfile.open(fileobj=raw, mode="w", format=tarfile.PAX_FORMAT) as archive:
        for entry in entries:
            info = tarfile.TarInfo(entry[0])
            info.mtime = 1795392000
            if len(entry) == 3:
                info.type, info.linkname = entry[1], entry[2]
                archive.addfile(info)
            else:
                info.size = len(entry[1])
                archive.addfile(info, io.BytesIO(entry[1]))
    return gzip.compress(raw.getvalue(), mtime=0)


def write(name, data):
    (out / name).write_bytes(data)


good = [("infrawerf-plugin.json", manifest), ("README.md", readme)]
write("good.zip", zip_bytes(good))
write("good-folder.tar.gz", tar_gz_bytes([("host-basics/infrawerf-plugin.json", manifest), ("host-basics/docs/README.md", readme)]))

write("bad-path_traversal.zip", zip_bytes(good + [("../escaped.txt", b"x")]))
write("bad-path_traversal-backslash.zip", zip_bytes(good + [("..\\escaped.txt", b"x")]))
write("bad-path_traversal-absolute.tar.gz", tar_gz_bytes(good + [("/tmp/escaped.txt", b"x")]))
write("bad-path_traversal-drive.zip", zip_bytes(good + [("C:/escaped.txt", b"x")]))
write("bad-path_traversal-duplicate.zip", zip_bytes(good + [("readme.md", b"x")]))
write("bad-unsafe_entry-symlink.tar.gz", tar_gz_bytes(good + [("link", tarfile.SYMTYPE, "/etc/passwd")]))
write("bad-unsafe_entry-hardlink.tar.gz", tar_gz_bytes(good + [("link", tarfile.LNKTYPE, "README.md")]))
write("bad-unsafe_entry-symlink.zip", zip_bytes(good + [("link", b"/etc/passwd")], symlink="link"))
write("bad-too_large-bomb.zip", zip_bytes(good + [("zeros.bin", b"\0" * (30 * 1024 * 1024))]))
write("bad-too_large-files.zip", zip_bytes(good + [(f"f/{n}.txt", b"x") for n in range(300)]))
write("bad-too_large-bomb.tar.gz", tar_gz_bytes(good + [("zeros.bin", b"\0" * (30 * 1024 * 1024))]))
write("bad-no_manifest.zip", zip_bytes([("README.md", readme)]))
write("bad-no_manifest-two-folders.zip", zip_bytes([("a/infrawerf-plugin.json", manifest), ("b/README.md", readme)]))
write("bad-unsupported_archive.zip", b"This is text, not an archive.\n")
invalid = json.loads(manifest)
invalid["id"] = "builtin.example.host-basics"
write("bad-invalid_manifest.zip", zip_bytes([("infrawerf-plugin.json", json.dumps(invalid).encode())]))

# minisign, pre-hashed (algorithm "ED"): Ed25519 over BLAKE2b-512 of the file,
# then a global signature over the signature and the trusted comment.
def minisign_key(seed: bytes):
    key = Ed25519PrivateKey.from_private_bytes(hashlib.sha256(seed).digest())
    public = key.public_key().public_bytes(serialization.Encoding.Raw, serialization.PublicFormat.Raw)
    key_id = hashlib.sha256(public).digest()[:8]
    text = base64.b64encode(b"Ed" + key_id + public).decode()
    return key, key_id, f"untrusted comment: minisign public key {key_id[::-1].hex().upper()}\n{text}\n"


def minisign_sign(key, key_id, data, name):
    signature = key.sign(hashlib.blake2b(data, digest_size=64).digest())
    trusted = f"timestamp:1795392000\tfile:{name}\thashed"
    global_signature = key.sign(signature + trusted.encode())
    return (f"untrusted comment: signature from InfraWerf TEST key\n"
            f"{base64.b64encode(b'ED' + key_id + signature).decode()}\n"
            f"trusted comment: {trusted}\n"
            f"{base64.b64encode(global_signature).decode()}\n").encode()


key, key_id, public = minisign_key(b"infrawerf test author key; signs nothing real")
other, other_id, other_public = minisign_key(b"another infrawerf test key")
(out / "author.pub").write_text(public, newline="\n")
(out / "other.pub").write_text(other_public, newline="\n")
good_zip = (out / "good.zip").read_bytes()
write("good.zip.minisig", minisign_sign(key, key_id, good_zip, "good.zip"))
write("good-folder.tar.gz.minisig", minisign_sign(key, key_id, (out / "good-folder.tar.gz").read_bytes(), "good-folder.tar.gz"))
tampered = bytearray(good_zip)
tampered[-30] ^= 1
write("bad-signature_invalid-tampered.zip", bytes(tampered))
write("bad-signature_invalid-tampered.zip.minisig", (out / "good.zip.minisig").read_bytes())
print(json.dumps({"key_id": key_id[::-1].hex().upper(), "good.zip": hashlib.sha256(good_zip).hexdigest()}))
