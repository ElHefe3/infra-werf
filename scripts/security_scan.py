"""Run scanners without exposing their reports in public Actions logs."""

import json
import os
from pathlib import Path
import subprocess
import sys


def run(command):
    try:
        return subprocess.run(command, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, check=False).returncode
    except OSError:
        return -1


def sarif_valid(path):
    try:
        data = json.loads(Path(path).read_text(encoding="utf-8"))
        return data.get("version") == "2.1.0" and isinstance(data.get("runs"), list) and bool(data["runs"])
    except (OSError, ValueError, AttributeError):
        return False


def main(args):
    if len(args) < 2:
        return 2
    kind = args[1]
    if kind == "osv" and len(args) == 5:
        executable, source, output = args[2:]
        Path(output).unlink(missing_ok=True)
        status = run([executable, "scan", "source", "--lockfile", source, "--format", "sarif", "--output", output])
        if status not in (0, 1) or not sarif_valid(output):
            print("Dependency scan failed or did not produce valid SARIF.")
            return 1
        print("Dependency scan completed; findings are available in code scanning.")
        return 0
    if kind == "semgrep" and len(args) == 3:
        output = args[2]
        Path(output).unlink(missing_ok=True)
        configs = ["rust/lang/security", "python/lang/security", "javascript/lang/security", "typescript/lang/security", "typescript/react/security"]
        command = ["semgrep", "scan", "--metrics=off", "--disable-version-check", "--strict", "--sarif", "--output", output, "--exclude", ".security-rules"]
        for config in configs:
            command += ["--config", os.path.join(".security-rules", config)]
        status = run(command + ["."])
        if status != 0 or not sarif_valid(output):
            print("Source scan failed or did not produce valid SARIF.")
            return 1
        print("Source scan completed; findings are available in code scanning.")
        return 0
    if kind == "gitleaks" and len(args) == 3:
        status = run([args[2], "git", "--redact", "--exit-code", "42", "--log-opts=--all", "."])
        if status == 42:
            print("Secret scan completed with findings; investigate privately.")
            return 0
        if status != 0:
            print("Secret scan failed; investigate privately.")
            return 1
        print("Secret scan completed without findings.")
        return 0
    print("Invalid security scan invocation.")
    return 2


if __name__ == "__main__":
    sys.exit(main(sys.argv))
