"""Publish a fixture's counters for the test harness that polls them.

The harness reads stats.json while the fixture is serving. On Windows,
replacing a file that another process has open fails with "Access is
denied" (PermissionError), even for a moment-long read. Raised inside an
SSH callback, that error was swallowed or turned into a protocol failure
and the file kept its old counts, so a poll such as "active connections
reach 0" could never succeed. The reader holds the file only briefly, so
retry until it lets go.
"""
import json
import sys
import time
from pathlib import Path


def publish(directory: Path, stats: dict) -> None:
    temporary = directory / 'stats-next.json'
    temporary.write_text(json.dumps(stats), encoding='utf-8')
    deadline = time.monotonic() + 5
    retries = 0
    while True:
        try:
            temporary.replace(directory / 'stats.json')
            if retries:
                print(f'stats.json was held open; published after {retries} retries', file=sys.stderr, flush=True)
            return
        except PermissionError:
            if time.monotonic() > deadline:
                raise
            retries += 1
            time.sleep(0.005)
