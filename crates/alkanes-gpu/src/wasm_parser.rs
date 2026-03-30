//! Host-side WASM module parser.
//!
//! Parses a WASM binary to extract the information needed to build
//! GPU input buffers: import count, function table (code offsets),
//! and the entry point (_start or execute function).
//!
//! This runs on the host (native), NOT inside the WASM VM.

use anyhow::{anyhow, Result};

/// Parsed WASM module information needed for GPU dispatch.
#[derive(Debug, Clone)]
pub struct WasmModuleInfo {
    /// Number of imported functions (host functions come first in the index space)
    pub import_count: u32,
    /// Function table: (code_byte_offset, local_count) for each non-imported function.
    /// Index 0 here corresponds to function index `import_count` in WASM.
    pub functions: Vec<FuncEntry>,
    /// Byte offset of the entry function's code body in the bytecode.
    /// This is the `_start` or designated entry point.
    pub entry_pc: u32,
    /// Names of imported functions, in import order.
    /// Used to build the import-to-host-function mapping table.
    pub import_names: Vec<String>,
    /// Raw code section bytes (all function bodies concatenated).
    /// The GPU shader reads bytecode from this.
    pub code_bytes: Vec<u8>,
    /// Global variable initial values (from the Global section).
    pub globals: Vec<GlobalInit>,
    /// Data segments to copy into linear memory at startup.
    pub data_segments: Vec<DataSegment>,
}

#[derive(Debug, Clone, Copy)]
pub struct FuncEntry {
    /// Byte offset within `code_bytes` where this function's body starts
    /// (after the local declarations).
    pub code_offset: u32,
    /// Number of local variable slots for this function.
    pub local_count: u32,
}

/// A WASM global variable's initial value.
#[derive(Debug, Clone)]
pub struct GlobalInit {
    /// Value type: 0x7F = i32, 0x7E = i64, 0x7D = f32, 0x7C = f64
    pub value_type: u8,
    /// Whether the global is mutable
    pub mutable: bool,
    /// Initial value (from the const init expression).
    /// For i32, only the low 32 bits are used; for i64, all 64 bits.
    pub init_value: u64,
}

/// A WASM data segment that initializes linear memory.
#[derive(Debug, Clone)]
pub struct DataSegment {
    /// Memory index (always 0 for single-memory modules)
    pub memory_index: u32,
    /// Destination offset in linear memory
    pub offset: u32,
    /// Raw bytes to copy into linear memory at `offset`
    pub data: Vec<u8>,
}

/// WASM section IDs
const SEC_TYPE: u8 = 1;
const SEC_IMPORT: u8 = 2;
const SEC_FUNCTION: u8 = 3;
const SEC_GLOBAL: u8 = 6;
const SEC_EXPORT: u8 = 7;
const SEC_CODE: u8 = 10;
const SEC_DATA: u8 = 11;

