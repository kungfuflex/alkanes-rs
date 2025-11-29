(module
 (type $0 (func (param i32 i32)))
 (type $1 (func (param i32) (result i32)))
 (type $2 (func (param i32 i32) (result i32)))
 (type $3 (func (param i64) (result i32)))
 (type $4 (func (result i32)))
 (type $5 (func))
 (global $~lib/rt/stub/offset (mut i32) (i32.const 0))
 (memory $0 0)
 (export "__execute" (func $assembly/index/__execute))
 (export "memory" (memory $0))
 (start $~start)
 (func $~lib/rt/common/OBJECT#set:gcInfo (param $0 i32) (param $1 i32)
  local.get $0
  local.get $1
  i32.store offset=4
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
 (func $../alkanes-asm-common/assembly/alkanes/types/AlkaneId#set:block (param $0 i32) (param $1 i32)
  local.get $0
  local.get $1
  i32.store
 )
 (func $~lib/as-bignum/assembly/integer/u128/u128#constructor (param $0 i64) (result i32)
  (local $1 i32)
  i32.const 16
  i32.const 5
  call $~lib/rt/stub/__new
  local.tee $1
  local.get $0
  i64.store
  local.get $1
  i64.const 0
  i64.store offset=8
  local.get $1
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
 (func $~lib/arraybuffer/ArrayBuffer#get:byteLength (param $0 i32) (result i32)
  local.get $0
  i32.const 20
  i32.sub
  i32.load offset=16
 )
 (func $../alkanes-asm-common/assembly/alkanes/types/ExtendedCallResponse#writeU128 (param $0 i32) (param $1 i32)
  (local $2 i32)
  (local $3 i32)
  local.get $0
  i32.load offset=4
  local.get $0
  i32.load
  i32.sub
  local.set $2
  local.get $0
  i32.load
  call $~lib/arraybuffer/ArrayBuffer#get:byteLength
  local.get $2
  i32.const 16
  i32.add
  i32.lt_s
  if
   local.get $0
   i32.load
   call $~lib/arraybuffer/ArrayBuffer#get:byteLength
   i32.const 1
   i32.shl
   call $~lib/arraybuffer/ArrayBuffer#constructor
   local.tee $3
   local.get $0
   i32.load
   local.get $2
   memory.copy
   local.get $0
   local.get $2
   local.get $3
   i32.add
   call $~lib/rt/common/OBJECT#set:gcInfo
   local.get $0
   local.get $3
   call $../alkanes-asm-common/assembly/alkanes/types/AlkaneId#set:block
  end
  local.get $0
  i32.load offset=4
  local.get $1
  i64.load
  i64.store
  local.get $0
  i32.load offset=4
  local.get $1
  i64.load offset=8
  i64.store offset=8
  local.get $0
  local.get $0
  i32.load offset=4
  i32.const 16
  i32.add
  call $~lib/rt/common/OBJECT#set:gcInfo
 )
 (func $assembly/index/__execute (result i32)
  (local $0 i32)
  (local $1 i32)
  (local $2 i32)
  (local $3 i32)
  i32.const 8
  i32.const 6
  call $~lib/rt/stub/__new
  local.tee $0
  i32.const 0
  call $../alkanes-asm-common/assembly/alkanes/types/AlkaneId#set:block
  local.get $0
  i32.const 0
  call $~lib/rt/common/OBJECT#set:gcInfo
  local.get $0
  i32.const 8192
  call $~lib/arraybuffer/ArrayBuffer#constructor
  call $../alkanes-asm-common/assembly/alkanes/types/AlkaneId#set:block
  local.get $0
  local.get $0
  i32.load
  call $~lib/rt/common/OBJECT#set:gcInfo
  local.get $0
  i64.const 0
  call $~lib/as-bignum/assembly/integer/u128/u128#constructor
  call $../alkanes-asm-common/assembly/alkanes/types/ExtendedCallResponse#writeU128
  local.get $0
  i64.const 0
  call $~lib/as-bignum/assembly/integer/u128/u128#constructor
  call $../alkanes-asm-common/assembly/alkanes/types/ExtendedCallResponse#writeU128
  local.get $0
  i64.const 2
  call $~lib/as-bignum/assembly/integer/u128/u128#constructor
  call $../alkanes-asm-common/assembly/alkanes/types/ExtendedCallResponse#writeU128
  local.get $0
  i32.load offset=4
  local.get $0
  i32.load
  i32.sub
  local.set $1
  local.get $0
  i32.load
  local.tee $0
  call $~lib/arraybuffer/ArrayBuffer#get:byteLength
  local.tee $2
  i32.const 0
  local.get $2
  i32.const 0
  i32.le_s
  select
  local.set $3
  local.get $1
  i32.const 0
  i32.lt_s
  if (result i32)
   local.get $1
   local.get $2
   i32.add
   local.tee $1
   i32.const 0
   local.get $1
   i32.const 0
   i32.gt_s
   select
  else
   local.get $1
   local.get $2
   local.get $1
   local.get $2
   i32.lt_s
   select
  end
  local.get $3
  i32.sub
  local.tee $1
  i32.const 0
  local.get $1
  i32.const 0
  i32.gt_s
  select
  local.tee $1
  i32.const 1
  call $~lib/rt/stub/__new
  local.tee $2
  local.get $0
  local.get $3
  i32.add
  local.get $1
  memory.copy
  local.get $2
 )
 (func $~start
  (local $0 i32)
  (local $1 i32)
  (local $2 i32)
  i32.const 1036
  global.set $~lib/rt/stub/offset
  i64.const 4
  call $~lib/as-bignum/assembly/integer/u128/u128#constructor
  local.set $0
  i64.const 65522
  call $~lib/as-bignum/assembly/integer/u128/u128#constructor
  local.set $1
  i32.const 8
  i32.const 4
  call $~lib/rt/stub/__new
  local.tee $2
  local.get $0
  call $../alkanes-asm-common/assembly/alkanes/types/AlkaneId#set:block
  local.get $2
  local.get $1
  i32.store offset=4
  i64.const 3
  call $~lib/as-bignum/assembly/integer/u128/u128#constructor
  drop
  i64.const 999
  call $~lib/as-bignum/assembly/integer/u128/u128#constructor
  drop
 )
)
