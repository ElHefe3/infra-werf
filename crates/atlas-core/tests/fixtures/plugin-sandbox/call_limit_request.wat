;; Passes a 2 MB request: refused as too large.
(module
  (import "infrawerf" "call" (func $call (param i32 i32) (result i64)))
  (import "infrawerf" "log" (func $log (param i32 i32)))
  (import "infrawerf" "result" (func $result (param i32 i32)))
  (memory (export "memory") 32)
  (func (export "alloc") (param i32) (result i32) (i32.const 0))
  (func (export "run") (result i32)
    (drop (call $call (i32.const 0) (i32.const 2000000)))
    (i32.const 0)))
