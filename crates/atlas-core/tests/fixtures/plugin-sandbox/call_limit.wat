;; Calls the host without end: stopped at the call limit.
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
  (data (i32.const 0) "again")
  (func (export "run") (result i32)
    (loop $storm (call $log (i32.const 0) (i32.const 5)) (br $storm))
    (i32.const 0)))
