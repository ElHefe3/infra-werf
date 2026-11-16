# InfraWerf plugin packs — commands and extractors (plugin API v2)

A pack is a plugin made only of data. From plugin API v2, a contribution at the `collector` or `health_check` point may carry one `command`: a read-only program to run on a server and a rule for reading its output. This page defines that. The rest of the manifest is in the [manifest reference](README.md); the finished example is in [`examples/host-basics`](../../examples/host-basics).

## What a pack can and cannot do

- **It runs through the owner's own SSH access**, in the same authenticated, host-pinned session and under the same limits as InfraWerf's built-in collectors. A pack never receives a key, a password or a session.
- **Nothing runs until the owner allows it.** The owner sees the exact command on the plugin's page in Settings, enables the plugin, and allows `ssh_session` and `run_approved_command` on each server. A command runs only when the owner presses Run for it; there is no schedule in API v2.
- **Every run is checked again.** If the owner withdraws a permission, disables the plugin, or turns all plugins off, the next run is refused, and a run in flight is cancelled and its process on the server is killed. The same happens when a run hits the time limit.
- **A command is a list of arguments, not a line of shell.** There are no pipes, redirects, variables, wildcards or quotes, because no shell reads it.
- **No sudo, shells, interpreters or network clients.** See [the program rule](#the-program-rule).
- **Results are not saved.** InfraWerf keeps the latest result of each command in memory until it closes or the plugin's permissions change. It never writes pack output to disk, and raw output never reaches the screen: only the extracted values do.

InfraWerf cannot prove that a program only reads. The rules below stop mistakes and obvious abuse; the owner reading the command is the real check. Write commands an owner can judge at a glance.

## Example

```json
{
  "id": "org.example.host-basics",
  "name": "Host basics",
  "version": "1.0.0",
  "api_version": 2,
  "description": "Disk usage, kernel version and two systemd health checks, read with standard commands.",
  "author": "Example Author",
  "license": "MIT",
  "capabilities": [
    "ssh_session",
    "run_approved_command"
  ],
  "contributions": [
    {
      "id": "disk-usage",
      "point": "collector",
      "title": "Disk usage",
      "capabilities": [
        "ssh_session",
        "run_approved_command"
      ],
      "command": {
        "argv": [
          "df",
          "-P",
          "-k"
        ],
        "extract": {
          "kind": "table",
          "columns": [
            "filesystem",
            "blocks_kib",
            "used_kib",
            "available_kib",
            "used_percent",
            "mounted_on"
          ],
          "skip_lines": 1
        }
      }
    },
    {
      "id": "kernel",
      "point": "collector",
      "title": "Kernel version",
      "capabilities": [
        "ssh_session",
        "run_approved_command"
      ],
      "command": {
        "argv": [
          "uname",
          "-r"
        ],
        "extract": {
          "kind": "line"
        }
      }
    },
    {
      "id": "system-state",
      "point": "health_check",
      "title": "System state",
      "capabilities": [
        "ssh_session",
        "run_approved_command"
      ],
      "command": {
        "argv": [
          "systemctl",
          "is-system-running"
        ],
        "extract": {
          "kind": "line"
        },
        "healthy": [
          "running"
        ]
      }
    },
    {
      "id": "service-active",
      "point": "health_check",
      "title": "Service is active",
      "capabilities": [
        "ssh_session",
        "run_approved_command"
      ],
      "command": {
        "argv": [
          "systemctl",
          "is-active",
          "{unit}"
        ],
        "parameters": [
          {
            "name": "unit",
            "source": "service"
          }
        ],
        "extract": {
          "kind": "line"
        },
        "healthy": [
          "active"
        ]
      }
    }
  ]
}
```

## The `command` object

| Field | Required | Rule |
| --- | --- | --- |
| `argv` | yes | The program, then its arguments: 1 to 16 entries. Each entry is 1 to 200 characters from letters, digits and `_ . , : / = + @ % -`, or is exactly `{name}` for a declared parameter. |
| `parameters` | no | Up to 4 values InfraWerf fills in. Each must be used in `argv`. |
| `extract` | yes | How the output is read: `line`, `table` or `json`. |
| `healthy` | health checks only | 1 to 8 values, each 1 to 64 characters, that mean healthy. |

A contribution with a `command` must list both `ssh_session` and `run_approved_command` in its `capabilities`, and the manifest must declare `"api_version": 2`.

### The program rule

The first entry of `argv` is a plain program name such as `df`, or an absolute path such as `/usr/bin/df`. It cannot be a parameter. These are refused with `forbidden_program`:

- shells and anything that runs another program: `sh`, `bash`, `env`, `xargs`, `find`, `awk`, `sed`, `timeout`, `nohup` and similar;
- interpreters: `python`, `perl`, `ruby`, `node`, `php`, `lua`;
- ways to become another user: `sudo`, `su`, `doas`, `pkexec`, `runuser`;
- network clients: `ssh`, `scp`, `rsync`, `curl`, `wget`, `nc`, `socat`, `telnet`;
- programs whose purpose is to change a system: `rm`, `mv`, `cp`, `dd`, `tee`, `chmod`, `chown`, `kill`, `reboot`, `shutdown`, `mount`, `mkdir`, `touch`, `tar`, `crontab` and similar.

The full list is `FORBIDDEN_PROGRAMS` in `crates/atlas-core/src/plugin.rs`.

<!-- expect: unsafe_argument -->
```json
{
  "id": "org.example.unsafe",
  "name": "Unsafe",
  "version": "1.0.0",
  "api_version": 2,
  "capabilities": ["ssh_session", "run_approved_command"],
  "contributions": [
    {
      "id": "big-files",
      "point": "collector",
      "title": "Big files",
      "capabilities": ["ssh_session", "run_approved_command"],
      "command": { "argv": ["du", "-a", "/var", "|", "sort", "-n"], "extract": { "kind": "line" } }
    }
  ]
}
```

That manifest is refused: `|` is not an argument a program would understand, and no shell is there to read it.

### Parameters

A parameter is a value that differs per server, such as a service name. The pack declares it; the owner picks its value from what InfraWerf discovered on that server. A pack cannot supply the value, and neither can anything typed by hand.

| Field | Rule |
| --- | --- |
| `name` | Lower-case letters, digits and underscores, starting with a letter, 32 characters at most. Written in `argv` as `{name}`, as a whole entry. |
| `source` | `service`: a system-scope systemd service from the server's inventory. `container`: a container from the server's inventory. |

Using `{name}` without declaring it is refused with `undeclared_parameter`; declaring one that `argv` never uses, with `unused_parameter`. When a command runs, InfraWerf checks the chosen workload again against what that authenticated session found, and refuses a name that is not a safe argument.

### Extractors

`extract` has a `kind` and the fields of that kind.

| `kind` | Fields | Result |
| --- | --- | --- |
| `line` | `line` (optional): which line, from 1 to 1000. Defaults to 1. | That line as one value. |
| `table` | `columns`: 1 to 12 names of lower-case letters, digits and underscores. `skip_lines` (optional): header lines to skip, 100 at most. `separator` (optional): `whitespace` (default), `tab` or `comma`. | One row per non-empty line. The last column takes the rest of the line. A line with too few cells makes the whole result unreadable. |
| `json` | `path`: dotted names with optional `[index]`, such as `pool.disks[0].state`, 200 characters at most. | The text, number or true/false value at that path. Objects and lists are not values. |

Every value has control characters removed and is cut to 200 characters. A table keeps 100 rows and says when there were more.

### Health checks

A `health_check` command reads one value (`line` or `json`) and lists the `healthy` values. The result is:

- **healthy** when the value is in the list;
- **unhealthy** for any other value;
- **unknown** when no value could be read, the command produced nothing, or it hit a limit.

A health check is judged by what the command printed even when the command reports failure, because status commands such as `systemctl is-active` print `inactive` and exit with an error. A collector needs the command to succeed.

## Limits

These are the limits of InfraWerf's built-in collectors, enforced by the same code:

| Limit | Value |
| --- | --- |
| Output of one run | 64 KiB. More ends the run as "output limit" with no result. |
| Time for one run | 30 seconds. Longer ends the run as "timed out" with no result. |
| Rows kept from a table | 100 |
| Characters kept per value | 200 |

## What the owner sees

- **Settings → Plugins → your plugin** lists each command exactly as written, with its parameters and how its output is read, before the plugin is enabled.
- **A server's Plugin data tab** lists the commands allowed on that server. Each has a Run button and shows its latest result with when it was read, whether it is still fresh (15 minutes) and which plugin produced it.
- **Settings → Plugin activity** records every run: what was asked, whether it was allowed, and whether the run finished, failed or was cancelled.

## Error codes

The codes for commands are listed with all others in the [manifest reference](README.md#error-codes).