/// Parse a WASM binary and extract module info for GPU dispatch.
pub fn parse_wasm_module(wasm: &[u8]) -> Result<WasmModuleInfo> {
    let mut pos: usize = 0;

    // Validate magic number and version
    if wasm.len() < 8 {
        return Err(anyhow!("WASM too short"));
    }
    if &wasm[0..4] != b"\0asm" {
        return Err(anyhow!("invalid WASM magic"));
    }
    if &wasm[4..8] != &[1, 0, 0, 0] {
        return Err(anyhow!("unsupported WASM version"));
    }
    pos = 8;

    let mut import_func_count: u32 = 0;
    let mut import_names: Vec<String> = Vec::new();
    let mut func_type_indices: Vec<u32> = Vec::new();
    let mut export_start: Option<u32> = None; // function index of _start export
    let mut code_section_start: usize = 0;
    let mut code_section_len: usize = 0;
    let mut functions: Vec<FuncEntry> = Vec::new();
    let mut globals: Vec<GlobalInit> = Vec::new();
    let mut data_segments: Vec<DataSegment> = Vec::new();

    // Parse sections
    while pos < wasm.len() {
        let section_id = wasm[pos];
        pos += 1;
        let (section_len, bytes_read) = read_leb128_u32(&wasm[pos..])?;
        pos += bytes_read;
        let section_end = pos + section_len as usize;

        match section_id {
            SEC_IMPORT => {
                let (count, br) = read_leb128_u32(&wasm[pos..])?;
                let mut p = pos + br;
                for _ in 0..count {
                    // module name
                    let (mlen, br) = read_leb128_u32(&wasm[p..])?;
                    p += br + mlen as usize;
                    // field name
                    let (flen, br) = read_leb128_u32(&wasm[p..])?;
                    let field_name_bytes = &wasm[p + br..p + br + flen as usize];
                    let field_name = String::from_utf8_lossy(field_name_bytes).to_string();
                    p += br + flen as usize;
                    // import kind
                    let kind = wasm[p];
                    p += 1;
                    match kind {
                        0x00 => {
                            // function import — skip type index
                            let (_, br) = read_leb128_u32(&wasm[p..])?;
                            p += br;
                            import_names.push(field_name);
                            import_func_count += 1;
                        }
                        0x01 => {
                            // table import
                            p += 1; // element type
                            let (flags, br) = read_leb128_u32(&wasm[p..])?;
                            p += br;
                            let (_, br) = read_leb128_u32(&wasm[p..])?;
                            p += br;
                            if flags & 1 != 0 {
                                let (_, br) = read_leb128_u32(&wasm[p..])?;
                                p += br;
                            }
                        }
                        0x02 => {
                            // memory import
                            let (flags, br) = read_leb128_u32(&wasm[p..])?;
                            p += br;
                            let (_, br) = read_leb128_u32(&wasm[p..])?;
                            p += br;
                            if flags & 1 != 0 {
                                let (_, br) = read_leb128_u32(&wasm[p..])?;
                                p += br;
                            }
                        }
                        0x03 => {
                            // global import
                            p += 1; // value type
                            p += 1; // mutability
                        }
                        _ => {}
                    }
                }
            }
            SEC_FUNCTION => {
                let (count, br) = read_leb128_u32(&wasm[pos..])?;
                let mut p = pos + br;
                for _ in 0..count {
                    let (type_idx, br) = read_leb128_u32(&wasm[p..])?;
                    p += br;
                    func_type_indices.push(type_idx);
                }
            }
            SEC_GLOBAL => {
                let (count, br) = read_leb128_u32(&wasm[pos..])?;
                let mut p = pos + br;
                for _ in 0..count {
                    // Global type: value_type + mutability
                    let value_type = wasm[p];
                    p += 1;
                    let mutable = wasm[p] != 0;
                    p += 1;
                    // Init expression: typically one const + end
                    let (init_value, bytes_consumed) = parse_init_expr(&wasm[p..])?;
                    p += bytes_consumed;
                    globals.push(GlobalInit {
                        value_type,
                        mutable,
                        init_value,
                    });
                }
            }
            SEC_EXPORT => {
                let (count, br) = read_leb128_u32(&wasm[pos..])?;
                let mut p = pos + br;
                for _ in 0..count {
                    // name
                    let (nlen, br) = read_leb128_u32(&wasm[p..])?;
                    p += br;
                    let name = &wasm[p..p + nlen as usize];
                    p += nlen as usize;
                    // kind
                    let kind = wasm[p];
                    p += 1;
                    // index
                    let (idx, br) = read_leb128_u32(&wasm[p..])?;
                    p += br;

                    if kind == 0x00 {
                        // Function export
                        if name == b"_start" || name == b"execute" || name == b"__execute" {
                            export_start = Some(idx);
                        }
                    }
                }
            }
            SEC_CODE => {
                code_section_start = pos;
                code_section_len = section_len as usize;

                let (count, br) = read_leb128_u32(&wasm[pos..])?;
                let mut p = pos + br;

                // Parse each function body
                for _ in 0..count {
                    let (body_len, br) = read_leb128_u32(&wasm[p..])?;
                    p += br;
                    let body_start = p;

                    // Parse locals
                    let (local_decl_count, br) = read_leb128_u32(&wasm[p..])?;
                    p += br;
                    let mut total_locals: u32 = 0;
                    for _ in 0..local_decl_count {
                        let (count, br) = read_leb128_u32(&wasm[p..])?;
                        p += br;
                        p += 1; // value type
                        total_locals += count;
                    }

                    // Code starts here (after locals)
                    let code_offset = (p - code_section_start) as u32;
                    functions.push(FuncEntry {
                        code_offset,
                        local_count: total_locals,
                    });

                    // Skip to end of body
                    p = body_start + body_len as usize;
                }
            }
            SEC_DATA => {
                let (count, br) = read_leb128_u32(&wasm[pos..])?;
                let mut p = pos + br;
                for _ in 0..count {
                    // Data segment flags
                    let (flags, br) = read_leb128_u32(&wasm[p..])?;
                    p += br;

                    match flags {
                        0 => {
                            // Active segment, memory 0, with offset expr
                            let (offset_val, expr_bytes) = parse_init_expr(&wasm[p..])?;
                            p += expr_bytes;
                            let (data_len, br) = read_leb128_u32(&wasm[p..])?;
                            p += br;
                            let data = wasm[p..p + data_len as usize].to_vec();
                            p += data_len as usize;
                            data_segments.push(DataSegment {
                                memory_index: 0,
                                offset: offset_val as u32,
                                data,
                            });
                        }
                        1 => {
                            // Passive segment (no memory index, no offset)
                            let (data_len, br) = read_leb128_u32(&wasm[p..])?;
                            p += br;
                            // Skip passive segments — they're loaded by memory.init
                            p += data_len as usize;
                        }
                        2 => {
                            // Active segment with explicit memory index
                            let (mem_idx, br) = read_leb128_u32(&wasm[p..])?;
                            p += br;
                            let (offset_val, expr_bytes) = parse_init_expr(&wasm[p..])?;
                            p += expr_bytes;
                            let (data_len, br) = read_leb128_u32(&wasm[p..])?;
                            p += br;
                            let data = wasm[p..p + data_len as usize].to_vec();
                            p += data_len as usize;
                            data_segments.push(DataSegment {
                                memory_index: mem_idx,
                                offset: offset_val as u32,
                                data,
                            });
                        }
                        _ => {
                            // Unknown flags — skip rest of section
                            break;
                        }
                    }
                }
            }
            _ => {
                // Skip unknown sections
            }
        }

        pos = section_end;
    }

    // Extract code section bytes
    let code_bytes = if code_section_len > 0 {
        wasm[code_section_start..code_section_start + code_section_len].to_vec()
    } else {
        Vec::new()
    };

    // Determine entry point
    let entry_pc = if let Some(export_idx) = export_start {
        if export_idx >= import_func_count {
            let internal_idx = (export_idx - import_func_count) as usize;
            if internal_idx < functions.len() {
                functions[internal_idx].code_offset
            } else {
                0
            }
        } else {
            // Exported function is an import — shouldn't happen for _start
            0
        }
    } else {
        // No _start or execute export — use first function
        if !functions.is_empty() {
            functions[0].code_offset
        } else {
            0
        }
    };

    Ok(WasmModuleInfo {
        import_count: import_func_count,
        import_names,
        functions,
        entry_pc,
        code_bytes,
        globals,
        data_segments,
    })
}

