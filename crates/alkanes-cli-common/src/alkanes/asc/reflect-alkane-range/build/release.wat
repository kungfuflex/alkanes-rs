(module
 (type $0 (func (param i32 i32)))
 (type $1 (func (param i32) (result i32)))
 (type $2 (func (result i32)))
 (type $3 (func (param i32 i32) (result i32)))
 (type $4 (func (param i32 i64)))
 (type $5 (func (param i32)))
 (type $6 (func (param i64 i64) (result i32)))
 (type $7 (func (param i32 i32 i32 i64) (result i32)))
 (type $8 (func))
 (import "env" "__request_context" (func $../alkanes-asm-common/assembly/runtime/__request_context (result i32)))
 (import "env" "__load_context" (func $../alkanes-asm-common/assembly/runtime/__load_context (param i32) (result i32)))
 (import "env" "__sequence" (func $../alkanes-asm-common/assembly/alkanes/runtime/__sequence (param i32)))
 (import "env" "__staticcall" (func $../alkanes-asm-common/assembly/alkanes/runtime/__staticcall (param i32 i32 i32 i64) (result i32)))
 (import "env" "__returndatacopy" (func $../alkanes-asm-common/assembly/alkanes/runtime/__returndatacopy (param i32)))
 (global $~lib/rt/stub/offset (mut i32) (i32.const 0))
 (global $assembly/index/OPCODE_GET_NAME (mut i32) (i32.const 0))
 (global $assembly/index/OPCODE_GET_SYMBOL (mut i32) (i32.const 0))
 (global $assembly/index/OPCODE_GET_TOTAL_SUPPLY (mut i32) (i32.const 0))
 (global $assembly/index/OPCODE_GET_CAP (mut i32) (i32.const 0))
 (global $assembly/index/OPCODE_GET_MINTED (mut i32) (i32.const 0))
 (global $assembly/index/OPCODE_GET_VALUE_PER_MINT (mut i32) (i32.const 0))
 (global $assembly/index/OPCODE_GET_DATA (mut i32) (i32.const 0))
 (global $~started (mut i32) (i32.const 0))
 (memory $0 1)
 (data $0 (i32.const 1036) "\1c")
 (data $0.1 (i32.const 1048) "\01")
 (export "__execute" (func $assembly/index/__execute))
 (export "memory" (memory $0))
 (export "_start" (func $~start))
 (func $../alkanes-asm-common/assembly/u128/u128#set:lo (param $0 i32) (param $1 i64)
  local.get $0
  local.get $1
  i64.store
 )
 (func $../alkanes-asm-common/assembly/u128/u128#set:hi (param $0 i32) (param $1 i64)
  local.get $0
  local.get $1
  i64.store offset=8
 )
 (func $~lib/rt/common/OBJECT#set:gcInfo (param $0 i32) (param $1 i32)
  local.get $0
  local.get $1
  i32.store offset=4
 )
 (func $~lib/rt/common/OBJECT#set:gcInfo2 (param $0 i32) (param $1 i32)
  local.get $0
  local.get $1
  i32.store offset=8
 )
 (func $~lib/rt/common/OBJECT#set:rtId (param $0 i32) (param $1 i32)
  local.get $0
  local.get $1
  i32.store offset=12
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
  local.tee $3
  i32.const 1073741820
  i32.gt_u
  if
   unreachable
  end
  global.get $~lib/rt/stub/offset
  local.set $5
  global.get $~lib/rt/stub/offset
  i32.const 4
  i32.add
  local.tee $2
  local.get $3
  i32.const 19
  i32.add
  i32.const -16
  i32.and
  i32.const 4
  i32.sub
  local.tee $6
  i32.add
  local.tee $3
  memory.size
  local.tee $4
  i32.const 16
  i32.shl
  i32.const 15
  i32.add
  i32.const -16
  i32.and
  local.tee $7
  i32.gt_u
  if
   local.get $4
   local.get $3
   local.get $7
   i32.sub
   i32.const 65535
   i32.add
   i32.const -65536
   i32.and
   i32.const 16
   i32.shr_u
   local.tee $7
   local.get $4
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
  local.get $3
  global.set $~lib/rt/stub/offset
  local.get $5
  local.get $6
  i32.store
  local.get $2
  i32.const 4
  i32.sub
  local.tee $3
  i32.const 0
  call $~lib/rt/common/OBJECT#set:gcInfo
  local.get $3
  i32.const 0
  call $~lib/rt/common/OBJECT#set:gcInfo2
  local.get $3
  local.get $1
  call $~lib/rt/common/OBJECT#set:rtId
  local.get $3
  local.get $0
  i32.store offset=16
  local.get $2
  i32.const 16
  i32.add
 )
 (func $../alkanes-asm-common/assembly/u128/u128#constructor (param $0 i64) (param $1 i64) (result i32)
  (local $2 i32)
  i32.const 16
  i32.const 4
  call $~lib/rt/stub/__new
  local.tee $2
  i64.const 0
  call $../alkanes-asm-common/assembly/u128/u128#set:lo
  local.get $2
  i64.const 0
  call $../alkanes-asm-common/assembly/u128/u128#set:hi
  local.get $2
  local.get $0
  call $../alkanes-asm-common/assembly/u128/u128#set:lo
  local.get $2
  local.get $1
  call $../alkanes-asm-common/assembly/u128/u128#set:hi
  local.get $2
 )
 (func $../alkanes-asm-common/assembly/u128/u128.from (param $0 i32) (result i32)
  local.get $0
  i64.extend_i32_s
  i64.const 0
  call $../alkanes-asm-common/assembly/u128/u128#constructor
 )
 (func $../alkanes-asm-common/assembly/alkanes/responder/AlkaneResponder#set:context (param $0 i32) (param $1 i32)
  local.get $0
  local.get $1
  i32.store
 )
 (func $~lib/arraybuffer/ArrayBuffer#constructor (param $0 i32) (result i32)
  (local $1 i32)
  local.get $0
  i32.const 1073741820
  i32.gt_u
  if
   unreachable
  end
  local.get $0
  i32.const 1
  call $~lib/rt/stub/__new
  local.tee $1
  i32.const 0
  local.get $0
  memory.fill
  local.get $1
 )
 (func $../alkanes-asm-common/assembly/parcel/AlkaneTransferParcel#constructor (result i32)
  (local $0 i32)
  (local $1 i32)
  (local $2 i32)
  i32.const 4
  i32.const 9
  call $~lib/rt/stub/__new
  local.tee $1
  i32.const 0
  call $../alkanes-asm-common/assembly/alkanes/responder/AlkaneResponder#set:context
  i32.const 0
  i32.const 1
  call $~lib/rt/stub/__new
  local.tee $2
  i32.const 1056
  i32.const 0
  memory.copy
  i32.const 16
  i32.const 11
  call $~lib/rt/stub/__new
  local.tee $0
  local.get $2
  i32.store
  local.get $0
  local.get $2
  i32.store offset=4
  local.get $0
  i32.const 0
  i32.store offset=8
  local.get $0
  i32.const 0
  i32.store offset=12
  local.get $1
  local.get $0
  call $../alkanes-asm-common/assembly/alkanes/responder/AlkaneResponder#set:context
  local.get $1
 )
 (func $../alkanes-asm-common/assembly/storage-map/StorageMap#constructor (result i32)
  (local $0 i32)
  (local $1 i32)
  (local $2 i32)
  i32.const 4
  i32.const 12
  call $~lib/rt/stub/__new
  local.tee $1
  i32.const 0
  call $../alkanes-asm-common/assembly/alkanes/responder/AlkaneResponder#set:context
  i32.const 16
  i32.const 14
  call $~lib/rt/stub/__new
  local.tee $0
  i32.const 0
  call $../alkanes-asm-common/assembly/alkanes/responder/AlkaneResponder#set:context
  local.get $0
  i32.const 0
  call $~lib/rt/common/OBJECT#set:gcInfo
  local.get $0
  i32.const 0
  call $~lib/rt/common/OBJECT#set:gcInfo2
  local.get $0
  i32.const 0
  call $~lib/rt/common/OBJECT#set:rtId
  i32.const 32
  i32.const 1
  call $~lib/rt/stub/__new
  local.tee $2
  i32.const 0
  i32.const 32
  memory.fill
  local.get $0
  local.get $2
  call $../alkanes-asm-common/assembly/alkanes/responder/AlkaneResponder#set:context
  local.get $0
  local.get $2
  call $~lib/rt/common/OBJECT#set:gcInfo
  local.get $0
  i32.const 32
  call $~lib/rt/common/OBJECT#set:gcInfo2
  local.get $0
  i32.const 0
  call $~lib/rt/common/OBJECT#set:rtId
  local.get $1
  local.get $0
  call $../alkanes-asm-common/assembly/alkanes/responder/AlkaneResponder#set:context
  local.get $1
 )
 (func $../alkanes-asm-common/assembly/alkanes/types/ExtendedCallResponse#constructor (result i32)
  (local $0 i32)
  i32.const 12
  i32.const 8
  call $~lib/rt/stub/__new
  local.tee $0
  i32.const 0
  call $../alkanes-asm-common/assembly/alkanes/responder/AlkaneResponder#set:context
  local.get $0
  i32.const 0
  call $~lib/rt/common/OBJECT#set:gcInfo
  local.get $0
  i32.const 0
  call $~lib/rt/common/OBJECT#set:gcInfo2
  local.get $0
  call $../alkanes-asm-common/assembly/parcel/AlkaneTransferParcel#constructor
  call $../alkanes-asm-common/assembly/alkanes/responder/AlkaneResponder#set:context
  local.get $0
  call $../alkanes-asm-common/assembly/storage-map/StorageMap#constructor
  call $~lib/rt/common/OBJECT#set:gcInfo
  local.get $0
  i32.const 0
  call $~lib/arraybuffer/ArrayBuffer#constructor
  call $~lib/rt/common/OBJECT#set:gcInfo2
  local.get $0
 )
 (func $../alkanes-asm-common/assembly/u128/u128.get:Zero (result i32)
  i64.const 0
  i64.const 0
  call $../alkanes-asm-common/assembly/u128/u128#constructor
 )
 (func $../alkanes-asm-common/assembly/u128/u128#lt (param $0 i32) (param $1 i32) (result i32)
  local.get $0
  i64.load offset=8
  local.get $1
  i64.load offset=8
  i64.lt_u
  if (result i32)
   i32.const 1
  else
   local.get $0
   i64.load offset=8
   local.get $1
   i64.load offset=8
   i64.eq
   if (result i32)
    local.get $0
    i64.load
    local.get $1
    i64.load
    i64.lt_u
   else
    i32.const 0
   end
  end
 )
 (func $../alkanes-asm-common/assembly/alkanes/utils/storeU128 (param $0 i32) (param $1 i32)
  local.get $0
  local.get $1
  i64.load
  i64.store
  local.get $0
  local.get $1
  i64.load offset=8
  i64.store offset=8
 )
 (func $~lib/arraybuffer/ArrayBuffer#get:byteLength (param $0 i32) (result i32)
  local.get $0
  i32.const 20
  i32.sub
  i32.load offset=16
 )
 (func $../alkanes-asm-common/assembly/alkanes/types/ExtendedCallResponse#appendData (param $0 i32) (param $1 i32)
  (local $2 i32)
  (local $3 i32)
  local.get $0
  i32.load offset=8
  call $~lib/arraybuffer/ArrayBuffer#get:byteLength
  if
   local.get $0
   i32.load offset=8
   call $~lib/arraybuffer/ArrayBuffer#get:byteLength
   local.set $2
   local.get $1
   call $~lib/arraybuffer/ArrayBuffer#get:byteLength
   local.get $2
   i32.add
   call $~lib/arraybuffer/ArrayBuffer#constructor
   local.tee $3
   local.get $0
   i32.load offset=8
   local.get $2
   memory.copy
   local.get $2
   local.get $3
   i32.add
   local.get $1
   local.get $1
   call $~lib/arraybuffer/ArrayBuffer#get:byteLength
   memory.copy
   local.get $0
   local.get $3
   call $~lib/rt/common/OBJECT#set:gcInfo2
  else
   local.get $0
   local.get $1
   call $~lib/rt/common/OBJECT#set:gcInfo2
  end
 )
 (func $../alkanes-asm-common/assembly/alkanes/types/ExtendedCallResponse#writeU128 (param $0 i32) (param $1 i32)
  (local $2 i32)
  i32.const 16
  call $~lib/arraybuffer/ArrayBuffer#constructor
  local.tee $2
  local.get $1
  call $../alkanes-asm-common/assembly/alkanes/utils/storeU128
  local.get $0
  local.get $2
  call $../alkanes-asm-common/assembly/alkanes/types/ExtendedCallResponse#appendData
 )
 (func $~lib/array/Array<../alkanes-asm-common/assembly/parcel/AlkaneTransfer>#__get (param $0 i32) (param $1 i32) (result i32)
  local.get $1
  local.get $0
  i32.load offset=12
  i32.ge_u
  if
   unreachable
  end
  local.get $0
  i32.load offset=4
  local.get $1
  i32.const 2
  i32.shl
  i32.add
  i32.load
  local.tee $0
  i32.eqz
  if
   unreachable
  end
  local.get $0
 )
 (func $../alkanes-asm-common/assembly/parcel/AlkaneTransferParcel#serialize (param $0 i32) (result i32)
  (local $1 i32)
  (local $2 i32)
  (local $3 i32)
  (local $4 i32)
  (local $5 i32)
  local.get $0
  i32.load
  i32.load offset=12
  i32.const 48
  i32.mul
  i32.const 16
  i32.add
  call $~lib/arraybuffer/ArrayBuffer#constructor
  local.tee $3
  local.get $0
  i32.load
  i32.load offset=12
  call $../alkanes-asm-common/assembly/u128/u128.from
  call $../alkanes-asm-common/assembly/alkanes/utils/storeU128
  i32.const 16
  local.set $1
  loop $for-loop|0
   local.get $2
   local.get $0
   i32.load
   i32.load offset=12
   i32.lt_s
   if
    local.get $1
    local.get $3
    i32.add
    local.tee $4
    local.get $0
    i32.load
    local.get $2
    call $~lib/array/Array<../alkanes-asm-common/assembly/parcel/AlkaneTransfer>#__get
    local.tee $5
    i32.load
    i32.load
    call $../alkanes-asm-common/assembly/alkanes/utils/storeU128
    local.get $4
    i32.const 16
    i32.add
    local.get $5
    i32.load
    i32.load offset=4
    call $../alkanes-asm-common/assembly/alkanes/utils/storeU128
    local.get $4
    i32.const 32
    i32.add
    local.get $5
    i32.load offset=4
    call $../alkanes-asm-common/assembly/alkanes/utils/storeU128
    local.get $1
    i32.const 48
    i32.add
    local.set $1
    local.get $2
    i32.const 1
    i32.add
    local.set $2
    br $for-loop|0
   end
  end
  local.get $3
 )
 (func $../alkanes-asm-common/assembly/storage-map/StorageMap#serialize (param $0 i32) (result i32)
  (local $1 i32)
  (local $2 i32)
  (local $3 i32)
  (local $4 i32)
  (local $5 i32)
  (local $6 i32)
  (local $7 i32)
  i32.const 4
  local.set $2
  loop $for-loop|0
   local.get $1
   local.get $0
   i32.load
   i32.load offset=12
   i32.lt_s
   if
    local.get $2
    local.get $0
    i32.load
    local.get $1
    call $~lib/array/Array<../alkanes-asm-common/assembly/parcel/AlkaneTransfer>#__get
    local.tee $2
    i32.load
    call $~lib/arraybuffer/ArrayBuffer#get:byteLength
    i32.const 8
    i32.add
    local.get $2
    i32.load offset=4
    call $~lib/arraybuffer/ArrayBuffer#get:byteLength
    i32.add
    i32.add
    local.set $2
    local.get $1
    i32.const 1
    i32.add
    local.set $1
    br $for-loop|0
   end
  end
  local.get $2
  call $~lib/arraybuffer/ArrayBuffer#constructor
  local.tee $2
  local.get $0
  i32.load
  i32.load offset=12
  i32.store
  i32.const 4
  local.set $1
  loop $for-loop|1
   local.get $3
   local.get $0
   i32.load
   i32.load offset=12
   i32.lt_s
   if
    local.get $1
    local.get $2
    i32.add
    local.get $0
    i32.load
    local.get $3
    call $~lib/array/Array<../alkanes-asm-common/assembly/parcel/AlkaneTransfer>#__get
    local.tee $5
    i32.load
    call $~lib/arraybuffer/ArrayBuffer#get:byteLength
    i32.store
    local.get $1
    i32.const 4
    i32.add
    local.set $6
    local.get $5
    i32.load
    local.set $7
    local.get $5
    i32.load
    call $~lib/arraybuffer/ArrayBuffer#get:byteLength
    local.set $4
    i32.const 0
    local.set $1
    loop $for-loop|2
     local.get $1
     local.get $4
     i32.lt_u
     if
      local.get $2
      local.get $6
      i32.add
      local.get $1
      i32.add
      local.get $1
      local.get $7
      i32.add
      i32.load8_u
      i32.store8
      local.get $1
      i32.const 1
      i32.add
      local.set $1
      br $for-loop|2
     end
    end
    local.get $4
    local.get $6
    i32.add
    local.tee $1
    local.get $2
    i32.add
    local.get $5
    i32.load offset=4
    call $~lib/arraybuffer/ArrayBuffer#get:byteLength
    i32.store
    local.get $1
    i32.const 4
    i32.add
    local.set $4
    local.get $5
    i32.load offset=4
    local.set $6
    local.get $5
    i32.load offset=4
    call $~lib/arraybuffer/ArrayBuffer#get:byteLength
    local.set $5
    i32.const 0
    local.set $1
    loop $for-loop|3
     local.get $1
     local.get $5
     i32.lt_u
     if
      local.get $2
      local.get $4
      i32.add
      local.get $1
      i32.add
      local.get $1
      local.get $6
      i32.add
      i32.load8_u
      i32.store8
      local.get $1
      i32.const 1
      i32.add
      local.set $1
      br $for-loop|3
     end
    end
    local.get $4
    local.get $5
    i32.add
    local.set $1
    local.get $3
    i32.const 1
    i32.add
    local.set $3
    br $for-loop|1
   end
  end
  local.get $2
 )
 (func $../alkanes-asm-common/assembly/alkanes/responder/AlkaneResponder#staticcall (param $0 i32) (param $1 i32) (result i32)
  (local $2 i32)
  (local $3 i32)
  (local $4 i32)
  block $__inlined_func$../alkanes-asm-common/assembly/alkanes/responder/AlkaneResponder#extcall$69 (result i32)
   i32.const 16
   call $~lib/arraybuffer/ArrayBuffer#constructor
   local.tee $3
   local.get $1
   call $../alkanes-asm-common/assembly/alkanes/utils/storeU128
   i32.const 12
   i32.const 17
   call $~lib/rt/stub/__new
   local.tee $1
   local.get $0
   call $~lib/rt/common/OBJECT#set:gcInfo2
   local.get $1
   i32.const 0
   call $../alkanes-asm-common/assembly/alkanes/responder/AlkaneResponder#set:context
   local.get $1
   i32.const 0
   call $~lib/rt/common/OBJECT#set:gcInfo
   local.get $3
   if
    local.get $1
    local.get $3
    call $../alkanes-asm-common/assembly/alkanes/responder/AlkaneResponder#set:context
    local.get $1
    i32.const 1
    call $~lib/rt/common/OBJECT#set:gcInfo
   else
    local.get $1
    i32.const 0
    call $~lib/arraybuffer/ArrayBuffer#constructor
    call $../alkanes-asm-common/assembly/alkanes/responder/AlkaneResponder#set:context
    local.get $1
    i32.const 0
    call $~lib/rt/common/OBJECT#set:gcInfo
   end
   local.get $1
   i32.load
   call $~lib/arraybuffer/ArrayBuffer#get:byteLength
   i32.const 32
   i32.add
   call $~lib/arraybuffer/ArrayBuffer#constructor
   local.tee $0
   local.get $1
   i32.load offset=8
   i32.load
   call $../alkanes-asm-common/assembly/alkanes/utils/storeU128
   local.get $0
   i32.const 16
   i32.add
   local.get $1
   i32.load offset=8
   i32.load offset=4
   call $../alkanes-asm-common/assembly/alkanes/utils/storeU128
   local.get $1
   i32.load
   local.set $3
   local.get $1
   i32.load
   call $~lib/arraybuffer/ArrayBuffer#get:byteLength
   local.set $1
   loop $for-loop|0
    local.get $1
    local.get $2
    i32.gt_u
    if
     local.get $0
     i32.const 32
     i32.add
     local.get $2
     i32.add
     local.get $2
     local.get $3
     i32.add
     i32.load8_u
     i32.store8
     local.get $2
     i32.const 1
     i32.add
     local.set $2
     br $for-loop|0
    end
   end
   i32.const 0
   local.get $0
   call $../alkanes-asm-common/assembly/parcel/AlkaneTransferParcel#constructor
   call $../alkanes-asm-common/assembly/parcel/AlkaneTransferParcel#serialize
   call $../alkanes-asm-common/assembly/storage-map/StorageMap#constructor
   call $../alkanes-asm-common/assembly/storage-map/StorageMap#serialize
   i64.const -1
   call $../alkanes-asm-common/assembly/alkanes/runtime/__staticcall
   local.tee $0
   i32.const 0
   i32.lt_s
   br_if $__inlined_func$../alkanes-asm-common/assembly/alkanes/responder/AlkaneResponder#extcall$69
   drop
   local.get $0
   call $~lib/arraybuffer/ArrayBuffer#constructor
   local.tee $1
   call $../alkanes-asm-common/assembly/alkanes/runtime/__returndatacopy
   i32.const 0
   local.set $2
   local.get $1
   i64.load
   local.get $1
   i64.load offset=8
   call $../alkanes-asm-common/assembly/u128/u128#constructor
   i64.load
   i32.wrap_i64
   i32.const 48
   i32.mul
   i32.const 16
   i32.add
   local.set $3
   local.get $1
   call $~lib/arraybuffer/ArrayBuffer#get:byteLength
   local.get $3
   i32.sub
   local.tee $0
   call $~lib/arraybuffer/ArrayBuffer#constructor
   local.set $4
   local.get $1
   local.get $3
   i32.add
   local.set $1
   loop $for-loop|00
    local.get $0
    local.get $2
    i32.gt_u
    if
     local.get $2
     local.get $4
     i32.add
     local.get $1
     local.get $2
     i32.add
     i32.load8_u
     i32.store8
     local.get $2
     i32.const 1
     i32.add
     local.set $2
     br $for-loop|00
    end
   end
   call $../alkanes-asm-common/assembly/parcel/AlkaneTransferParcel#constructor
   local.set $0
   i32.const 8
   i32.const 16
   call $~lib/rt/stub/__new
   local.tee $1
   local.get $0
   call $../alkanes-asm-common/assembly/alkanes/responder/AlkaneResponder#set:context
   local.get $1
   local.get $4
   call $~lib/rt/common/OBJECT#set:gcInfo
   local.get $1
  end
 )
 (func $assembly/index/enrichAlkane (param $0 i32) (param $1 i32) (result i32)
  (local $2 i32)
  (local $3 i32)
  local.get $1
  local.get $0
  i32.load
  call $../alkanes-asm-common/assembly/alkanes/types/ExtendedCallResponse#writeU128
  local.get $1
  local.get $0
  i32.load offset=4
  call $../alkanes-asm-common/assembly/alkanes/types/ExtendedCallResponse#writeU128
  local.get $0
  global.get $assembly/index/OPCODE_GET_NAME
  call $../alkanes-asm-common/assembly/alkanes/responder/AlkaneResponder#staticcall
  local.tee $3
  if
   i32.const 1
   local.set $2
   local.get $1
   local.get $3
   i32.load offset=4
   local.tee $3
   call $~lib/arraybuffer/ArrayBuffer#get:byteLength
   call $../alkanes-asm-common/assembly/u128/u128.from
   call $../alkanes-asm-common/assembly/alkanes/types/ExtendedCallResponse#writeU128
   local.get $1
   local.get $3
   call $../alkanes-asm-common/assembly/alkanes/types/ExtendedCallResponse#appendData
  else
   local.get $1
   call $../alkanes-asm-common/assembly/u128/u128.get:Zero
   call $../alkanes-asm-common/assembly/alkanes/types/ExtendedCallResponse#writeU128
  end
  local.get $0
  global.get $assembly/index/OPCODE_GET_SYMBOL
  call $../alkanes-asm-common/assembly/alkanes/responder/AlkaneResponder#staticcall
  local.tee $3
  if
   i32.const 1
   local.set $2
   local.get $1
   local.get $3
   i32.load offset=4
   local.tee $3
   call $~lib/arraybuffer/ArrayBuffer#get:byteLength
   call $../alkanes-asm-common/assembly/u128/u128.from
   call $../alkanes-asm-common/assembly/alkanes/types/ExtendedCallResponse#writeU128
   local.get $1
   local.get $3
   call $../alkanes-asm-common/assembly/alkanes/types/ExtendedCallResponse#appendData
  else
   local.get $1
   call $../alkanes-asm-common/assembly/u128/u128.get:Zero
   call $../alkanes-asm-common/assembly/alkanes/types/ExtendedCallResponse#writeU128
  end
  local.get $0
  global.get $assembly/index/OPCODE_GET_TOTAL_SUPPLY
  call $../alkanes-asm-common/assembly/alkanes/responder/AlkaneResponder#staticcall
  local.tee $3
  if (result i32)
   local.get $3
   i32.load offset=4
   call $~lib/arraybuffer/ArrayBuffer#get:byteLength
   i32.const 16
   i32.ge_s
  else
   i32.const 0
  end
  if
   i32.const 1
   local.set $2
   local.get $1
   local.get $3
   i32.load offset=4
   local.tee $3
   i64.load
   local.get $3
   i64.load offset=8
   call $../alkanes-asm-common/assembly/u128/u128#constructor
   call $../alkanes-asm-common/assembly/alkanes/types/ExtendedCallResponse#writeU128
  else
   local.get $1
   call $../alkanes-asm-common/assembly/u128/u128.get:Zero
   call $../alkanes-asm-common/assembly/alkanes/types/ExtendedCallResponse#writeU128
  end
  local.get $0
  global.get $assembly/index/OPCODE_GET_CAP
  call $../alkanes-asm-common/assembly/alkanes/responder/AlkaneResponder#staticcall
  local.tee $3
  if (result i32)
   local.get $3
   i32.load offset=4
   call $~lib/arraybuffer/ArrayBuffer#get:byteLength
   i32.const 16
   i32.ge_s
  else
   i32.const 0
  end
  if
   i32.const 1
   local.set $2
   local.get $1
   local.get $3
   i32.load offset=4
   local.tee $3
   i64.load
   local.get $3
   i64.load offset=8
   call $../alkanes-asm-common/assembly/u128/u128#constructor
   call $../alkanes-asm-common/assembly/alkanes/types/ExtendedCallResponse#writeU128
  else
   local.get $1
   call $../alkanes-asm-common/assembly/u128/u128.get:Zero
   call $../alkanes-asm-common/assembly/alkanes/types/ExtendedCallResponse#writeU128
  end
  local.get $0
  global.get $assembly/index/OPCODE_GET_MINTED
  call $../alkanes-asm-common/assembly/alkanes/responder/AlkaneResponder#staticcall
  local.tee $3
  if (result i32)
   local.get $3
   i32.load offset=4
   call $~lib/arraybuffer/ArrayBuffer#get:byteLength
   i32.const 16
   i32.ge_s
  else
   i32.const 0
  end
  if
   i32.const 1
   local.set $2
   local.get $1
   local.get $3
   i32.load offset=4
   local.tee $3
   i64.load
   local.get $3
   i64.load offset=8
   call $../alkanes-asm-common/assembly/u128/u128#constructor
   call $../alkanes-asm-common/assembly/alkanes/types/ExtendedCallResponse#writeU128
  else
   local.get $1
   call $../alkanes-asm-common/assembly/u128/u128.get:Zero
   call $../alkanes-asm-common/assembly/alkanes/types/ExtendedCallResponse#writeU128
  end
  local.get $0
  global.get $assembly/index/OPCODE_GET_VALUE_PER_MINT
  call $../alkanes-asm-common/assembly/alkanes/responder/AlkaneResponder#staticcall
  local.tee $3
  if (result i32)
   local.get $3
   i32.load offset=4
   call $~lib/arraybuffer/ArrayBuffer#get:byteLength
   i32.const 16
   i32.ge_s
  else
   i32.const 0
  end
  if
   i32.const 1
   local.set $2
   local.get $1
   local.get $3
   i32.load offset=4
   local.tee $3
   i64.load
   local.get $3
   i64.load offset=8
   call $../alkanes-asm-common/assembly/u128/u128#constructor
   call $../alkanes-asm-common/assembly/alkanes/types/ExtendedCallResponse#writeU128
  else
   local.get $1
   call $../alkanes-asm-common/assembly/u128/u128.get:Zero
   call $../alkanes-asm-common/assembly/alkanes/types/ExtendedCallResponse#writeU128
  end
  local.get $0
  global.get $assembly/index/OPCODE_GET_DATA
  call $../alkanes-asm-common/assembly/alkanes/responder/AlkaneResponder#staticcall
  local.tee $0
  if
   i32.const 1
   local.set $2
   local.get $1
   local.get $0
   i32.load offset=4
   local.tee $0
   call $~lib/arraybuffer/ArrayBuffer#get:byteLength
   call $../alkanes-asm-common/assembly/u128/u128.from
   call $../alkanes-asm-common/assembly/alkanes/types/ExtendedCallResponse#writeU128
   local.get $1
   local.get $0
   call $../alkanes-asm-common/assembly/alkanes/types/ExtendedCallResponse#appendData
  else
   local.get $1
   call $../alkanes-asm-common/assembly/u128/u128.get:Zero
   call $../alkanes-asm-common/assembly/alkanes/types/ExtendedCallResponse#writeU128
  end
  local.get $2
 )
 (func $../alkanes-asm-common/assembly/u128/u128.get:One (result i32)
  i64.const 1
  i64.const 0
  call $../alkanes-asm-common/assembly/u128/u128#constructor
 )
 (func $../alkanes-asm-common/assembly/u128/u128#add (param $0 i32) (param $1 i32) (result i32)
  (local $2 i64)
  local.get $0
  i64.load
  local.get $1
  i64.load
  i64.add
  local.tee $2
  local.get $0
  i64.load
  local.get $2
  i64.gt_u
  i64.extend_i32_s
  local.get $0
  i64.load offset=8
  local.get $1
  i64.load offset=8
  i64.add
  i64.add
  call $../alkanes-asm-common/assembly/u128/u128#constructor
 )
 (func $../alkanes-asm-common/assembly/alkanes/types/ExtendedCallResponse#finalize (param $0 i32) (result i32)
  (local $1 i32)
  (local $2 i32)
  (local $3 i32)
  (local $4 i32)
  (local $5 i32)
  (local $6 i32)
  (local $7 i32)
  local.get $0
  i32.load
  call $../alkanes-asm-common/assembly/parcel/AlkaneTransferParcel#serialize
  local.set $7
  local.get $0
  i32.load offset=4
  call $../alkanes-asm-common/assembly/storage-map/StorageMap#serialize
  local.set $5
  local.get $7
  call $~lib/arraybuffer/ArrayBuffer#get:byteLength
  local.get $5
  call $~lib/arraybuffer/ArrayBuffer#get:byteLength
  i32.add
  local.get $0
  i32.load offset=8
  call $~lib/arraybuffer/ArrayBuffer#get:byteLength
  i32.add
  call $~lib/arraybuffer/ArrayBuffer#constructor
  local.set $1
  local.get $7
  call $~lib/arraybuffer/ArrayBuffer#get:byteLength
  local.set $6
  loop $for-loop|0
   local.get $2
   local.get $6
   i32.lt_u
   if
    local.get $1
    local.get $2
    i32.add
    local.get $2
    local.get $7
    i32.add
    i32.load8_u
    i32.store8
    local.get $2
    i32.const 1
    i32.add
    local.set $2
    br $for-loop|0
   end
  end
  local.get $5
  call $~lib/arraybuffer/ArrayBuffer#get:byteLength
  local.set $2
  loop $for-loop|1
   local.get $2
   local.get $3
   i32.gt_u
   if
    local.get $1
    local.get $6
    i32.add
    local.get $3
    i32.add
    local.get $3
    local.get $5
    i32.add
    i32.load8_u
    i32.store8
    local.get $3
    i32.const 1
    i32.add
    local.set $3
    br $for-loop|1
   end
  end
  local.get $0
  i32.load offset=8
  call $~lib/arraybuffer/ArrayBuffer#get:byteLength
  i32.const 0
  i32.gt_s
  if
   local.get $0
   i32.load offset=8
   local.set $3
   local.get $2
   local.get $6
   i32.add
   local.set $2
   local.get $0
   i32.load offset=8
   call $~lib/arraybuffer/ArrayBuffer#get:byteLength
   local.set $0
   loop $for-loop|2
    local.get $0
    local.get $4
    i32.gt_u
    if
     local.get $1
     local.get $2
     i32.add
     local.get $4
     i32.add
     local.get $3
     local.get $4
     i32.add
     i32.load8_u
     i32.store8
     local.get $4
     i32.const 1
     i32.add
     local.set $4
     br $for-loop|2
    end
   end
  end
  local.get $1
 )
 (func $assembly/index/__execute (result i32)
  (local $0 i32)
  (local $1 i32)
  (local $2 i32)
  (local $3 i32)
  (local $4 i32)
  (local $5 i32)
  (local $6 i64)
  (local $7 i64)
  (local $8 i64)
  (local $9 i64)
  (local $10 i32)
  (local $11 i32)
  i32.const 4
  i32.const 5
  call $~lib/rt/stub/__new
  local.tee $0
  if (result i32)
   local.get $0
  else
   i32.const 0
   i32.const 0
   call $~lib/rt/stub/__new
  end
  i32.const 0
  call $../alkanes-asm-common/assembly/alkanes/responder/AlkaneResponder#set:context
  call $../alkanes-asm-common/assembly/runtime/__request_context
  call $~lib/arraybuffer/ArrayBuffer#constructor
  local.tee $0
  call $../alkanes-asm-common/assembly/runtime/__load_context
  drop
  local.get $0
  i32.const 96
  i32.add
  local.tee $0
  i64.load offset=16
  local.set $6
  local.get $0
  i64.load offset=24
  local.set $7
  local.get $0
  i64.load offset=32
  local.set $8
  local.get $0
  i64.load offset=40
  local.set $9
  local.get $0
  i64.load
  local.get $0
  i64.load offset=8
  call $../alkanes-asm-common/assembly/u128/u128#constructor
  local.set $10
  local.get $6
  local.get $7
  call $../alkanes-asm-common/assembly/u128/u128#constructor
  local.set $2
  local.get $8
  local.get $9
  call $../alkanes-asm-common/assembly/u128/u128#constructor
  local.set $0
  i32.const 16
  call $~lib/arraybuffer/ArrayBuffer#constructor
  local.tee $1
  call $../alkanes-asm-common/assembly/alkanes/runtime/__sequence
  local.get $1
  i64.load
  local.get $1
  i64.load offset=8
  call $../alkanes-asm-common/assembly/u128/u128#constructor
  local.tee $1
  local.get $0
  local.get $0
  i64.load offset=8
  local.get $1
  i64.load offset=8
  i64.gt_u
  if (result i32)
   i32.const 1
  else
   local.get $0
   i64.load offset=8
   local.get $1
   i64.load offset=8
   i64.eq
   if (result i32)
    local.get $0
    i64.load
    local.get $1
    i64.load
    i64.gt_u
   else
    i32.const 0
   end
  end
  select
  local.set $11
  call $../alkanes-asm-common/assembly/alkanes/types/ExtendedCallResponse#constructor
  local.set $3
  call $../alkanes-asm-common/assembly/u128/u128.get:Zero
  local.set $0
  call $../alkanes-asm-common/assembly/alkanes/types/ExtendedCallResponse#constructor
  local.set $4
  local.get $2
  local.set $1
  loop $while-continue|0
   local.get $1
   local.get $11
   call $../alkanes-asm-common/assembly/u128/u128#lt
   if (result i32)
    i32.const 1
   else
    local.get $1
    i64.load
    local.get $11
    i64.load
    i64.eq
    if (result i32)
     local.get $1
     i64.load offset=8
     local.get $11
     i64.load offset=8
     i64.eq
    else
     i32.const 0
    end
   end
   if
    block $while-break|0
     i32.const 8
     i32.const 7
     call $~lib/rt/stub/__new
     local.tee $5
     local.get $10
     call $../alkanes-asm-common/assembly/alkanes/responder/AlkaneResponder#set:context
     local.get $5
     local.get $1
     call $~lib/rt/common/OBJECT#set:gcInfo
     local.get $5
     local.get $4
     call $assembly/index/enrichAlkane
     if
      local.get $0
      call $../alkanes-asm-common/assembly/u128/u128.get:One
      call $../alkanes-asm-common/assembly/u128/u128#add
      local.set $0
     end
     local.get $1
     call $../alkanes-asm-common/assembly/u128/u128.get:One
     call $../alkanes-asm-common/assembly/u128/u128#add
     local.tee $1
     local.get $2
     call $../alkanes-asm-common/assembly/u128/u128#lt
     br_if $while-break|0
     br $while-continue|0
    end
   end
  end
  local.get $3
  local.get $0
  call $../alkanes-asm-common/assembly/alkanes/types/ExtendedCallResponse#writeU128
  local.get $3
  local.get $4
  call $../alkanes-asm-common/assembly/alkanes/types/ExtendedCallResponse#finalize
  call $../alkanes-asm-common/assembly/alkanes/types/ExtendedCallResponse#appendData
  local.get $3
  call $../alkanes-asm-common/assembly/alkanes/types/ExtendedCallResponse#finalize
 )
 (func $~start
  global.get $~started
  if
   return
  end
  i32.const 1
  global.set $~started
  i32.const 1068
  global.set $~lib/rt/stub/offset
  i32.const 99
  call $../alkanes-asm-common/assembly/u128/u128.from
  global.set $assembly/index/OPCODE_GET_NAME
  i32.const 100
  call $../alkanes-asm-common/assembly/u128/u128.from
  global.set $assembly/index/OPCODE_GET_SYMBOL
  i32.const 101
  call $../alkanes-asm-common/assembly/u128/u128.from
  global.set $assembly/index/OPCODE_GET_TOTAL_SUPPLY
  i32.const 102
  call $../alkanes-asm-common/assembly/u128/u128.from
  global.set $assembly/index/OPCODE_GET_CAP
  i32.const 103
  call $../alkanes-asm-common/assembly/u128/u128.from
  global.set $assembly/index/OPCODE_GET_MINTED
  i32.const 104
  call $../alkanes-asm-common/assembly/u128/u128.from
  global.set $assembly/index/OPCODE_GET_VALUE_PER_MINT
  i32.const 1000
  call $../alkanes-asm-common/assembly/u128/u128.from
  global.set $assembly/index/OPCODE_GET_DATA
 )
)
