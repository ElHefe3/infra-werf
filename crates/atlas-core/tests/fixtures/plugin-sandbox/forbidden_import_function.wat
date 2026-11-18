;; Imports a host function that is not in the contract: refused before it runs.
(module
  (import "infrawerf" "run_command" (func $run (param i32 i32) (result i64)))
  (memory (export "memory") 1)
  (func (export "alloc") (param i32) (result i32) (i32.const 0))
  (func (export "run") (result i32) (i32.const 0)))