/// Parse a WASM init expression (const expr).
/// Returns (value, bytes_consumed).
/// Supports i32.const, i64.const, and global.get (returns 0 for global.get).
fn parse_init_expr(data: &[u8]) -> Result<(u64, usize)> {
    let mut p: usize = 0;
    let opcode = data[p];
    p += 1;
    let value = match opcode {
        0x41 => {
            // i32.const
            let (val, br) = read_leb128_i32(&data[p..])?;
            p += br;
            val as u32 as u64
        }
        0x42 => {
            // i64.const
            let (val, br) = read_leb128_i64(&data[p..])?;
            p += br;
            val as u64
        }
        0x23 => {
            // global.get — reference to another global
            let (_idx, br) = read_leb128_u32(&data[p..])?;
            p += br;
            0u64 // Can't resolve at parse time; return 0
        }
        _ => {
            // Unknown init expr opcode
            0u64
        }
    };
    // Expect 0x0B (end)
    if p < data.len() && data[p] == 0x0B {
        p += 1;
    }
    Ok((value, p))
}

/// Read a LEB128-encoded u32 from a byte slice.
/// Returns (value, bytes_consumed).
fn read_leb128_u32(data: &[u8]) -> Result<(u32, usize)> {
    let mut result: u32 = 0;
    let mut shift: u32 = 0;
    for (i, &byte) in data.iter().enumerate().take(5) {
        result |= ((byte & 0x7F) as u32) << shift;
        if byte & 0x80 == 0 {
            return Ok((result, i + 1));
        }
        shift += 7;
    }
    Err(anyhow!("LEB128 overflow"))
}

