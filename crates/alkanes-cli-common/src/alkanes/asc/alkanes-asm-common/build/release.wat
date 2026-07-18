(module
 (type $0 (func (param i32)))
 (type $1 (func (param i32) (result i32)))
 (type $2 (func (result i32)))
 (type $3 (func (param i32 i32 i32 i64) (result i32)))
 (type $4 (func (param i32 i32) (result i32)))
 (type $5 (func))
 (type $6 (func (param i32 i32 i32) (result i32)))
 (type $7 (func (param i32 i32 i32 i32)))
 (type $8 (func (param i32 i32 i32)))
 (import "env" "abort" (func $assembly/alkanes/runtime/abort (param i32 i32 i32 i32)))
 (import "env" "__load_storage" (func $assembly/alkanes/runtime/__load_storage (param i32 i32) (result i32)))
 (import "env" "__request_storage" (func $assembly/alkanes/runtime/__request_storage (param i32) (result i32)))
 (import "env" "__log" (func $assembly/alkanes/runtime/__log (param i32)))
 (import "env" "__balance" (func $assembly/alkanes/runtime/__balance (param i32 i32 i32)))
 (import "env" "__request_context" (func $assembly/alkanes/runtime/__request_context (result i32)))
 (import "env" "__load_context" (func $assembly/alkanes/runtime/__load_context (param i32) (result i32)))
 (import "env" "__sequence" (func $assembly/alkanes/runtime/__sequence (param i32)))
 (import "env" "__fuel" (func $assembly/alkanes/runtime/__fuel (param i32)))
 (import "env" "__height" (func $assembly/alkanes/runtime/__height (param i32)))
 (import "env" "__returndatacopy" (func $assembly/alkanes/runtime/__returndatacopy (param i32)))
 (import "env" "__request_transaction" (func $assembly/alkanes/runtime/__request_transaction (result i32)))
 (import "env" "__load_transaction" (func $assembly/alkanes/runtime/__load_transaction (param i32)))
 (import "env" "__request_block" (func $assembly/alkanes/runtime/__request_block (result i32)))
 (import "env" "__load_block" (func $assembly/alkanes/runtime/__load_block (param i32)))
 (import "env" "__call" (func $assembly/alkanes/runtime/__call (param i32 i32 i32 i64) (result i32)))
 (import "env" "__staticcall" (func $assembly/alkanes/runtime/__staticcall (param i32 i32 i32 i64) (result i32)))
 (import "env" "__delegatecall" (func $assembly/alkanes/runtime/__delegatecall (param i32 i32 i32 i64) (result i32)))
 (global $assembly/alkanes/responder/ExtcallType.CALL i32 (i32.const 0))
 (global $assembly/alkanes/responder/ExtcallType.STATICCALL i32 (i32.const 1))
 (global $assembly/alkanes/responder/ExtcallType.DELEGATECALL i32 (i32.const 2))
 (global $~lib/rt/stub/offset (mut i32) (i32.const 0))
 (global $~lib/rt/__rtti_base i32 (i32.const 1024))
 (memory $0 1)
 (data $0 (i32.const 1024) "\05\00\00\00 \00\00\00 \00\00\00 \00\00\00\00\00\00\00 ")
 (export "memcpy" (func $assembly/utils/memcpy/memcpy))
 (export "toPointer" (func $assembly/utils/pointer/toPointer))
 (export "abort" (func $assembly/alkanes/runtime/abort))
 (export "__load_storage" (func $assembly/alkanes/runtime/__load_storage))
 (export "__request_storage" (func $assembly/alkanes/runtime/__request_storage))
 (export "__log" (func $assembly/alkanes/runtime/__log))
 (export "__balance" (func $assembly/alkanes/runtime/__balance))
 (export "__request_context" (func $assembly/alkanes/runtime/__request_context))
 (export "__load_context" (func $assembly/alkanes/runtime/__load_context))
 (export "__sequence" (func $assembly/alkanes/runtime/__sequence))
 (export "__fuel" (func $assembly/alkanes/runtime/__fuel))
 (export "__height" (func $assembly/alkanes/runtime/__height))
 (export "__returndatacopy" (func $assembly/alkanes/runtime/__returndatacopy))
 (export "__request_transaction" (func $assembly/alkanes/runtime/__request_transaction))
 (export "__load_transaction" (func $assembly/alkanes/runtime/__load_transaction))
 (export "__request_block" (func $assembly/alkanes/runtime/__request_block))
 (export "__load_block" (func $assembly/alkanes/runtime/__load_block))
 (export "__call" (func $assembly/alkanes/runtime/__call))
 (export "__staticcall" (func $assembly/alkanes/runtime/__staticcall))
 (export "__delegatecall" (func $assembly/alkanes/runtime/__delegatecall))
 (export "ExtcallType.CALL" (global $assembly/alkanes/responder/ExtcallType.CALL))
 (export "ExtcallType.STATICCALL" (global $assembly/alkanes/responder/ExtcallType.STATICCALL))
 (export "ExtcallType.DELEGATECALL" (global $assembly/alkanes/responder/ExtcallType.DELEGATECALL))
 (export "__new" (func $~lib/rt/stub/__new))
 (export "__pin" (func $assembly/utils/pointer/toPointer))
 (export "__unpin" (func $~lib/rt/stub/__unpin))
 (export "__collect" (func $~lib/rt/stub/__collect))
 (export "__rtti_base" (global $~lib/rt/__rtti_base))
 (export "memory" (memory $0))
 (start $~start)
 (func $assembly/utils/memcpy/memcpy (param $0 i32) (param $1 i32) (param $2 i32) (result i32)
  local.get $0
  local.get $1
  local.get $2
  memory.copy
  local.get $0
 )
 (func $assembly/utils/pointer/toPointer (param $0 i32) (result i32)
  local.get $0
 )
 (func $~lib/rt/stub/__new (param $0 i32) (param $1 i32) (result i32)
  (local $2 i32)
  (local $3 i32)
  (local $4 i32)
  (local $5 i32)
  (local $6 i32)
  (local $7 i32)
  local.get $0
  i32.const 1073741804
  i32.gt_u
  if
   unreachable
  end
  local.get $0
  i32.const 16
  i32.add
  local.tee $4
  i32.const 1073741820
  i32.gt_u
  if
   unreachable
  end
  global.get $~lib/rt/stub/offset
  local.set $3
  global.get $~lib/rt/stub/offset
  i32.const 4
  i32.add
  local.tee $2
  local.get $4
  i32.const 19
  i32.add
  i32.const -16
  i32.and
  i32.const 4
  i32.sub
  local.tee $4
  i32.add
  local.tee $5
  memory.size
  local.tee $6
  i32.const 16
  i32.shl
  i32.const 15
  i32.add
  i32.const -16
  i32.and
  local.tee $7
  i32.gt_u
  if
   local.get $6
   local.get $5
   local.get $7
   i32.sub
   i32.const 65535
   i32.add
   i32.const -65536
   i32.and
   i32.const 16
   i32.shr_u
   local.tee $7
   local.get $6
   local.get $7
   i32.gt_s
   select
   memory.grow
   i32.const 0
   i32.lt_s
   if
    local.get $7
    memory.grow
    i32.const 0
    i32.lt_s
    if
     unreachable
    end
   end
  end
  local.get $5
  global.set $~lib/rt/stub/offset
  local.get $3
  local.get $4
  i32.store
  local.get $2
  i32.const 4
  i32.sub
  local.tee $3
  i32.const 0
  i32.store offset=4
  local.get $3
  i32.const 0
  i32.store offset=8
  local.get $3
  local.get $1
  i32.store offset=12
  local.get $3
  local.get $0
  i32.store offset=16
  local.get $2
  i32.const 16
  i32.add
 )
 (func $~lib/rt/stub/__unpin (param $0 i32)
 )
 (func $~lib/rt/stub/__collect
 )
 (func $~start
  i32.const 1052
  global.set $~lib/rt/stub/offset
 )
)
