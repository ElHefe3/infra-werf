;; Its alloc returns memory the answer cannot fit in: stopped, nothing is written outside.
(module
  (import "infrawerf" "call" (func $call (param i32 i32) (result i64)))
  (import "infrawerf" "log" (func $log (param i32 i32)))
  (import "infrawerf" "result" (func $result (param i32 i32)))
  (memory (export "memory") 1)
  (data (i32.const 0) "{\22function\22:\22server\22,\22args\22:{}}")
  ;; alloc hands back an address past the end of memory.
  (func (export "alloc") (param i32) (result i32) (i32.const 65530))
  (func (export "run") (result i32)
    (drop (call $call (i32.const 0) (i32.const 31)))
    (i32.const 0)))
