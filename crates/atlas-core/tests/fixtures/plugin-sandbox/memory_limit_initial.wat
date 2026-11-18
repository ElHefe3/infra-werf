;; Asks for 64 MiB at start: refused at the memory limit.
(module
  (memory (export "memory") 1024)
  (func (export "alloc") (param i32) (result i32) (i32.const 0))
  (func (export "run") (result i32) (i32.const 0)))
