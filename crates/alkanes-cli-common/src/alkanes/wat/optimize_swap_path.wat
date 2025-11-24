;; Optimal Swap Path Finder
;; This WASM module finds the best swap path between two tokens using on-chain liquidity data
;;
;; Context inputs:
;;   inputs[0] = factory_block
;;   inputs[1] = factory_tx
;;   inputs[2] = input_token_block
;;   inputs[3] = input_token_tx
;;   inputs[4] = output_token_block
;;   inputs[5] = output_token_tx
;;   inputs[6] = input_amount
;;   inputs[7] = max_hops (1-4)
;;   inputs[8] = min_liquidity_threshold
;;
;; Returns: Optimal path as a sequence of AlkaneIds
;; Format: [path_length(16 bytes)][token0_block(16)][token0_tx(16)][token1_block(16)][token1_tx(16)]...

(module
  ;; Import runtime functions
  (import "env" "abort" (func $abort (param i32 i32 i32 i32)))
  (import "env" "__request_context" (func $__request_context (result i32)))
  (import "env" "__load_context" (func $__load_context (param i32)))
  (import "env" "__staticcall" (func $__staticcall (param i32 i32 i32 i64) (result i32)))
  (import "env" "__returndatacopy" (func $__returndatacopy (param i32)))
  (import "env" "__log" (func $__log (param i32)))

  ;; Memory declaration (16 pages = 1MB for complex path finding)
  (memory (export "memory") 16)

  ;; Memory layout:
  ;; 0-1023: Context buffer
  ;; 1024-2047: Working buffer for cellpacks
  ;; 2048-4095: Pool list (up to 128 pools with details)
  ;; 4096-8191: Path search working memory
  ;; 8192+: Response buffer

  ;; Global variables
  (global $context_ptr (mut i32) (i32.const 0))
  (global $factory_block (mut i64) (i64.const 0))
  (global $factory_tx (mut i64) (i64.const 0))
  (global $input_token_block (mut i64) (i64.const 0))
  (global $input_token_tx (mut i64) (i64.const 0))
  (global $output_token_block (mut i64) (i64.const 0))
  (global $output_token_tx (mut i64) (i64.const 0))
  (global $input_amount (mut i64) (i64.const 0))
  (global $max_hops (mut i32) (i32.const 2))
  (global $min_liquidity (mut i64) (i64.const 1000))
  (global $pool_count (mut i32) (i32.const 0))

  ;; Helper: Load u128 from memory (only uses lower 64 bits for now)
  (func $load_u128 (param $addr i32) (result i64)
    (local.get $addr)
    (i64.load)
  )

  ;; Helper: Store u128 to memory (only stores lower 64 bits for now)
  (func $store_u128 (param $addr i32) (param $val i64)
    (local.get $addr)
    (local.get $val)
    (i64.store)
    ;; Store upper 64 bits as 0
    (local.get $addr)
    (i32.const 8)
    (i32.add)
    (i64.const 0)
    (i64.store)
  )

  ;; Parse context and extract inputs
  (func $parse_context
    (local $size i32)
    (local $ptr i32)
    (local $inputs_offset i32)

    ;; Request context size
    (call $__request_context)
    (local.set $size)

    ;; Allocate context buffer
    (i32.const 0)
    (local.set $ptr)
    (global.set $context_ptr (local.get $ptr))

    ;; Load context
    (local.get $ptr)
    (call $__load_context)
    (drop)

    ;; Parse context:
    ;; - myself: AlkaneId (32 bytes)
    ;; - caller: AlkaneId (32 bytes)
    ;; - vout: u128 (16 bytes)
    ;; - incoming_alkanes: AlkaneTransferParcel (variable, but we can skip)
    ;; - inputs: Vec<u128>
    
    ;; Skip to inputs (simplified: skip 80 bytes)
    (local.get $ptr)
    (i32.const 80)
    (i32.add)
    (local.set $inputs_offset)

    ;; Load inputs
    (global.set $factory_block (call $load_u128 (local.get $inputs_offset)))
    (global.set $factory_tx (call $load_u128 (i32.add (local.get $inputs_offset) (i32.const 16))))
    (global.set $input_token_block (call $load_u128 (i32.add (local.get $inputs_offset) (i32.const 32))))
    (global.set $input_token_tx (call $load_u128 (i32.add (local.get $inputs_offset) (i32.const 48))))
    (global.set $output_token_block (call $load_u128 (i32.add (local.get $inputs_offset) (i32.const 64))))
    (global.set $output_token_tx (call $load_u128 (i32.add (local.get $inputs_offset) (i32.const 80))))
    (global.set $input_amount (call $load_u128 (i32.add (local.get $inputs_offset) (i32.const 96))))
    (global.set $max_hops (i32.wrap_i64 (call $load_u128 (i32.add (local.get $inputs_offset) (i32.const 112)))))
    (global.set $min_liquidity (call $load_u128 (i32.add (local.get $inputs_offset) (i32.const 128))))
  )

  ;; Build cellpack for calling factory
  (func $build_cellpack (param $opcode i64) (result i32)
    (local $cellpack_ptr i32)
    (local $offset i32)

    ;; Cellpack at 1024
    (i32.const 1024)
    (local.set $cellpack_ptr)
    (i32.const 0)
    (local.set $offset)

    ;; Format: [target_block(16)][target_tx(16)][inputs_count(16)][input0(16)]...
    
    ;; Store target (factory)
    (call $store_u128 
      (i32.add (local.get $cellpack_ptr) (local.get $offset))
      (global.get $factory_block))
    (local.set $offset (i32.add (local.get $offset) (i32.const 16)))

    (call $store_u128 
      (i32.add (local.get $cellpack_ptr) (local.get $offset))
      (global.get $factory_tx))
    (local.set $offset (i32.add (local.get $offset) (i32.const 16)))

    ;; Store inputs count (1 = opcode)
    (call $store_u128 
      (i32.add (local.get $cellpack_ptr) (local.get $offset))
      (i64.const 1))
    (local.set $offset (i32.add (local.get $offset) (i32.const 16)))

    ;; Store opcode
    (call $store_u128 
      (i32.add (local.get $cellpack_ptr) (local.get $offset))
      (local.get $opcode))

    (local.get $cellpack_ptr)
  )

  ;; Call factory to get all pools
  (func $get_all_pools (result i32)
    (local $cellpack i32)
    (local $result i32)
    (local $response_size i32)

    ;; Build cellpack for opcode 3 (GET_ALL_POOLS)
    (call $build_cellpack (i64.const 3))
    (local.set $cellpack)

    ;; Make staticcall (no alkanes, no checkpoint, max fuel)
    (call $__staticcall 
      (local.get $cellpack)
      (i32.const 0)  ;; No incoming alkanes
      (i32.const 0)  ;; No checkpoint
      (i64.const 0xFFFFFFFFFFFFFFFF))  ;; Max fuel
    (local.set $result)

    ;; Check result (0 = success)
    (local.get $result)
    (i32.const 0)
    (i32.ne)
    (if
      (then
        ;; Call failed
        (i32.const 0)
        (return)
      )
    )

    ;; Copy return data to pool list buffer (2048)
    (i32.const 2048)
    (call $__returndatacopy)

    ;; Parse pool count (first 16 bytes)
    (i32.const 2048)
    (i64.load)
    (i32.wrap_i64)
    (global.set $pool_count)

    (global.get $pool_count)
  )

  ;; Simple path finder: just returns direct path
  ;; TODO: Implement multi-hop path finding
  (func $find_optimal_path (result i32)
    (local $response_ptr i32)
    (local $offset i32)

    ;; For now, just return a simple direct path
    ;; Response buffer at 8192
    (i32.const 8192)
    (local.set $response_ptr)
    (i32.const 0)
    (local.set $offset)

    ;; Path length (2 tokens = 1 hop)
    (call $store_u128 
      (i32.add (local.get $response_ptr) (local.get $offset))
      (i64.const 2))
    (local.set $offset (i32.add (local.get $offset) (i32.const 16)))

    ;; Input token
    (call $store_u128 
      (i32.add (local.get $response_ptr) (local.get $offset))
      (global.get $input_token_block))
    (local.set $offset (i32.add (local.get $offset) (i32.const 16)))

    (call $store_u128 
      (i32.add (local.get $response_ptr) (local.get $offset))
      (global.get $input_token_tx))
    (local.set $offset (i32.add (local.get $offset) (i32.const 16)))

    ;; Output token
    (call $store_u128 
      (i32.add (local.get $response_ptr) (local.get $offset))
      (global.get $output_token_block))
    (local.set $offset (i32.add (local.get $offset) (i32.const 16)))

    (call $store_u128 
      (i32.add (local.get $response_ptr) (local.get $offset))
      (global.get $output_token_tx))

    ;; Return total size
    (i32.const 80)  ;; 16 + 16 + 16 + 16 + 16
  )

  ;; Main execution function
  (func (export "__execute") (result i32)
    (local $pool_count i32)
    (local $path_size i32)

    ;; Parse context inputs
    (call $parse_context)

    ;; Get all pools from factory
    (call $get_all_pools)
    (local.set $pool_count)

    ;; Find optimal path
    (call $find_optimal_path)
    (local.set $path_size)

    ;; Return pointer to result
    (i32.const 8192)
  )
)
