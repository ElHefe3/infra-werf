"""Writes the WebAssembly text fixtures for the plugin sandbox (T048).

    python scripts/plugin_sandbox_fixtures.py

`prototype.wat` is the prototype plugin: it reads through granted host
functions. Every other file is a runaway or escape attempt; its name says
how it must be stopped.
"""
import json, pathlib

out = pathlib.Path(__file__).resolve().parent.parent / "crates/atlas-core/tests/fixtures/plugin-sandbox"
out.mkdir(parents=True, exist_ok=True)

IMPORTS = """  (import "infrawerf" "call" (func $call (param i32 i32) (result i64)))
  (import "infrawerf" "log" (func $log (param i32 i32)))
  (import "infrawerf" "result" (func $result (param i32 i32)))"""

ALLOC = """  (memory (export "memory") 2)
  (global $heap (mut i32) (i32.const 8192))
  ;; A bump allocator: the host asks for room for each answer.
  (func (export "alloc") (param $len i32) (result i32)
    (local $p i32)
    (local.set $p (global.get $heap))
    (global.set $heap (i32.add (global.get $heap) (local.get $len)))
    (local.get $p))"""


def wat_string(text: str) -> str:
    return "".join(c if c.isalnum() or c in " :,{}[]_-." else "\\%02x" % ord(c) for c in text)


def data(offset: int, text: str) -> tuple[str, int]:
    return f'  (data (i32.const {offset}) "{wat_string(text)}")', len(text.encode())


def write(name: str, body: str, comment: str) -> None:
    (out / f"{name}.wat").write_text(f";; {comment}\n{body}\n", newline="\n")


# The prototype: reads the server and its inventory, logs both answers and
# returns the server answer as its result.
server_req, server_len = data(0, json.dumps({"function": "server", "args": {}}, separators=(",", ":")))
inv_req, inv_len = data(256, json.dumps({"function": "inventory", "args": {}}, separators=(",", ":")))
note, note_len = data(512, "prototype: reading the server and its inventory")
write("prototype", f"""(module
{IMPORTS}
{ALLOC}
{server_req}
{inv_req}
{note}
  ;; ptr and len of an answer packed as (ptr << 32) | len
  (func $ptr (param $a i64) (result i32) (i32.wrap_i64 (i64.shr_u (local.get $a) (i64.const 32))))
  (func $len (param $a i64) (result i32) (i32.wrap_i64 (local.get $a)))
  (func (export "run") (result i32)
    (local $server i64) (local $inventory i64)
    (call $log (i32.const 512) (i32.const {note_len}))
    (local.set $server (call $call (i32.const 0) (i32.const {server_len})))
    (call $log (call $ptr (local.get $server)) (call $len (local.get $server)))
    (local.set $inventory (call $call (i32.const 256) (i32.const {inv_len})))
    (call $log (call $ptr (local.get $inventory)) (call $len (local.get $inventory)))
    (call $result (call $ptr (local.get $server)) (call $len (local.get $server)))
    (i32.const 0)))""", "The prototype plugin: reads through granted host functions.")

unknown_req, unknown_len = data(0, json.dumps({"function": "run_shell", "args": {"command": "rm -rf /"}}, separators=(",", ":")))
write("unknown_function", f"""(module
{IMPORTS}
{ALLOC}
{unknown_req}
  (func (export "run") (result i32)
    (local $a i64)
    (local.set $a (call $call (i32.const 0) (i32.const {unknown_len})))
    (call $log (i32.wrap_i64 (i64.shr_u (local.get $a) (i64.const 32))) (i32.wrap_i64 (local.get $a)))
    (i32.const 0)))""", "Asks for a host function that does not exist: answered with unknown_function.")

write("out_of_fuel", f"""(module
{ALLOC}
  (func (export "run") (result i32)
    (loop $forever (br $forever))
    (i32.const 0)))""", "Loops forever: stopped by fuel.")

write("start_loop", f"""(module
{ALLOC}
  (func $start (loop $forever (br $forever)))
  (start $start)
  (func (export "run") (result i32) (i32.const 0)))""", "Loops forever in its start function, before run: stopped by fuel.")

