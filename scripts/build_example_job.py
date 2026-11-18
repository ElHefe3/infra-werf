"""Builds the Systemd backups example's job module (T051).

    mise run exec -- python -I scripts/build_example_job.py [--check]

Compiles examples/systemd-backups-job for wasm32-unknown-unknown and copies the
module to examples/systemd-backups/job.wasm. Paths on the builder's PC
(Cargo's registry, this repository) are rewritten to /cargo and /src, so the
module is the same on every PC and carries no user name. With --check, it
builds and fails if the committed module differs, without replacing it.

Needs the target once: rustup target add wasm32-unknown-unknown
"""
import os
import shutil
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
CRATE = ROOT / "examples" / "systemd-backups-job"
MODULE = ROOT / "examples" / "systemd-backups" / "job.wasm"
BUILT = CRATE / "target" / "wasm32-unknown-unknown" / "release" / "systemd_backups_job.wasm"


def main() -> int:
    cargo_home = Path(os.environ.get("CARGO_HOME", Path.home() / ".cargo")).resolve()
    remaps = [f"--remap-path-prefix={cargo_home}=/cargo", f"--remap-path-prefix={ROOT}=/src"]
    env = dict(os.environ, RUSTFLAGS=" ".join(remaps), CARGO_ENCODED_RUSTFLAGS="\x1f".join(remaps))
    subprocess.run(
        ["cargo", "build", "--release", "--locked", "--target", "wasm32-unknown-unknown",
         "--manifest-path", str(CRATE / "Cargo.toml")],
        check=True, env=env,
    )
    built = BUILT.read_bytes()
    for marker in (str(cargo_home).encode(), str(ROOT).encode(), str(Path.home()).encode()):
        if marker in built:
            print(f"the module still holds a local path ({marker.decode()}); not copied", file=sys.stderr)
            return 1
    if "--check" in sys.argv:
        same = MODULE.exists() and MODULE.read_bytes() == built
        print("job.wasm matches its source" if same else "job.wasm differs from its source: rebuild it")
        return 0 if same else 1
    shutil.copyfile(BUILT, MODULE)
    print(f"wrote {MODULE.relative_to(ROOT)} ({len(built)} bytes)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
