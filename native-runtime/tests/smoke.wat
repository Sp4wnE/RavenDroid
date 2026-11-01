(module
  (memory 1)
  (func (export "run") (result i32) i32.const 42)
  (func (export "bounds_trap") (result i32)
    i32.const 65536
    i32.load))
