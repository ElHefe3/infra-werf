;; Loops forever in its start function, before run: stopped by fuel.
(module
  (memory (export "memory") 2)
  (global $heap (mut i32) (i32.const 8192))
  ;; A bump allocator: the host asks for room for each answer.
  (func (export "alloc") (param $len i32) (result i32)
    (local $p i32)
    (local.set $p (global.get $heap))
    (global.set $heap (i32.add (global.get $heap) (local.get $len)))
    (local.get $p))
  (func $start (loop $forever (br $forever)))
  (start $start)
  (func (export "run") (result i32) (i32.const 0)))
