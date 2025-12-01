(module
 (type $0 (func (param i32) (result i32)))
 (type $1 (func (result i32)))
 (type $2 (func (param i32 i32) (result i32)))
 (type $3 (func (param i32 i32 i32 i64) (result i32)))
 (type $4 (func (param i32 i32 i32 i32)))
 (type $5 (func))
 (import "env" "__request_context" (func $../alkanes-asm-common/assembly/alkanes/runtime/__request_context (result i32)))
 (import "env" "abort" (func $~lib/builtins/abort (param i32 i32 i32 i32)))
 (import "env" "__load_context" (func $../alkanes-asm-common/assembly/alkanes/runtime/__load_context (param i32) (result i32)))
 (import "env" "__staticcall" (func $../alkanes-asm-common/assembly/alkanes/runtime/__staticcall (param i32 i32 i32 i64) (result i32)))
 (import "env" "__call" (func $../alkanes-asm-common/assembly/alkanes/runtime/__call (param i32 i32 i32 i64) (result i32)))
 (global $~lib/rt/stub/offset (mut i32) (i32.const 0))
 (global $~started (mut i32) (i32.const 0))
 (memory $0 1)
 (data $0 (i32.const 1036) ",")
 (data $0.1 (i32.const 1048) "\02\00\00\00\1c\00\00\00I\00n\00v\00a\00l\00i\00d\00 \00l\00e\00n\00g\00t\00h")
 (data $1 (i32.const 1084) "<")
 (data $1.1 (i32.const 1096) "\02\00\00\00&\00\00\00~\00l\00i\00b\00/\00a\00r\00r\00a\00y\00b\00u\00f\00f\00e\00r\00.\00t\00s")
 (data $2 (i32.const 1148) "<")
 (data $2.1 (i32.const 1160) "\02\00\00\00(\00\00\00A\00l\00l\00o\00c\00a\00t\00i\00o\00n\00 \00t\00o\00o\00 \00l\00a\00r\00g\00e")
 (data $3 (i32.const 1212) "<")
 (data $3.1 (i32.const 1224) "\02\00\00\00\1e\00\00\00~\00l\00i\00b\00/\00r\00t\00/\00s\00t\00u\00b\00.\00t\00s")
 (data $4 (i32.const 1276) "\1c")
 (data $4.1 (i32.const 1288) "\01")
 (data $5 (i32.const 1308) ",")
 (data $5.1 (i32.const 1320) "\02\00\00\00\1a\00\00\00~\00l\00i\00b\00/\00a\00r\00r\00a\00y\00.\00t\00s")
 (data $6 (i32.const 1356) "<")
 (data $6.1 (i32.const 1368) "\02\00\00\00$\00\00\00I\00n\00d\00e\00x\00 \00o\00u\00t\00 \00o\00f\00 \00r\00a\00n\00g\00e")
 (data $7 (i32.const 1420) "|")
 (data $7.1 (i32.const 1432) "\02\00\00\00^\00\00\00E\00l\00e\00m\00e\00n\00t\00 \00t\00y\00p\00e\00 \00m\00u\00s\00t\00 \00b\00e\00 \00n\00u\00l\00l\00a\00b\00l\00e\00 \00i\00f\00 \00a\00r\00r\00a\00y\00 \00i\00s\00 \00h\00o\00l\00e\00y")
 (export "__execute" (func $assembly/index/__execute))
 (export "memory" (memory $0))
 (export "_start" (func $~start))
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
   i32.const 1168
   i32.const 1232
   i32.const 86
   i32.const 30
   call $~lib/builtins/abort
   unreachable
  end
  local.get $0
  i32.const 16
  i32.add
  local.tee $4
  i32.const 1073741820
  i32.gt_u
  if
   i32.const 1168
   i32.const 1232
   i32.const 33
   i32.const 29
   call $~lib/builtins/abort
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
 (func $~lib/arraybuffer/ArrayBuffer#constructor (param $0 i32) (result i32)
  (local $1 i32)
  local.get $0
  i32.const 1073741820
  i32.gt_u
  if
   i32.const 1056
   i32.const 1104
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
 (func $~lib/array/Array<../alkanes-asm-common/assembly/parcel/AlkaneTransfer>#__get (param $0 i32) (param $1 i32) (result i32)
  local.get $1
  local.get $0
  i32.load offset=12
  i32.ge_u
  if
   i32.const 1376
   i32.const 1328
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
   i32.const 1328
   i32.const 118
   i32.const 40
   call $~lib/builtins/abort
   unreachable
  end
  local.get $0
 )
 (func $../alkanes-asm-common/assembly/alkanes/types/ExtendedCallResponse#finalize (param $0 i32) (result i32)
  (local $1 i32)
  (local $2 i32)
  (local $3 i32)
  (local $4 i32)
  (local $5 i32)
  (local $6 i32)
  (local $7 i64)
  (local $8 i32)
  (local $9 i32)
  (local $10 i32)
  (local $11 i32)
  (local $12 i32)
  (local $13 i32)
  local.get $0
  i32.load
  local.tee $8
  i32.load
  i32.load offset=12
  i32.const 48
  i32.mul
  i32.const 16
  i32.add
  call $~lib/arraybuffer/ArrayBuffer#constructor
  local.set $9
  local.get $8
  i32.load
  i64.load32_s offset=12
  local.set $7
  i32.const 16
  i32.const 8
  call $~lib/rt/stub/__new
  local.tee $1
  i64.const 0
  i64.store
  local.get $1
  i64.const 0
  i64.store offset=8
  local.get $1
  local.get $7
  i64.store
  local.get $1
  i64.const 0
  i64.store offset=8
  local.get $9
  local.get $1
  i64.load
  i64.store
  local.get $9
  local.get $1
  i64.load offset=8
  i64.store offset=8
  i32.const 16
  local.set $1
  loop $for-loop|0
   local.get $2
   local.get $8
   i32.load
   local.tee $10
   i32.load offset=12
   i32.lt_s
   if
    local.get $1
    local.get $9
    i32.add
    local.tee $11
    local.get $10
    local.get $2
    call $~lib/array/Array<../alkanes-asm-common/assembly/parcel/AlkaneTransfer>#__get
    local.tee $10
    i32.load
    i32.load
    local.tee $12
    i64.load
    i64.store
    local.get $11
    local.get $12
    i64.load offset=8
    i64.store offset=8
    local.get $11
    i32.const 16
    i32.add
    local.tee $12
    local.get $10
    i32.load
    i32.load offset=4
    local.tee $13
    i64.load
    i64.store
    local.get $12
    local.get $13
    i64.load offset=8
    i64.store offset=8
    local.get $11
    i32.const 32
    i32.add
    local.tee $11
    local.get $10
    i32.load offset=4
    local.tee $10
    i64.load
    i64.store
    local.get $11
    local.get $10
    i64.load offset=8
    i64.store offset=8
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
  local.get $0
  i32.load offset=4
  local.set $8
  i32.const 0
  local.set $2
  i32.const 4
  local.set $1
  loop $for-loop|00
   local.get $2
   local.get $8
   i32.load
   local.tee $10
   i32.load offset=12
   i32.lt_s
   if
    local.get $1
    local.get $10
    local.get $2
    call $~lib/array/Array<../alkanes-asm-common/assembly/parcel/AlkaneTransfer>#__get
    local.tee $1
    i32.load offset=4
    i32.const 20
    i32.sub
    i32.load offset=16
    local.get $1
    i32.load
    i32.const 20
    i32.sub
    i32.load offset=16
    i32.const 8
    i32.add
    i32.add
    i32.add
    local.set $1
    local.get $2
    i32.const 1
    i32.add
    local.set $2
    br $for-loop|00
   end
  end
  local.get $1
  call $~lib/arraybuffer/ArrayBuffer#constructor
  local.tee $1
  local.get $8
  i32.load
  i32.load offset=12
  i32.store
  i32.const 4
  local.set $2
  loop $for-loop|1
   local.get $3
   local.get $8
   i32.load
   local.tee $10
   i32.load offset=12
   i32.lt_s
   if
    local.get $1
    local.get $2
    i32.add
    local.get $10
    local.get $3
    call $~lib/array/Array<../alkanes-asm-common/assembly/parcel/AlkaneTransfer>#__get
    local.tee $10
    i32.load
    i32.const 20
    i32.sub
    i32.load offset=16
    i32.store
    local.get $2
    i32.const 4
    i32.add
    local.set $11
    local.get $10
    i32.load
    local.tee $2
    local.set $12
    local.get $2
    i32.const 20
    i32.sub
    i32.load offset=16
    local.set $13
    i32.const 0
    local.set $2
    loop $for-loop|2
     local.get $2
     local.get $13
     i32.lt_u
     if
      local.get $1
      local.get $11
      i32.add
      local.get $2
      i32.add
      local.get $2
      local.get $12
      i32.add
      i32.load8_u
      i32.store8
      local.get $2
      i32.const 1
      i32.add
      local.set $2
      br $for-loop|2
     end
    end
    local.get $11
    local.get $13
    i32.add
    local.tee $2
    local.get $1
    i32.add
    local.get $10
    i32.load offset=4
    i32.const 20
    i32.sub
    i32.load offset=16
    i32.store
    local.get $2
    i32.const 4
    i32.add
    local.set $11
    local.get $10
    i32.load offset=4
    local.set $12
    local.get $10
    i32.load offset=4
    i32.const 20
    i32.sub
    i32.load offset=16
    local.set $10
    i32.const 0
    local.set $2
    loop $for-loop|3
     local.get $2
     local.get $10
     i32.lt_u
     if
      local.get $1
      local.get $11
      i32.add
      local.get $2
      i32.add
      local.get $2
      local.get $12
      i32.add
      i32.load8_u
      i32.store8
      local.get $2
      i32.const 1
      i32.add
      local.set $2
      br $for-loop|3
     end
    end
    local.get $10
    local.get $11
    i32.add
    local.set $2
    local.get $3
    i32.const 1
    i32.add
    local.set $3
    br $for-loop|1
   end
  end
  local.get $0
  i32.load offset=8
  i32.const 20
  i32.sub
  i32.load offset=16
  local.get $9
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
  local.set $2
  local.get $9
  i32.const 20
  i32.sub
  i32.load offset=16
  local.set $3
  loop $for-loop|001
   local.get $3
   local.get $4
   i32.gt_u
   if
    local.get $2
    local.get $4
    i32.add
    local.get $4
    local.get $9
    i32.add
    i32.load8_u
    i32.store8
    local.get $4
    i32.const 1
    i32.add
    local.set $4
    br $for-loop|001
   end
  end
  local.get $1
  i32.const 20
  i32.sub
  i32.load offset=16
  local.set $4
  loop $for-loop|12
   local.get $4
   local.get $5
   i32.gt_u
   if
    local.get $2
    local.get $3
    i32.add
    local.get $5
    i32.add
    local.get $1
    local.get $5
    i32.add
    i32.load8_u
    i32.store8
    local.get $5
    i32.const 1
    i32.add
    local.set $5
    br $for-loop|12
   end
  end
  local.get $0
  i32.load offset=8
  local.tee $0
  i32.const 20
  i32.sub
  i32.load offset=16
  local.tee $1
  i32.const 0
  i32.gt_s
  if
   local.get $3
   local.get $4
   i32.add
   local.set $3
   loop $for-loop|23
    local.get $1
    local.get $6
    i32.gt_u
    if
     local.get $2
     local.get $3
     i32.add
     local.get $6
     i32.add
     local.get $0
     local.get $6
     i32.add
     i32.load8_u
     i32.store8
     local.get $6
     i32.const 1
     i32.add
     local.set $6
     br $for-loop|23
    end
   end
  end
  local.get $2
 )
 (func $assembly/index/__execute (result i32)
  (local $0 i32)
  (local $1 i64)
  (local $2 i64)
  (local $3 i32)
  (local $4 i64)
  (local $5 i32)
  (local $6 i32)
  call $../alkanes-asm-common/assembly/alkanes/runtime/__request_context
  local.tee $0
  call $~lib/arraybuffer/ArrayBuffer#constructor
  local.tee $3
  call $../alkanes-asm-common/assembly/alkanes/runtime/__load_context
  drop
  local.get $3
  i64.load
  drop
  local.get $3
  i64.load offset=8
  drop
  local.get $3
  i64.load offset=16
  local.set $2
  local.get $3
  i64.load offset=24
  local.set $4
  local.get $0
  i32.const 96
  i32.gt_s
  if (result i64)
   local.get $3
   i64.load offset=96
  else
   i64.const 0
  end
  local.set $1
  i32.const 12
  i32.const 4
  call $~lib/rt/stub/__new
  local.tee $3
  i32.const 0
  i32.store
  local.get $3
  i32.const 0
  i32.store offset=4
  local.get $3
  i32.const 0
  i32.store offset=8
  i32.const 4
  i32.const 5
  call $~lib/rt/stub/__new
  local.tee $0
  i32.const 0
  i32.store
  i32.const 0
  i32.const 1
  call $~lib/rt/stub/__new
  local.tee $5
  i32.const 1296
  i32.const 0
  memory.copy
  i32.const 16
  i32.const 9
  call $~lib/rt/stub/__new
  local.tee $6
  local.get $5
  i32.store
  local.get $6
  local.get $5
  i32.store offset=4
  local.get $6
  i32.const 0
  i32.store offset=8
  local.get $6
  i32.const 0
  i32.store offset=12
  local.get $0
  local.get $6
  i32.store
  local.get $3
  local.get $0
  i32.store
  i32.const 4
  i32.const 10
  call $~lib/rt/stub/__new
  local.tee $0
  i32.const 0
  i32.store
  i32.const 16
  i32.const 12
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
  i32.const 0
  i32.store offset=12
  i32.const 32
  i32.const 1
  call $~lib/rt/stub/__new
  local.tee $6
  i32.const 0
  i32.const 32
  memory.fill
  local.get $5
  local.get $6
  i32.store
  local.get $5
  local.get $6
  i32.store offset=4
  local.get $5
  i32.const 32
  i32.store offset=8
  local.get $5
  i32.const 0
  i32.store offset=12
  local.get $0
  local.get $5
  i32.store
  local.get $3
  local.get $0
  i32.store offset=4
  local.get $3
  i32.const 0
  call $~lib/arraybuffer/ArrayBuffer#constructor
  i32.store offset=8
  local.get $1
  i64.const -1
  i64.eq
  if
   i32.const 48
   call $~lib/arraybuffer/ArrayBuffer#constructor
   local.tee $0
   i64.const 2
   i64.store
   local.get $0
   i64.const 0
   i64.store offset=8
   local.get $0
   i64.const 0
   i64.store offset=16
   local.get $0
   i64.const 0
   i64.store offset=24
   local.get $0
   i64.const 102
   i64.store offset=32
   local.get $0
   i64.const 0
   i64.store offset=40
   i32.const 16
   call $~lib/arraybuffer/ArrayBuffer#constructor
   local.tee $5
   i64.const 0
   i64.store
   local.get $5
   i64.const 0
   i64.store offset=8
   i32.const 4
   call $~lib/arraybuffer/ArrayBuffer#constructor
   local.tee $6
   i32.const 0
   i32.store
   local.get $0
   local.get $5
   local.get $6
   i64.const 10000
   call $../alkanes-asm-common/assembly/alkanes/runtime/__staticcall
   local.set $5
   i32.const 8
   call $~lib/arraybuffer/ArrayBuffer#constructor
   local.tee $0
   local.get $5
   i64.extend_i32_s
   i64.store
  else
   i32.const 48
   call $~lib/arraybuffer/ArrayBuffer#constructor
   local.tee $0
   i64.const 5
   i64.store
   local.get $0
   i64.const 0
   i64.store offset=8
   local.get $0
   local.get $2
   i64.store offset=16
   local.get $0
   local.get $4
   i64.store offset=24
   local.get $0
   i64.const -1
   i64.store offset=32
   local.get $0
   i64.const 0
   i64.store offset=40
   i32.const 16
   call $~lib/arraybuffer/ArrayBuffer#constructor
   local.tee $5
   i64.const 0
   i64.store
   local.get $5
   i64.const 0
   i64.store offset=8
   i32.const 4
   call $~lib/arraybuffer/ArrayBuffer#constructor
   local.tee $6
   i32.const 0
   i32.store
   local.get $0
   local.get $5
   local.get $6
   i64.const 50000
   call $../alkanes-asm-common/assembly/alkanes/runtime/__call
   local.set $5
   i32.const 8
   call $~lib/arraybuffer/ArrayBuffer#constructor
   local.tee $0
   local.get $5
   i64.extend_i32_s
   local.tee $1
   i64.const -4294967296
   i64.or
   local.get $1
   local.get $5
   i32.const 0
   i32.lt_s
   select
   i64.store
  end
  local.get $3
  local.get $0
  i32.store offset=8
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
  i32.const 1548
  global.set $~lib/rt/stub/offset
 )
)