/// Read a LEB128-encoded signed i32 from a byte slice.
/// Returns (value, bytes_consumed).
fn read_leb128_i32(data: &[u8]) -> Result<(i32, usize)> {
    let mut result: i32 = 0;
    let mut shift: u32 = 0;
    let mut byte: u8 = 0;
    for (i, &b) in data.iter().enumerate().take(5) {
        byte = b;
        result |= ((b & 0x7F) as i32) << shift;
        shift += 7;
        if b & 0x80 == 0 {
            // Sign extend if needed
            if shift < 32 && (byte & 0x40) != 0 {
                result |= !0i32 << shift;
            }
            return Ok((result, i + 1));
        }
    }
    Err(anyhow!("LEB128 i32 overflow"))
}

/// Read a LEB128-encoded signed i64 from a byte slice.
/// Returns (value, bytes_consumed).
fn read_leb128_i64(data: &[u8]) -> Result<(i64, usize)> {
    let mut result: i64 = 0;
    let mut shift: u32 = 0;
    let mut byte: u8 = 0;
    for (i, &b) in data.iter().enumerate().take(10) {
        byte = b;
        result |= ((b & 0x7F) as i64) << shift;
        shift += 7;
        if b & 0x80 == 0 {
            // Sign extend if needed
            if shift < 64 && (byte & 0x40) != 0 {
                result |= !0i64 << shift;
            }
            return Ok((result, i + 1));
        }
    }
    Err(anyhow!("LEB128 i64 overflow"))
}

/// Map a WASM import field name to a host function ID.
/// Returns 255 for unknown imports.
pub fn host_function_id(name: &str) -> u32 {
    match name {
        "abort" | "_ZN15alkanes_runtime7imports5abort" => 0,  // HOST_ABORT
        "__load_storage" | "_ZN15alkanes_runtime7imports14__load_storage" => 1,
        "__request_storage" | "_ZN15alkanes_runtime7imports17__request_storage" => 2,
        "__log" | "_ZN15alkanes_runtime7imports5__log" => 3,
        "__balance" | "_ZN15alkanes_runtime7imports9__balance" => 4,
        "__request_context" | "_ZN15alkanes_runtime7imports17__request_context" => 5,
        "__load_context" | "_ZN15alkanes_runtime7imports14__load_context" => 6,
        "__sequence" | "_ZN15alkanes_runtime7imports10__sequence" => 7,
        "__fuel" | "_ZN15alkanes_runtime7imports6__fuel" => 8,
        "__height" | "_ZN15alkanes_runtime7imports8__height" => 9,
        "__returndatacopy" => 10,
        "__request_transaction" => 11,
        "__load_transaction" => 12,
        "__request_block" => 13,
        "__load_block" => 14,
        "__call" | "_ZN15alkanes_runtime7imports6__call" => 15,
        "__delegatecall" => 16,
        "__staticcall" => 17,
        _ => 255,
    }
}

