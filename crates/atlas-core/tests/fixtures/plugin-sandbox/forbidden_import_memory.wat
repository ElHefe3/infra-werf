;; Asks InfraWerf for its memory: refused before it runs.
(module
  (import "infrawerf" "memory" (memory 1))
  (func (export "alloc") (param i32) (result i32) (i32.const 0))
  (func (export "run") (result i32) (i32.const 0)))
