"""Keeps InfraWerf's public repository free of an owner's private lab data (T050, D062).

The owner's own names, accounts, addresses and paths live in `.public-terms.json`
at the repository root. That file is ignored by Git and never leaves the owner's
PC; this script holds only generic rules.

    python -I scripts/public_repo.py check [--history] [--repo PATH]
        Fails if the tree (or, with --history, any blob or commit message in the
        repository's history) holds a private key, a token, a personal Windows
        path, a private address outside the documented test ranges, a personal
        e-mail address, or any of the owner's terms.

    python -I scripts/public_repo.py scrub
        Applies the owner's replacements to the tracked text files of the
        working tree. Review and commit the result.

    python -I scripts/public_repo.py copy --dest PATH [--ref REF]
        Writes a new repository at PATH (which must not exist) whose `main`
        branch is REF's full history with the owner's replacements applied to
        every file and commit message, then checks that history. The source
        repository is not changed.

`.public-terms.json`:

    {"replace": [{"pattern": "<regex>", "with": "<replacement>"}, ...],
     "forbid": ["<regex>", ...]}

Every `replace` pattern is also forbidden; `forbid` adds terms to check only.
"""
from __future__ import annotations

import argparse
import ipaddress
import json
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
TERMS_FILE = ".public-terms.json"

# Addresses that may appear: documentation and test values, not anyone's lab.
ALLOWED_NETWORKS = [ipaddress.ip_network(n) for n in ("10.0.0.0/24", "192.168.20.0/24", "192.168.1.10/32", "127.0.0.0/8", "192.0.2.0/24", "198.51.100.0/24", "203.0.113.0/24")]
ALLOWED_EMAIL = re.compile(rb"(?i)@(?:example\.(?:com|org|net)|users\.noreply\.github\.com|anthropic\.com)$|noreply"
                           # File names and unit names that look like addresses: 128x128@2x.png, worker@2.service.
                           rb"|\.(?:png|svg|ico|service|socket|timer|json|js|ts|tsx|rs|md)$")

GENERIC = {
    "private key": re.compile(rb"-----BEGIN [A-Z ]*PRIVATE KEY-----\s*\r?\n[A-Za-z0-9+/=]{40,}"),
    "token": re.compile(rb"\b(?:gh[pousr]_[A-Za-z0-9]{30,}|github_pat_[A-Za-z0-9_]{40,}|AKIA[0-9A-Z]{16}|xox[baprs]-[A-Za-z0-9-]{10,}|sk-[A-Za-z0-9]{32,}|glpat-[A-Za-z0-9_-]{20,})"),
    "personal Windows path": re.compile(rb"(?i)\b[a-z]:[\\/]+(?:\\)?users[\\/]+(?:\\)?(?!your-user\b|example\b|owner\b|public\b|default\b|<)[a-z0-9._-]+"),
    "private address": re.compile(rb"\b(?:10|172|192|100)\.\d{1,3}\.\d{1,3}\.\d{1,3}\b"),
    "e-mail address": re.compile(rb"\b[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}\b"),
}


def git(repo: Path, *args: str, input: bytes | None = None) -> bytes:
    return subprocess.run(["git", "-C", str(repo), *args], input=input, capture_output=True, check=True).stdout


def load_terms(repo: Path) -> tuple[list[tuple[re.Pattern, bytes]], list[re.Pattern]]:
    path = repo / TERMS_FILE
    if not path.exists():
        path = ROOT / TERMS_FILE
    if not path.exists():
        return [], []
    data = json.loads(path.read_text(encoding="utf-8"))
    replace = [(re.compile(r["pattern"].encode()), r["with"].encode()) for r in data.get("replace", [])]
    forbid = [p for p, _ in replace] + [re.compile(f.encode()) for f in data.get("forbid", [])]
    return replace, forbid


def is_binary(data: bytes) -> bool:
    return b"\0" in data[:8000]


def scrub_bytes(data: bytes, replace) -> bytes:
    if is_binary(data):
        return data
    for pattern, replacement in replace:
        data = pattern.sub(replacement, data)
    return data


def problems(data: bytes, forbid) -> list[str]:
    if is_binary(data):
        return []
    found = []
    for name, pattern in GENERIC.items():
        for match in pattern.finditer(data):
            value = match.group(0)
            if name == "private address":
                try:
                    address = ipaddress.ip_address(value.decode())
                except ValueError:
                    continue
                shared = address in ipaddress.ip_network("100.64.0.0/10")
                if not (address.is_private or shared) or any(address in n for n in ALLOWED_NETWORKS):
                    continue
            if name == "e-mail address" and ALLOWED_EMAIL.search(value):
                continue
            found.append(f"{name}: {value.decode(errors='replace')}")
    for pattern in forbid:
        for match in pattern.finditer(data):
            # The owner's own term is not echoed: say where, not what.
            found.append(f"owner term #{forbid.index(pattern) + 1} at byte {match.start()}")
    return found


