(module
 (type $0 (func (param i32) (result i32)))
 (type $1 (func (param i32 i32)))
 (type $2 (func (param i32 i32) (result i32)))
 (type $3 (func (param i32 i32 i32) (result i32)))
 (type $4 (func))
 (type $5 (func (param i32)))
 (type $6 (func (param i32 i32 i32)))
 (type $7 (func (param i32 i32) (result f64)))
 (type $8 (func (param i32 i32 i32 i32 i32) (result i32)))
 (type $9 (func (param i32 i32 i32 i32)))
 (import "env" "abort" (func $~lib/builtins/abort (param i32 i32 i32 i32)))
 (import "env" "__log" (func $assembly/logging/__log (param i32)))
 (global $~lib/rt/stub/startOffset (mut i32) (i32.const 0))
 (global $~lib/rt/stub/offset (mut i32) (i32.const 0))
 (global $assembly/logging/console (mut i32) (i32.const 0))
 (global $assembly/hex/hexLookupTable i32 (i32.const 160))
 (global $assembly/sha256/K i32 (i32.const 992))
 (global $~lib/shared/runtime/Runtime.Stub i32 (i32.const 0))
 (global $~lib/shared/runtime/Runtime.Minimal i32 (i32.const 1))
 (global $~lib/shared/runtime/Runtime.Incremental i32 (i32.const 2))
 (global $~lib/native/ASC_RUNTIME i32 (i32.const 0))
 (global $~argumentsLength (mut i32) (i32.const 0))
 (global $~lib/memory/__heap_base i32 (i32.const 2428))
 (memory $0 1 32768)
 (data $0 (i32.const 12) "<\00\00\00\00\00\00\00\00\00\00\00\02\00\00\00(\00\00\00A\00l\00l\00o\00c\00a\00t\00i\00o\00n\00 \00t\00o\00o\00 \00l\00a\00r\00g\00e\00\00\00\00\00")
 (data $1 (i32.const 76) "<\00\00\00\00\00\00\00\00\00\00\00\02\00\00\00\1e\00\00\00~\00l\00i\00b\00/\00r\00t\00/\00s\00t\00u\00b\00.\00t\00s\00\00\00\00\00\00\00\00\00\00\00\00\00\00\00")
 (data $2 (i32.const 140) "\1c\02\00\00\00\00\00\00\00\00\00\00\05\00\00\00\00\02\00\00000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f202122232425262728292a2b2c2d2e2f303132333435363738393a3b3c3d3e3f404142434445464748494a4b4c4d4e4f505152535455565758595a5b5c5d5e5f606162636465666768696a6b6c6d6e6f707172737475767778797a7b7c7d7e7f808182838485868788898a8b8c8d8e8f909192939495969798999a9b9c9d9e9fa0a1a2a3a4a5a6a7a8a9aaabacadaeafb0b1b2b3b4b5b6b7b8b9babbbcbdbebfc0c1c2c3c4c5c6c7c8c9cacbcccdcecfd0d1d2d3d4d5d6d7d8d9dadbdcdddedfe0e1e2e3e4e5e6e7e8e9eaebecedeeeff0f1f2f3f4f5f6f7f8f9fafbfcfdfeff\00\00\00\00\00\00\00\00\00\00\00\00")
 (data $3 (i32.const 684) "\1c\01\00\00\00\00\00\00\00\00\00\00\01\00\00\00\00\01\00\00\98/\8aB\91D7q\cf\fb\c0\b5\a5\db\b5\e9[\c2V9\f1\11\f1Y\a4\82?\92\d5^\1c\ab\98\aa\07\d8\01[\83\12\be\851$\c3}\0cUt]\ber\fe\b1\de\80\a7\06\dc\9bt\f1\9b\c1\c1i\9b\e4\86G\be\ef\c6\9d\c1\0f\cc\a1\0c$o,\e9-\aa\84tJ\dc\a9\b0\\\da\88\f9vRQ>\98m\c61\a8\c8\'\03\b0\c7\7fY\bf\f3\0b\e0\c6G\91\a7\d5Qc\ca\06g))\14\85\n\b7\'8!\1b.\fcm,M\13\r8STs\ne\bb\njv.\c9\c2\81\85,r\92\a1\e8\bf\a2Kf\1a\a8p\8bK\c2\a3Ql\c7\19\e8\92\d1$\06\99\d6\855\0e\f4p\a0j\10\16\c1\a4\19\08l7\1eLwH\'\b5\bc\b04\b3\0c\1c9J\aa\d8NO\ca\9c[\f3o.h\ee\82\8ftoc\a5x\14x\c8\84\08\02\c7\8c\fa\ff\be\90\eblP\a4\f7\a3\f9\be\f2xq\c6\00\00\00\00\00\00\00\00\00\00\00\00")
 (data $4 (i32.const 972) ",\00\00\00\00\00\00\00\00\00\00\00\06\00\00\00\10\00\00\00\c0\02\00\00\c0\02\00\00\00\01\00\00@\00\00\00\00\00\00\00\00\00\00\00\00\00\00\00")
 (data $5 (i32.const 1020) ",\00\00\00\00\00\00\00\00\00\00\00\02\00\00\00\1c\00\00\00I\00n\00v\00a\00l\00i\00d\00 \00l\00e\00n\00g\00t\00h\00")
 (data $6 (i32.const 1068) "<\00\00\00\00\00\00\00\00\00\00\00\02\00\00\00&\00\00\00~\00l\00i\00b\00/\00a\00r\00r\00a\00y\00b\00u\00f\00f\00e\00r\00.\00t\00s\00\00\00\00\00\00\00")
 (data $7 (i32.const 1132) "\1c\00\00\00\00\00\00\00\00\00\00\00\02\00\00\00\00\00\00\00\00\00\00\00\00\00\00\00\00\00\00\00")
 (data $8 (i32.const 1164) "\0c\03\00\00\00\00\00\00\00\00\00\00\02\00\00\00\fc\02\00\000\001\000\000\000\000\000\000\000\001\007\004\00a\000\001\006\00b\00a\00d\008\009\002\007\00e\00e\007\003\009\00a\00f\00f\007\002\008\009\00f\005\005\007\004\00b\008\00f\005\007\00f\00c\00c\003\002\002\007\002\008\002\009\00a\00c\003\00a\00e\00f\003\00e\00d\00b\009\009\005\009\00a\00d\005\008\000\000\000\000\000\000\000\000\004\008\004\007\003\000\004\004\000\002\002\000\001\008\00d\001\00f\00c\002\001\002\00a\00f\00f\00b\004\001\00d\00d\003\002\002\00f\00d\00c\00d\00c\008\00d\000\004\007\004\00d\007\00e\00a\00b\007\00a\00b\004\001\004\00a\007\001\002\003\006\001\00f\008\001\00f\009\003\00d\00f\007\00f\00d\009\00b\00e\00d\000\002\002\000\000\00d\007\00d\004\003\001\00a\00c\003\00c\007\00b\006\001\00a\004\006\00b\003\000\00f\009\001\001\007\009\007\00f\00d\00e\004\00e\00d\00e\003\000\001\004\004\003\004\00c\00b\000\000\001\004\009\00a\00b\007\001\00e\00e\00d\002\006\003\003\005\00f\000\002\000\001\00f\00f\00f\00f\00f\00f\00f\00f\000\002\000\000\001\001\001\000\002\004\000\001\000\000\000\000\000\000\001\009\007\006\00a\009\001\004\002\000\004\002\000\00e\005\006\000\007\009\001\005\000\00b\005\000\00f\00b\000\006\001\007\00d\00c\00e\004\00c\003\007\004\00b\00d\006\001\00e\00c\00c\00e\00a\008\008\00a\00c\000\00a\00e\001\00f\005\000\005\000\000\000\000\000\000\000\000\001\009\007\006\00a\009\001\004\001\000\00f\009\005\008\00c\00b\00e\009\00c\00f\006\00d\002\006\00c\002\00e\002\00a\00c\00e\003\009\007\006\006\00c\002\00c\00d\00b\008\007\001\007\009\00f\007\008\008\00a\00c\000\000\000\000\000\000\000\000\00")
 (data $9 (i32.const 1948) "<\00\00\00\00\00\00\00\00\00\00\00\02\00\00\00$\00\00\00I\00n\00d\00e\00x\00 \00o\00u\00t\00 \00o\00f\00 \00r\00a\00n\00g\00e\00\00\00\00\00\00\00\00\00")
 (data $10 (i32.const 2012) "<\00\00\00\00\00\00\00\00\00\00\00\02\00\00\00$\00\00\00~\00l\00i\00b\00/\00t\00y\00p\00e\00d\00a\00r\00r\00a\00y\00.\00t\00s\00\00\00\00\00\00\00\00\00")
 (data $11 (i32.const 2076) "|\00\00\00\00\00\00\00\00\00\00\00\02\00\00\00^\00\00\00S\00H\00A\002\005\006\00:\00 \00c\00a\00n\00\'\00t\00 \00u\00p\00d\00a\00t\00e\00 \00b\00e\00c\00a\00u\00s\00e\00 \00h\00a\00s\00h\00 \00w\00a\00s\00 \00f\00i\00n\00i\00s\00h\00e\00d\00.\00\00\00\00\00\00\00\00\00\00\00\00\00\00\00")
 (data $12 (i32.const 2204) "<\00\00\00\00\00\00\00\00\00\00\00\02\00\00\00$\00\00\00a\00s\00s\00e\00m\00b\00l\00y\00/\00s\00h\00a\002\005\006\00.\00t\00s\00\00\00\00\00\00\00\00\00")
 (data $13 (i32.const 2268) ",\00\00\00\00\00\00\00\00\00\00\00\02\00\00\00\1a\00\00\00~\00l\00i\00b\00/\00a\00r\00r\00a\00y\00.\00t\00s\00\00\00")
 (data $14 (i32.const 2316) ",\00\00\00\00\00\00\00\00\00\00\00\02\00\00\00\1c\00\00\00~\00l\00i\00b\00/\00s\00t\00r\00i\00n\00g\00.\00t\00s\00")
 (data $15 (i32.const 2364) "<\00\00\00\00\00\00\00\00\00\00\00\02\00\00\00$\00\00\00U\00n\00p\00a\00i\00r\00e\00d\00 \00s\00u\00r\00r\00o\00g\00a\00t\00e\00\00\00\00\00\00\00\00\00")
 (table $0 1 1 funcref)
 (elem $0 (i32.const 1))
 (export "decodeHex" (func $assembly/index/decodeHex))
 (export "test_sha256" (func $assembly/index/test_sha256))
 (export "memory" (memory $0))
 (start $~start)
 (func $~lib/rt/stub/maybeGrowMemory (param $newOffset i32)
  (local $pagesBefore i32)
  (local $maxOffset i32)
  (local $pagesNeeded i32)
  (local $4 i32)
  (local $5 i32)
  (local $pagesWanted i32)
  memory.size
  local.set $pagesBefore
  local.get $pagesBefore
  i32.const 16
  i32.shl
  i32.const 15
  i32.add
  i32.const 15
  i32.const -1
  i32.xor
  i32.and
  local.set $maxOffset
  local.get $newOffset
  local.get $maxOffset
  i32.gt_u
  if
   local.get $newOffset
   local.get $maxOffset
   i32.sub
   i32.const 65535
   i32.add
   i32.const 65535
   i32.const -1
   i32.xor
   i32.and
   i32.const 16
   i32.shr_u
   local.set $pagesNeeded
   local.get $pagesBefore
   local.tee $4
   local.get $pagesNeeded
   local.tee $5
   local.get $4
   local.get $5
   i32.gt_s
   select
   local.set $pagesWanted
   local.get $pagesWanted
   memory.grow
   i32.const 0
   i32.lt_s
   if
    local.get $pagesNeeded
    memory.grow
    i32.const 0
    i32.lt_s
    if
     unreachable
    end
   end
  end
  local.get $newOffset
  global.set $~lib/rt/stub/offset
 )
 (func $~lib/rt/common/BLOCK#set:mmInfo (param $this i32) (param $mmInfo i32)
  local.get $this
  local.get $mmInfo
  i32.store
 )
 (func $~lib/rt/stub/__alloc (param $size i32) (result i32)
  (local $block i32)
  (local $ptr i32)
  (local $size|3 i32)
  (local $payloadSize i32)
  local.get $size
  i32.const 1073741820
  i32.gt_u
  if
   i32.const 32
   i32.const 96
   i32.const 33
   i32.const 29
   call $~lib/builtins/abort
   unreachable
  end
  global.get $~lib/rt/stub/offset
  local.set $block
  global.get $~lib/rt/stub/offset
  i32.const 4
  i32.add
  local.set $ptr
  block $~lib/rt/stub/computeSize|inlined.0 (result i32)
   local.get $size
   local.set $size|3
   local.get $size|3
   i32.const 4
   i32.add
   i32.const 15
   i32.add
   i32.const 15
   i32.const -1
   i32.xor
   i32.and
   i32.const 4
   i32.sub
   br $~lib/rt/stub/computeSize|inlined.0
  end
  local.set $payloadSize
  local.get $ptr
  local.get $payloadSize
  i32.add
  call $~lib/rt/stub/maybeGrowMemory
  local.get $block
  local.get $payloadSize
  call $~lib/rt/common/BLOCK#set:mmInfo
  local.get $ptr
  return
 )
 (func $~lib/rt/common/OBJECT#set:gcInfo (param $this i32) (param $gcInfo i32)
  local.get $this
  local.get $gcInfo
  i32.store offset=4
 )
 (func $~lib/rt/common/OBJECT#set:gcInfo2 (param $this i32) (param $gcInfo2 i32)
  local.get $this
  local.get $gcInfo2
  i32.store offset=8
 )
 (func $~lib/rt/common/OBJECT#set:rtId (param $this i32) (param $rtId i32)
  local.get $this
  local.get $rtId
  i32.store offset=12
 )
 (func $~lib/rt/common/OBJECT#set:rtSize (param $this i32) (param $rtSize i32)
  local.get $this
  local.get $rtSize
  i32.store offset=16
 )
 (func $~lib/rt/stub/__new (param $size i32) (param $id i32) (result i32)
  (local $ptr i32)
  (local $object i32)
  local.get $size
  i32.const 1073741804
  i32.gt_u
  if
   i32.const 32
   i32.const 96
   i32.const 86
   i32.const 30
   call $~lib/builtins/abort
   unreachable
  end
  i32.const 16
  local.get $size
  i32.add
  call $~lib/rt/stub/__alloc
  local.set $ptr
  local.get $ptr
  i32.const 4
  i32.sub
  local.set $object
  local.get $object
  i32.const 0
  call $~lib/rt/common/OBJECT#set:gcInfo
  local.get $object
  i32.const 0
  call $~lib/rt/common/OBJECT#set:gcInfo2
  local.get $object
  local.get $id
  call $~lib/rt/common/OBJECT#set:rtId
  local.get $object
  local.get $size
  call $~lib/rt/common/OBJECT#set:rtSize
  local.get $ptr
  i32.const 16
  i32.add
  return
 )
 (func $~lib/object/Object#constructor (param $this i32) (result i32)
  local.get $this
  i32.eqz
  if
   i32.const 0
   i32.const 0
   call $~lib/rt/stub/__new
   local.set $this
  end
  local.get $this
 )
 (func $assembly/logging/Console#constructor (param $this i32) (result i32)
  local.get $this
  i32.eqz
  if
   i32.const 0
   i32.const 4
   call $~lib/rt/stub/__new
   local.set $this
  end
  local.get $this
  call $~lib/object/Object#constructor
  local.set $this
  local.get $this
 )
 (func $start:assembly/logging
  global.get $~lib/memory/__heap_base
  i32.const 4
  i32.add
  i32.const 15
  i32.add
  i32.const 15
  i32.const -1
  i32.xor
  i32.and
  i32.const 4
  i32.sub
  global.set $~lib/rt/stub/startOffset
  global.get $~lib/rt/stub/startOffset
  global.set $~lib/rt/stub/offset
  i32.const 0
  call $assembly/logging/Console#constructor
  global.set $assembly/logging/console
 )
 (func $start:assembly/index
  call $start:assembly/logging
 )
 (func $~lib/arraybuffer/ArrayBuffer#constructor (param $this i32) (param $length i32) (result i32)
  (local $buffer i32)
  local.get $length
  i32.const 1073741820
  i32.gt_u
  if
   i32.const 1040
   i32.const 1088
   i32.const 52
   i32.const 43
   call $~lib/builtins/abort
   unreachable
  end
  local.get $length
  i32.const 1
  call $~lib/rt/stub/__new
  local.set $buffer
  i32.const 0
  global.get $~lib/shared/runtime/Runtime.Incremental
  i32.ne
  drop
  local.get $buffer
  i32.const 0
  local.get $length
  memory.fill
  local.get $buffer
  return
 )
 (func $~lib/rt/common/OBJECT#get:rtSize (param $this i32) (result i32)
  local.get $this
  i32.load offset=16
 )
 (func $~lib/string/String#get:length (param $this i32) (result i32)
  local.get $this
  i32.const 20
  i32.sub
  call $~lib/rt/common/OBJECT#get:rtSize
  i32.const 1
  i32.shr_u
  return
 )
 (func $~lib/string/String#substring (param $this i32) (param $start i32) (param $end i32) (result i32)
  (local $len i32)
  (local $4 i32)
  (local $5 i32)
  (local $6 i32)
  (local $7 i32)
  (local $finalStart i32)
  (local $9 i32)
  (local $10 i32)
  (local $11 i32)
  (local $12 i32)
  (local $finalEnd i32)
  (local $14 i32)
  (local $15 i32)
  (local $fromPos i32)
  (local $17 i32)
  (local $18 i32)
  (local $toPos i32)
  (local $size i32)
  (local $out i32)
  local.get $this
  call $~lib/string/String#get:length
  local.set $len
  local.get $start
  local.tee $4
  i32.const 0
  local.tee $5
  local.get $4
  local.get $5
  i32.gt_s
  select
  local.tee $6
  local.get $len
  local.tee $7
  local.get $6
  local.get $7
  i32.lt_s
  select
  local.set $finalStart
  local.get $end
  local.tee $9
  i32.const 0
  local.tee $10
  local.get $9
  local.get $10
  i32.gt_s
  select
  local.tee $11
  local.get $len
  local.tee $12
  local.get $11
  local.get $12
  i32.lt_s
  select
  local.set $finalEnd
  local.get $finalStart
  local.tee $14
  local.get $finalEnd
  local.tee $15
  local.get $14
  local.get $15
  i32.lt_s
  select
  i32.const 1
  i32.shl
  local.set $fromPos
  local.get $finalStart
  local.tee $17
  local.get $finalEnd
  local.tee $18
  local.get $17
  local.get $18
  i32.gt_s
  select
  i32.const 1
  i32.shl
  local.set $toPos
  local.get $toPos
  local.get $fromPos
  i32.sub
  local.set $size
  local.get $size
  i32.eqz
  if
   i32.const 1152
   return
  end
  local.get $fromPos
  i32.eqz
  if (result i32)
   local.get $toPos
   local.get $len
   i32.const 1
   i32.shl
   i32.eq
  else
   i32.const 0
  end
  if
   local.get $this
   return
  end
  local.get $size
  i32.const 2
  call $~lib/rt/stub/__new
  local.set $out
  local.get $out
  local.get $this
  local.get $fromPos
  i32.add
  local.get $size
  memory.copy
  local.get $out
  return
 )
 (func $~lib/util/string/isSpace (param $c i32) (result i32)
  (local $1 i32)
  local.get $c
  i32.const 5760
  i32.lt_u
  if
   local.get $c
   i32.const 128
   i32.or
   i32.const 160
   i32.eq
   if (result i32)
    i32.const 1
   else
    local.get $c
    i32.const 9
    i32.sub
    i32.const 13
    i32.const 9
    i32.sub
    i32.le_u
   end
   return
  end
  local.get $c
  i32.const 8192
  i32.sub
  i32.const 8202
  i32.const 8192
  i32.sub
  i32.le_u
  if
   i32.const 1
   return
  end
  block $break|0
   block $case6|0
    block $case5|0
     block $case4|0
      block $case3|0
       block $case2|0
        block $case1|0
         block $case0|0
          local.get $c
          local.set $1
          local.get $1
          i32.const 5760
          i32.eq
          br_if $case0|0
          local.get $1
          i32.const 8232
          i32.eq
          br_if $case1|0
          local.get $1
          i32.const 8233
          i32.eq
          br_if $case2|0
          local.get $1
          i32.const 8239
          i32.eq
          br_if $case3|0
          local.get $1
          i32.const 8287
          i32.eq
          br_if $case4|0
          local.get $1
          i32.const 12288
          i32.eq
          br_if $case5|0
          local.get $1
          i32.const 65279
          i32.eq
          br_if $case6|0
          br $break|0
         end
        end
       end
      end
     end
    end
   end
   i32.const 1
   return
  end
  i32.const 0
  return
 )
 (func $~lib/util/string/strtol<f64> (param $str i32) (param $radix i32) (result f64)
  (local $len i32)
  (local $ptr i32)
  (local $code i32)
  (local $sign f64)
  (local $6 i32)
  (local $num f64)
  (local $initial i32)
  (local $9 i32)
  local.get $str
  call $~lib/string/String#get:length
  local.set $len
  local.get $len
  i32.eqz
  if
   i32.const 1
   drop
   f64.const nan:0x8000000000000
   return
  end
  local.get $str
  local.set $ptr
  local.get $ptr
  i32.load16_u
  local.set $code
  loop $while-continue|0
   local.get $code
   call $~lib/util/string/isSpace
   if
    local.get $ptr
    i32.const 2
    i32.add
    local.tee $ptr
    i32.load16_u
    local.set $code
    local.get $len
    i32.const 1
    i32.sub
    local.set $len
    br $while-continue|0
   end
  end
  f64.const 1
  local.set $sign
  local.get $code
  i32.const 45
  i32.eq
  if (result i32)
   i32.const 1
  else
   local.get $code
   i32.const 43
   i32.eq
  end
  if
   local.get $len
   i32.const 1
   i32.sub
   local.tee $len
   i32.eqz
   if
    i32.const 1
    drop
    f64.const nan:0x8000000000000
    return
   end
   local.get $code
   i32.const 45
   i32.eq
   if
    f64.const -1
    local.set $sign
   end
   local.get $ptr
   i32.const 2
   i32.add
   local.tee $ptr
   i32.load16_u
   local.set $code
  end
  local.get $radix
  if
   local.get $radix
   i32.const 2
   i32.lt_s
   if (result i32)
    i32.const 1
   else
    local.get $radix
    i32.const 36
    i32.gt_s
   end
   if
    i32.const 1
    drop
    f64.const nan:0x8000000000000
    return
   end
   local.get $radix
   i32.const 16
   i32.eq
   if
    local.get $len
    i32.const 2
    i32.gt_s
    if (result i32)
     local.get $code
     i32.const 48
     i32.eq
    else
     i32.const 0
    end
    if (result i32)
     local.get $ptr
     i32.load16_u offset=2
     i32.const 32
     i32.or
     i32.const 120
     i32.eq
    else
     i32.const 0
    end
    if
     local.get $ptr
     i32.const 4
     i32.add
     local.set $ptr
     local.get $len
     i32.const 2
     i32.sub
     local.set $len
    end
   end
  else
   local.get $code
   i32.const 48
   i32.eq
   if (result i32)
    local.get $len
    i32.const 2
    i32.gt_s
   else
    i32.const 0
   end
   if
    block $break|1
     block $case2|1
      block $case1|1
       block $case0|1
        local.get $ptr
        i32.load16_u offset=2
        i32.const 32
        i32.or
        local.set $6
        local.get $6
        i32.const 98
        i32.eq
        br_if $case0|1
        local.get $6
        i32.const 111
        i32.eq
        br_if $case1|1
        local.get $6
        i32.const 120
        i32.eq
        br_if $case2|1
        br $break|1
       end
       local.get $ptr
       i32.const 4
       i32.add
       local.set $ptr
       local.get $len
       i32.const 2
       i32.sub
       local.set $len
       i32.const 2
       local.set $radix
       br $break|1
      end
      local.get $ptr
      i32.const 4
      i32.add
      local.set $ptr
      local.get $len
      i32.const 2
      i32.sub
      local.set $len
      i32.const 8
      local.set $radix
      br $break|1
     end
     local.get $ptr
     i32.const 4
     i32.add
     local.set $ptr
     local.get $len
     i32.const 2
     i32.sub
     local.set $len
     i32.const 16
     local.set $radix
     br $break|1
    end
   end
   local.get $radix
   i32.eqz
   if
    i32.const 10
    local.set $radix
   end
  end
  f64.const 0
  local.set $num
  local.get $len
  i32.const 1
  i32.sub
  local.set $initial
  block $while-break|2
   loop $while-continue|2
    local.get $len
    local.tee $9
    i32.const 1
    i32.sub
    local.set $len
    local.get $9
    if
     local.get $ptr
     i32.load16_u
     local.set $code
     local.get $code
     i32.const 48
     i32.sub
     i32.const 10
     i32.lt_u
     if
      local.get $code
      i32.const 48
      i32.sub
      local.set $code
     else
      local.get $code
      i32.const 65
      i32.sub
      i32.const 90
      i32.const 65
      i32.sub
      i32.le_u
      if
       local.get $code
       i32.const 65
       i32.const 10
       i32.sub
       i32.sub
       local.set $code
      else
       local.get $code
       i32.const 97
       i32.sub
       i32.const 122
       i32.const 97
       i32.sub
       i32.le_u
       if
        local.get $code
        i32.const 97
        i32.const 10
        i32.sub
        i32.sub
        local.set $code
       end
      end
     end
     local.get $code
     local.get $radix
     i32.ge_u
     if
      local.get $initial
      local.get $len
      i32.eq
      if
       i32.const 1
       drop
       f64.const nan:0x8000000000000
       return
      end
      br $while-break|2
     end
     local.get $num
     local.get $radix
     f64.convert_i32_s
     f64.mul
     local.get $code
     f64.convert_i32_u
     f64.add
     local.set $num
     local.get $ptr
     i32.const 2
     i32.add
     local.set $ptr
     br $while-continue|2
    end
   end
  end
  local.get $sign
  local.get $num
  f64.mul
  return
 )
 (func $~lib/string/parseInt (param $str i32) (param $radix i32) (result f64)
  local.get $str
  local.get $radix
  call $~lib/util/string/strtol<f64>
  return
 )
 (func $assembly/index/decodeHex (param $hex i32) (result i32)
  (local $result i32)
  (local $i i32)
  i32.const 0
  local.get $hex
  call $~lib/string/String#get:length
  i32.const 2
  i32.div_s
  call $~lib/arraybuffer/ArrayBuffer#constructor
  local.set $result
  i32.const 0
  local.set $i
  loop $for-loop|0
   local.get $i
   local.get $hex
   call $~lib/string/String#get:length
   i32.lt_s
   if
    local.get $result
    local.get $i
    i32.const 2
    i32.div_s
    i32.add
    local.get $hex
    local.get $i
    local.get $i
    i32.const 2
    i32.add
    call $~lib/string/String#substring
    i32.const 16
    call $~lib/string/parseInt
    i32.trunc_sat_f64_u
    i32.store8
    local.get $i
    i32.const 2
    i32.add
    local.set $i
    br $for-loop|0
   end
  end
  local.get $result
  return
 )
 (func $assembly/box/Box#set:start (param $this i32) (param $start i32)
  local.get $this
  local.get $start
  i32.store
 )
 (func $assembly/box/Box#set:len (param $this i32) (param $len i32)
  local.get $this
  local.get $len
  i32.store offset=4
 )
 (func $assembly/box/Box#constructor (param $this i32) (param $start i32) (param $len i32) (result i32)
  local.get $this
  i32.eqz
  if
   i32.const 8
   i32.const 7
   call $~lib/rt/stub/__new
   local.set $this
  end
  local.get $this
  i32.const 0
  call $assembly/box/Box#set:start
  local.get $this
  i32.const 0
  call $assembly/box/Box#set:len
  local.get $this
  local.get $start
  call $assembly/box/Box#set:start
  local.get $this
  local.get $len
  call $assembly/box/Box#set:len
  local.get $this
 )
 (func $~lib/arraybuffer/ArrayBuffer#get:byteLength (param $this i32) (result i32)
  local.get $this
  i32.const 20
  i32.sub
  call $~lib/rt/common/OBJECT#get:rtSize
  return
 )
 (func $assembly/box/Box.from (param $data i32) (result i32)
  i32.const 0
  local.get $data
  local.get $data
  call $~lib/arraybuffer/ArrayBuffer#get:byteLength
  call $assembly/box/Box#constructor
  return
 )
 (func $assembly/box/Box#get:len (param $this i32) (result i32)
  local.get $this
  i32.load offset=4
 )
 (func $assembly/box/Box#get:start (param $this i32) (result i32)
  local.get $this
  i32.load
 )
 (func $assembly/memcpy/memcpy (param $dest i32) (param $src i32) (param $len i32) (result i32)
  local.get $dest
  local.get $src
  local.get $len
  memory.copy
  local.get $dest
  return
 )
 (func $assembly/box/Box#toArrayBuffer (param $this i32) (result i32)
  (local $result i32)
  i32.const 0
  local.get $this
  call $assembly/box/Box#get:len
  call $~lib/arraybuffer/ArrayBuffer#constructor
  local.set $result
  local.get $result
  local.get $this
  call $assembly/box/Box#get:start
  local.get $this
  call $assembly/box/Box#get:len
  call $assembly/memcpy/memcpy
  drop
  local.get $result
  return
 )
 (func $assembly/sha256/Hash#set:digestLength (param $this i32) (param $digestLength i32)
  local.get $this
  local.get $digestLength
  i32.store
 )
 (func $assembly/sha256/Hash#set:blockSize (param $this i32) (param $blockSize i32)
  local.get $this
  local.get $blockSize
  i32.store offset=4
 )
 (func $~lib/rt/stub/__link (param $parentPtr i32) (param $childPtr i32) (param $expectMultiple i32)
 )
 (func $~lib/arraybuffer/ArrayBufferView#set:buffer (param $this i32) (param $buffer i32)
  local.get $this
  local.get $buffer
  i32.store
  local.get $this
  local.get $buffer
  i32.const 0
  call $~lib/rt/stub/__link
 )
 (func $~lib/arraybuffer/ArrayBufferView#set:dataStart (param $this i32) (param $dataStart i32)
  local.get $this
  local.get $dataStart
  i32.store offset=4
 )
 (func $~lib/arraybuffer/ArrayBufferView#set:byteLength (param $this i32) (param $byteLength i32)
  local.get $this
  local.get $byteLength
  i32.store offset=8
 )
 (func $~lib/arraybuffer/ArrayBufferView#constructor (param $this i32) (param $length i32) (param $alignLog2 i32) (result i32)
  (local $buffer i32)
  local.get $this
  i32.eqz
  if
   i32.const 12
   i32.const 3
   call $~lib/rt/stub/__new
   local.set $this
  end
  local.get $this
  i32.const 0
  call $~lib/arraybuffer/ArrayBufferView#set:buffer
  local.get $this
  i32.const 0
  call $~lib/arraybuffer/ArrayBufferView#set:dataStart
  local.get $this
  i32.const 0
  call $~lib/arraybuffer/ArrayBufferView#set:byteLength
  local.get $length
  i32.const 1073741820
  local.get $alignLog2
  i32.shr_u
  i32.gt_u
  if
   i32.const 1040
   i32.const 1088
   i32.const 19
   i32.const 57
   call $~lib/builtins/abort
   unreachable
  end
  local.get $length
  local.get $alignLog2
  i32.shl
  local.tee $length
  i32.const 1
  call $~lib/rt/stub/__new
  local.set $buffer
  i32.const 0
  global.get $~lib/shared/runtime/Runtime.Incremental
  i32.ne
  drop
  local.get $buffer
  i32.const 0
  local.get $length
  memory.fill
  local.get $this
  local.get $buffer
  call $~lib/arraybuffer/ArrayBufferView#set:buffer
  local.get $this
  local.get $buffer
  call $~lib/arraybuffer/ArrayBufferView#set:dataStart
  local.get $this
  local.get $length
  call $~lib/arraybuffer/ArrayBufferView#set:byteLength
  local.get $this
 )
 (func $~lib/typedarray/Int32Array#constructor (param $this i32) (param $length i32) (result i32)
  local.get $this
  i32.eqz
  if
   i32.const 12
   i32.const 9
   call $~lib/rt/stub/__new
   local.set $this
  end
  local.get $this
  local.get $length
  i32.const 2
  call $~lib/arraybuffer/ArrayBufferView#constructor
  local.set $this
  local.get $this
 )
 (func $assembly/sha256/Hash#set:state (param $this i32) (param $state i32)
  local.get $this
  local.get $state
  i32.store offset=8
  local.get $this
  local.get $state
  i32.const 0
  call $~lib/rt/stub/__link
 )
 (func $assembly/sha256/Hash#set:temp (param $this i32) (param $temp i32)
  local.get $this
  local.get $temp
  i32.store offset=12
  local.get $this
  local.get $temp
  i32.const 0
  call $~lib/rt/stub/__link
 )
 (func $~lib/typedarray/Uint8Array#constructor (param $this i32) (param $length i32) (result i32)
  local.get $this
  i32.eqz
  if
   i32.const 12
   i32.const 10
   call $~lib/rt/stub/__new
   local.set $this
  end
  local.get $this
  local.get $length
  i32.const 0
  call $~lib/arraybuffer/ArrayBufferView#constructor
  local.set $this
  local.get $this
 )
 (func $assembly/sha256/Hash#set:buffer (param $this i32) (param $buffer i32)
  local.get $this
  local.get $buffer
  i32.store offset=16
  local.get $this
  local.get $buffer
  i32.const 0
  call $~lib/rt/stub/__link
 )
 (func $assembly/sha256/Hash#set:bufferLength (param $this i32) (param $bufferLength i32)
  local.get $this
  local.get $bufferLength
  i32.store offset=20
 )
 (func $assembly/sha256/Hash#set:bytesHashed (param $this i32) (param $bytesHashed i32)
  local.get $this
  local.get $bytesHashed
  i32.store offset=24
 )
 (func $assembly/sha256/Hash#get:state (param $this i32) (result i32)
  local.get $this
  i32.load offset=8
 )
 (func $~lib/arraybuffer/ArrayBufferView#get:byteLength (param $this i32) (result i32)
  local.get $this
  i32.load offset=8
 )
 (func $~lib/arraybuffer/ArrayBufferView#get:dataStart (param $this i32) (result i32)
  local.get $this
  i32.load offset=4
 )
 (func $~lib/typedarray/Int32Array#__set (param $this i32) (param $index i32) (param $value i32)
  local.get $index
  local.get $this
  call $~lib/arraybuffer/ArrayBufferView#get:byteLength
  i32.const 2
  i32.shr_u
  i32.ge_u
  if
   i32.const 1968
   i32.const 2032
   i32.const 747
   i32.const 64
   call $~lib/builtins/abort
   unreachable
  end
  local.get $this
  call $~lib/arraybuffer/ArrayBufferView#get:dataStart
  local.get $index
  i32.const 2
  i32.shl
  i32.add
  local.get $value
  i32.store
 )
 (func $assembly/sha256/Hash#set:finished (param $this i32) (param $finished i32)
  local.get $this
  local.get $finished
  i32.store8 offset=28
 )
 (func $assembly/sha256/Hash#reset (param $this i32) (result i32)
  local.get $this
  call $assembly/sha256/Hash#get:state
  i32.const 0
  i32.const 1779033703
  call $~lib/typedarray/Int32Array#__set
  local.get $this
  call $assembly/sha256/Hash#get:state
  i32.const 1
  i32.const -1150833019
  call $~lib/typedarray/Int32Array#__set
  local.get $this
  call $assembly/sha256/Hash#get:state
  i32.const 2
  i32.const 1013904242
  call $~lib/typedarray/Int32Array#__set
  local.get $this
  call $assembly/sha256/Hash#get:state
  i32.const 3
  i32.const -1521486534
  call $~lib/typedarray/Int32Array#__set
  local.get $this
  call $assembly/sha256/Hash#get:state
  i32.const 4
  i32.const 1359893119
  call $~lib/typedarray/Int32Array#__set
  local.get $this
  call $assembly/sha256/Hash#get:state
  i32.const 5
  i32.const -1694144372
  call $~lib/typedarray/Int32Array#__set
  local.get $this
  call $assembly/sha256/Hash#get:state
  i32.const 6
  i32.const 528734635
  call $~lib/typedarray/Int32Array#__set
  local.get $this
  call $assembly/sha256/Hash#get:state
  i32.const 7
  i32.const 1541459225
  call $~lib/typedarray/Int32Array#__set
  local.get $this
  i32.const 0
  call $assembly/sha256/Hash#set:bufferLength
  local.get $this
  i32.const 0
  call $assembly/sha256/Hash#set:bytesHashed
  local.get $this
  i32.const 0
  call $assembly/sha256/Hash#set:finished
  local.get $this
  return
 )
 (func $assembly/sha256/Hash#constructor (param $this i32) (result i32)
  local.get $this
  i32.eqz
  if
   i32.const 29
   i32.const 8
   call $~lib/rt/stub/__new
   local.set $this
  end
  local.get $this
  i32.const 0
  call $assembly/sha256/Hash#set:digestLength
  local.get $this
  i32.const 0
  call $assembly/sha256/Hash#set:blockSize
  local.get $this
  i32.const 0
  call $assembly/sha256/Hash#set:state
  local.get $this
  i32.const 0
  call $assembly/sha256/Hash#set:temp
  local.get $this
  i32.const 0
  call $assembly/sha256/Hash#set:buffer
  local.get $this
  i32.const 0
  call $assembly/sha256/Hash#set:bufferLength
  local.get $this
  i32.const 0
  call $assembly/sha256/Hash#set:bytesHashed
  local.get $this
  i32.const 0
  call $assembly/sha256/Hash#set:finished
  local.get $this
  i32.const 32
  call $assembly/sha256/Hash#set:digestLength
  local.get $this
  i32.const 64
  call $assembly/sha256/Hash#set:blockSize
  local.get $this
  i32.const 0
  i32.const 8
  call $~lib/typedarray/Int32Array#constructor
  call $assembly/sha256/Hash#set:state
  local.get $this
  i32.const 0
  i32.const 64
  call $~lib/typedarray/Int32Array#constructor
  call $assembly/sha256/Hash#set:temp
  local.get $this
  i32.const 0
  i32.const 128
  call $~lib/typedarray/Uint8Array#constructor
  call $assembly/sha256/Hash#set:buffer
  local.get $this
  i32.const 0
  call $assembly/sha256/Hash#set:bufferLength
  local.get $this
  i32.const 0
  call $assembly/sha256/Hash#set:bytesHashed
  local.get $this
  call $assembly/sha256/Hash#reset
  drop
  local.get $this
 )
 (func $~lib/typedarray/Uint8Array.wrap (param $buffer i32) (param $byteOffset i32) (param $length i32) (result i32)
  (local $buffer|3 i32)
  (local $byteOffset|4 i32)
  (local $len i32)
  (local $byteLength i32)
  (local $bufferByteLength i32)
  (local $out i32)
  block $"~lib/typedarray/WRAP<~lib/typedarray/Uint8Array,u8>|inlined.0" (result i32)
   local.get $buffer
   local.set $buffer|3
   local.get $byteOffset
   local.set $byteOffset|4
   local.get $length
   local.set $len
   local.get $buffer|3
   call $~lib/arraybuffer/ArrayBuffer#get:byteLength
   local.set $bufferByteLength
   local.get $byteOffset|4
   local.get $bufferByteLength
   i32.gt_u
   local.get $byteOffset|4
   i32.const 0
   i32.and
   i32.or
   if
    i32.const 1968
    i32.const 2032
    i32.const 1860
    i32.const 5
    call $~lib/builtins/abort
    unreachable
   end
   local.get $len
   i32.const 0
   i32.lt_s
   if
    local.get $len
    i32.const -1
    i32.eq
    if
     local.get $bufferByteLength
     i32.const 0
     i32.and
     if
      i32.const 1040
      i32.const 2032
      i32.const 1865
      i32.const 9
      call $~lib/builtins/abort
      unreachable
     end
     local.get $bufferByteLength
     local.get $byteOffset|4
     i32.sub
     local.set $byteLength
    else
     i32.const 1040
     i32.const 2032
     i32.const 1869
     i32.const 7
     call $~lib/builtins/abort
     unreachable
    end
   else
    local.get $len
    i32.const 0
    i32.shl
    local.set $byteLength
    local.get $byteOffset|4
    local.get $byteLength
    i32.add
    local.get $bufferByteLength
    i32.gt_s
    if
     i32.const 1040
     i32.const 2032
     i32.const 1874
     i32.const 7
     call $~lib/builtins/abort
     unreachable
    end
   end
   i32.const 12
   i32.const 10
   call $~lib/rt/stub/__new
   local.set $out
   local.get $out
   local.get $buffer|3
   i32.store
   local.get $out
   local.get $buffer|3
   i32.const 0
   call $~lib/rt/stub/__link
   local.get $out
   local.get $byteLength
   i32.store offset=8
   local.get $out
   local.get $buffer|3
   local.get $byteOffset|4
   i32.add
   i32.store offset=4
   local.get $out
   br $"~lib/typedarray/WRAP<~lib/typedarray/Uint8Array,u8>|inlined.0"
  end
  return
 )
 (func $~lib/typedarray/Uint8Array.wrap@varargs (param $buffer i32) (param $byteOffset i32) (param $length i32) (result i32)
  block $2of2
   block $1of2
    block $0of2
     block $outOfRange
      global.get $~argumentsLength
      i32.const 1
      i32.sub
      br_table $0of2 $1of2 $2of2 $outOfRange
     end
     unreachable
    end
    i32.const 0
    local.set $byteOffset
   end
   i32.const -1
   local.set $length
  end
  local.get $buffer
  local.get $byteOffset
  local.get $length
  call $~lib/typedarray/Uint8Array.wrap
 )
 (func $assembly/sha256/Hash#get:finished (param $this i32) (result i32)
  local.get $this
  i32.load8_u offset=28
 )
 (func $assembly/sha256/Hash#get:bytesHashed (param $this i32) (result i32)
  local.get $this
  i32.load offset=24
 )
 (func $assembly/sha256/Hash#get:bufferLength (param $this i32) (result i32)
  local.get $this
  i32.load offset=20
 )
 (func $~lib/typedarray/Uint8Array#__get (param $this i32) (param $index i32) (result i32)
  local.get $index
  local.get $this
  call $~lib/arraybuffer/ArrayBufferView#get:byteLength
  i32.ge_u
  if
   i32.const 1968
   i32.const 2032
   i32.const 167
   i32.const 45
   call $~lib/builtins/abort
   unreachable
  end
  local.get $this
  call $~lib/arraybuffer/ArrayBufferView#get:dataStart
  local.get $index
  i32.add
  i32.load8_u
  return
 )
 (func $assembly/sha256/Hash#get:buffer (param $this i32) (result i32)
  local.get $this
  i32.load offset=16
 )
 (func $~lib/typedarray/Uint8Array#__set (param $this i32) (param $index i32) (param $value i32)
  local.get $index
  local.get $this
  call $~lib/arraybuffer/ArrayBufferView#get:byteLength
  i32.ge_u
  if
   i32.const 1968
   i32.const 2032
   i32.const 178
   i32.const 45
   call $~lib/builtins/abort
   unreachable
  end
  local.get $this
  call $~lib/arraybuffer/ArrayBufferView#get:dataStart
  local.get $index
  i32.add
  local.get $value
  i32.store8
 )
 (func $assembly/sha256/Hash#get:temp (param $this i32) (result i32)
  local.get $this
  i32.load offset=12
 )
 (func $~lib/typedarray/Int32Array#__get (param $this i32) (param $index i32) (result i32)
  local.get $index
  local.get $this
  call $~lib/arraybuffer/ArrayBufferView#get:byteLength
  i32.const 2
  i32.shr_u
  i32.ge_u
  if
   i32.const 1968
   i32.const 2032
   i32.const 736
   i32.const 64
   call $~lib/builtins/abort
   unreachable
  end
  local.get $this
  call $~lib/arraybuffer/ArrayBufferView#get:dataStart
  local.get $index
  i32.const 2
  i32.shl
  i32.add
  i32.load
  return
 )
 (func $~lib/array/Array<u32>#get:length_ (param $this i32) (result i32)
  local.get $this
  i32.load offset=12
 )
 (func $~lib/array/Array<u32>#get:dataStart (param $this i32) (result i32)
  local.get $this
  i32.load offset=4
 )
 (func $~lib/array/Array<u32>#__get (param $this i32) (param $index i32) (result i32)
  (local $value i32)
  local.get $index
  local.get $this
  call $~lib/array/Array<u32>#get:length_
  i32.ge_u
  if
   i32.const 1968
   i32.const 2288
   i32.const 114
   i32.const 42
   call $~lib/builtins/abort
   unreachable
  end
  local.get $this
  call $~lib/array/Array<u32>#get:dataStart
  local.get $index
  i32.const 2
  i32.shl
  i32.add
  i32.load
  local.set $value
  i32.const 0
  drop
  local.get $value
  return
 )
 (func $assembly/sha256/hashBlocks (param $w i32) (param $v i32) (param $p i32) (param $pos i32) (param $len i32) (result i32)
  (local $a i32)
  (local $b i32)
  (local $c i32)
  (local $d i32)
  (local $e i32)
  (local $f i32)
  (local $g i32)
  (local $h i32)
  (local $u i32)
  (local $i i32)
  (local $j i32)
  (local $t1 i32)
  (local $t2 i32)
  loop $while-continue|0
   local.get $len
   i32.const 64
   i32.ge_s
   if
    local.get $v
    i32.const 0
    call $~lib/typedarray/Int32Array#__get
    local.set $a
    local.get $v
    i32.const 1
    call $~lib/typedarray/Int32Array#__get
    local.set $b
    local.get $v
    i32.const 2
    call $~lib/typedarray/Int32Array#__get
    local.set $c
    local.get $v
    i32.const 3
    call $~lib/typedarray/Int32Array#__get
    local.set $d
    local.get $v
    i32.const 4
    call $~lib/typedarray/Int32Array#__get
    local.set $e
    local.get $v
    i32.const 5
    call $~lib/typedarray/Int32Array#__get
    local.set $f
    local.get $v
    i32.const 6
    call $~lib/typedarray/Int32Array#__get
    local.set $g
    local.get $v
    i32.const 7
    call $~lib/typedarray/Int32Array#__get
    local.set $h
    i32.const 0
    local.set $i
    loop $for-loop|1
     local.get $i
     i32.const 16
     i32.lt_s
     if
      local.get $pos
      local.get $i
      i32.const 4
      i32.mul
      i32.add
      local.set $j
      local.get $w
      local.get $i
      local.get $p
      local.get $j
      call $~lib/typedarray/Uint8Array#__get
      i32.const 255
      i32.and
      i32.const 24
      i32.shl
      local.get $p
      local.get $j
      i32.const 1
      i32.add
      call $~lib/typedarray/Uint8Array#__get
      i32.const 255
      i32.and
      i32.const 16
      i32.shl
      i32.or
      local.get $p
      local.get $j
      i32.const 2
      i32.add
      call $~lib/typedarray/Uint8Array#__get
      i32.const 255
      i32.and
      i32.const 8
      i32.shl
      i32.or
      local.get $p
      local.get $j
      i32.const 3
      i32.add
      call $~lib/typedarray/Uint8Array#__get
      i32.const 255
      i32.and
      i32.or
      call $~lib/typedarray/Int32Array#__set
      local.get $i
      i32.const 1
      i32.add
      local.set $i
      br $for-loop|1
     end
    end
    i32.const 16
    local.set $i
    loop $for-loop|2
     local.get $i
     i32.const 64
     i32.lt_s
     if
      local.get $w
      local.get $i
      i32.const 2
      i32.sub
      call $~lib/typedarray/Int32Array#__get
      local.set $u
      local.get $u
      i32.const 17
      i32.shr_u
      local.get $u
      i32.const 32
      i32.const 17
      i32.sub
      i32.shl
      i32.or
      local.get $u
      i32.const 19
      i32.shr_u
      local.get $u
      i32.const 32
      i32.const 19
      i32.sub
      i32.shl
      i32.or
      i32.xor
      local.get $u
      i32.const 10
      i32.shr_u
      i32.xor
      local.set $t1
      local.get $w
      local.get $i
      i32.const 15
      i32.sub
      call $~lib/typedarray/Int32Array#__get
      local.set $u
      local.get $u
      i32.const 7
      i32.shr_u
      local.get $u
      i32.const 32
      i32.const 7
      i32.sub
      i32.shl
      i32.or
      local.get $u
      i32.const 18
      i32.shr_u
      local.get $u
      i32.const 32
      i32.const 18
      i32.sub
      i32.shl
      i32.or
      i32.xor
      local.get $u
      i32.const 3
      i32.shr_u
      i32.xor
      local.set $t2
      local.get $w
      local.get $i
      local.get $t1
      local.get $w
      local.get $i
      i32.const 7
      i32.sub
      call $~lib/typedarray/Int32Array#__get
      i32.add
      i32.const 0
      i32.or
      local.get $t2
      local.get $w
      local.get $i
      i32.const 16
      i32.sub
      call $~lib/typedarray/Int32Array#__get
      i32.add
      i32.const 0
      i32.or
      i32.add
      call $~lib/typedarray/Int32Array#__set
      local.get $i
      i32.const 1
      i32.add
      local.set $i
      br $for-loop|2
     end
    end
    i32.const 0
    local.set $i
    loop $for-loop|3
     local.get $i
     i32.const 64
     i32.lt_s
     if
      local.get $e
      i32.const 6
      i32.shr_u
      local.get $e
      i32.const 32
      i32.const 6
      i32.sub
      i32.shl
      i32.or
      local.get $e
      i32.const 11
      i32.shr_u
      local.get $e
      i32.const 32
      i32.const 11
      i32.sub
      i32.shl
      i32.or
      i32.xor
      local.get $e
      i32.const 25
      i32.shr_u
      local.get $e
      i32.const 32
      i32.const 25
      i32.sub
      i32.shl
      i32.or
      i32.xor
      local.get $e
      local.get $f
      i32.and
      local.get $e
      i32.const -1
      i32.xor
      local.get $g
      i32.and
      i32.xor
      i32.add
      i32.const 0
      i32.or
      local.get $h
      global.get $assembly/sha256/K
      local.get $i
      call $~lib/array/Array<u32>#__get
      local.get $w
      local.get $i
      call $~lib/typedarray/Int32Array#__get
      i32.add
      i32.const 0
      i32.or
      i32.add
      i32.const 0
      i32.or
      i32.add
      i32.const 0
      i32.or
      local.set $t1
      local.get $a
      i32.const 2
      i32.shr_u
      local.get $a
      i32.const 32
      i32.const 2
      i32.sub
      i32.shl
      i32.or
      local.get $a
      i32.const 13
      i32.shr_u
      local.get $a
      i32.const 32
      i32.const 13
      i32.sub
      i32.shl
      i32.or
      i32.xor
      local.get $a
      i32.const 22
      i32.shr_u
      local.get $a
      i32.const 32
      i32.const 22
      i32.sub
      i32.shl
      i32.or
      i32.xor
      local.get $a
      local.get $b
      i32.and
      local.get $a
      local.get $c
      i32.and
      i32.xor
      local.get $b
      local.get $c
      i32.and
      i32.xor
      i32.add
      i32.const 0
      i32.or
      local.set $t2
      local.get $g
      local.set $h
      local.get $f
      local.set $g
      local.get $e
      local.set $f
      local.get $d
      local.get $t1
      i32.add
      i32.const 0
      i32.or
      local.set $e
      local.get $c
      local.set $d
      local.get $b
      local.set $c
      local.get $a
      local.set $b
      local.get $t1
      local.get $t2
      i32.add
      i32.const 0
      i32.or
      local.set $a
      local.get $i
      i32.const 1
      i32.add
      local.set $i
      br $for-loop|3
     end
    end
    local.get $v
    i32.const 0
    local.get $v
    i32.const 0
    call $~lib/typedarray/Int32Array#__get
    local.get $a
    i32.add
    call $~lib/typedarray/Int32Array#__set
    local.get $v
    i32.const 1
    local.get $v
    i32.const 1
    call $~lib/typedarray/Int32Array#__get
    local.get $b
    i32.add
    call $~lib/typedarray/Int32Array#__set
    local.get $v
    i32.const 2
    local.get $v
    i32.const 2
    call $~lib/typedarray/Int32Array#__get
    local.get $c
    i32.add
    call $~lib/typedarray/Int32Array#__set
    local.get $v
    i32.const 3
    local.get $v
    i32.const 3
    call $~lib/typedarray/Int32Array#__get
    local.get $d
    i32.add
    call $~lib/typedarray/Int32Array#__set
    local.get $v
    i32.const 4
    local.get $v
    i32.const 4
    call $~lib/typedarray/Int32Array#__get
    local.get $e
    i32.add
    call $~lib/typedarray/Int32Array#__set
    local.get $v
    i32.const 5
    local.get $v
    i32.const 5
    call $~lib/typedarray/Int32Array#__get
    local.get $f
    i32.add
    call $~lib/typedarray/Int32Array#__set
    local.get $v
    i32.const 6
    local.get $v
    i32.const 6
    call $~lib/typedarray/Int32Array#__get
    local.get $g
    i32.add
    call $~lib/typedarray/Int32Array#__set
    local.get $v
    i32.const 7
    local.get $v
    i32.const 7
    call $~lib/typedarray/Int32Array#__get
    local.get $h
    i32.add
    call $~lib/typedarray/Int32Array#__set
    local.get $pos
    i32.const 64
    i32.add
    local.set $pos
    local.get $len
    i32.const 64
    i32.sub
    local.set $len
    br $while-continue|0
   end
  end
  local.get $pos
  return
 )
 (func $assembly/sha256/Hash#update (param $this i32) (param $data i32) (param $dataLength i32) (result i32)
  (local $dataPos i32)
  (local $setMem i32)
  (local $5 i32)
  (local $6 i32)
  (local $7 i32)
  (local $8 i32)
  local.get $this
  call $assembly/sha256/Hash#get:finished
  if
   i32.const 2096
   i32.const 2224
   i32.const 196
   i32.const 7
   call $~lib/builtins/abort
   unreachable
  end
  i32.const 0
  local.set $dataPos
  i32.const 1
  local.set $setMem
  local.get $this
  local.get $this
  call $assembly/sha256/Hash#get:bytesHashed
  local.get $dataLength
  i32.add
  call $assembly/sha256/Hash#set:bytesHashed
  local.get $this
  call $assembly/sha256/Hash#get:bufferLength
  i32.const 0
  i32.gt_s
  if
   loop $while-continue|0
    local.get $this
    call $assembly/sha256/Hash#get:bufferLength
    i32.const 64
    i32.lt_s
    if (result i32)
     local.get $dataLength
     i32.const 0
     i32.gt_s
    else
     i32.const 0
    end
    if
     local.get $this
     call $assembly/sha256/Hash#get:buffer
     local.get $this
     local.get $this
     call $assembly/sha256/Hash#get:bufferLength
     local.tee $6
     i32.const 1
     i32.add
     call $assembly/sha256/Hash#set:bufferLength
     local.get $6
     local.get $data
     local.get $dataPos
     local.tee $5
     i32.const 1
     i32.add
     local.set $dataPos
     local.get $5
     call $~lib/typedarray/Uint8Array#__get
     call $~lib/typedarray/Uint8Array#__set
     local.get $dataLength
     i32.const 1
     i32.sub
     local.set $dataLength
     br $while-continue|0
    end
   end
   local.get $this
   call $assembly/sha256/Hash#get:bufferLength
   i32.const 64
   i32.eq
   if
    local.get $this
    call $assembly/sha256/Hash#get:temp
    local.get $this
    call $assembly/sha256/Hash#get:state
    local.get $this
    call $assembly/sha256/Hash#get:buffer
    i32.const 0
    i32.const 64
    call $assembly/sha256/hashBlocks
    drop
    local.get $this
    i32.const 0
    call $assembly/sha256/Hash#set:bufferLength
   end
  end
  local.get $dataLength
  i32.const 64
  i32.ge_s
  if
   local.get $this
   call $assembly/sha256/Hash#get:temp
   local.get $this
   call $assembly/sha256/Hash#get:state
   local.get $data
   local.get $dataPos
   local.get $dataLength
   call $assembly/sha256/hashBlocks
   local.set $dataPos
   local.get $dataLength
   i32.const 64
   i32.rem_s
   local.set $dataLength
  end
  loop $while-continue|1
   local.get $dataLength
   i32.const 0
   i32.gt_s
   if
    local.get $this
    call $assembly/sha256/Hash#get:buffer
    local.get $this
    local.get $this
    call $assembly/sha256/Hash#get:bufferLength
    local.tee $8
    i32.const 1
    i32.add
    call $assembly/sha256/Hash#set:bufferLength
    local.get $8
    local.get $data
    local.get $dataPos
    local.tee $7
    i32.const 1
    i32.add
    local.set $dataPos
    local.get $7
    call $~lib/typedarray/Uint8Array#__get
    call $~lib/typedarray/Uint8Array#__set
    local.get $dataLength
    i32.const 1
    i32.sub
    local.set $dataLength
    br $while-continue|1
   end
  end
  local.get $this
  return
 )
 (func $~lib/typedarray/Uint8Array#get:length (param $this i32) (result i32)
  local.get $this
  call $~lib/arraybuffer/ArrayBufferView#get:byteLength
  return
 )
 (func $assembly/sha256/Hash#update@varargs (param $this i32) (param $data i32) (param $dataLength i32) (result i32)
  block $1of1
   block $0of1
    block $outOfRange
     global.get $~argumentsLength
     i32.const 1
     i32.sub
     br_table $0of1 $1of1 $outOfRange
    end
    unreachable
   end
   local.get $data
   call $~lib/typedarray/Uint8Array#get:length
   local.set $dataLength
  end
  local.get $this
  local.get $data
  local.get $dataLength
  call $assembly/sha256/Hash#update
 )
 (func $assembly/sha256/Hash#get:digestLength (param $this i32) (result i32)
  local.get $this
  i32.load
 )
 (func $assembly/sha256/Hash#finish (param $this i32) (param $out i32) (result i32)
  (local $bytesHashed i32)
  (local $left i32)
  (local $bitLenHi i32)
  (local $bitLenLo i32)
  (local $padLength i32)
  (local $i i32)
  (local $i|8 i32)
  local.get $this
  call $assembly/sha256/Hash#get:finished
  i32.eqz
  if
   local.get $this
   call $assembly/sha256/Hash#get:bytesHashed
   local.set $bytesHashed
   local.get $this
   call $assembly/sha256/Hash#get:bufferLength
   local.set $left
   local.get $bytesHashed
   i32.const 536870912
   i32.div_s
   i32.const 0
   i32.or
   local.set $bitLenHi
   local.get $bytesHashed
   i32.const 3
   i32.shl
   local.set $bitLenLo
   local.get $bytesHashed
   i32.const 64
   i32.rem_s
   i32.const 56
   i32.lt_s
   if (result i32)
    i32.const 64
   else
    i32.const 128
   end
   local.set $padLength
   local.get $this
   call $assembly/sha256/Hash#get:buffer
   local.get $left
   i32.const 128
   call $~lib/typedarray/Uint8Array#__set
   local.get $left
   i32.const 1
   i32.add
   local.set $i
   loop $for-loop|0
    local.get $i
    local.get $padLength
    i32.const 8
    i32.sub
    i32.lt_s
    if
     local.get $this
     call $assembly/sha256/Hash#get:buffer
     local.get $i
     i32.const 0
     call $~lib/typedarray/Uint8Array#__set
     local.get $i
     i32.const 1
     i32.add
     local.set $i
     br $for-loop|0
    end
   end
   local.get $this
   call $assembly/sha256/Hash#get:buffer
   local.get $padLength
   i32.const 8
   i32.sub
   local.get $bitLenHi
   i32.const 24
   i32.shr_u
   i32.const 255
   i32.and
   call $~lib/typedarray/Uint8Array#__set
   local.get $this
   call $assembly/sha256/Hash#get:buffer
   local.get $padLength
   i32.const 7
   i32.sub
   local.get $bitLenHi
   i32.const 16
   i32.shr_u
   i32.const 255
   i32.and
   call $~lib/typedarray/Uint8Array#__set
   local.get $this
   call $assembly/sha256/Hash#get:buffer
   local.get $padLength
   i32.const 6
   i32.sub
   local.get $bitLenHi
   i32.const 8
   i32.shr_u
   i32.const 255
   i32.and
   call $~lib/typedarray/Uint8Array#__set
   local.get $this
   call $assembly/sha256/Hash#get:buffer
   local.get $padLength
   i32.const 5
   i32.sub
   local.get $bitLenHi
   i32.const 0
   i32.shr_u
   i32.const 255
   i32.and
   call $~lib/typedarray/Uint8Array#__set
   local.get $this
   call $assembly/sha256/Hash#get:buffer
   local.get $padLength
   i32.const 4
   i32.sub
   local.get $bitLenLo
   i32.const 24
   i32.shr_u
   i32.const 255
   i32.and
   call $~lib/typedarray/Uint8Array#__set
   local.get $this
   call $assembly/sha256/Hash#get:buffer
   local.get $padLength
   i32.const 3
   i32.sub
   local.get $bitLenLo
   i32.const 16
   i32.shr_u
   i32.const 255
   i32.and
   call $~lib/typedarray/Uint8Array#__set
   local.get $this
   call $assembly/sha256/Hash#get:buffer
   local.get $padLength
   i32.const 2
   i32.sub
   local.get $bitLenLo
   i32.const 8
   i32.shr_u
   i32.const 255
   i32.and
   call $~lib/typedarray/Uint8Array#__set
   local.get $this
   call $assembly/sha256/Hash#get:buffer
   local.get $padLength
   i32.const 1
   i32.sub
   local.get $bitLenLo
   i32.const 0
   i32.shr_u
   i32.const 255
   i32.and
   call $~lib/typedarray/Uint8Array#__set
   local.get $this
   call $assembly/sha256/Hash#get:temp
   local.get $this
   call $assembly/sha256/Hash#get:state
   local.get $this
   call $assembly/sha256/Hash#get:buffer
   i32.const 0
   local.get $padLength
   call $assembly/sha256/hashBlocks
   drop
   local.get $this
   i32.const 1
   call $assembly/sha256/Hash#set:finished
  end
  i32.const 0
  local.set $i|8
  loop $for-loop|1
   local.get $i|8
   i32.const 8
   i32.lt_s
   if
    local.get $out
    local.get $i|8
    i32.const 4
    i32.mul
    i32.const 0
    i32.add
    local.get $this
    call $assembly/sha256/Hash#get:state
    local.get $i|8
    call $~lib/typedarray/Int32Array#__get
    i32.const 24
    i32.shr_u
    i32.const 255
    i32.and
    call $~lib/typedarray/Uint8Array#__set
    local.get $out
    local.get $i|8
    i32.const 4
    i32.mul
    i32.const 1
    i32.add
    local.get $this
    call $assembly/sha256/Hash#get:state
    local.get $i|8
    call $~lib/typedarray/Int32Array#__get
    i32.const 16
    i32.shr_u
    i32.const 255
    i32.and
    call $~lib/typedarray/Uint8Array#__set
    local.get $out
    local.get $i|8
    i32.const 4
    i32.mul
    i32.const 2
    i32.add
    local.get $this
    call $assembly/sha256/Hash#get:state
    local.get $i|8
    call $~lib/typedarray/Int32Array#__get
    i32.const 8
    i32.shr_u
    i32.const 255
    i32.and
    call $~lib/typedarray/Uint8Array#__set
    local.get $out
    local.get $i|8
    i32.const 4
    i32.mul
    i32.const 3
    i32.add
    local.get $this
    call $assembly/sha256/Hash#get:state
    local.get $i|8
    call $~lib/typedarray/Int32Array#__get
    i32.const 0
    i32.shr_u
    i32.const 255
    i32.and
    call $~lib/typedarray/Uint8Array#__set
    local.get $i|8
    i32.const 1
    i32.add
    local.set $i|8
    br $for-loop|1
   end
  end
  local.get $this
  return
 )
 (func $assembly/sha256/Hash#digest (param $this i32) (result i32)
  (local $out i32)
  i32.const 0
  local.get $this
  call $assembly/sha256/Hash#get:digestLength
  call $~lib/typedarray/Uint8Array#constructor
  local.set $out
  local.get $this
  local.get $out
  call $assembly/sha256/Hash#finish
  drop
  local.get $out
  return
 )
 (func $~lib/typedarray/Int32Array#get:length (param $this i32) (result i32)
  local.get $this
  call $~lib/arraybuffer/ArrayBufferView#get:byteLength
  i32.const 2
  i32.shr_u
  return
 )
 (func $assembly/sha256/Hash#clean (param $this i32)
  (local $i i32)
  (local $i|2 i32)
  i32.const 0
  local.set $i
  loop $for-loop|0
   local.get $i
   local.get $this
   call $assembly/sha256/Hash#get:buffer
   call $~lib/typedarray/Uint8Array#get:length
   i32.lt_s
   if
    local.get $this
    call $assembly/sha256/Hash#get:buffer
    local.get $i
    i32.const 0
    call $~lib/typedarray/Uint8Array#__set
    local.get $i
    i32.const 1
    i32.add
    local.set $i
    br $for-loop|0
   end
  end
  i32.const 0
  local.set $i|2
  loop $for-loop|1
   local.get $i|2
   local.get $this
   call $assembly/sha256/Hash#get:temp
   call $~lib/typedarray/Int32Array#get:length
   i32.lt_s
   if
    local.get $this
    call $assembly/sha256/Hash#get:temp
    local.get $i|2
    i32.const 0
    call $~lib/typedarray/Int32Array#__set
    local.get $i|2
    i32.const 1
    i32.add
    local.set $i|2
    br $for-loop|1
   end
  end
  local.get $this
  call $assembly/sha256/Hash#reset
  drop
 )
 (func $~lib/arraybuffer/ArrayBufferView#get:buffer (param $this i32) (result i32)
  local.get $this
  i32.load
 )
 (func $assembly/sha256/sha256 (param $data i32) (result i32)
  (local $h i32)
  (local $digest i32)
  i32.const 0
  call $assembly/sha256/Hash#constructor
  local.get $data
  i32.const 0
  i32.const 1
  global.set $~argumentsLength
  i32.const 0
  call $~lib/typedarray/Uint8Array.wrap@varargs
  i32.const 1
  global.set $~argumentsLength
  i32.const 0
  call $assembly/sha256/Hash#update@varargs
  local.set $h
  local.get $h
  call $assembly/sha256/Hash#digest
  local.set $digest
  local.get $h
  call $assembly/sha256/Hash#clean
  local.get $digest
  call $~lib/arraybuffer/ArrayBufferView#get:buffer
  return
 )
 (func $assembly/hex/encodeHexUTF8 (param $start i32) (param $len i32) (result i32)
  (local $result i32)
  (local $i i32)
  i32.const 0
  i32.const 2
  local.get $len
  i32.const 2
  i32.mul
  i32.add
  call $~lib/arraybuffer/ArrayBuffer#constructor
  local.set $result
  local.get $result
  i32.const 30768
  i32.store16
  i32.const 0
  local.set $i
  loop $for-loop|0
   local.get $i
   local.get $len
   i32.lt_u
   if
    i32.const 2
    local.get $result
    i32.add
    local.get $i
    i32.const 2
    i32.mul
    i32.add
    global.get $assembly/hex/hexLookupTable
    i32.const 2
    local.get $start
    local.get $i
    i32.add
    i32.load8_u
    i32.mul
    i32.add
    i32.load16_u
    i32.store16
    local.get $i
    i32.const 1
    i32.add
    local.set $i
    br $for-loop|0
   end
  end
  local.get $result
  return
 )
 (func $~lib/rt/common/BLOCK#get:mmInfo (param $this i32) (result i32)
  local.get $this
  i32.load
 )
 (func $~lib/rt/stub/__realloc (param $ptr i32) (param $size i32) (result i32)
  (local $block i32)
  (local $actualSize i32)
  (local $isLast i32)
  (local $size|5 i32)
  (local $payloadSize i32)
  (local $7 i32)
  (local $8 i32)
  (local $newPtr i32)
  local.get $ptr
  i32.const 0
  i32.ne
  if (result i32)
   local.get $ptr
   i32.const 15
   i32.and
   i32.eqz
  else
   i32.const 0
  end
  i32.eqz
  if
   i32.const 0
   i32.const 96
   i32.const 45
   i32.const 3
   call $~lib/builtins/abort
   unreachable
  end
  local.get $ptr
  i32.const 4
  i32.sub
  local.set $block
  local.get $block
  call $~lib/rt/common/BLOCK#get:mmInfo
  local.set $actualSize
  local.get $ptr
  local.get $actualSize
  i32.add
  global.get $~lib/rt/stub/offset
  i32.eq
  local.set $isLast
  block $~lib/rt/stub/computeSize|inlined.1 (result i32)
   local.get $size
   local.set $size|5
   local.get $size|5
   i32.const 4
   i32.add
   i32.const 15
   i32.add
   i32.const 15
   i32.const -1
   i32.xor
   i32.and
   i32.const 4
   i32.sub
   br $~lib/rt/stub/computeSize|inlined.1
  end
  local.set $payloadSize
  local.get $size
  local.get $actualSize
  i32.gt_u
  if
   local.get $isLast
   if
    local.get $size
    i32.const 1073741820
    i32.gt_u
    if
     i32.const 32
     i32.const 96
     i32.const 52
     i32.const 33
     call $~lib/builtins/abort
     unreachable
    end
    local.get $ptr
    local.get $payloadSize
    i32.add
    call $~lib/rt/stub/maybeGrowMemory
    local.get $block
    local.get $payloadSize
    call $~lib/rt/common/BLOCK#set:mmInfo
   else
    local.get $payloadSize
    local.tee $7
    local.get $actualSize
    i32.const 1
    i32.shl
    local.tee $8
    local.get $7
    local.get $8
    i32.gt_u
    select
    call $~lib/rt/stub/__alloc
    local.set $newPtr
    local.get $newPtr
    local.get $ptr
    local.get $actualSize
    memory.copy
    local.get $newPtr
    local.tee $ptr
    i32.const 4
    i32.sub
    local.set $block
   end
  else
   local.get $isLast
   if
    local.get $ptr
    local.get $payloadSize
    i32.add
    global.set $~lib/rt/stub/offset
    local.get $block
    local.get $payloadSize
    call $~lib/rt/common/BLOCK#set:mmInfo
   end
  end
  local.get $ptr
  return
 )
 (func $~lib/rt/stub/__renew (param $oldPtr i32) (param $size i32) (result i32)
  (local $newPtr i32)
  local.get $size
  i32.const 1073741804
  i32.gt_u
  if
   i32.const 32
   i32.const 96
   i32.const 99
   i32.const 30
   call $~lib/builtins/abort
   unreachable
  end
  local.get $oldPtr
  i32.const 16
  i32.sub
  i32.const 16
  local.get $size
  i32.add
  call $~lib/rt/stub/__realloc
  local.set $newPtr
  local.get $newPtr
  i32.const 4
  i32.sub
  local.get $size
  call $~lib/rt/common/OBJECT#set:rtSize
  local.get $newPtr
  i32.const 16
  i32.add
  return
 )
 (func $~lib/string/String.UTF8.decodeUnsafe (param $buf i32) (param $len i32) (param $nullTerminated i32) (result i32)
  (local $bufOff i32)
  (local $bufEnd i32)
  (local $str i32)
  (local $strOff i32)
  (local $u0 i32)
  (local $u1 i32)
  (local $u2 i32)
  (local $lo i32)
  (local $hi i32)
  local.get $buf
  local.set $bufOff
  local.get $buf
  local.get $len
  i32.add
  local.set $bufEnd
  local.get $bufEnd
  local.get $bufOff
  i32.ge_u
  i32.eqz
  if
   i32.const 0
   i32.const 2336
   i32.const 770
   i32.const 7
   call $~lib/builtins/abort
   unreachable
  end
  local.get $len
  i32.const 1
  i32.shl
  i32.const 2
  call $~lib/rt/stub/__new
  local.set $str
  local.get $str
  local.set $strOff
  block $while-break|0
   loop $while-continue|0
    local.get $bufOff
    local.get $bufEnd
    i32.lt_u
    if
     local.get $bufOff
     i32.load8_u
     local.set $u0
     local.get $bufOff
     i32.const 1
     i32.add
     local.set $bufOff
     local.get $u0
     i32.const 128
     i32.and
     i32.eqz
     if
      local.get $nullTerminated
      local.get $u0
      i32.eqz
      i32.and
      if
       br $while-break|0
      end
      local.get $strOff
      local.get $u0
      i32.store16
     else
      local.get $bufEnd
      local.get $bufOff
      i32.eq
      if
       br $while-break|0
      end
      local.get $bufOff
      i32.load8_u
      i32.const 63
      i32.and
      local.set $u1
      local.get $bufOff
      i32.const 1
      i32.add
      local.set $bufOff
      local.get $u0
      i32.const 224
      i32.and
      i32.const 192
      i32.eq
      if
       local.get $strOff
       local.get $u0
       i32.const 31
       i32.and
       i32.const 6
       i32.shl
       local.get $u1
       i32.or
       i32.store16
      else
       local.get $bufEnd
       local.get $bufOff
       i32.eq
       if
        br $while-break|0
       end
       local.get $bufOff
       i32.load8_u
       i32.const 63
       i32.and
       local.set $u2
       local.get $bufOff
       i32.const 1
       i32.add
       local.set $bufOff
       local.get $u0
       i32.const 240
       i32.and
       i32.const 224
       i32.eq
       if
        local.get $u0
        i32.const 15
        i32.and
        i32.const 12
        i32.shl
        local.get $u1
        i32.const 6
        i32.shl
        i32.or
        local.get $u2
        i32.or
        local.set $u0
       else
        local.get $bufEnd
        local.get $bufOff
        i32.eq
        if
         br $while-break|0
        end
        local.get $u0
        i32.const 7
        i32.and
        i32.const 18
        i32.shl
        local.get $u1
        i32.const 12
        i32.shl
        i32.or
        local.get $u2
        i32.const 6
        i32.shl
        i32.or
        local.get $bufOff
        i32.load8_u
        i32.const 63
        i32.and
        i32.or
        local.set $u0
        local.get $bufOff
        i32.const 1
        i32.add
        local.set $bufOff
       end
       local.get $u0
       i32.const 65536
       i32.lt_u
       if
        local.get $strOff
        local.get $u0
        i32.store16
       else
        local.get $u0
        i32.const 65536
        i32.sub
        local.set $u0
        local.get $u0
        i32.const 10
        i32.shr_u
        i32.const 55296
        i32.or
        local.set $lo
        local.get $u0
        i32.const 1023
        i32.and
        i32.const 56320
        i32.or
        local.set $hi
        local.get $strOff
        local.get $lo
        local.get $hi
        i32.const 16
        i32.shl
        i32.or
        i32.store
        local.get $strOff
        i32.const 2
        i32.add
        local.set $strOff
       end
      end
     end
     local.get $strOff
     i32.const 2
     i32.add
     local.set $strOff
     br $while-continue|0
    end
   end
  end
  local.get $str
  local.get $strOff
  local.get $str
  i32.sub
  call $~lib/rt/stub/__renew
  return
 )
 (func $~lib/string/String.UTF8.decode (param $buf i32) (param $nullTerminated i32) (result i32)
  local.get $buf
  local.get $buf
  call $~lib/arraybuffer/ArrayBuffer#get:byteLength
  local.get $nullTerminated
  call $~lib/string/String.UTF8.decodeUnsafe
  return
 )
 (func $assembly/hex/encodeHex (param $start i32) (param $len i32) (result i32)
  local.get $start
  local.get $len
  call $assembly/hex/encodeHexUTF8
  i32.const 0
  call $~lib/string/String.UTF8.decode
  return
 )
 (func $assembly/box/Box#toHexString (param $this i32) (result i32)
  local.get $this
  call $assembly/box/Box#get:start
  local.get $this
  call $assembly/box/Box#get:len
  call $assembly/hex/encodeHex
  return
 )
 (func $~lib/string/String.UTF8.byteLength (param $str i32) (param $nullTerminated i32) (result i32)
  (local $strOff i32)
  (local $strEnd i32)
  (local $bufLen i32)
  (local $c1 i32)
  local.get $str
  local.set $strOff
  local.get $strOff
  local.get $str
  i32.const 20
  i32.sub
  call $~lib/rt/common/OBJECT#get:rtSize
  i32.add
  local.set $strEnd
  local.get $nullTerminated
  i32.const 0
  i32.ne
  local.set $bufLen
  block $while-break|0
   loop $while-continue|0
    local.get $strOff
    local.get $strEnd
    i32.lt_u
    if
     local.get $strOff
     i32.load16_u
     local.set $c1
     local.get $c1
     i32.const 128
     i32.lt_u
     if
      local.get $nullTerminated
      local.get $c1
      i32.eqz
      i32.and
      if
       br $while-break|0
      end
      local.get $bufLen
      i32.const 1
      i32.add
      local.set $bufLen
     else
      local.get $c1
      i32.const 2048
      i32.lt_u
      if
       local.get $bufLen
       i32.const 2
       i32.add
       local.set $bufLen
      else
       local.get $c1
       i32.const 64512
       i32.and
       i32.const 55296
       i32.eq
       if (result i32)
        local.get $strOff
        i32.const 2
        i32.add
        local.get $strEnd
        i32.lt_u
       else
        i32.const 0
       end
       if
        local.get $strOff
        i32.load16_u offset=2
        i32.const 64512
        i32.and
        i32.const 56320
        i32.eq
        if
         local.get $bufLen
         i32.const 4
         i32.add
         local.set $bufLen
         local.get $strOff
         i32.const 4
         i32.add
         local.set $strOff
         br $while-continue|0
        end
       end
       local.get $bufLen
       i32.const 3
       i32.add
       local.set $bufLen
      end
     end
     local.get $strOff
     i32.const 2
     i32.add
     local.set $strOff
     br $while-continue|0
    end
   end
  end
  local.get $bufLen
  return
 )
 (func $~lib/string/String.UTF8.encodeUnsafe (param $str i32) (param $len i32) (param $buf i32) (param $nullTerminated i32) (param $errorMode i32) (result i32)
  (local $strEnd i32)
  (local $bufOff i32)
  (local $c1 i32)
  (local $b0 i32)
  (local $b1 i32)
  (local $c2 i32)
  (local $b0|11 i32)
  (local $b1|12 i32)
  (local $b2 i32)
  (local $b3 i32)
  (local $b0|15 i32)
  (local $b1|16 i32)
  (local $b2|17 i32)
  (local $18 i32)
  local.get $str
  local.get $len
  i32.const 1
  i32.shl
  i32.add
  local.set $strEnd
  local.get $buf
  local.set $bufOff
  loop $while-continue|0
   local.get $str
   local.get $strEnd
   i32.lt_u
   if
    local.get $str
    i32.load16_u
    local.set $c1
    local.get $c1
    i32.const 128
    i32.lt_u
    if
     local.get $bufOff
     local.get $c1
     i32.store8
     local.get $bufOff
     i32.const 1
     i32.add
     local.set $bufOff
     local.get $nullTerminated
     local.get $c1
     i32.eqz
     i32.and
     if
      local.get $bufOff
      local.get $buf
      i32.sub
      return
     end
    else
     local.get $c1
     i32.const 2048
     i32.lt_u
     if
      local.get $c1
      i32.const 6
      i32.shr_u
      i32.const 192
      i32.or
      local.set $b0
      local.get $c1
      i32.const 63
      i32.and
      i32.const 128
      i32.or
      local.set $b1
      local.get $bufOff
      local.get $b1
      i32.const 8
      i32.shl
      local.get $b0
      i32.or
      i32.store16
      local.get $bufOff
      i32.const 2
      i32.add
      local.set $bufOff
     else
      local.get $c1
      i32.const 63488
      i32.and
      i32.const 55296
      i32.eq
      if
       local.get $c1
       i32.const 56320
       i32.lt_u
       if (result i32)
        local.get $str
        i32.const 2
        i32.add
        local.get $strEnd
        i32.lt_u
       else
        i32.const 0
       end
       if
        local.get $str
        i32.load16_u offset=2
        local.set $c2
        local.get $c2
        i32.const 64512
        i32.and
        i32.const 56320
        i32.eq
        if
         i32.const 65536
         local.get $c1
         i32.const 1023
         i32.and
         i32.const 10
         i32.shl
         i32.add
         local.get $c2
         i32.const 1023
         i32.and
         i32.or
         local.set $c1
         local.get $c1
         i32.const 18
         i32.shr_u
         i32.const 240
         i32.or
         local.set $b0|11
         local.get $c1
         i32.const 12
         i32.shr_u
         i32.const 63
         i32.and
         i32.const 128
         i32.or
         local.set $b1|12
         local.get $c1
         i32.const 6
         i32.shr_u
         i32.const 63
         i32.and
         i32.const 128
         i32.or
         local.set $b2
         local.get $c1
         i32.const 63
         i32.and
         i32.const 128
         i32.or
         local.set $b3
         local.get $bufOff
         local.get $b3
         i32.const 24
         i32.shl
         local.get $b2
         i32.const 16
         i32.shl
         i32.or
         local.get $b1|12
         i32.const 8
         i32.shl
         i32.or
         local.get $b0|11
         i32.or
         i32.store
         local.get $bufOff
         i32.const 4
         i32.add
         local.set $bufOff
         local.get $str
         i32.const 4
         i32.add
         local.set $str
         br $while-continue|0
        end
       end
       local.get $errorMode
       i32.const 0
       i32.ne
       if
        local.get $errorMode
        i32.const 2
        i32.eq
        if
         i32.const 2384
         i32.const 2336
         i32.const 742
         i32.const 49
         call $~lib/builtins/abort
         unreachable
        end
        i32.const 65533
        local.set $c1
       end
      end
      local.get $c1
      i32.const 12
      i32.shr_u
      i32.const 224
      i32.or
      local.set $b0|15
      local.get $c1
      i32.const 6
      i32.shr_u
      i32.const 63
      i32.and
      i32.const 128
      i32.or
      local.set $b1|16
      local.get $c1
      i32.const 63
      i32.and
      i32.const 128
      i32.or
      local.set $b2|17
      local.get $bufOff
      local.get $b1|16
      i32.const 8
      i32.shl
      local.get $b0|15
      i32.or
      i32.store16
      local.get $bufOff
      local.get $b2|17
      i32.store8 offset=2
      local.get $bufOff
      i32.const 3
      i32.add
      local.set $bufOff
     end
    end
    local.get $str
    i32.const 2
    i32.add
    local.set $str
    br $while-continue|0
   end
  end
  local.get $nullTerminated
  if
   local.get $bufOff
   local.tee $18
   i32.const 1
   i32.add
   local.set $bufOff
   local.get $18
   i32.const 0
   i32.store8
  end
  local.get $bufOff
  local.get $buf
  i32.sub
  return
 )
 (func $~lib/string/String.UTF8.encode (param $str i32) (param $nullTerminated i32) (param $errorMode i32) (result i32)
  (local $buf i32)
  local.get $str
  local.get $nullTerminated
  call $~lib/string/String.UTF8.byteLength
  i32.const 1
  call $~lib/rt/stub/__new
  local.set $buf
  local.get $str
  local.get $str
  call $~lib/string/String#get:length
  local.get $buf
  local.get $nullTerminated
  local.get $errorMode
  call $~lib/string/String.UTF8.encodeUnsafe
  drop
  local.get $buf
  return
 )
 (func $~lib/string/String.UTF8.encode@varargs (param $str i32) (param $nullTerminated i32) (param $errorMode i32) (result i32)
  block $2of2
   block $1of2
    block $0of2
     block $outOfRange
      global.get $~argumentsLength
      i32.const 1
      i32.sub
      br_table $0of2 $1of2 $2of2 $outOfRange
     end
     unreachable
    end
    i32.const 0
    local.set $nullTerminated
   end
   i32.const 0
   local.set $errorMode
  end
  local.get $str
  local.get $nullTerminated
  local.get $errorMode
  call $~lib/string/String.UTF8.encode
 )
 (func $assembly/logging/Console#log (param $this i32) (param $v i32)
  local.get $v
  i32.const 1
  i32.const 2
  global.set $~argumentsLength
  i32.const 0
  call $~lib/string/String.UTF8.encode@varargs
  call $assembly/logging/__log
 )
 (func $assembly/index/test_sha256
  (local $hash i32)
  i32.const 1184
  call $assembly/index/decodeHex
  call $assembly/box/Box.from
  call $assembly/box/Box#toArrayBuffer
  call $assembly/sha256/sha256
  local.set $hash
  global.get $assembly/logging/console
  local.get $hash
  call $assembly/box/Box.from
  call $assembly/box/Box#toHexString
  call $assembly/logging/Console#log
 )
 (func $~start
  call $start:assembly/index
 )
)
