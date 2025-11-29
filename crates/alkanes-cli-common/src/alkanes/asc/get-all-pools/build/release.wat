(module
 (type $0 (func (param i32) (result i32)))
 (type $1 (func (result i32)))
 (type $2 (func (param i32 i32) (result i32)))
 (type $3 (func (param i32 i32 i32 i32)))
 (type $4 (func (param i64 i64) (result i32)))
 (type $5 (func (param i32 i32 i32) (result i32)))
 (type $6 (func (param i32 i32)))
 (type $7 (func (param i32 i32 i32 i64) (result i32)))
 (type $8 (func (param i32)))
 (type $9 (func))
 (import "env" "abort" (func $~lib/builtins/abort (param i32 i32 i32 i32)))
 (import "env" "__staticcall" (func $../alkanes-asm-common/assembly/alkanes/runtime/__staticcall (param i32 i32 i32 i64) (result i32)))
 (import "env" "__returndatacopy" (func $../alkanes-asm-common/assembly/alkanes/runtime/__returndatacopy (param i32)))
 (global $~lib/rt/stub/offset (mut i32) (i32.const 0))
 (global $assembly/index/FACTORY (mut i32) (i32.const 0))
 (global $assembly/index/GET_ALL_POOLS_OPCODE (mut i32) (i32.const 0))
 (global $~started (mut i32) (i32.const 0))
 (memory $0 1)
 (data $0 (i32.const 1036) "<")
 (data $0.1 (i32.const 1048) "\02\00\00\00(\00\00\00A\00l\00l\00o\00c\00a\00t\00i\00o\00n\00 \00t\00o\00o\00 \00l\00a\00r\00g\00e")
 (data $1 (i32.const 1100) "<")
 (data $1.1 (i32.const 1112) "\02\00\00\00\1e\00\00\00~\00l\00i\00b\00/\00r\00t\00/\00s\00t\00u\00b\00.\00t\00s")
 (data $2 (i32.const 1164) "\1c")
 (data $2.1 (i32.const 1176) "\01")
 (data $3 (i32.const 1196) ",")
 (data $3.1 (i32.const 1208) "\02\00\00\00\1c\00\00\00I\00n\00v\00a\00l\00i\00d\00 \00l\00e\00n\00g\00t\00h")
 (data $4 (i32.const 1244) ",")
 (data $4.1 (i32.const 1256) "\02\00\00\00\1a\00\00\00~\00l\00i\00b\00/\00a\00r\00r\00a\00y\00.\00t\00s")
 (data $5 (i32.const 1292) "<")
 (data $5.1 (i32.const 1304) "\02\00\00\00&\00\00\00~\00l\00i\00b\00/\00a\00r\00r\00a\00y\00b\00u\00f\00f\00e\00r\00.\00t\00s")
 (data $6 (i32.const 1356) "<")
 (data $6.1 (i32.const 1368) "\02\00\00\00$\00\00\00I\00n\00d\00e\00x\00 \00o\00u\00t\00 \00o\00f\00 \00r\00a\00n\00g\00e")
 (data $7 (i32.const 1420) "|")
 (data $7.1 (i32.const 1432) "\02\00\00\00^\00\00\00E\00l\00e\00m\00e\00n\00t\00 \00t\00y\00p\00e\00 \00m\00u\00s\00t\00 \00b\00e\00 \00n\00u\00l\00l\00a\00b\00l\00e\00 \00i\00f\00 \00a\00r\00r\00a\00y\00 \00i\00s\00 \00h\00o\00l\00e\00y")
 (export "__execute" (func $assembly/index/__execute))
 (export "memory" (memory $0))
 (export "_start" (func $~start))
 (func $~lib/rt/stub/__alloc (param $0 i32) (result i32)
  (local $1 i32)
  (local $2 i32)
  (local $3 i32)
  (local $4 i32)
  (local $5 i32)
  local.get $0
  i32.const 1073741820
  i32.gt_u
  if
   i32.const 1056
   i32.const 1120
   i32.const 33
   i32.const 29
   call $~lib/builtins/abort
   unreachable
  end
  global.get $~lib/rt/stub/offset
  local.set $1
  global.get $~lib/rt/stub/offset
  i32.const 4
  i32.add
  local.tee $2
  local.get $0
  i32.const 19
  i32.add
  i32.const -16
  i32.and
  i32.const 4
  i32.sub
  local.tee $0
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
  local.tee $5
  i32.gt_u
  if
   local.get $4
   local.get $3
   local.get $5
   i32.sub
   i32.const 65535
   i32.add
   i32.const -65536
   i32.and
   i32.const 16
   i32.shr_u
   local.tee $5
   local.get $4
   local.get $5
   i32.gt_s
   select
   memory.grow
   i32.const 0
   i32.lt_s
   if
    local.get $5
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
  local.get $1
  local.get $0
  i32.store
  local.get $2
 )
 (func $~lib/rt/stub/__new (param $0 i32) (param $1 i32) (result i32)
  (local $2 i32)
  (local $3 i32)
  local.get $0
  i32.const 1073741804
  i32.gt_u
  if
   i32.const 1056
   i32.const 1120
   i32.const 86
   i32.const 30
   call $~lib/builtins/abort
   unreachable
  end
  local.get $0
  i32.const 16
  i32.add
  call $~lib/rt/stub/__alloc
  local.tee $3
  i32.const 4
  i32.sub
  local.tee $2
  i32.const 0
  i32.store offset=4
  local.get $2
  i32.const 0
  i32.store offset=8
  local.get $2
  local.get $1
  i32.store offset=12
  local.get $2
  local.get $0
  i32.store offset=16
  local.get $3
  i32.const 16
  i32.add
 )
 (func $../alkanes-asm-common/assembly/u128/u128#constructor (param $0 i64) (param $1 i64) (result i32)
  (local $2 i32)
  i32.const 16
  i32.const 5
  call $~lib/rt/stub/__new
  local.tee $2
  i64.const 0
  i64.store
  local.get $2
  i64.const 0
  i64.store offset=8
  local.get $2
  local.get $0
  i64.store
  local.get $2
  local.get $1
  i64.store offset=8
  local.get $2
 )
 (func $~lib/rt/__newArray (param $0 i32) (param $1 i32) (param $2 i32) (result i32)
  (local $3 i32)
  (local $4 i32)
  local.get $0
  i32.const 2
  i32.shl
  local.tee $4
  i32.const 1
  call $~lib/rt/stub/__new
  local.set $3
  local.get $2
  if
   local.get $3
   local.get $2
   local.get $4
   memory.copy
  end
  i32.const 16
  local.get $1
  call $~lib/rt/stub/__new
  local.tee $1
  local.get $3
  i32.store
  local.get $1
  local.get $3
  i32.store offset=4
  local.get $1
  local.get $4
  i32.store offset=8
  local.get $1
  local.get $0
  i32.store offset=12
  local.get $1
 )
 (func $../alkanes-asm-common/assembly/parcel/AlkaneTransferParcel#constructor (result i32)
  (local $0 i32)
  i32.const 4
  i32.const 9
  call $~lib/rt/stub/__new
  local.tee $0
  i32.const 0
  i32.store
  local.get $0
  i32.const 0
  i32.const 11
  i32.const 1184
  call $~lib/rt/__newArray
  i32.store
  local.get $0
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
  i32.store
  i32.const 16
  i32.const 14
  call $~lib/rt/stub/__new
  local.tee $0
  i32.const 0
  i32.store
  local.get $0
  i32.const 0
  i32.store offset=4
  local.get $0
  i32.const 0
  i32.store offset=8
  local.get $0
  i32.const 0
  i32.store offset=12
  i32.const 32
  i32.const 1
  call $~lib/rt/stub/__new
  local.tee $2
  i32.const 0
  i32.const 32
  memory.fill
  local.get $0
  local.get $2
  i32.store
  local.get $0
  local.get $2
  i32.store offset=4
  local.get $0
  i32.const 32
  i32.store offset=8
  local.get $0
  i32.const 0
  i32.store offset=12
  local.get $1
  local.get $0
  i32.store
  local.get $1
 )
 (func $~lib/arraybuffer/ArrayBuffer#constructor (param $0 i32) (result i32)
  (local $1 i32)
  local.get $0
  i32.const 1073741820
  i32.gt_u
  if
   i32.const 1216
   i32.const 1312
   i32.const 52
   i32.const 43
   call $~lib/builtins/abort
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
 (func $~lib/array/Array<../alkanes-asm-common/assembly/u128/u128>#__set (param $0 i32) (param $1 i32)
  (local $2 i32)
  (local $3 i32)
  (local $4 i32)
  (local $5 i32)
  (local $6 i32)
  (local $7 i32)
  (local $8 i32)
  (local $9 i32)
  (local $10 i32)
  local.get $0
  i32.load offset=12
  i32.eqz
  if
   local.get $0
   i32.load offset=8
   local.tee $6
   i32.const 2
   i32.shr_u
   i32.eqz
   if
    local.get $0
    i32.load
    local.set $5
    i32.const 32
    i32.const 1073741820
    local.get $6
    i32.const 1
    i32.shl
    local.tee $2
    local.get $2
    i32.const 1073741820
    i32.ge_u
    select
    local.tee $2
    local.get $2
    i32.const 32
    i32.le_u
    select
    local.tee $7
    i32.const 1073741804
    i32.gt_u
    if
     i32.const 1056
     i32.const 1120
     i32.const 99
     i32.const 30
     call $~lib/builtins/abort
     unreachable
    end
    local.get $5
    i32.const 16
    i32.sub
    local.tee $2
    i32.const 15
    i32.and
    i32.const 1
    local.get $2
    select
    if
     i32.const 0
     i32.const 1120
     i32.const 45
     i32.const 3
     call $~lib/builtins/abort
     unreachable
    end
    global.get $~lib/rt/stub/offset
    local.get $2
    i32.const 4
    i32.sub
    local.tee $4
    i32.load
    local.tee $8
    local.get $2
    i32.add
    i32.eq
    local.set $9
    local.get $7
    i32.const 16
    i32.add
    local.tee $10
    i32.const 19
    i32.add
    i32.const -16
    i32.and
    i32.const 4
    i32.sub
    local.set $3
    local.get $8
    local.get $10
    i32.lt_u
    if
     local.get $9
     if
      local.get $10
      i32.const 1073741820
      i32.gt_u
      if
       i32.const 1056
       i32.const 1120
       i32.const 52
       i32.const 33
       call $~lib/builtins/abort
       unreachable
      end
      local.get $2
      local.get $3
      i32.add
      local.tee $8
      memory.size
      local.tee $9
      i32.const 16
      i32.shl
      i32.const 15
      i32.add
      i32.const -16
      i32.and
      local.tee $10
      i32.gt_u
      if
       local.get $9
       local.get $8
       local.get $10
       i32.sub
       i32.const 65535
       i32.add
       i32.const -65536
       i32.and
       i32.const 16
       i32.shr_u
       local.tee $10
       local.get $9
       local.get $10
       i32.gt_s
       select
       memory.grow
       i32.const 0
       i32.lt_s
       if
        local.get $10
        memory.grow
        i32.const 0
        i32.lt_s
        if
         unreachable
        end
       end
      end
      local.get $8
      global.set $~lib/rt/stub/offset
      local.get $4
      local.get $3
      i32.store
     else
      local.get $3
      local.get $8
      i32.const 1
      i32.shl
      local.tee $4
      local.get $3
      local.get $4
      i32.gt_u
      select
      call $~lib/rt/stub/__alloc
      local.tee $3
      local.get $2
      local.get $8
      memory.copy
      local.get $3
      local.set $2
     end
    else
     local.get $9
     if
      local.get $2
      local.get $3
      i32.add
      global.set $~lib/rt/stub/offset
      local.get $4
      local.get $3
      i32.store
     end
    end
    local.get $2
    i32.const 4
    i32.sub
    local.get $7
    i32.store offset=16
    local.get $6
    local.get $2
    i32.const 16
    i32.add
    local.tee $2
    i32.add
    i32.const 0
    local.get $7
    local.get $6
    i32.sub
    memory.fill
    local.get $2
    local.get $5
    i32.ne
    if
     local.get $0
     local.get $2
     i32.store
     local.get $0
     local.get $2
     i32.store offset=4
    end
    local.get $0
    local.get $7
    i32.store offset=8
   end
   local.get $0
   i32.const 1
   i32.store offset=12
  end
  local.get $0
  i32.load offset=4
  local.get $1
  i32.store
 )
 (func $~lib/array/Array<../alkanes-asm-common/assembly/u128/u128>#__get (param $0 i32) (param $1 i32) (result i32)
  local.get $1
  local.get $0
  i32.load offset=12
  i32.ge_u
  if
   i32.const 1376
   i32.const 1264
   i32.const 114
   i32.const 42
   call $~lib/builtins/abort
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
   i32.const 1440
   i32.const 1264
   i32.const 118
   i32.const 40
   call $~lib/builtins/abort
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
  (local $6 i32)
  (local $7 i32)
  local.get $0
  i32.load
  i32.load offset=12
  i32.const 48
  i32.mul
  i32.const 16
  i32.add
  call $~lib/arraybuffer/ArrayBuffer#constructor
  local.tee $2
  local.get $0
  i32.load
  i64.load32_s offset=12
  i64.const 0
  call $../alkanes-asm-common/assembly/u128/u128#constructor
  local.tee $1
  i64.load
  i64.store
  local.get $2
  local.get $1
  i64.load offset=8
  i64.store offset=8
  i32.const 16
  local.set $1
  loop $for-loop|0
   local.get $3
   local.get $0
   i32.load
   local.tee $4
   i32.load offset=12
   i32.lt_s
   if
    local.get $1
    local.get $2
    i32.add
    local.tee $6
    local.get $4
    local.get $3
    call $~lib/array/Array<../alkanes-asm-common/assembly/u128/u128>#__get
    local.tee $4
    i32.load
    i32.load
    local.tee $5
    i64.load
    i64.store
    local.get $6
    local.get $5
    i64.load offset=8
    i64.store offset=8
    local.get $6
    i32.const 16
    i32.add
    local.tee $5
    local.get $4
    i32.load
    i32.load offset=4
    local.tee $7
    i64.load
    i64.store
    local.get $5
    local.get $7
    i64.load offset=8
    i64.store offset=8
    local.get $6
    i32.const 32
    i32.add
    local.tee $5
    local.get $4
    i32.load offset=4
    local.tee $4
    i64.load
    i64.store
    local.get $5
    local.get $4
    i64.load offset=8
    i64.store offset=8
    local.get $1
    i32.const 48
    i32.add
    local.set $1
    local.get $3
    i32.const 1
    i32.add
    local.set $3
    br $for-loop|0
   end
  end
  local.get $2
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
   local.tee $4
   i32.load offset=12
   i32.lt_s
   if
    local.get $2
    local.get $4
    local.get $1
    call $~lib/array/Array<../alkanes-asm-common/assembly/u128/u128>#__get
    local.tee $2
    i32.load offset=4
    i32.const 20
    i32.sub
    i32.load offset=16
    local.get $2
    i32.load
    i32.const 20
    i32.sub
    i32.load offset=16
    i32.const 8
    i32.add
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
  local.tee $4
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
   local.tee $2
   i32.load offset=12
   i32.lt_s
   if
    local.get $1
    local.get $4
    i32.add
    local.get $2
    local.get $3
    call $~lib/array/Array<../alkanes-asm-common/assembly/u128/u128>#__get
    local.tee $5
    i32.load
    i32.const 20
    i32.sub
    i32.load offset=16
    i32.store
    local.get $1
    i32.const 4
    i32.add
    local.set $6
    local.get $5
    i32.load
    local.tee $1
    local.set $7
    local.get $1
    i32.const 20
    i32.sub
    i32.load offset=16
    local.set $2
    i32.const 0
    local.set $1
    loop $for-loop|2
     local.get $1
     local.get $2
     i32.lt_u
     if
      local.get $4
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
    local.get $2
    local.get $6
    i32.add
    local.tee $1
    local.get $4
    i32.add
    local.get $5
    i32.load offset=4
    i32.const 20
    i32.sub
    i32.load offset=16
    i32.store
    local.get $1
    i32.const 4
    i32.add
    local.set $2
    local.get $5
    i32.load offset=4
    local.set $6
    local.get $5
    i32.load offset=4
    i32.const 20
    i32.sub
    i32.load offset=16
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
    local.get $2
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
  local.get $4
 )
 (func $assembly/index/__execute (result i32)
  (local $0 i32)
  (local $1 i32)
  (local $2 i32)
  (local $3 i32)
  (local $4 i32)
  (local $5 i32)
  (local $6 i32)
  (local $7 i32)
  (local $8 i32)
  (local $9 i32)
  i32.const 4
  i32.const 6
  call $~lib/rt/stub/__new
  local.tee $5
  if (result i32)
   local.get $5
  else
   i32.const 0
   i32.const 0
   call $~lib/rt/stub/__new
  end
  i32.const 0
  i32.store
  i32.const 12
  i32.const 8
  call $~lib/rt/stub/__new
  local.tee $5
  i32.const 0
  i32.store
  local.get $5
  i32.const 0
  i32.store offset=4
  local.get $5
  i32.const 0
  i32.store offset=8
  local.get $5
  call $../alkanes-asm-common/assembly/parcel/AlkaneTransferParcel#constructor
  i32.store
  local.get $5
  call $../alkanes-asm-common/assembly/storage-map/StorageMap#constructor
  i32.store offset=4
  local.get $5
  i32.const 0
  call $~lib/arraybuffer/ArrayBuffer#constructor
  i32.store offset=8
  global.get $assembly/index/GET_ALL_POOLS_OPCODE
  local.set $6
  global.get $assembly/index/FACTORY
  local.set $7
  i32.const 1
  i32.const 17
  i32.const 0
  call $~lib/rt/__newArray
  local.tee $8
  i32.load offset=4
  drop
  local.get $8
  local.get $6
  call $~lib/array/Array<../alkanes-asm-common/assembly/u128/u128>#__set
  block $__inlined_func$../alkanes-asm-common/assembly/alkanes/responder/AlkaneResponder#extcall$3 (result i32)
   i32.const 8
   i32.const 18
   call $~lib/rt/stub/__new
   local.tee $6
   local.get $7
   i32.store
   local.get $6
   local.get $8
   i32.store offset=4
   local.get $6
   i32.load offset=4
   i32.load offset=12
   i32.const 4
   i32.shl
   i32.const 32
   i32.add
   call $~lib/arraybuffer/ArrayBuffer#constructor
   local.tee $7
   local.get $6
   i32.load
   i32.load
   local.tee $8
   i64.load
   i64.store
   local.get $7
   local.get $8
   i64.load offset=8
   i64.store offset=8
   local.get $7
   i32.const 16
   i32.add
   local.tee $8
   local.get $6
   i32.load
   i32.load offset=4
   local.tee $9
   i64.load
   i64.store
   local.get $8
   local.get $9
   i64.load offset=8
   i64.store offset=8
   loop $for-loop|0
    local.get $1
    local.get $6
    i32.load offset=4
    local.tee $8
    i32.load offset=12
    i32.lt_s
    if
     local.get $7
     local.get $1
     i32.const 4
     i32.shl
     i32.const 32
     i32.add
     i32.add
     local.tee $9
     local.get $8
     local.get $1
     call $~lib/array/Array<../alkanes-asm-common/assembly/u128/u128>#__get
     local.tee $8
     i64.load
     i64.store
     local.get $9
     local.get $8
     i64.load offset=8
     i64.store offset=8
     local.get $1
     i32.const 1
     i32.add
     local.set $1
     br $for-loop|0
    end
   end
   i32.const 0
   local.get $7
   call $../alkanes-asm-common/assembly/parcel/AlkaneTransferParcel#constructor
   call $../alkanes-asm-common/assembly/parcel/AlkaneTransferParcel#serialize
   call $../alkanes-asm-common/assembly/storage-map/StorageMap#constructor
   call $../alkanes-asm-common/assembly/storage-map/StorageMap#serialize
   i64.const -1
   call $../alkanes-asm-common/assembly/alkanes/runtime/__staticcall
   local.tee $1
   i32.const 0
   i32.lt_s
   br_if $__inlined_func$../alkanes-asm-common/assembly/alkanes/responder/AlkaneResponder#extcall$3
   drop
   local.get $1
   call $~lib/arraybuffer/ArrayBuffer#constructor
   local.tee $1
   call $../alkanes-asm-common/assembly/alkanes/runtime/__returndatacopy
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
   local.set $6
   local.get $1
   i32.const 20
   i32.sub
   i32.load offset=16
   local.get $6
   i32.sub
   local.tee $7
   call $~lib/arraybuffer/ArrayBuffer#constructor
   local.set $8
   local.get $1
   local.get $6
   i32.add
   local.set $1
   loop $for-loop|00
    local.get $0
    local.get $7
    i32.lt_u
    if
     local.get $0
     local.get $8
     i32.add
     local.get $0
     local.get $1
     i32.add
     i32.load8_u
     i32.store8
     local.get $0
     i32.const 1
     i32.add
     local.set $0
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
   i32.store
   local.get $1
   local.get $8
   i32.store offset=4
   local.get $1
  end
  local.tee $0
  if
   local.get $5
   local.get $0
   i32.load offset=4
   i32.store offset=8
  end
  local.get $5
  i32.load
  call $../alkanes-asm-common/assembly/parcel/AlkaneTransferParcel#serialize
  local.set $0
  local.get $5
  i32.load offset=4
  call $../alkanes-asm-common/assembly/storage-map/StorageMap#serialize
  local.set $1
  local.get $5
  i32.load offset=8
  i32.const 20
  i32.sub
  i32.load offset=16
  local.get $0
  i32.const 20
  i32.sub
  i32.load offset=16
  local.get $1
  i32.const 20
  i32.sub
  i32.load offset=16
  i32.add
  i32.add
  call $~lib/arraybuffer/ArrayBuffer#constructor
  local.set $6
  local.get $0
  i32.const 20
  i32.sub
  i32.load offset=16
  local.set $7
  loop $for-loop|01
   local.get $2
   local.get $7
   i32.lt_u
   if
    local.get $2
    local.get $6
    i32.add
    local.get $0
    local.get $2
    i32.add
    i32.load8_u
    i32.store8
    local.get $2
    i32.const 1
    i32.add
    local.set $2
    br $for-loop|01
   end
  end
  local.get $1
  i32.const 20
  i32.sub
  i32.load offset=16
  local.set $0
  loop $for-loop|1
   local.get $0
   local.get $3
   i32.gt_u
   if
    local.get $6
    local.get $7
    i32.add
    local.get $3
    i32.add
    local.get $1
    local.get $3
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
  local.get $5
  i32.load offset=8
  local.tee $1
  i32.const 20
  i32.sub
  i32.load offset=16
  local.tee $2
  i32.const 0
  i32.gt_s
  if
   local.get $0
   local.get $7
   i32.add
   local.set $0
   loop $for-loop|2
    local.get $2
    local.get $4
    i32.gt_u
    if
     local.get $0
     local.get $6
     i32.add
     local.get $4
     i32.add
     local.get $1
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
  local.get $6
 )
 (func $~start
  (local $0 i32)
  (local $1 i32)
  (local $2 i32)
  global.get $~started
  if
   return
  end
  i32.const 1
  global.set $~started
  i32.const 1548
  global.set $~lib/rt/stub/offset
  i64.const 4
  i64.const 0
  call $../alkanes-asm-common/assembly/u128/u128#constructor
  local.set $1
  i64.const 65522
  i64.const 0
  call $../alkanes-asm-common/assembly/u128/u128#constructor
  local.set $2
  i32.const 8
  i32.const 4
  call $~lib/rt/stub/__new
  local.tee $0
  local.get $1
  i32.store
  local.get $0
  local.get $2
  i32.store offset=4
  local.get $0
  global.set $assembly/index/FACTORY
  i64.const 3
  i64.const 0
  call $../alkanes-asm-common/assembly/u128/u128#constructor
  global.set $assembly/index/GET_ALL_POOLS_OPCODE
 )
)
