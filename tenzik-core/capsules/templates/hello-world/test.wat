;; Simple test WASM module for Tenzik testing
;; This module demonstrates the basic interface expected by Tenzik capsules

(module
  ;; Import host functions (if needed)
  ;; (import "env" "hash_commit" (func $hash_commit (param i32 i32) (result i32)))
  
  ;; Export memory so host can read/write data
  (memory (export "memory") 1)
  
  ;; Main run function - Tenzik capsules must export this
  ;; Parameters: (input_ptr: i32, input_len: i32) -> i32 (encoded output_len << 16 | output_ptr)
  (func (export "run") (param $input_ptr i32) (param $input_len i32) (result i32)
    (local $output_ptr i32)
    (local $output_len i32)
    
    ;; Set output pointer to start after input (1KB offset + input size)
    (local.set $output_ptr (i32.add (i32.const 1024) (local.get $input_len)))
    
    ;; Simple test: copy input to output with "Hello " prefix
    (i32.store8 (local.get $output_ptr) (i32.const 72))      ;; 'H'
    (i32.store8 (i32.add (local.get $output_ptr) (i32.const 1)) (i32.const 101))   ;; 'e'
    (i32.store8 (i32.add (local.get $output_ptr) (i32.const 2)) (i32.const 108))   ;; 'l'
    (i32.store8 (i32.add (local.get $output_ptr) (i32.const 3)) (i32.const 108))   ;; 'l'
    (i32.store8 (i32.add (local.get $output_ptr) (i32.const 4)) (i32.const 111))   ;; 'o'
    (i32.store8 (i32.add (local.get $output_ptr) (i32.const 5)) (i32.const 32))    ;; ' '
    
    ;; Copy input after "Hello "
    (call $memcpy 
      (i32.add (local.get $output_ptr) (i32.const 6))
      (local.get $input_ptr)
      (local.get $input_len))
    
    ;; Set output length
    (local.set $output_len (i32.add (i32.const 6) (local.get $input_len)))
    
    ;; Return encoded result: (output_len << 16) | output_ptr
    (i32.or 
      (i32.shl (local.get $output_len) (i32.const 16))
      (local.get $output_ptr))
  )
  
  ;; Helper function to copy memory
  (func $memcpy (param $dest i32) (param $src i32) (param $len i32)
    (local $i i32)
    (local.set $i (i32.const 0))
    (block $break
      (loop $continue
        (br_if $break (i32.ge_u (local.get $i) (local.get $len)))
        (i32.store8
          (i32.add (local.get $dest) (local.get $i))
          (i32.load8_u (i32.add (local.get $src) (local.get $i))))
        (local.set $i (i32.add (local.get $i) (i32.const 1)))
        (br $continue)
      )
    )
  )
)
