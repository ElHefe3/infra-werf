;; Recurses without end: stopped at the stack limit.
(module
  (memory (export "memory") 2)
  (global $heap (mut i32) (i32.const 8192))
  ;; A bump allocator: the host asks for room for each answer.
  (func (export "alloc") (param $len i32) (result i32)
    (local $p i32)
    (local.set $p (global.get $heap))
    (global.set $heap (i32.add (global.get $heap) (local.get $len)))
    (local.get $p))
  (func $down (param $n i32) (result i32) (call $down (i32.add (local.get $n) (i32.const 1))))
  (func (export "run") (result i32) (call $down (i32.const 0))))
