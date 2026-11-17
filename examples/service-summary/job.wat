;; Service summary: reads the server's workloads through InfraWerf and
;; returns them as its result. It changes nothing.
;; Build: the test `job_modules_match_their_sources` writes job.wasm.
(module
  (import "infrawerf" "call" (func $call (param i32 i32) (result i64)))
  (import "infrawerf" "log" (func $log (param i32 i32)))
  (import "infrawerf" "result" (func $result (param i32 i32)))
  (memory (export "memory") 2)
  (global $heap (mut i32) (i32.const 16384))
  ;; A bump allocator: the host asks for room for each answer.
  (func (export "alloc") (param $len i32) (result i32)
    (local $p i32)
    (local.set $p (global.get $heap))
    (global.set $heap (i32.add (global.get $heap) (local.get $len)))
    (local.get $p))
  ;; An answer comes back as (ptr << 32) | len.
  (func $ptr (param $a i64) (result i32) (i32.wrap_i64 (i64.shr_u (local.get $a) (i64.const 32))))
  (func $len (param $a i64) (result i32) (i32.wrap_i64 (local.get $a)))
  (data (i32.const 0) "service summary: reading this server's workloads")
  (data (i32.const 512) "{\22function\22:\22workloads\22,\22args\22:{}}")
  (func (export "run") (result i32)
    (local $answer i64)
    (call $log (i32.const 0) (i32.const 48))
    (local.set $answer (call $call (i32.const 512) (i32.const 34)))
    (call $result (call $ptr (local.get $answer)) (call $len (local.get $answer)))
    (i32.const 0)))