/// Build the import mapping table from parsed import names.
/// Returns a Vec of host function IDs, one per imported function.
pub fn build_import_map(import_names: &[String]) -> Vec<u32> {
    import_names.iter().map(|name| host_function_id(name)).collect()
}

/// Pack globals into u32 words for the GPU input buffer.
/// Layout: for each global, [value_lo: u32, value_hi: u32].
pub fn pack_globals(globals: &[GlobalInit]) -> Vec<u32> {
    let mut words = Vec::with_capacity(globals.len() * 2);
    for g in globals {
        words.push(g.init_value as u32);          // lo
        words.push((g.init_value >> 32) as u32);   // hi
    }
    words
}

/// Pack data segments into u32 words for the GPU input buffer.
/// Layout per segment: [offset: u32, byte_length: u32, data_words...]
/// where data_words are the raw bytes packed into u32 little-endian.
pub fn pack_data_segments(segments: &[DataSegment]) -> Vec<u32> {
    let mut words = Vec::new();
    for seg in segments {
        words.push(seg.offset);
        words.push(seg.data.len() as u32);
        // Pack bytes into u32 words
        let word_count = (seg.data.len() + 3) / 4;
        for w in 0..word_count {
            let mut val: u32 = 0;
            for b in 0..4 {
                let idx = w * 4 + b;
                if idx < seg.data.len() {
                    val |= (seg.data[idx] as u32) << (b * 8);
                }
            }
            words.push(val);
        }
    }
    words
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a minimal valid WASM module with one function that does i32.const 42; end
    fn minimal_wasm() -> Vec<u8> {
        let mut w = Vec::new();

        // Magic + version
        w.extend_from_slice(b"\0asm");
        w.extend_from_slice(&[1, 0, 0, 0]);

        // Type section: one function type () -> ()
        w.push(SEC_TYPE);
        w.extend_from_slice(&[4]); // section length
        w.push(1); // 1 type
        w.push(0x60); // func type
        w.push(0); // 0 params
        w.push(0); // 0 results

        // Function section: one function, type index 0
        w.push(SEC_FUNCTION);
        w.extend_from_slice(&[2]); // section length
        w.push(1); // 1 function
        w.push(0); // type index 0

        // Export section: export function 0 as "_start"
        w.push(SEC_EXPORT);
        let export_name = b"_start";
        let export_section_len = 1 + 1 + export_name.len() + 1 + 1;
        w.push(export_section_len as u8);
        w.push(1); // 1 export
        w.push(export_name.len() as u8);
        w.extend_from_slice(export_name);
        w.push(0x00); // function export
        w.push(0); // function index 0

        // Code section: one function body
        let body = vec![
            0x00, // 0 local declarations
            0x41, 0x2A, // i32.const 42
            0x1A, // drop
            0x0B, // end
        ];
        let code_section_body_len = 1 + 1 + body.len(); // count(1) + body_len_leb(1) + body
        w.push(SEC_CODE);
        w.push(code_section_body_len as u8);
        w.push(1); // 1 function body
        w.push(body.len() as u8); // body length
        w.extend_from_slice(&body);

        w
    }

    #[test]
    fn test_parse_minimal_wasm() {
        let wasm = minimal_wasm();
        let info = parse_wasm_module(&wasm).unwrap();

        assert_eq!(info.import_count, 0);
        assert_eq!(info.functions.len(), 1);
        assert!(info.entry_pc > 0, "entry_pc should point to code");
        assert!(!info.code_bytes.is_empty());
        assert!(info.globals.is_empty());
        assert!(info.data_segments.is_empty());
    }

    /// WASM with 3 imported functions and 2 internal functions
    fn wasm_with_imports() -> Vec<u8> {
        let mut w = Vec::new();
        w.extend_from_slice(b"\0asm");
        w.extend_from_slice(&[1, 0, 0, 0]);

        // Helper: write a section (id + LEB128 length + body)
        fn write_section(w: &mut Vec<u8>, id: u8, body: &[u8]) {
            w.push(id);
            // LEB128 encode body length
            let mut len = body.len() as u32;
            loop {
                let mut byte = (len & 0x7F) as u8;
                len >>= 7;
                if len != 0 { byte |= 0x80; }
                w.push(byte);
                if len == 0 { break; }
            }
            w.extend_from_slice(body);
        }

        // Type section: () -> () and () -> (i32)
        write_section(&mut w, SEC_TYPE, &[
            2,    // 2 types
            0x60, 0, 0,             // type 0: () -> ()
            0x60, 0, 1, 0x7F,      // type 1: () -> (i32)
        ]);

        // Import section: 3 function imports from "env"
        let mut import_body = Vec::new();
        import_body.push(3); // 3 imports
        for (name, type_idx) in [("a", 0u8), ("b", 0u8), ("c", 1u8)] {
            import_body.push(3); // "env" length
            import_body.extend_from_slice(b"env");
            import_body.push(name.len() as u8);
            import_body.extend_from_slice(name.as_bytes());
            import_body.push(0x00); // function import
            import_body.push(type_idx);
        }
        write_section(&mut w, SEC_IMPORT, &import_body);

        // Function section: 2 internal functions
        write_section(&mut w, SEC_FUNCTION, &[2, 0, 1]);

        // Export: function 3 (import_count=3 + internal 0) as "_start"
        let mut export_body = Vec::new();
        export_body.push(1); // 1 export
        export_body.push(6); // name length
        export_body.extend_from_slice(b"_start");
        export_body.push(0x00); // function export
        export_body.push(3);   // function index 3
        write_section(&mut w, SEC_EXPORT, &export_body);

        // Code section: 2 function bodies
        let body1: Vec<u8> = vec![0x00, 0x0B]; // 0 locals, end
        let body2: Vec<u8> = vec![0x00, 0x41, 0x2A, 0x0B]; // 0 locals, i32.const 42, end
        let mut code_body = Vec::new();
        code_body.push(2); // 2 function bodies
        code_body.push(body1.len() as u8);
        code_body.extend_from_slice(&body1);
        code_body.push(body2.len() as u8);
        code_body.extend_from_slice(&body2);
        write_section(&mut w, SEC_CODE, &code_body);

        w
    }

    #[test]
    fn test_parse_wasm_with_imports() {
        let wasm = wasm_with_imports();
        let info = parse_wasm_module(&wasm).unwrap();

        assert_eq!(info.import_count, 3);
        assert_eq!(info.functions.len(), 2);
        assert!(info.entry_pc > 0);
    }

    #[test]
    fn test_import_names_extracted() {
        let wasm = wasm_with_imports();
        let info = parse_wasm_module(&wasm).unwrap();
        assert_eq!(info.import_names.len(), 3);
        assert_eq!(info.import_names[0], "a");
        assert_eq!(info.import_names[1], "b");
        assert_eq!(info.import_names[2], "c");
    }

    #[test]
    fn test_host_function_id_mapping() {
        assert_eq!(host_function_id("abort"), 0);
        assert_eq!(host_function_id("__load_storage"), 1);
        assert_eq!(host_function_id("__request_storage"), 2);
        assert_eq!(host_function_id("__request_context"), 5);
        assert_eq!(host_function_id("__height"), 9);
        assert_eq!(host_function_id("__call"), 15);
        assert_eq!(host_function_id("unknown_func"), 255);
    }

    #[test]
    fn test_build_import_map() {
        let names = vec![
            "__request_context".to_string(),
            "__load_context".to_string(),
            "__height".to_string(),
            "abort".to_string(),
            "__request_storage".to_string(),
            "__load_storage".to_string(),
        ];
        let map = build_import_map(&names);
        assert_eq!(map, vec![5, 6, 9, 0, 2, 1]);
    }

    #[test]
    fn test_function_table_for_gpu() {
        let wasm = wasm_with_imports();
        let info = parse_wasm_module(&wasm).unwrap();

        // Build the function table as GPU expects it
        let func_table: Vec<(u32, u32)> = info
            .functions
            .iter()
            .map(|f| (f.code_offset, f.local_count))
            .collect();

        assert_eq!(func_table.len(), 2);
        // Both functions have 0 locals
        assert_eq!(func_table[0].1, 0);
        assert_eq!(func_table[1].1, 0);
        // Second function should have a later offset than first
        assert!(func_table[1].0 > func_table[0].0);
    }

    /// Build a WASM module with globals and a data segment
    fn wasm_with_globals_and_data() -> Vec<u8> {
        let mut w = Vec::new();
        w.extend_from_slice(b"\0asm");
        w.extend_from_slice(&[1, 0, 0, 0]);

        fn write_section(w: &mut Vec<u8>, id: u8, body: &[u8]) {
            w.push(id);
            let mut len = body.len() as u32;
            loop {
                let mut byte = (len & 0x7F) as u8;
                len >>= 7;
                if len != 0 { byte |= 0x80; }
                w.push(byte);
                if len == 0 { break; }
            }
            w.extend_from_slice(body);
        }

        // Type section
        write_section(&mut w, SEC_TYPE, &[1, 0x60, 0, 0]);

        // Function section
        write_section(&mut w, SEC_FUNCTION, &[1, 0]);

        // Global section: 3 globals
        // global[0]: i32 mutable, init = 1048576 (0x100000)
        // global[1]: i32 immutable, init = 1064024
        // global[2]: i32 immutable, init = 42
        let mut global_body = Vec::new();
        global_body.push(3); // 3 globals

        // global[0]: i32, mutable, i32.const 1048576
        global_body.push(0x7F); // i32
        global_body.push(0x01); // mutable
        global_body.push(0x41); // i32.const
        // LEB128 encode 1048576 = 0x100000
        // 1048576 = 0b100000000000000000000
        // LEB128: 0x80, 0x80, 0xC0, 0x00
        global_body.extend_from_slice(&[0x80, 0x80, 0xC0, 0x00]);
        global_body.push(0x0B); // end

        // global[1]: i32, immutable, i32.const 1064024 (0x103C58)
        global_body.push(0x7F);
        global_body.push(0x00); // immutable
        global_body.push(0x41);
        // LEB128 encode 1064024
        // 1064024 = 0x103C58
        // In LEB128: 0xD8, 0xF8, 0xC0, 0x00
        global_body.extend_from_slice(&[0xD8, 0xF8, 0xC0, 0x00]);
        global_body.push(0x0B);

        // global[2]: i32, immutable, i32.const 42
        global_body.push(0x7F);
        global_body.push(0x00);
        global_body.push(0x41);
        global_body.push(42);
        global_body.push(0x0B);

        write_section(&mut w, SEC_GLOBAL, &global_body);

        // Export section
        let mut export_body = Vec::new();
        export_body.push(1);
        export_body.push(6);
        export_body.extend_from_slice(b"_start");
        export_body.push(0x00);
        export_body.push(0);
        write_section(&mut w, SEC_EXPORT, &export_body);

        // Code section
        let body = vec![0x00, 0x0B]; // 0 locals, end
        let mut code_body = Vec::new();
        code_body.push(1);
        code_body.push(body.len() as u8);
        code_body.extend_from_slice(&body);
        write_section(&mut w, SEC_CODE, &code_body);

        // Data section: one active segment at offset 256, 8 bytes
        let mut data_body = Vec::new();
        data_body.push(1); // 1 segment
        data_body.push(0); // flags = 0 (active, memory 0)
        data_body.push(0x41); // i32.const
        data_body.extend_from_slice(&[0x80, 0x02]); // LEB128 256
        data_body.push(0x0B); // end
        data_body.push(8);    // data length = 8
        data_body.extend_from_slice(&[0x48, 0x65, 0x6C, 0x6C, 0x6F, 0x21, 0x00, 0x00]); // "Hello!\0\0"
        write_section(&mut w, SEC_DATA, &data_body);

        w
    }

    #[test]
    fn test_parse_globals() {
        let wasm = wasm_with_globals_and_data();
        let info = parse_wasm_module(&wasm).unwrap();

        assert_eq!(info.globals.len(), 3);

        // global[0]: __stack_pointer = 1048576
        assert_eq!(info.globals[0].value_type, 0x7F);
        assert!(info.globals[0].mutable);
        assert_eq!(info.globals[0].init_value, 1048576);

        // global[1]: __data_end = 1064024
        assert_eq!(info.globals[1].value_type, 0x7F);
        assert!(!info.globals[1].mutable);
        assert_eq!(info.globals[1].init_value, 1064024);

        // global[2]: simple = 42
        assert_eq!(info.globals[2].init_value, 42);
    }

    #[test]
    fn test_parse_data_segments() {
        let wasm = wasm_with_globals_and_data();
        let info = parse_wasm_module(&wasm).unwrap();

        assert_eq!(info.data_segments.len(), 1);
        assert_eq!(info.data_segments[0].memory_index, 0);
        assert_eq!(info.data_segments[0].offset, 256);
        assert_eq!(info.data_segments[0].data.len(), 8);
        assert_eq!(&info.data_segments[0].data[..6], b"Hello!");
    }

    #[test]
    fn test_pack_globals() {
        let globals = vec![
            GlobalInit { value_type: 0x7F, mutable: true, init_value: 1048576 },
            GlobalInit { value_type: 0x7F, mutable: false, init_value: 42 },
        ];
        let packed = pack_globals(&globals);
        assert_eq!(packed.len(), 4); // 2 globals * 2 words each
        assert_eq!(packed[0], 1048576); // global[0] lo
        assert_eq!(packed[1], 0);       // global[0] hi
        assert_eq!(packed[2], 42);      // global[1] lo
        assert_eq!(packed[3], 0);       // global[1] hi
    }

    #[test]
    fn test_pack_data_segments() {
        let segments = vec![
            DataSegment {
                memory_index: 0,
                offset: 256,
                data: vec![0x48, 0x65, 0x6C, 0x6C, 0x6F, 0x21],
            },
        ];
        let packed = pack_data_segments(&segments);
        // [offset, length, word0, word1]
        assert_eq!(packed[0], 256);  // offset
        assert_eq!(packed[1], 6);    // length
        // "Hell" = 0x6C6C6548
        assert_eq!(packed[2], 0x6C6C6548);
        // "o!\0\0" padded = 0x0000216F
        assert_eq!(packed[3], 0x0000216F);
    }

    #[test]
    fn test_parse_real_contract() {
        let path = "/tmp/contract_2_0.wasm";
        if !std::path::Path::new(path).exists() {
            eprintln!("Skipping: {} not found", path);
            return;
        }
        let wasm = std::fs::read(path).unwrap();
        let info = parse_wasm_module(&wasm).unwrap();
        println!("Globals: {}", info.globals.len());
        for (i, g) in info.globals.iter().enumerate() {
            println!("  global[{}]: type=0x{:02x}, mutable={}, init=0x{:x} ({})", 
                i, g.value_type, g.mutable, g.init_value, g.init_value);
        }
        println!("Data segments: {}", info.data_segments.len());
        for (i, d) in info.data_segments.iter().enumerate() {
            println!("  seg[{}]: offset=0x{:x} ({}), size={}", i, d.offset, d.offset, d.data.len());
        }
        let packed = pack_globals(&info.globals);
        println!("Packed globals: {:?}", packed);
        assert_eq!(info.globals.len(), 3);
        assert_eq!(info.globals[0].init_value, 1048576); // __stack_pointer
    }

}
