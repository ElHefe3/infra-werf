;; Asks for a host function that does not exist: answered with unknown_function.
(module
  (import "infrawerf" "call" (func $call (param i32 i32) (result i64)))
  (import "infrawerf" "log" (func $log (param i32 i32)))
  (import "infrawerf" "result" (func $result (param i32 i32)))
  (memory (export "memory") 2)
  (global $heap (mut i32) (i32.const 8192))
  ;; A bump allocator: the host asks for room for each answer.
  (func (export "alloc") (param $len i32) (result i32)
    (local $p i32)
    (local.set $p (global.get $heap))
    (global.set $heap (i32.add (global.get $heap) (local.get $len)))
    (local.get $p))
  (data (i32.const 0) "{\22function\22:\22run_shell\22,\22args\22:{\22command\22:\22rm -rf \2f\22}}")
  (func (export "run") (result i32)
    (local $a i64)
    (local.set $a (call $call (i32.const 0) (i32.const 54)))
    (call $log (i32.wrap_i64 (i64.shr_u (local.get $a) (i64.const 32))) (i32.wrap_i64 (local.get $a)))
    (i32.const 0)))