def tracked(repo: Path) -> list[str]:
    return [p for p in git(repo, "ls-files", "-z").decode().split("\0") if p]


def check(repo: Path, history: bool) -> int:
    _, forbid = load_terms(repo)
    if not forbid:
        print(f"note: no {TERMS_FILE}; checking generic rules only", file=sys.stderr)
    failures = 0
    for path in tracked(repo):
        if path == TERMS_FILE:
            continue
        file = repo / path
        if not file.is_file():
            continue
        for problem in problems(file.read_bytes(), forbid):
            print(f"{path}: {problem}")
            failures += 1
    if history:
        paths = {}
        for line in git(repo, "rev-list", "--objects", "--all").decode(errors="replace").splitlines():
            sha, _, path = line.partition(" ")
            if path:
                paths.setdefault(sha, path)
        batch = subprocess.Popen(["git", "-C", str(repo), "cat-file", "--batch"], stdin=subprocess.PIPE, stdout=subprocess.PIPE)
        for sha, path in paths.items():
            batch.stdin.write(sha.encode() + b"\n")
            batch.stdin.flush()
            header = batch.stdout.readline().split()
            size = int(header[2])
            data = batch.stdout.read(size)
            batch.stdout.read(1)
            if header[1] != b"blob":
                continue
            for problem in problems(data, forbid):
                print(f"history {sha[:10]} {path}: {problem}")
                failures += 1
        batch.stdin.close()
        batch.wait()
        for record in git(repo, "log", "--all", "--format=%H%x00%an <%ae>%x00%cn <%ce>%x00%B%x01").split(b"\x01"):
            parts = record.strip(b"\n").split(b"\x00")
            if len(parts) < 4:
                continue
            for problem in problems(b"\n".join(parts[1:]), forbid):
                print(f"commit {parts[0][:10].decode()}: {problem}")
                failures += 1
    scope = "tree and history" if history else "tree"
    print(f"{failures} problem(s) in the {scope} of {repo}")
    return 1 if failures else 0


def scrub(repo: Path) -> int:
    replace, _ = load_terms(repo)
    if not replace:
        print(f"no {TERMS_FILE}: nothing to replace", file=sys.stderr)
        return 1
    changed = 0
    for path in tracked(repo):
        file = repo / path
        if path == TERMS_FILE or not file.is_file():
            continue
        before = file.read_bytes()
        after = scrub_bytes(before, replace)
        if after != before:
            file.write_bytes(after)
            print(f"scrubbed {path}")
            changed += 1
    print(f"{changed} file(s) changed")
    return 0


def copy(repo: Path, dest: Path, ref: str) -> int:
    replace, _ = load_terms(repo)
    if not replace:
        print(f"no {TERMS_FILE}: refusing to make a public copy without the owner's replacements", file=sys.stderr)
        return 1
    if dest.exists():
        print(f"{dest} exists; choose a new folder", file=sys.stderr)
        return 1
    full = git(repo, "rev-parse", "--symbolic-full-name", ref).decode().strip() or ref
    stream = git(repo, "fast-export", "--reencode=yes", "--signed-tags=strip", "--tag-of-filtered-object=drop",
                 "--refspec", f"{full}:refs/heads/main", full)
    out, i, blocks = bytearray(), 0, 0
    while i < len(stream):
        end = stream.index(b"\n", i) + 1
        line = stream[i:end]
        i = end
        if line.startswith(b"data "):
            size = int(line[5:].strip())
            data = stream[i:i + size]
            i += size
            data = scrub_bytes(data, replace)
            out += b"data %d\n" % len(data) + data
            blocks += 1
        else:
            out += line
    dest.mkdir(parents=True)
    git(dest, "init", "-q", "-b", "main")
    git(dest, "config", "core.autocrlf", "false")
    git(dest, "fast-import", "--quiet", input=bytes(out))
    git(dest, "reset", "-q", "--hard", "main")
    commits = int(git(dest, "rev-list", "--count", "main").strip())
    print(f"public copy at {dest}: {commits} commits, {blocks} files and messages rewritten where needed")
    return check(dest, history=True)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = parser.add_subparsers(dest="command", required=True)
    c = sub.add_parser("check")
    c.add_argument("--history", action="store_true")
    c.add_argument("--repo", type=Path, default=ROOT)
    sub.add_parser("scrub")
    p = sub.add_parser("copy")
    p.add_argument("--dest", type=Path, required=True)
    p.add_argument("--ref", default="HEAD")
    args = parser.parse_args()
    if args.command == "check":
        return check(args.repo.resolve(), args.history)
    if args.command == "scrub":
        return scrub(ROOT)
    return copy(ROOT, args.dest.resolve(), args.ref)


if __name__ == "__main__":
    sys.exit(main())