write("memory_limit", f"""(module
{ALLOC}
  (func (export "run") (result i32)
    (loop $grow (drop (memory.grow (i32.const 16))) (br $grow))
    (i32.const 0)))""", "Grows its memory without end: stopped at the memory limit.")

write("memory_limit_initial", f"""(module
  (memory (export "memory") 1024)
  (func (export "alloc") (param i32) (result i32) (i32.const 0))
  (func (export "run") (result i32) (i32.const 0)))""", "Asks for 64 MiB at start: refused at the memory limit.")

write("stack_limit", f"""(module
{ALLOC}
  (func $down (param $n i32) (result i32) (call $down (i32.add (local.get $n) (i32.const 1))))
  (func (export "run") (result i32) (call $down (i32.const 0))))""", "Recurses without end: stopped at the stack limit.")

msg, msg_len = data(0, "again")
write("call_limit", f"""(module
{IMPORTS}
{ALLOC}
{msg}
  (func (export "run") (result i32)
    (loop $storm (call $log (i32.const 0) (i32.const {msg_len})) (br $storm))
    (i32.const 0)))""", "Calls the host without end: stopped at the call limit.")

write("call_limit_request", f"""(module
{IMPORTS}
  (memory (export "memory") 32)
  (func (export "alloc") (param i32) (result i32) (i32.const 0))
  (func (export "run") (result i32)
    (drop (call $call (i32.const 0) (i32.const 2000000)))
    (i32.const 0)))""", "Passes a 2 MB request: refused as too large.")

write("contract_violation_pointer", f"""(module
{IMPORTS}
{ALLOC}
  (func (export "run") (result i32)
    (drop (call $call (i32.const 2147418112) (i32.const 64)))
    (i32.const 0)))""", "Points the host outside its memory: stopped, the host reads nothing.")

bad_req, bad_len = data(0, json.dumps({"function": "server", "args": {}}, separators=(",", ":")))
write("contract_violation_alloc", f"""(module
{IMPORTS}
  (memory (export "memory") 1)
{bad_req}
  ;; alloc hands back an address past the end of memory.
  (func (export "alloc") (param i32) (result i32) (i32.const 65530))
  (func (export "run") (result i32)
    (drop (call $call (i32.const 0) (i32.const {bad_len})))
    (i32.const 0)))""", "Its alloc returns memory the answer cannot fit in: stopped, nothing is written outside.")

write("contract_violation_no_run", f"""(module
{ALLOC})""", "Exports no run function.")

write("forbidden_import_wasi", """(module
  (import "wasi_snapshot_preview1" "fd_write" (func $fd_write (param i32 i32 i32 i32) (result i32)))
  (memory (export "memory") 1)
  (func (export "alloc") (param i32) (result i32) (i32.const 0))
  (func (export "run") (result i32) (i32.const 0)))""", "Imports the WASI system interface: refused before it runs.")

write("forbidden_import_memory", """(module
  (import "infrawerf" "memory" (memory 1))
  (func (export "alloc") (param i32) (result i32) (i32.const 0))
  (func (export "run") (result i32) (i32.const 0)))""", "Asks InfraWerf for its memory: refused before it runs.")

write("forbidden_import_function", """(module
  (import "infrawerf" "run_command" (func $run (param i32 i32) (result i64)))
  (memory (export "memory") 1)
  (func (export "alloc") (param i32) (result i32) (i32.const 0))
  (func (export "run") (result i32) (i32.const 0)))""", "Imports a host function that is not in the contract: refused before it runs.")

write("trapped", f"""(module
{ALLOC}
  (func (export "run") (result i32) unreachable))""", "Traps.")

write("cancelled", f"""(module
{IMPORTS}
{ALLOC}
{msg}
  (func (export "run") (result i32)
    (loop $more (call $log (i32.const 0) (i32.const {msg_len})) (br $more))
    (i32.const 0)))""", "Keeps calling while its permissions are withdrawn: stopped as cancelled.")

(out / "invalid_module.wasm").write_bytes(b"this is not WebAssembly\n")
print(sorted(p.name for p in out.iterdir()))
