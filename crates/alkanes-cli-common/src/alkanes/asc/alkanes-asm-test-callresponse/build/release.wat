(module
 (type $0 (func (param i32) (result i32)))
 (type $1 (func (param i32 i32 i32 i32) (result i32)))
 (type $2 (func (param i32 i32) (result i32)))
 (type $3 (func (param i32 i32 i32) (result i32)))
 (type $4 (func (param i32 i32 i32 i32)))
 (type $5 (func (param i32 i32)))
 (type $6 (func (param i32 i32 i32)))
 (type $7 (func (result i32)))
 (type $8 (func))
 (import "env" "abort" (func $~lib/builtins/abort (param i32 i32 i32 i32)))
 (global $~lib/rt/stub/offset (mut i32) (i32.const 0))
 (global $~started (mut i32) (i32.const 0))
 (memory $0 1)
 (data $0 (i32.const 1036) ",")
 (data $0.1 (i32.const 1048) "\02\00\00\00\1c\00\00\00I\00n\00v\00a\00l\00i\00d\00 \00l\00e\00n\00g\00t\00h")
 (data $1 (i32.const 1084) ",")
 (data $1.1 (i32.const 1096) "\02\00\00\00\1a\00\00\00~\00l\00i\00b\00/\00a\00r\00r\00a\00y\00.\00t\00s")
 (data $2 (i32.const 1132) "<")
 (data $2.1 (i32.const 1144) "\02\00\00\00(\00\00\00A\00l\00l\00o\00c\00a\00t\00i\00o\00n\00 \00t\00o\00o\00 \00l\00a\00r\00g\00e")
 (data $3 (i32.const 1196) "<")
 (data $3.1 (i32.const 1208) "\02\00\00\00\1e\00\00\00~\00l\00i\00b\00/\00r\00t\00/\00s\00t\00u\00b\00.\00t\00s")
 (data $4 (i32.const 1260) "<")
 (data $4.1 (i32.const 1272) "\02\00\00\00&\00\00\00~\00l\00i\00b\00/\00a\00r\00r\00a\00y\00b\00u\00f\00f\00e\00r\00.\00t\00s")
 (data $5 (i32.const 1324) "\1c")
 (data $5.1 (i32.const 1336) "\0f\00\00\00\08\00\00\00\01")
 (data $6 (i32.const 1356) "\1c")
 (data $6.1 (i32.const 1368) "\11\00\00\00\08\00\00\00\02")
 (data $7 (i32.const 1388) "\1c")
 (data $7.1 (i32.const 1400) "\12\00\00\00\08\00\00\00\03")
 (data $8 (i32.const 1420) "<")
 (data $8.1 (i32.const 1432) "\02\00\00\00$\00\00\00I\00n\00d\00e\00x\00 \00o\00u\00t\00 \00o\00f\00 \00r\00a\00n\00g\00e")
 (data $9 (i32.const 1484) "\1c")
 (data $9.1 (i32.const 1496) "\15\00\00\00\08\00\00\00\04")
 (table $0 5 5 funcref)
 (elem $0 (i32.const 1) $../alkanes-asm-common/assembly/parcel/AlkaneTransferParcel#serialize~anonymous|0 $../alkanes-asm-common/assembly/utils/box/Box.concat~anonymous|0 $../alkanes-asm-common/assembly/utils/box/Box.concat~anonymous|1 $../alkanes-asm-common/assembly/storage-map/StorageMap#serialize~anonymous|0)
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
   i32.const 1152
   i32.const 1216
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
   i32.const 1152
   i32.const 1216
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
 (func $~lib/arraybuffer/ArrayBuffer#constructor (param $0 i32) (result i32)
  (local $1 i32)
  local.get $0
  i32.const 1073741820
  i32.gt_u
  if
   i32.const 1056
   i32.const 1280
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
 (func $../alkanes-asm-common/assembly/alkanes/utils/storeU128 (param $0 i32) (param $1 i32)
  (local $2 i64)
  (local $3 i32)
  (local $4 i64)
  i32.const 16
  i32.const 8
  call $~lib/rt/stub/__new
  local.tee $3
  i64.const -1
  i64.store
  local.get $3
  i64.const -1
  i64.store offset=8
  local.get $1
  i64.load
  local.get $3
  i64.load
  i64.and
  local.set $2
  local.get $1
  i64.load offset=8
  local.get $3
  i64.load offset=8
  i64.and
  local.set $4
  i32.const 16
  i32.const 8
  call $~lib/rt/stub/__new
  local.tee $3
  local.get $2
  i64.store
  local.get $3
  local.get $4
  i64.store offset=8
  local.get $1
  i64.load offset=8
  local.get $1
  i64.load
  i64.const 0
  i64.and
  i64.or
  local.set $2
  i32.const 16
  i32.const 8
  call $~lib/rt/stub/__new
  local.tee $1
  local.get $2
  i64.store
  local.get $1
  i64.const 0
  i64.store offset=8
  local.get $0
  local.get $3
  i64.load
  i64.store
  local.get $0
  local.get $1
  i64.load
  i64.store offset=8
 )
 (func $../alkanes-asm-common/assembly/utils/box/Box#constructor (param $0 i32) (param $1 i32) (result i32)
  (local $2 i32)
  i32.const 8
  i32.const 13
  call $~lib/rt/stub/__new
  local.tee $2
  i32.const 0
  i32.store
  local.get $2
  i32.const 0
  i32.store offset=4
  local.get $2
  local.get $0
  i32.store
  local.get $2
  local.get $1
  i32.store offset=4
  local.get $2
 )
 (func $../alkanes-asm-common/assembly/parcel/AlkaneTransferParcel#serialize~anonymous|0 (param $0 i32) (param $1 i32) (param $2 i32) (result i32)
  i32.const 48
  call $~lib/arraybuffer/ArrayBuffer#constructor
  local.tee $1
  local.get $0
  i32.load
  i32.load
  call $../alkanes-asm-common/assembly/alkanes/utils/storeU128
  local.get $1
  i32.const 16
  i32.add
  local.get $0
  i32.load
  i32.load offset=4
  call $../alkanes-asm-common/assembly/alkanes/utils/storeU128
  local.get $1
  i32.const 32
  i32.add
  local.get $0
  i32.load offset=4
  call $../alkanes-asm-common/assembly/alkanes/utils/storeU128
  local.get $1
  local.get $1
  i32.const 20
  i32.sub
  i32.load offset=16
  call $../alkanes-asm-common/assembly/utils/box/Box#constructor
 )
 (func $~lib/rt/__newArray (param $0 i32) (result i32)
  (local $1 i32)
  (local $2 i32)
  (local $3 i32)
  local.get $0
  i32.const 2
  i32.shl
  local.tee $3
  i32.const 1
  call $~lib/rt/stub/__new
  local.set $2
  i32.const 16
  i32.const 14
  call $~lib/rt/stub/__new
  local.tee $1
  local.get $2
  i32.store
  local.get $1
  local.get $2
  i32.store offset=4
  local.get $1
  local.get $3
  i32.store offset=8
  local.get $1
  local.get $0
  i32.store offset=12
  local.get $1
 )
 (func $~lib/array/Array<../alkanes-asm-common/assembly/parcel/AlkaneTransfer>#map<../alkanes-asm-common/assembly/utils/box/Box> (param $0 i32) (param $1 i32) (result i32)
  (local $2 i32)
  (local $3 i32)
  (local $4 i32)
  (local $5 i32)
  (local $6 i32)
  local.get $0
  i32.load offset=12
  local.tee $3
  call $~lib/rt/__newArray
  local.tee $4
  i32.load offset=4
  local.set $5
  loop $for-loop|0
   local.get $2
   local.get $3
   local.get $0
   i32.load offset=12
   local.tee $6
   local.get $3
   local.get $6
   i32.lt_s
   select
   i32.lt_s
   if
    local.get $5
    local.get $2
    i32.const 2
    i32.shl
    local.tee $6
    i32.add
    local.get $0
    i32.load offset=4
    local.get $6
    i32.add
    i32.load
    local.get $2
    local.get $0
    local.get $1
    i32.load
    call_indirect (type $3)
    i32.store
    local.get $2
    i32.const 1
    i32.add
    local.set $2
    br $for-loop|0
   end
  end
  local.get $4
 )
 (func $../alkanes-asm-common/assembly/utils/box/Box.concat~anonymous|0 (param $0 i32) (param $1 i32) (param $2 i32) (param $3 i32) (result i32)
  local.get $0
  local.get $1
  i32.load offset=4
  i32.add
 )
 (func $../alkanes-asm-common/assembly/utils/box/Box.concat~anonymous|1 (param $0 i32) (param $1 i32) (param $2 i32) (param $3 i32) (result i32)
  local.get $0
  local.get $1
  i32.load
  local.get $1
  i32.load offset=4
  memory.copy
  local.get $0
  local.get $1
  i32.load offset=4
  i32.add
 )
 (func $../alkanes-asm-common/assembly/utils/box/Box.concat (param $0 i32) (result i32)
  (local $1 i32)
  (local $2 i32)
  (local $3 i32)
  (local $4 i32)
  (local $5 i32)
  local.get $0
  i32.load offset=12
  local.set $3
  loop $for-loop|0
   local.get $1
   local.get $3
   local.get $0
   i32.load offset=12
   local.tee $4
   local.get $3
   local.get $4
   i32.lt_s
   select
   i32.lt_s
   if
    local.get $2
    local.get $0
    i32.load offset=4
    local.get $1
    i32.const 2
    i32.shl
    i32.add
    i32.load
    local.get $1
    local.get $0
    i32.const 1376
    i32.load
    call_indirect (type $1)
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
  local.set $2
  i32.const 0
  local.set $1
  local.get $0
  i32.load offset=12
  local.set $3
  loop $for-loop|01
   local.get $1
   local.get $3
   local.get $0
   i32.load offset=12
   local.tee $5
   local.get $3
   local.get $5
   i32.lt_s
   select
   i32.lt_s
   if
    local.get $2
    local.get $0
    i32.load offset=4
    local.get $1
    i32.const 2
    i32.shl
    i32.add
    i32.load
    local.get $1
    local.get $0
    i32.const 1408
    i32.load
    call_indirect (type $1)
    local.set $2
    local.get $1
    i32.const 1
    i32.add
    local.set $1
    br $for-loop|01
   end
  end
  local.get $4
 )
 (func $~lib/array/Array<../alkanes-asm-common/assembly/utils/box/Box>#__set (param $0 i32) (param $1 i32) (param $2 i32)
  (local $3 i32)
  (local $4 i32)
  (local $5 i32)
  (local $6 i32)
  (local $7 i32)
  (local $8 i32)
  (local $9 i32)
  (local $10 i32)
  (local $11 i32)
  local.get $1
  local.get $0
  i32.load offset=12
  i32.ge_u
  if
   local.get $1
   i32.const 0
   i32.lt_s
   if
    i32.const 1440
    i32.const 1104
    i32.const 130
    i32.const 22
    call $~lib/builtins/abort
    unreachable
   end
   local.get $1
   i32.const 1
   i32.add
   local.tee $5
   local.get $0
   i32.load offset=8
   local.tee $11
   i32.const 2
   i32.shr_u
   i32.gt_u
   if
    local.get $5
    i32.const 268435455
    i32.gt_u
    if
     i32.const 1056
     i32.const 1104
     i32.const 19
     i32.const 48
     call $~lib/builtins/abort
     unreachable
    end
    local.get $0
    i32.load
    local.set $10
    i32.const 1073741820
    local.get $11
    i32.const 1
    i32.shl
    local.tee $3
    local.get $3
    i32.const 1073741820
    i32.ge_u
    select
    local.tee $4
    i32.const 8
    local.get $5
    local.get $5
    i32.const 8
    i32.le_u
    select
    i32.const 2
    i32.shl
    local.tee $3
    local.get $3
    local.get $4
    i32.lt_u
    select
    local.tee $9
    i32.const 1073741804
    i32.gt_u
    if
     i32.const 1152
     i32.const 1216
     i32.const 99
     i32.const 30
     call $~lib/builtins/abort
     unreachable
    end
    local.get $10
    i32.const 16
    i32.sub
    local.tee $4
    i32.const 15
    i32.and
    i32.const 1
    local.get $4
    select
    if
     i32.const 0
     i32.const 1216
     i32.const 45
     i32.const 3
     call $~lib/builtins/abort
     unreachable
    end
    global.get $~lib/rt/stub/offset
    local.get $4
    i32.const 4
    i32.sub
    local.tee $8
    i32.load
    local.tee $6
    local.get $4
    i32.add
    i32.eq
    local.set $5
    local.get $9
    i32.const 16
    i32.add
    local.tee $3
    i32.const 19
    i32.add
    i32.const -16
    i32.and
    i32.const 4
    i32.sub
    local.set $7
    local.get $3
    local.get $6
    i32.gt_u
    if
     local.get $5
     if
      local.get $3
      i32.const 1073741820
      i32.gt_u
      if
       i32.const 1152
       i32.const 1216
       i32.const 52
       i32.const 33
       call $~lib/builtins/abort
       unreachable
      end
      local.get $4
      local.get $7
      i32.add
      local.tee $6
      memory.size
      local.tee $5
      i32.const 16
      i32.shl
      i32.const 15
      i32.add
      i32.const -16
      i32.and
      local.tee $3
      i32.gt_u
      if
       local.get $5
       local.get $6
       local.get $3
       i32.sub
       i32.const 65535
       i32.add
       i32.const -65536
       i32.and
       i32.const 16
       i32.shr_u
       local.tee $3
       local.get $3
       local.get $5
       i32.lt_s
       select
       memory.grow
       i32.const 0
       i32.lt_s
       if
        local.get $3
        memory.grow
        i32.const 0
        i32.lt_s
        if
         unreachable
        end
       end
      end
      local.get $6
      global.set $~lib/rt/stub/offset
      local.get $8
      local.get $7
      i32.store
     else
      local.get $7
      local.get $6
      i32.const 1
      i32.shl
      local.tee $3
      local.get $3
      local.get $7
      i32.lt_u
      select
      call $~lib/rt/stub/__alloc
      local.tee $3
      local.get $4
      local.get $6
      memory.copy
      local.get $3
      local.set $4
     end
    else
     local.get $5
     if
      local.get $4
      local.get $7
      i32.add
      global.set $~lib/rt/stub/offset
      local.get $8
      local.get $7
      i32.store
     end
    end
    local.get $4
    i32.const 4
    i32.sub
    local.get $9
    i32.store offset=16
    local.get $4
    i32.const 16
    i32.add
    local.tee $3
    local.get $11
    i32.add
    i32.const 0
    local.get $9
    local.get $11
    i32.sub
    memory.fill
    local.get $3
    local.get $10
    i32.ne
    if
     local.get $0
     local.get $3
     i32.store
     local.get $0
     local.get $3
     i32.store offset=4
    end
    local.get $0
    local.get $9
    i32.store offset=8
   end
   local.get $0
   local.get $1
   i32.const 1
   i32.add
   i32.store offset=12
  end
  local.get $0
  i32.load offset=4
  local.get $1
  i32.const 2
  i32.shl
  i32.add
  local.get $2
  i32.store
 )
 (func $../alkanes-asm-common/assembly/storage-map/StorageMap#serialize~anonymous|0 (param $0 i32) (param $1 i32) (param $2 i32) (result i32)
  (local $3 i32)
  i32.const 4
  call $~lib/arraybuffer/ArrayBuffer#constructor
  local.tee $3
  local.get $0
  i32.load
  i32.const 20
  i32.sub
  i32.load offset=16
  i32.store
  i32.const 4
  call $~lib/arraybuffer/ArrayBuffer#constructor
  local.tee $2
  local.get $0
  i32.load offset=4
  i32.const 20
  i32.sub
  i32.load offset=16
  i32.store
  i32.const 4
  call $~lib/rt/__newArray
  local.tee $1
  i32.load offset=4
  drop
  local.get $1
  i32.const 0
  local.get $3
  local.get $3
  i32.const 20
  i32.sub
  i32.load offset=16
  call $../alkanes-asm-common/assembly/utils/box/Box#constructor
  call $~lib/array/Array<../alkanes-asm-common/assembly/utils/box/Box>#__set
  local.get $1
  i32.const 1
  local.get $0
  i32.load
  local.tee $3
  local.get $3
  i32.const 20
  i32.sub
  i32.load offset=16
  call $../alkanes-asm-common/assembly/utils/box/Box#constructor
  call $~lib/array/Array<../alkanes-asm-common/assembly/utils/box/Box>#__set
  local.get $1
  i32.const 2
  local.get $2
  local.get $2
  i32.const 20
  i32.sub
  i32.load offset=16
  call $../alkanes-asm-common/assembly/utils/box/Box#constructor
  call $~lib/array/Array<../alkanes-asm-common/assembly/utils/box/Box>#__set
  local.get $1
  i32.const 3
  local.get $0
  i32.load offset=4
  local.tee $0
  local.get $0
  i32.const 20
  i32.sub
  i32.load offset=16
  call $../alkanes-asm-common/assembly/utils/box/Box#constructor
  call $~lib/array/Array<../alkanes-asm-common/assembly/utils/box/Box>#__set
  local.get $1
  call $../alkanes-asm-common/assembly/utils/box/Box.concat
  local.tee $0
  local.get $0
  i32.const 20
  i32.sub
  i32.load offset=16
  call $../alkanes-asm-common/assembly/utils/box/Box#constructor
 )
 (func $../alkanes-asm-common/assembly/alkanes/types/ExtendedCallResponse#finalize (param $0 i32) (result i32)
  (local $1 i32)
  (local $2 i32)
  (local $3 i32)
  (local $4 i64)
  (local $5 i32)
  (local $6 i32)
  (local $7 i32)
  (local $8 i32)
  (local $9 i32)
  local.get $0
  i32.load
  local.tee $5
  i32.load
  i32.const 1344
  call $~lib/array/Array<../alkanes-asm-common/assembly/parcel/AlkaneTransfer>#map<../alkanes-asm-common/assembly/utils/box/Box>
  call $../alkanes-asm-common/assembly/utils/box/Box.concat
  local.set $6
  i32.const 2
  call $~lib/rt/__newArray
  local.tee $7
  i32.load offset=4
  drop
  local.get $5
  i32.load
  i64.load32_s offset=12
  local.set $4
  i32.const 16
  i32.const 8
  call $~lib/rt/stub/__new
  local.tee $5
  local.get $4
  i64.store
  local.get $5
  local.get $4
  i64.const 63
  i64.shr_s
  i64.store offset=8
  i32.const 16
  i32.const 19
  call $~lib/rt/stub/__new
  local.tee $8
  i32.const 0
  i32.store
  local.get $8
  i32.const 0
  i32.store offset=4
  local.get $8
  i32.const 0
  i32.store offset=8
  local.get $8
  i32.const 0
  i32.store offset=12
  i32.const 16
  i32.const 1
  call $~lib/rt/stub/__new
  local.tee $9
  i64.const 0
  i64.store align=1
  local.get $9
  i64.const 0
  i64.store offset=8 align=1
  local.get $8
  local.get $9
  i32.store
  local.get $8
  local.get $9
  i32.store offset=4
  local.get $8
  i32.const 16
  i32.store offset=8
  local.get $8
  i32.const 16
  i32.store offset=12
  local.get $8
  i32.load offset=4
  local.tee $9
  local.get $5
  i64.load
  i64.store
  local.get $9
  local.get $5
  i64.load offset=8
  i64.store offset=8
  local.get $7
  i32.const 0
  local.get $8
  i32.load
  local.tee $5
  local.get $5
  i32.const 20
  i32.sub
  i32.load offset=16
  call $../alkanes-asm-common/assembly/utils/box/Box#constructor
  call $~lib/array/Array<../alkanes-asm-common/assembly/utils/box/Box>#__set
  local.get $7
  i32.const 1
  local.get $6
  local.get $6
  i32.const 20
  i32.sub
  i32.load offset=16
  call $../alkanes-asm-common/assembly/utils/box/Box#constructor
  call $~lib/array/Array<../alkanes-asm-common/assembly/utils/box/Box>#__set
  local.get $7
  call $../alkanes-asm-common/assembly/utils/box/Box.concat
  local.set $5
  local.get $0
  i32.load offset=4
  local.set $6
  i32.const 4
  call $~lib/arraybuffer/ArrayBuffer#constructor
  local.tee $7
  local.get $6
  i32.load
  i32.load offset=12
  i32.store
  local.get $6
  i32.load
  i32.const 1504
  call $~lib/array/Array<../alkanes-asm-common/assembly/parcel/AlkaneTransfer>#map<../alkanes-asm-common/assembly/utils/box/Box>
  call $../alkanes-asm-common/assembly/utils/box/Box.concat
  local.set $6
  i32.const 2
  call $~lib/rt/__newArray
  local.tee $8
  i32.load offset=4
  drop
  local.get $8
  i32.const 0
  local.get $7
  local.get $7
  i32.const 20
  i32.sub
  i32.load offset=16
  call $../alkanes-asm-common/assembly/utils/box/Box#constructor
  call $~lib/array/Array<../alkanes-asm-common/assembly/utils/box/Box>#__set
  local.get $8
  i32.const 1
  local.get $6
  local.get $6
  i32.const 20
  i32.sub
  i32.load offset=16
  call $../alkanes-asm-common/assembly/utils/box/Box#constructor
  call $~lib/array/Array<../alkanes-asm-common/assembly/utils/box/Box>#__set
  local.get $8
  call $../alkanes-asm-common/assembly/utils/box/Box.concat
  local.set $6
  local.get $0
  i32.load offset=8
  i32.const 20
  i32.sub
  i32.load offset=16
  local.get $5
  i32.const 20
  i32.sub
  i32.load offset=16
  local.get $6
  i32.const 20
  i32.sub
  i32.load offset=16
  i32.add
  i32.add
  call $~lib/arraybuffer/ArrayBuffer#constructor
  local.set $7
  local.get $5
  i32.const 20
  i32.sub
  i32.load offset=16
  local.set $8
  loop $for-loop|0
   local.get $1
   local.get $8
   i32.lt_u
   if
    local.get $1
    local.get $7
    i32.add
    local.get $1
    local.get $5
    i32.add
    i32.load8_u
    i32.store8
    local.get $1
    i32.const 1
    i32.add
    local.set $1
    br $for-loop|0
   end
  end
  local.get $6
  i32.const 20
  i32.sub
  i32.load offset=16
  local.set $1
  loop $for-loop|1
   local.get $1
   local.get $2
   i32.gt_u
   if
    local.get $7
    local.get $8
    i32.add
    local.get $2
    i32.add
    local.get $2
    local.get $6
    i32.add
    i32.load8_u
    i32.store8
    local.get $2
    i32.const 1
    i32.add
    local.set $2
    br $for-loop|1
   end
  end
  local.get $0
  i32.load offset=8
  local.tee $0
  i32.const 20
  i32.sub
  i32.load offset=16
  local.tee $2
  i32.const 0
  i32.gt_s
  if
   local.get $1
   local.get $8
   i32.add
   local.set $1
   loop $for-loop|2
    local.get $2
    local.get $3
    i32.gt_u
    if
     local.get $1
     local.get $7
     i32.add
     local.get $3
     i32.add
     local.get $0
     local.get $3
     i32.add
     i32.load8_u
     i32.store8
     local.get $3
     i32.const 1
     i32.add
     local.set $3
     br $for-loop|2
    end
   end
  end
  local.get $7
 )
 (func $assembly/index/__execute (result i32)
  (local $0 i32)
  (local $1 i32)
  (local $2 i32)
  (local $3 i32)
  i32.const 12
  i32.const 4
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
  i32.const 4
  i32.const 5
  call $~lib/rt/stub/__new
  local.tee $1
  i32.const 0
  i32.store
  i32.const 16
  i32.const 9
  call $~lib/rt/stub/__new
  local.tee $2
  i32.const 0
  i32.store
  local.get $2
  i32.const 0
  i32.store offset=4
  local.get $2
  i32.const 0
  i32.store offset=8
  local.get $2
  i32.const 0
  i32.store offset=12
  i32.const 32
  i32.const 1
  call $~lib/rt/stub/__new
  local.tee $3
  i32.const 0
  i32.const 32
  memory.fill
  local.get $2
  local.get $3
  i32.store
  local.get $2
  local.get $3
  i32.store offset=4
  local.get $2
  i32.const 32
  i32.store offset=8
  local.get $2
  i32.const 0
  i32.store offset=12
  local.get $1
  local.get $2
  i32.store
  local.get $0
  local.get $1
  i32.store
  i32.const 4
  i32.const 10
  call $~lib/rt/stub/__new
  local.tee $1
  i32.const 0
  i32.store
  i32.const 16
  i32.const 12
  call $~lib/rt/stub/__new
  local.tee $2
  i32.const 0
  i32.store
  local.get $2
  i32.const 0
  i32.store offset=4
  local.get $2
  i32.const 0
  i32.store offset=8
  local.get $2
  i32.const 0
  i32.store offset=12
  i32.const 32
  i32.const 1
  call $~lib/rt/stub/__new
  local.tee $3
  i32.const 0
  i32.const 32
  memory.fill
  local.get $2
  local.get $3
  i32.store
  local.get $2
  local.get $3
  i32.store offset=4
  local.get $2
  i32.const 32
  i32.store offset=8
  local.get $2
  i32.const 0
  i32.store offset=12
  local.get $1
  local.get $2
  i32.store
  local.get $0
  local.get $1
  i32.store offset=4
  local.get $0
  i32.const 0
  call $~lib/arraybuffer/ArrayBuffer#constructor
  i32.store offset=8
  i32.const 4
  call $~lib/arraybuffer/ArrayBuffer#constructor
  local.tee $1
  i32.const 1
  i32.store8
  local.get $1
  i32.const 2
  i32.store8 offset=1
  local.get $1
  i32.const 3
  i32.store8 offset=2
  local.get $1
  i32.const 4
  i32.store8 offset=3
  local.get $0
  local.get $1
  i32.store offset=8
  local.get $0
  call $../alkanes-asm-common/assembly/alkanes/types/ExtendedCallResponse#finalize
 )
 (func $~start
  global.get $~started
  if
   return
  end
  i32.const 1
  global.set $~started
  i32.const 1516
  global.set $~lib/rt/stub/offset
 )
)
