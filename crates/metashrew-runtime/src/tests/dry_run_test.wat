;; crates/metashrew-runtime/src/tests/dry_run_test.wat
(module
  (import "env" "__get" (func $__get (param i32 i32)))
  (import "env" "__flush" (func $__flush (param i32)))
  (import "env" "__is_dry_run" (func $__is_dry_run (result i32)))
  (import "env" "__log" (func $__log (param i32)))
  (import "env" "__host_len" (func $__host_len (result i32)))
  (import "env" "__load_input" (func $__load_input (param i32)))
  (import "env" "abort" (func $abort (param i32 i32 i32 i32)))
  (memory (export "memory") 1)
  
  (data (i32.const 0) "read_key_1")
  (data (i32.const 10) "read_key_2")
  (data (i32.const 20) "write_key_1")
  (data (i32.const 31) "write_value_1")
  (data (i32.const 44) "write_key_2")
  (data (i32.const 55) "write_value_2")

  ;; A simple utility to create an ArrayBuffer in memory for passing to host functions.
  ;; It takes a pointer to the raw data and its length, and writes the ArrayBuffer
  ;; structure (length prefix + data) to a destination pointer.
  (func $create_array_buffer (param $dest_ptr i32) (param $src_ptr i32) (param $len i32) (result i32)
    (i32.store (local.get $dest_ptr) (local.get $len))
    (memory.copy 
      (i32.add (local.get $dest_ptr) (i32.const 4))
      (local.get $src_ptr)
      (local.get $len)
    )
    (i32.add (local.get $dest_ptr) (i32.const 4))
  )

  ;; This function creates a valid, though minimal, KeyValueFlush protobuf message.
  ;; It contains two key-value pairs: ("write_key_1", "write_value_1") and ("write_key_2", "write_value_2").
  ;; The bytes represent the protobuf wire format.
  (data (i32.const 500) "\0a\1a\0a\0bwrite_key_1\12\0dwrite_value_1\0a\1a\0a\0bwrite_key_2\12\0dwrite_value_2")
  (func $create_flush_payload (param $dest_ptr i32) (result i32)
    (call $create_array_buffer
      (local.get $dest_ptr)
      (i32.const 500)
      (i32.const 56) ;; Total length of the protobuf message
    )
  )

  (func (export "_start")
    (if (call $__is_dry_run)
      (then
        ;; Read key 1
        (call $__get
          (call $create_array_buffer (i32.const 100) (i32.const 0) (i32.const 10))
          (i32.const 150)
        )
        (call $__get
          (call $create_array_buffer (i32.const 200) (i32.const 10) (i32.const 10))
          (i32.const 250)
        )
        ;; Flush two key-value pairs
        (call $__flush (call $create_flush_payload (i32.const 300)))
      )
    )
  )
)